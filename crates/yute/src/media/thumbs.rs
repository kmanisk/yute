use super::encoded::EncodedCover;
use ratatui::layout::Size;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufReader, Cursor};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Notify;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CoverKey {
    url: String,
    size: Size,
    generation: u64,
}
use yute_core::media::cache::DiskCache;
use yute_core::media::thumbs::ThumbnailLoader;

struct AnimatedCover {
    frames: Vec<EncodedCover>,
    delays: Vec<Duration>,
    index: usize,
    next_at: Instant,
}

type AnimatedResult = (CoverKey, Option<(Vec<EncodedCover>, Vec<Duration>)>);

pub struct ThumbnailManager {
    trace_requests: HashMap<CoverKey, (std::time::Instant, bool)>,
    trace_completed: HashSet<CoverKey>,
    generation: u64,
    picker: Picker,
    loader: ThumbnailLoader,
    ram_lru: HashMap<String, StatefulProtocol>,
    lru_order: VecDeque<String>,
    max_ram_entries: usize,
    covers: HashMap<CoverKey, EncodedCover>,
    cover_order: VecDeque<CoverKey>,
    cover_pending: HashSet<CoverKey>,
    cover_failed: HashSet<CoverKey>,
    disk_checked: HashSet<CoverKey>,
    wanted: HashSet<String>,
    animated: HashMap<CoverKey, AnimatedCover>,
    animated_order: VecDeque<CoverKey>,
    animated_pending: HashSet<CoverKey>,
    animated_failed: HashSet<CoverKey>,
    animated_wanted: HashSet<CoverKey>,
    animated_tx: Sender<AnimatedResult>,
    animated_rx: Receiver<AnimatedResult>,
    notify: Arc<Notify>,
    cover_tx: Sender<(CoverKey, Option<EncodedCover>, bool)>,
    cover_rx: Receiver<(CoverKey, Option<EncodedCover>, bool)>,
}

impl ThumbnailManager {
    pub fn new(picker: Picker, cache: DiskCache) -> Self {
        let (cover_tx, cover_rx) = channel();
        let (animated_tx, animated_rx) = channel();
        let loader = ThumbnailLoader::new(cache);
        let notify = loader.notifier();
        Self {
            disk_checked: HashSet::new(),
            wanted: HashSet::new(),
            animated: HashMap::new(),
            animated_order: VecDeque::new(),
            animated_pending: HashSet::new(),
            animated_failed: HashSet::new(),
            animated_wanted: HashSet::new(),
            animated_tx,
            animated_rx,
            notify,
            trace_requests: HashMap::new(),
            trace_completed: HashSet::new(),
            covers: HashMap::new(),
            cover_order: VecDeque::new(),
            cover_pending: HashSet::new(),
            cover_failed: HashSet::new(),
            cover_tx,
            cover_rx,
            generation: 0,
            picker,
            loader,
            ram_lru: HashMap::new(),
            lru_order: VecDeque::new(),
            max_ram_entries: 200,
        }
    }

    pub fn set_budget(&mut self, mb: u64) {
        self.loader.set_budget(mb);
    }
    pub fn notifier(&self) -> Arc<Notify> {
        self.notify.clone()
    }
    pub fn supports_partial(&self) -> bool {
        matches!(
            self.picker.protocol_type(),
            ratatui_image::picker::ProtocolType::Kitty
                | ratatui_image::picker::ProtocolType::Halfblocks
        )
    }
    pub fn invalidate_placements(&self) {
        for cover in self.covers.values() {
            cover.invalidate();
        }
    }
    pub fn begin_frame(&mut self) {
        self.wanted.clear();
        self.animated_wanted.clear();
    }
    pub fn finish_frame(&mut self, preloads: Vec<String>, width: u32, height: u32) {
        for (index, url) in preloads.into_iter().enumerate() {
            self.wanted.insert(url.clone());
            if index < 8 {
                self.loader.near_visible(&url, width, height);
            } else {
                self.loader.prefetch(&url, width, height);
            }
        }
        self.loader.retain_wanted(&self.wanted);
    }
    fn encoded_path(&self, key: &CoverKey) -> std::path::PathBuf {
        let font = self.picker.font_size();
        let descriptor = format!(
            "encoded-v2:{:?}:{}:{}:{}:{}:{}:{}",
            self.picker.protocol_type(),
            font.width,
            font.height,
            key.size.width,
            key.size.height,
            self.picker.tmux_detected(),
            key.url
        );
        self.loader.cache().encoded_dir().join(format!(
            "{}.cover",
            yute_core::media::thumbs::opaque_cache_key(&descriptor)
        ))
    }
    pub fn compute_thumbnail_rows(&self, card_cols: u16) -> u16 {
        let font_size = self.picker.font_size();
        let (cell_w, cell_h) = if font_size.width > 0 && font_size.height > 0 {
            (font_size.width as f32, font_size.height as f32)
        } else {
            (10.0, 20.0)
        };

        let rows = (card_cols as f32 * cell_w / cell_h * 9.0 / 16.0).round() as u16;
        rows.clamp(4, 7)
    }

    pub fn poll_loaded(&mut self) -> bool {
        let loaded_images = self.loader.poll_loaded();
        let mut has_loaded = !loaded_images.is_empty();
        while let Ok((key, protocol, disk)) = self.cover_rx.try_recv() {
            if key.generation != self.generation {
                continue;
            }
            self.cover_pending.remove(&key);
            has_loaded = true;
            if let Some(protocol) = protocol {
                if self.covers.len() >= 80 {
                    if let Some(oldest) = self.cover_order.pop_front() {
                        self.covers.remove(&oldest);
                    }
                }
                self.cover_order.push_back(key.clone());
                self.covers.insert(key, protocol);
            } else {
                if disk {
                    self.disk_checked.insert(key);
                } else {
                    self.cover_failed.insert(key);
                }
            }
        }
        while let Ok((key, result)) = self.animated_rx.try_recv() {
            self.animated_pending.remove(&key);
            if key.generation != self.generation {
                continue;
            }
            has_loaded = true;
            if let Some((frames, delays)) = result.filter(|(frames, _)| !frames.is_empty()) {
                let first_delay = delays
                    .first()
                    .copied()
                    .unwrap_or(Duration::from_millis(120));
                while self.animated.len() >= 8 {
                    let Some(oldest) = self.animated_order.pop_front() else {
                        break;
                    };
                    self.animated.remove(&oldest);
                }
                self.animated_order.retain(|existing| existing != &key);
                self.animated_order.push_back(key.clone());
                self.animated.insert(
                    key,
                    AnimatedCover {
                        frames,
                        delays,
                        index: 0,
                        next_at: Instant::now() + first_delay,
                    },
                );
            } else {
                if self.animated_failed.len() >= 64 {
                    self.animated_failed.clear();
                }
                self.animated_failed.insert(key);
            }
        }
        for (url, img) in loaded_images {
            let proto = self.picker.new_resize_protocol((*img).clone());
            self.insert_lru(url, proto);
        }
        has_loaded
    }

    /// Decode cache is shared with other views. Exact-size cover preparation and
    /// graphics encoding run only on a cache miss, away from the UI thread.
    pub fn get_cover_protocol(&mut self, url: &str, size: Size) -> Option<&EncodedCover> {
        if size.width == 0 || size.height == 0 {
            return None;
        }
        self.wanted.insert(url.into());
        let key = CoverKey {
            url: url.to_string(),
            size,
            generation: self.generation,
        };
        let font = self.picker.font_size();
        let encoded_path = self.encoded_path(&key);
        if self.cover_pending.len() >= 16 && !self.covers.contains_key(&key) {
            return None;
        }
        if !self.disk_checked.contains(&key)
            && !self.covers.contains_key(&key)
            && !self.cover_pending.contains(&key)
        {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                self.cover_pending.insert(key.clone());
                let key = key.clone();
                let tx = self.cover_tx.clone();
                let notify = self.notify.clone();
                runtime.spawn_blocking(move || {
                    let result = EncodedCover::load(&encoded_path, key.size);
                    let _ = tx.send((key, result, true));
                    notify.notify_one();
                });
            }
        }
        if yute_core::trace::enabled() && !self.trace_completed.contains(&key) {
            let warm = self.loader.is_cached_for(
                url,
                size.width as u32 * font.width as u32,
                size.height as u32 * font.height as u32,
            ) || self.encoded_path(&key).is_file();
            self.trace_requests
                .entry(key.clone())
                .or_insert((std::time::Instant::now(), warm));
        }
        if self.covers.contains_key(&key) {
            if let Some((start, warm)) = self.trace_requests.remove(&key) {
                self.trace_completed.insert(key.clone());
                yute_core::trace::duration(
                    if warm {
                        "thumbnail_warm_visible"
                    } else {
                        "thumbnail_cold_visible"
                    },
                    start.elapsed(),
                    1,
                );
            }
            self.cover_order.retain(|k| k != &key);
            self.cover_order.push_back(key.clone());
            return self.covers.get(&key);
        }
        if self.cover_pending.contains(&key)
            || self.cover_failed.contains(&key)
            || self.cover_pending.len() >= 16
        {
            return None;
        }
        let image = self.loader.get_shared(
            url,
            u32::from(size.width) * u32::from(font.width),
            u32::from(size.height) * u32::from(font.height),
        )?;
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return None;
        };
        self.cover_pending.insert(key.clone());
        let picker = self.picker.clone();
        let tx = self.cover_tx.clone();
        let notify = self.notify.clone();
        let encoded_path = self.encoded_path(&key);
        let cache = self.loader.cache().clone();
        runtime.spawn_blocking(move || {
            let font = picker.font_size();
            let image = cover_image(
                &image,
                u32::from(size.width) * u32::from(font.width.max(1)),
                u32::from(size.height) * u32::from(font.height.max(1)),
            );
            let protocol = picker
                .new_protocol(image, size, ratatui_image::Resize::Crop(None))
                .ok()
                .map(|protocol| EncodedCover::new(&protocol, size));
            if let Some(protocol) = &protocol {
                protocol.save(&encoded_path);
                cache.prune_background();
            }
            let _ = tx.send((key, protocol, false));
            notify.notify_one();
        });
        None
    }

    /// Animated comment media is loaded only when visible. Decoding and terminal
    /// encoding are bounded so a GIF cannot create an unbounded redraw or memory loop.
    pub fn get_animated_cover_protocol(&mut self, url: &str, size: Size) -> Option<&EncodedCover> {
        if size.width == 0 || size.height == 0 {
            return None;
        }
        let key = CoverKey {
            url: url.to_string(),
            size,
            generation: self.generation,
        };
        self.animated_wanted.insert(key.clone());
        if let Some(animation) = self.animated.get_mut(&key) {
            if animation.frames.len() > 1 && Instant::now() >= animation.next_at {
                animation.index = (animation.index + 1) % animation.frames.len();
                animation.frames[animation.index].invalidate();
                let delay = animation
                    .delays
                    .get(animation.index)
                    .copied()
                    .unwrap_or(Duration::from_millis(120));
                animation.next_at = Instant::now() + delay;
            }
            return animation.frames.get(animation.index);
        }
        if self.animated_pending.contains(&key) || self.animated_failed.contains(&key) {
            return None;
        }
        // At most four animated downloads/decodes may be in flight. Static
        // thumbnails use their own bounded pipeline and are never blocked by GIFs.
        if self.animated_pending.len() >= 4 {
            return None;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return None;
        };
        self.animated_pending.insert(key.clone());
        let picker = self.picker.clone();
        let tx = self.animated_tx.clone();
        let notify = self.notify.clone();
        let request_key = key.clone();
        runtime.spawn(async move {
            let bytes = match yute_core::http::client()
                .get(&request_key.url)
                .timeout(Duration::from_secs(12))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => response.bytes().await.ok(),
                _ => None,
            };
            let result = if let Some(bytes) = bytes.filter(|bytes| bytes.len() <= 12 * 1024 * 1024)
            {
                let size = request_key.size;
                tokio::task::spawn_blocking(move || decode_animation(&bytes, picker, size))
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            };
            let _ = tx.send((request_key, result));
            notify.notify_one();
        });
        None
    }

    pub fn has_active_animation(&self) -> bool {
        self.animated_wanted.iter().any(|key| {
            self.animated
                .get(key)
                .is_some_and(|animation| animation.frames.len() > 1)
        })
    }

    pub fn animation_due(&self) -> bool {
        let now = Instant::now();
        self.animated_wanted.iter().any(|key| {
            self.animated
                .get(key)
                .is_some_and(|animation| animation.frames.len() > 1 && now >= animation.next_at)
        })
    }

    pub fn is_portrait(&self, url: &str) -> bool {
        let Some(protocol) = self.ram_lru.get(url) else {
            return false;
        };
        let size = protocol.size_for(
            ratatui_image::Resize::Fit(None),
            ratatui::layout::Size::new(1000, 1000),
        );
        let font = self.picker.font_size();
        u32::from(size.height) * u32::from(font.height)
            > u32::from(size.width) * u32::from(font.width)
    }

    pub fn get_protocol(&mut self, url: &str) -> Option<&mut StatefulProtocol> {
        self.wanted.insert(url.into());
        if self.ram_lru.contains_key(url) {
            if let Some(pos) = self.lru_order.iter().position(|u| u == url) {
                self.lru_order.remove(pos);
            }
            self.lru_order.push_back(url.to_string());
            return self.ram_lru.get_mut(url);
        }

        if let Some(img) = self.loader.get_image(url) {
            let proto = self.picker.new_resize_protocol(img.clone());
            self.insert_lru(url.to_string(), proto);
            return self.ram_lru.get_mut(url);
        }

        None
    }

    fn insert_lru(&mut self, url: String, proto: StatefulProtocol) {
        if self.ram_lru.len() >= self.max_ram_entries {
            if let Some(oldest) = self.lru_order.pop_front() {
                self.ram_lru.remove(&oldest);
            }
        }
        self.lru_order.push_back(url.clone());
        self.ram_lru.insert(url, proto);
    }

    pub fn cache(&self) -> &DiskCache {
        self.loader.cache()
    }

    pub fn refresh_cell_size(&mut self, width: u16, height: u16) {
        if self.picker.font_size().width == width && self.picker.font_size().height == height {
            return;
        }
        let kind = self.picker.protocol_type();
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize((width, height).into());
        picker.set_protocol_type(kind);
        self.set_picker(picker);
    }

    pub fn set_picker(&mut self, picker: Picker) {
        if self.picker.protocol_type() == picker.protocol_type()
            && (
                self.picker.font_size().width,
                self.picker.font_size().height,
            ) == (picker.font_size().width, picker.font_size().height)
        {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        self.covers.clear();
        self.cover_order.clear();
        self.cover_pending.clear();
        self.cover_failed.clear();
        self.animated.clear();
        self.animated_order.clear();
        self.animated_pending.clear();
        self.animated_failed.clear();
        self.animated_wanted.clear();
        self.disk_checked.clear();
        self.trace_requests.clear();
        self.trace_completed.clear();
        self.picker = picker;
        self.ram_lru.clear();
        self.lru_order.clear();
    }
}

fn decode_animation(
    bytes: &[u8],
    picker: Picker,
    size: Size,
) -> Option<(Vec<EncodedCover>, Vec<Duration>)> {
    use image::AnimationDecoder;
    let format = image::guess_format(bytes).ok()?;
    let frames = match format {
        image::ImageFormat::Gif => {
            let decoder =
                image::codecs::gif::GifDecoder::new(BufReader::new(Cursor::new(bytes))).ok()?;
            decoder
                .into_frames()
                .take(24)
                .collect::<image::ImageResult<Vec<_>>>()
                .ok()?
        }
        image::ImageFormat::WebP => {
            let decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes)).ok()?;
            decoder
                .into_frames()
                .take(24)
                .collect::<image::ImageResult<Vec<_>>>()
                .ok()?
        }
        _ => return None,
    };
    if frames.is_empty() {
        return None;
    }
    let font = picker.font_size();
    let pixel_w = u32::from(size.width) * u32::from(font.width.max(1));
    let pixel_h = u32::from(size.height) * u32::from(font.height.max(1));
    // Prevent pathological comment media from dominating encode time/RAM.
    if pixel_w.saturating_mul(pixel_h) > 1_500_000 {
        return None;
    }
    let mut encoded = Vec::with_capacity(frames.len());
    let mut delays = Vec::with_capacity(frames.len());
    for frame in frames {
        let (num, den) = frame.delay().numer_denom_ms();
        let ms = if den == 0 {
            120
        } else {
            (u64::from(num) / u64::from(den)).clamp(70, 1000)
        };
        let image = image::DynamicImage::ImageRgba8(frame.into_buffer());
        let image = yute_core::media::thumbs::resize(&image, pixel_w, pixel_h, false);
        let protocol = picker
            .new_protocol(image, size, ratatui_image::Resize::Fit(None))
            .ok()?;
        encoded.push(EncodedCover::new(&protocol, size));
        delays.push(Duration::from_millis(ms));
    }
    Some((encoded, delays))
}

pub use yute_core::media::thumbs::cache_key_for_url;

/// Center crop while retaining aspect ratio; no blur, stretch or synthetic canvas.
fn cover_image(image: &image::DynamicImage, width: u32, height: u32) -> image::DynamicImage {
    yute_core::media::thumbs::resize(
        &yute_core::media::thumbs::remove_thumbnail_bars(image),
        width,
        height,
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cover_fills_exact_pixels_and_crops_without_stretching() {
        let mut image = image::RgbImage::from_pixel(100, 200, image::Rgb([255, 0, 0]));
        for y in 50..150 {
            for x in 0..100 {
                image.put_pixel(x, y, image::Rgb([0, 255, 0]));
            }
        }
        let covered = cover_image(&image.into(), 160, 90).to_rgb8();
        assert_eq!(covered.dimensions(), (160, 90));
        assert_eq!(*covered.get_pixel(80, 45), image::Rgb([0, 255, 0]));
        assert_eq!(*covered.get_pixel(0, 0), image::Rgb([0, 255, 0]));
    }
}
