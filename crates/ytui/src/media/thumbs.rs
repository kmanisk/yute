use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use std::collections::{HashMap, VecDeque};
use ytui_core::media::cache::DiskCache;
use ytui_core::media::thumbs::ThumbnailLoader;

pub struct ThumbnailManager {
    picker: Picker,
    loader: ThumbnailLoader,
    ram_lru: HashMap<String, StatefulProtocol>,
    lru_order: VecDeque<String>,
    max_ram_entries: usize,
}

impl ThumbnailManager {
    pub fn new(picker: Picker, cache: DiskCache) -> Self {
        Self {
            picker,
            loader: ThumbnailLoader::new(cache),
            ram_lru: HashMap::new(),
            lru_order: VecDeque::new(),
            max_ram_entries: 200,
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
        rows.clamp(4, 7)
    }

    pub fn poll_loaded(&mut self) -> bool {
        let loaded_images = self.loader.poll_loaded();
        let has_loaded = !loaded_images.is_empty();
        for (url, img) in loaded_images {
            let proto = self.picker.new_resize_protocol(img);
            self.insert_lru(url, proto);
        }
        has_loaded
    }

    pub fn get_protocol(&mut self, url: &str) -> Option<&mut StatefulProtocol> {
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

    #[allow(dead_code)]
    pub fn set_picker(&mut self, picker: Picker) {
        self.picker = picker;
        self.ram_lru.clear();
        self.lru_order.clear();
    }
}

pub use ytui_core::media::thumbs::cache_key_for_url;
