use ratatui::layout::Size;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::Protocol;
use ratatui_image::protocol::StatefulProtocol;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{channel, Receiver, Sender};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CoverKey {
    url: String,
    size: Size,
    generation: u64,
}
use yute_core::media::cache::DiskCache;
use yute_core::media::thumbs::ThumbnailLoader;

pub struct ThumbnailManager {
    trace_requests: HashMap<CoverKey, (std::time::Instant, bool)>,
    trace_completed: HashSet<CoverKey>,
    generation: u64,
    picker: Picker,
    loader: ThumbnailLoader,
    ram_lru: HashMap<String, StatefulProtocol>,
    lru_order: VecDeque<String>,
    max_ram_entries: usize,
    covers: HashMap<CoverKey, Protocol>,
    cover_order: VecDeque<CoverKey>,
    cover_pending: HashSet<CoverKey>,
    cover_failed: HashSet<CoverKey>,
    cover_tx: Sender<(CoverKey, Option<Protocol>)>,
    cover_rx: Receiver<(CoverKey, Option<Protocol>)>,
}

impl ThumbnailManager {
    pub fn new(picker: Picker, cache: DiskCache) -> Self {
        let (cover_tx, cover_rx) = channel();
        Self {
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
        let mut has_loaded = !loaded_images.is_empty();
        while let Ok((key, protocol)) = self.cover_rx.try_recv() {
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
                self.cover_failed.insert(key);
            }
        }
        for (url, img) in loaded_images {
            let proto = self.picker.new_resize_protocol(img);
            self.insert_lru(url, proto);
        }
        has_loaded
    }

    /// Decode cache is shared with other views. Exact-size cover preparation and
    /// graphics encoding run only on a cache miss, away from the UI thread.
    pub fn get_cover_protocol(&mut self, url: &str, size: Size) -> Option<&Protocol> {
        if size.width == 0 || size.height == 0 {
            return None;
        }
        let key = CoverKey {
            url: url.to_string(),
            size,
            generation: self.generation,
        };
        if yute_core::trace::enabled() && !self.trace_completed.contains(&key) {
            self.trace_requests
                .entry(key.clone())
                .or_insert_with(|| (std::time::Instant::now(), self.loader.is_cached(url)));
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
        let image = self.loader.get_image(url)?.clone();
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return None;
        };
        self.cover_pending.insert(key.clone());
        let picker = self.picker.clone();
        let tx = self.cover_tx.clone();
        runtime.spawn_blocking(move || {
            let font = picker.font_size();
            let image = cover_image(
                &image,
                u32::from(size.width) * u32::from(font.width.max(1)),
                u32::from(size.height) * u32::from(font.height.max(1)),
            );
            let protocol = picker
                .new_protocol(image, size, ratatui_image::Resize::Crop(None))
                .ok();
            let _ = tx.send((key, protocol));
        });
        None
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
        let kind = self.picker.protocol_type();
        #[allow(deprecated)]
        let mut picker = Picker::from_fontsize((width, height).into());
        picker.set_protocol_type(kind);
        self.set_picker(picker);
    }

    pub fn set_picker(&mut self, picker: Picker) {
        self.generation = self.generation.wrapping_add(1);
        self.covers.clear();
        self.cover_order.clear();
        self.cover_pending.clear();
        self.cover_failed.clear();
        self.trace_requests.clear();
        self.trace_completed.clear();
        self.picker = picker;
        self.ram_lru.clear();
        self.lru_order.clear();
    }
}

pub use yute_core::media::thumbs::cache_key_for_url;

/// Center crop while retaining aspect ratio; no blur, stretch or synthetic canvas.
fn cover_image(image: &image::DynamicImage, width: u32, height: u32) -> image::DynamicImage {
    image.resize_to_fill(
        width.max(1),
        height.max(1),
        image::imageops::FilterType::Triangle,
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
