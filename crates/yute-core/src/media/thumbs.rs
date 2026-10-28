use crate::media::cache::DiskCache;
use image::DynamicImage;
use sha2::{Digest, Sha256};
use std::{
    collections::{BinaryHeap, HashMap, VecDeque},
    path::PathBuf,
    sync::{
        mpsc::{channel, Receiver},
        Arc, OnceLock,
    },
    time::{Duration, Instant},
};
use tokio::sync::{Notify, Semaphore};
use tokio_util::sync::CancellationToken;

pub fn normalize_thumbnail_url(url: &str) -> std::borrow::Cow<'_, str> {
    if url.starts_with("//") {
        format!("https:{url}").into()
    } else {
        url.into()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Prefetch,
    Visible,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SourceKey {
    url: String,
    width: u32,
    height: u32,
}
struct Job {
    key: SourceKey,
    path: PathBuf,
    priority: Priority,
    sequence: u64,
    cancelled: CancellationToken,
}
impl PartialEq for Job {
    fn eq(&self, other: &Self) -> bool {
        self.sequence == other.sequence
    }
}
impl Eq for Job {}
impl PartialOrd for Job {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Job {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}
struct Response {
    key: SourceKey,
    image: Option<Arc<DynamicImage>>,
    sequence: u64,
}
pub struct ThumbnailLoader {
    cache: DiskCache,
    in_flight: HashMap<SourceKey, (CancellationToken, Priority, u64)>,
    failed: HashMap<SourceKey, Instant>,
    ram: HashMap<SourceKey, Arc<DynamicImage>>,
    order: VecDeque<SourceKey>,
    max_entries: usize,
    sequence: u64,
    tx: tokio::sync::mpsc::Sender<Job>,
    rx: Receiver<Response>,
    notify: Arc<Notify>,
}
impl ThumbnailLoader {
    pub fn new(cache: DiskCache) -> Self {
        let (tx, mut requests) = tokio::sync::mpsc::channel::<Job>(128);
        let (results, rx) = channel();
        let notify = Arc::new(Notify::new());
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let notify = notify.clone();
            runtime.spawn(async move {
                let mut queue: BinaryHeap<Job> = BinaryHeap::new();
                let mut running = tokio::task::JoinSet::new();
                let mut prefetch_running = 0usize;
                loop {
                    while running.len() < 12 {
                        let Some(job) = queue.pop() else { break; };
                        if job.cancelled.is_cancelled() { continue; }
                        // Reserve eight slots for visible work even when disk-cache
                        // checks delay its admission by a few milliseconds.
                        if job.priority == Priority::Prefetch && prefetch_running >= 4 {
                            queue.push(job);
                            break;
                        }
                        if job.priority == Priority::Prefetch { prefetch_running += 1; }
                        let results = results.clone();
                        let notify = notify.clone();
                        running.spawn(async move {
                            let image = tokio::select! {
                                _ = job.cancelled.cancelled() => None,
                                image = fetch_thumbnail(&job.key.url, job.key.width, job.key.height, &job.path) => image,
                            };
                            let _ = results.send(Response { key: job.key, image: image.map(Arc::new), sequence: job.sequence });
                            notify.notify_one();
                            job.priority
                        });
                    }
                    tokio::select! {
                        Some(job) = requests.recv() => {
                            queue.retain(|queued| !queued.cancelled.is_cancelled());
                            if queue.len() < 128 { queue.push(job); }
                            else {
                                let _ = results.send(Response { key:job.key, image:None, sequence:job.sequence });
                                notify.notify_one();
                            }
                            while queue.len() < 128 {
                                match requests.try_recv() { Ok(job) => queue.push(job), Err(_) => break, }
                            }
                        }
                        Some(result) = running.join_next(), if !running.is_empty() => {
                            if !matches!(result, Ok(Priority::Visible)) { prefetch_running = prefetch_running.saturating_sub(1); }
                        }
                        else => break,
                    }
                }
            });
        }
        Self {
            cache,
            in_flight: HashMap::new(),
            failed: HashMap::new(),
            ram: HashMap::new(),
            order: VecDeque::new(),
            max_entries: 64,
            sequence: 0,
            tx,
            rx,
            notify,
        }
    }
    pub fn set_budget(&mut self, mb: u64) {
        self.cache = self.cache.with_limit(mb);
        let cache = self.cache.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn_blocking(move || cache.prune_thumbnails());
        }
    }
    pub fn notifier(&self) -> Arc<Notify> {
        self.notify.clone()
    }
    pub fn poll_loaded(&mut self) -> Vec<(String, Arc<DynamicImage>)> {
        let mut loaded = Vec::new();
        while let Ok(response) = self.rx.try_recv() {
            // A superseded prefetch must not remove its newer visible request.
            if self
                .in_flight
                .get(&response.key)
                .is_none_or(|(token, _, sequence)| {
                    token.is_cancelled() || *sequence != response.sequence
                })
            {
                continue;
            }
            self.in_flight.remove(&response.key);
            if let Some(image) = response.image {
                self.failed.remove(&response.key);
                self.order.retain(|key| key != &response.key);
                self.order.push_back(response.key.clone());
                self.ram.insert(response.key.clone(), image.clone());
                while self.ram.len() > self.max_entries {
                    if let Some(old) = self.order.pop_front() {
                        self.ram.remove(&old);
                    }
                }
                loaded.push((response.key.url, image));
                self.cache.prune_background();
            } else {
                self.failed.insert(response.key, Instant::now());
            }
        }
        loaded
    }
    fn key(url: &str, width: u32, height: u32) -> SourceKey {
        SourceKey {
            url: url.into(),
            width: if width <= 320 { 320 } else { 480 },
            height: height.clamp(1, 480),
        }
    }
    fn request(&mut self, key: &SourceKey, priority: Priority) {
        if self.ram.contains_key(key) {
            self.order.retain(|old| old != key);
            self.order.push_back(key.clone());
            return;
        }
        if let Some((token, old, _)) = self.in_flight.get(key) {
            if *old >= priority {
                return;
            }
            token.cancel();
        }
        if self
            .failed
            .get(key)
            .is_some_and(|at| at.elapsed() < Duration::from_secs(10))
        {
            return;
        }
        let cancelled = CancellationToken::new();
        self.sequence = self.sequence.wrapping_add(1);
        let path = self.cache.thumbs_dir().join(format!(
            "{}.jpg",
            source_cache_key(&key.url, key.width, key.height)
        ));
        let job = Job {
            key: key.clone(),
            path,
            priority,
            sequence: self.sequence,
            cancelled: cancelled.clone(),
        };
        if self.tx.try_send(job).is_ok() {
            self.in_flight
                .insert(key.clone(), (cancelled, priority, self.sequence));
        }
    }
    pub fn get_shared(&mut self, url: &str, width: u32, height: u32) -> Option<Arc<DynamicImage>> {
        if url.is_empty() {
            return None;
        }
        let key = Self::key(url, width, height);
        self.request(&key, Priority::Visible);
        self.ram.get(&key).cloned()
    }
    pub fn is_cached_for(&self, url: &str, width: u32, height: u32) -> bool {
        let key = Self::key(url, width, height);
        self.ram.contains_key(&key)
            || self
                .cache
                .thumbs_dir()
                .join(format!(
                    "{}.jpg",
                    source_cache_key(url, key.width, key.height)
                ))
                .is_file()
    }
    pub fn get_image(&mut self, url: &str) -> Option<&DynamicImage> {
        let key = Self::key(url, 480, 360);
        self.request(&key, Priority::Visible);
        self.ram.get(&key).map(Arc::as_ref)
    }
    pub fn prefetch(&mut self, url: &str, width: u32, height: u32) {
        if !url.is_empty() {
            self.request(&Self::key(url, width, height), Priority::Prefetch);
        }
    }
    pub fn retain_wanted(&mut self, wanted: &std::collections::HashSet<String>) {
        self.in_flight.retain(|key, (cancelled, _, _)| {
            if wanted.contains(&key.url) {
                true
            } else {
                cancelled.cancel();
                false
            }
        });
    }
    pub fn is_cached(&self, url: &str) -> bool {
        self.ram.keys().any(|key| key.url == url)
            || self
                .cache
                .thumbs_dir()
                .join(format!("{}.jpg", cache_key_for_url(url)))
                .is_file()
    }
    pub fn cache(&self) -> &DiskCache {
        &self.cache
    }
    pub fn has_pending(&self) -> bool {
        !self.in_flight.is_empty()
    }
}
impl Drop for ThumbnailLoader {
    fn drop(&mut self) {
        for (cancelled, _, _) in self.in_flight.values() {
            cancelled.cancel();
        }
    }
}

pub fn source_cache_key(url: &str, width: u32, height: u32) -> String {
    format!("{}_{}x{}", cache_key_for_url(url), width, height)
}
pub fn thumbnail_candidates(url: &str, width: u32) -> Vec<String> {
    let original = normalize_thumbnail_url(url).into_owned();
    if let Some(id) = video_id(&original) {
        let first = if width <= 320 {
            "mqdefault"
        } else {
            "hqdefault"
        };
        let mut urls = vec![format!("https://i.ytimg.com/vi/{id}/{first}.jpg")];
        if !original.contains("maxres") && original != urls[0] {
            urls.push(original.clone());
        }
        for variant in ["hqdefault", "mqdefault", "default"] {
            let candidate = format!("https://i.ytimg.com/vi/{id}/{variant}.jpg");
            if !urls.contains(&candidate) {
                urls.push(candidate);
            }
        }
        urls
    } else {
        let sized = avatar_thumbnail_url(&original, width);
        if sized != original {
            vec![sized, original]
        } else {
            vec![original]
        }
    }
}
/// Only rewrite documented Google image sizing tokens, preserving crop/privacy
/// flags, query strings, and opaque image IDs. Unknown suffixes pass through.
pub fn avatar_thumbnail_url(url: &str, width: u32) -> String {
    let normalized = normalize_thumbnail_url(url);
    let Ok(mut parsed) = reqwest::Url::parse(&normalized) else {
        return normalized.into_owned();
    };
    let host = parsed.host_str().unwrap_or_default();
    if !(host == "yt3.ggpht.com"
        || host == "yt3.googleusercontent.com"
        || host.ends_with(".googleusercontent.com"))
    {
        return normalized.into_owned();
    }
    let size = width.clamp(88, 512).to_string();
    if let Some((base, suffix)) = parsed.path().rsplit_once('=') {
        let tokens = suffix
            .split('-')
            .map(|token| {
                if token.len() > 1
                    && matches!(token.as_bytes()[0], b's' | b'w' | b'h')
                    && token[1..].bytes().all(|c| c.is_ascii_digit())
                {
                    format!("{}{size}", &token[..1])
                } else {
                    token.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("-");
        if tokens == suffix {
            return normalized.into_owned();
        }
        let path = format!("{base}={tokens}");
        parsed.set_path(&path);
    }
    parsed.into()
}
fn video_id(url: &str) -> Option<&str> {
    let base = url.split('?').next()?;
    let pos = base
        .find("/vi/")
        .map(|p| p + 4)
        .or_else(|| base.find("/vi_webp/").map(|p| p + 9))?;
    let id = base[pos..].split('/').next()?;
    if !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        Some(id)
    } else {
        None
    }
}
/// Shared GUI/TUI source loading. HTTP concurrency and decode dimensions are bounded.
pub async fn fetch_thumbnail(
    url: &str,
    width: u32,
    height: u32,
    path: &std::path::Path,
) -> Option<DynamicImage> {
    static PERMITS: OnceLock<Semaphore> = OnceLock::new();
    let _permit = PERMITS
        .get_or_init(|| Semaphore::new(12))
        .acquire()
        .await
        .ok()?;
    let disk = path.to_owned();
    if let Some(image) = tokio::task::spawn_blocking(move || {
        let image = image::open(&disk).ok()?;
        touch(&disk);
        Some(image)
    })
    .await
    .ok()
    .flatten()
    {
        return Some(image);
    }
    let client = crate::http::client();
    for candidate in thumbnail_candidates(url, width) {
        let Ok(response) = client
            .get(candidate)
            .timeout(Duration::from_secs(3))
            .send()
            .await
        else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        if response
            .content_length()
            .is_some_and(|n| n > 4 * 1024 * 1024)
        {
            continue;
        }
        let Ok(bytes) = response.bytes().await else {
            continue;
        };
        if bytes.len() > 4 * 1024 * 1024 {
            continue;
        }
        let path = path.to_owned();
        let image = tokio::task::spawn_blocking(move || {
            let image = image::load_from_memory(&bytes).ok()?;
            let image = if image.width() > width || image.height() > height {
                let ratio = (width as f64 / image.width() as f64)
                    .min(height as f64 / image.height() as f64);
                resize(
                    &image,
                    (image.width() as f64 * ratio).round().max(1.) as u32,
                    (image.height() as f64 * ratio).round().max(1.) as u32,
                    false,
                )
            } else {
                image
            };
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = image.to_rgb8().save(&path);
            Some(image)
        })
        .await
        .ok()
        .flatten();
        if image.is_some() {
            return image;
        }
    }
    None
}
pub fn touch(path: &std::path::Path) {
    if let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) {
        let _ = file.set_modified(std::time::SystemTime::now());
    }
}
pub fn resize(image: &DynamicImage, width: u32, height: u32, cover: bool) -> DynamicImage {
    use fast_image_resize::{
        images::Image, FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer,
    };
    let pixels = image.to_rgba8();
    let Ok(source) = Image::from_vec_u8(
        image.width(),
        image.height(),
        pixels.into_raw(),
        PixelType::U8x4,
    ) else {
        return image.clone();
    };
    let mut target = Image::new(width.max(1), height.max(1), PixelType::U8x4);
    let mut options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Bilinear));
    if cover {
        options = options.fit_into_destination(Some((0.5, 0.5)));
    }
    if Resizer::new()
        .resize(&source, &mut target, &options)
        .is_err()
    {
        return image.resize_to_fill(
            width.max(1),
            height.max(1),
            image::imageops::FilterType::Triangle,
        );
    }
    image::RgbaImage::from_raw(width.max(1), height.max(1), target.into_vec())
        .map(DynamicImage::ImageRgba8)
        .unwrap_or_else(|| image.clone())
}

pub fn cache_key_for_url(url: &str) -> String {
    if let Some(pos) = url.find("/vi/") {
        let rest = &url[pos + 4..];
        if let Some(slash) = rest.find('/') {
            let id = &rest[..slash];
            if !id.is_empty() {
                return format!("yt_{id}");
            }
        }
    } else if let Some(pos) = url.find("/vi_webp/") {
        let rest = &url[pos + 9..];
        if let Some(slash) = rest.find('/') {
            let id = &rest[..slash];
            if !id.is_empty() {
                return format!("yt_{id}");
            }
        }
    }

    let clean = match url.split_once('?') {
        Some((base, _)) => base,
        None => url,
    };
    format!("{:x}", Sha256::digest(clean.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[tokio::test(flavor = "current_thread")]
    async fn thumbnail_requests_do_not_block_the_async_runtime() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/fixture.png", listener.local_addr().unwrap());
        let mut png = std::io::Cursor::new(Vec::new());
        DynamicImage::new_rgb8(8, 8)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let png = png.into_inner();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            let received = stream.read(&mut request).await.unwrap();
            assert!(received > 0);
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        png.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            stream.write_all(&png).await.unwrap();
        });
        let directory = tempfile::tempdir().unwrap();
        let mut loader = ThumbnailLoader::new(DiskCache::new_in(directory.path().to_owned(), 1));
        let path = loader
            .cache()
            .thumbs_dir()
            .join(format!("{}.jpg", source_cache_key(&url, 480, 360)));
        assert!(loader.get_image(&url).is_none());
        let image = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                if let Some((_, image)) = loader.poll_loaded().pop() {
                    break image;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("thumbnail task must allow other futures to run");
        assert_eq!((image.width(), image.height()), (8, 8));
        server.await.unwrap();
        let _ = std::fs::remove_file(path);
    }
}

pub fn opaque_cache_key(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

pub fn card_thumbnail_url(card: &crate::backend::models::Card) -> std::borrow::Cow<'_, str> {
    if !card.thumb_url().is_empty() {
        return normalize_thumbnail_url(card.thumb_url());
    }
    let id = match card {
        crate::backend::models::Card::Video(video) => Some(video.id.as_str()),
        crate::backend::models::Card::Playlist(playlist) => playlist.first_video_id.as_deref(),
        _ => None,
    };
    if let Some(id) = id.filter(|id| {
        id.len() == 11
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    }) {
        format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg").into()
    } else {
        "".into()
    }
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[test]
    fn variants_and_size_keys_are_bounded_and_distinct() {
        let candidates =
            thumbnail_candidates("https://i.ytimg.com/vi/abcdefghijk/maxresdefault.jpg", 280);
        assert!(candidates[0].ends_with("mqdefault.jpg"));
        assert!(candidates.iter().any(|s| s.ends_with("hqdefault.jpg")));
        assert!(candidates.iter().all(|s| !s.contains("maxres")));
        assert_ne!(
            source_cache_key(&candidates[0], 320, 180),
            source_cache_key(&candidates[0], 480, 270)
        );
        assert_ne!(
            opaque_cache_key("sixel:10x20:28x8"),
            opaque_cache_key("kitty:10x20:28x8")
        );
        assert_eq!(
            thumbnail_candidates("//cdn.example.invalid/avatar", 80),
            vec!["https://cdn.example.invalid/avatar"]
        );
    }
    #[test]
    fn priority_queue_prefers_visible_and_preserves_order() {
        let make = |priority, sequence| Job {
            key: SourceKey {
                url: String::new(),
                width: 320,
                height: 180,
            },
            path: PathBuf::new(),
            priority,
            sequence,
            cancelled: CancellationToken::new(),
        };
        let mut queue = BinaryHeap::new();
        queue.push(make(Priority::Prefetch, 0));
        queue.push(make(Priority::Visible, 2));
        queue.push(make(Priority::Visible, 1));
        assert_eq!(queue.pop().unwrap().sequence, 1);
        assert_eq!(queue.pop().unwrap().sequence, 2);
        assert_eq!(queue.pop().unwrap().sequence, 0);
    }
    #[tokio::test]
    async fn cancellation_discards_queued_offscreen_requests_and_notifies_completion() {
        let directory = tempfile::tempdir().unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mut png = std::io::Cursor::new(Vec::new());
        DynamicImage::new_rgb8(8, 8)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let png = png.into_inner();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            assert!(stream.read(&mut request).await.unwrap() > 0);
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        png.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            stream.write_all(&png).await.unwrap();
            assert!(
                tokio::time::timeout(Duration::from_millis(100), listener.accept())
                    .await
                    .is_err()
            );
        });
        let mut loader = ThumbnailLoader::new(DiskCache::new_in(directory.path().to_owned(), 1));
        for n in 0..20 {
            loader.prefetch(&format!("http://{address}/image-{n}.png"), 320, 180);
        }
        let visible = format!("http://{address}/image-0.png");
        loader.retain_wanted(&std::collections::HashSet::from([visible.clone()]));
        let notify = loader.notifier();
        let loaded = tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                notify.notified().await;
                let loaded = loader.poll_loaded();
                if !loaded.is_empty() {
                    break loaded;
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].0, visible);
        server.await.unwrap();
    }
}
