use crate::media::cache::DiskCache;
use image::DynamicImage;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

pub struct ThumbnailManager {
    picker: Picker,
    cache: DiskCache,
    in_flight: HashSet<String>,
    ram_lru: HashMap<String, StatefulProtocol>,
    lru_order: VecDeque<String>,
    max_ram_entries: usize,
    tx_req: Sender<(String, PathBuf)>,
    rx_res: Receiver<(String, Option<DynamicImage>)>,
}

impl ThumbnailManager {
    pub fn new(picker: Picker, cache: DiskCache) -> Self {
        let (tx_req, rx_req) = channel::<(String, PathBuf)>();
        let (tx_res, rx_res) = channel::<(String, Option<DynamicImage>)>();

        // Spawn async worker on the tokio runtime if active
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(8))
                    .build()
                    .unwrap_or_default();

                while let Ok((url, cache_path)) = rx_req.recv() {
                    let client_c = client.clone();
                    let tx_c = tx_res.clone();

                    tokio::spawn(async move {
                        if cache_path.exists() {
                            let path_c = cache_path.clone();
                            let loaded = tokio::task::spawn_blocking(move || {
                                image::open(&path_c).ok()
                            }).await.unwrap_or(None);

                            if let Some(img) = loaded {
                                let _ = tx_c.send((url, Some(img)));
                                return;
                            }
                        }

                        if let Ok(resp) = client_c.get(&url).send().await {
                            if let Ok(bytes) = resp.bytes().await {
                                let path_c = cache_path.clone();
                                let decoded = tokio::task::spawn_blocking(move || {
                                    if let Ok(img) = image::load_from_memory(&bytes) {
                                        let _ = img.save(&path_c);
                                        Some(img)
                                    } else {
                                        None
                                    }
                                }).await.unwrap_or(None);

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
            picker,
            cache,
            in_flight: HashSet::new(),
            ram_lru: HashMap::new(),
            lru_order: VecDeque::new(),
            max_ram_entries: 80,
            tx_req,
            rx_res,
        }
    }

    pub fn compute_thumbnail_rows(&self, card_cols: u16) -> u16 {
        let font_size = self.picker.font_size();
        let (cell_w, cell_h) = if font_size.width > 0 && font_size.height > 0 {
            (font_size.width as f32, font_size.height as f32)
        } else {
            (10.0, 20.0)
        };

        let rows = (card_cols as f32 * cell_w / cell_h * 9.0 / 16.0).round() as u16;
        rows.max(4).min(18)
    }

    pub fn poll_loaded(&mut self) -> bool {
        let mut loaded = false;
        while let Ok((url, maybe_img)) = self.rx_res.try_recv() {
            self.in_flight.remove(&url);
            if let Some(img) = maybe_img {
                let proto = self.picker.new_resize_protocol(img);
                self.insert_lru(url, proto);
                loaded = true;
            }
        }
        loaded
    }

    pub fn get_protocol(&mut self, url: &str) -> Option<&mut StatefulProtocol> {
        if self.ram_lru.contains_key(url) {
            if let Some(pos) = self.lru_order.iter().position(|u| u == url) {
                self.lru_order.remove(pos);
            }
            self.lru_order.push_back(url.to_string());
            return self.ram_lru.get_mut(url);
        }

        if !self.in_flight.contains(url) && !url.is_empty() {
            self.in_flight.insert(url.to_string());
            let hash = format!("{:x}", Sha256::digest(url.as_bytes()));
            let cache_path = self.cache.thumbs_dir().join(format!("{hash}.jpg"));
            let _ = self.tx_req.send((url.to_string(), cache_path));
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
        &self.cache
    }

    #[allow(dead_code)]
    pub fn set_picker(&mut self, picker: Picker) {
        self.picker = picker;
        self.ram_lru.clear();
        self.lru_order.clear();
    }
}
