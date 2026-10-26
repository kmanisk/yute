use crate::media::cache::DiskCache;
use image::DynamicImage;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

/// YouTube channel avatars can use protocol-relative CDN URLs.
pub fn normalize_thumbnail_url(url: &str) -> std::borrow::Cow<'_, str> {
    if url.starts_with("//") {
        format!("https:{url}").into()
    } else {
        url.into()
    }
}

pub struct ThumbnailLoader {
    cache: DiskCache,
    in_flight: HashSet<String>,
    failed: HashSet<String>,
    ram_lru: HashMap<String, DynamicImage>,
    lru_order: VecDeque<String>,
    max_ram_entries: usize,
    tx_req: tokio::sync::mpsc::UnboundedSender<(String, PathBuf)>,
    rx_res: Receiver<(String, Option<DynamicImage>)>,
}

impl ThumbnailLoader {
    pub fn new(cache: DiskCache) -> Self {
        let (tx_req, mut rx_req) = tokio::sync::mpsc::unbounded_channel::<(String, PathBuf)>();
        let (tx_res, rx_res) = channel::<(String, Option<DynamicImage>)>();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(8))
                    .build()
                    .unwrap_or_default();

                while let Some((url, cache_path)) = rx_req.recv().await {
                    let client_c = client.clone();
                    let tx_c = tx_res.clone();

                    tokio::spawn(async move {
                        if cache_path.exists() {
                            let path_c = cache_path.clone();
                            let loaded =
                                tokio::task::spawn_blocking(move || image::open(&path_c).ok())
                                    .await
                                    .unwrap_or(None);

                            if let Some(img) = loaded {
                                let _ = tx_c.send((url, Some(img)));
                                return;
                            }
                        }

                        if let Ok(resp) = client_c
                            .get(normalize_thumbnail_url(&url).as_ref())
                            .send()
                            .await
                        {
                            if let Ok(bytes) = resp.bytes().await {
                                let path_c = cache_path.clone();
                                let decoded = tokio::task::spawn_blocking(move || {
                                    if let Ok(img) = image::load_from_memory(&bytes) {
                                        let thumb = if img.width() > 640 || img.height() > 360 {
                                            img.thumbnail(640, 360)
                                        } else {
                                            img
                                        };
                                        let _ = thumb.save(&path_c);
                                        Some(thumb)
                                    } else {
                                        None
                                    }
                                })
                                .await
                                .unwrap_or(None);

                                if let Some(img) = decoded {
                                    let _ = tx_c.send((url, Some(img)));
                                    return;
                                }
                            }
                        }
                        let _ = tx_c.send((url, None));
                    });
                }
            });
        }

        Self {
            cache,
            in_flight: HashSet::new(),
            failed: HashSet::new(),
            ram_lru: HashMap::new(),
            lru_order: VecDeque::new(),
            max_ram_entries: 200,
            tx_req,
            rx_res,
        }
    }

    pub fn poll_loaded(&mut self) -> Vec<(String, DynamicImage)> {
        let mut loaded = Vec::new();
        while let Ok((url, maybe_img)) = self.rx_res.try_recv() {
            self.in_flight.remove(&url);
            if let Some(img) = maybe_img {
                self.insert_lru(url.clone(), img.clone());
                loaded.push((url, img));
            } else {
                self.failed.insert(url);
            }
        }
        loaded
    }

    pub fn is_cached(&self, url: &str) -> bool {
        self.ram_lru.contains_key(url)
            || self
                .cache
                .thumbs_dir()
                .join(format!("{}.jpg", cache_key_for_url(url)))
                .is_file()
    }
    pub fn get_image(&mut self, url: &str) -> Option<&DynamicImage> {
        if self.ram_lru.contains_key(url) {
            if let Some(pos) = self.lru_order.iter().position(|u| u == url) {
                self.lru_order.remove(pos);
            }
            self.lru_order.push_back(url.to_string());
            return self.ram_lru.get(url);
        }

        if !self.in_flight.contains(url) && !self.failed.contains(url) && !url.is_empty() {
            self.in_flight.insert(url.to_string());
            let key = cache_key_for_url(url);
            let cache_path = self.cache.thumbs_dir().join(format!("{key}.jpg"));
            let _ = self.tx_req.send((url.to_string(), cache_path));
        }

        None
    }

    fn insert_lru(&mut self, url: String, img: DynamicImage) {
        if self.ram_lru.len() >= self.max_ram_entries {
            if let Some(oldest) = self.lru_order.pop_front() {
                self.ram_lru.remove(&oldest);
            }
        }
        self.lru_order.push_back(url.clone());
        self.ram_lru.insert(url, img);
    }

    pub fn cache(&self) -> &DiskCache {
        &self.cache
    }
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
        let mut loader = ThumbnailLoader::new(DiskCache::new(1));
        let path = loader
            .cache()
            .thumbs_dir()
            .join(format!("{}.jpg", cache_key_for_url(&url)));
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
