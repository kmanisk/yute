use gtk4::gdk;
use gtk4::gio;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;
use yute_core::media::thumbs::source_cache_key;

#[derive(Clone)]
pub struct ThumbnailLoader {
    cache_dir: PathBuf,
    cache: yute_core::media::cache::DiskCache,
    ram_cache: Rc<RefCell<HashMap<String, gdk::Texture>>>,
    lru_order: Rc<RefCell<VecDeque<String>>>,
    max_ram_items: usize,
}

impl Default for ThumbnailLoader {
    fn default() -> Self {
        Self::new(150)
    }
}

impl ThumbnailLoader {
    pub fn new(max_ram_items: usize) -> Self {
        Self::new_with_budget(max_ram_items, 256)
    }
    pub fn new_with_budget(max_ram_items: usize, cache_mb: u64) -> Self {
        let cache = yute_core::media::cache::DiskCache::new(cache_mb).with_limit(cache_mb);
        let cache_dir = cache.thumbs_dir();
        let _ = std::fs::create_dir_all(&cache_dir);

        Self {
            cache_dir,
            cache,
            ram_cache: Rc::new(RefCell::new(HashMap::new())),
            lru_order: Rc::new(RefCell::new(VecDeque::new())),
            max_ram_items,
        }
    }

    pub fn load_thumbnail<F>(&self, url: &str, on_loaded: F)
    where
        F: FnOnce(gdk::Texture) + 'static,
    {
        let url = url.trim();
        if url.is_empty() {
            return;
        }

        // 1. Check in-memory cache
        if let Some(tex) = self.ram_cache.borrow().get(url) {
            on_loaded(tex.clone());
            return;
        }

        let key = source_cache_key(url, 480, 360);
        let disk_path = self.cache_dir.join(format!("{key}.jpg"));

        // 2. Check disk cache
        if disk_path.exists() {
            let file = gio::File::for_path(&disk_path);
            if let Ok(texture) = gdk::Texture::from_file(&file) {
                self.insert_ram(url.to_string(), texture.clone());
                on_loaded(texture);
                return;
            }
        }

        // 3. Download in background on Tokio
        let url_owned = url.to_string();
        let path_owned = disk_path.clone();
        let loader_clone = self.clone();

        let url_task = url_owned.clone();
        let path_task = path_owned.clone();

        crate::async_runtime::spawn_task(
            async move {
                let path = path_task.clone();
                yute_core::media::thumbs::fetch_thumbnail(&url_task, 480, 360, &path)
                    .await
                    .is_some()
            },
            move |success| {
                if success {
                    loader_clone.cache.prune_background();
                    let file = gio::File::for_path(&path_owned);
                    if let Ok(texture) = gdk::Texture::from_file(&file) {
                        loader_clone.insert_ram(url_owned, texture.clone());
                        on_loaded(texture);
                    }
                }
            },
        );
    }

    fn insert_ram(&self, url: String, texture: gdk::Texture) {
        let mut ram = self.ram_cache.borrow_mut();
        let mut lru = self.lru_order.borrow_mut();

        if ram.len() >= self.max_ram_items {
            if let Some(oldest) = lru.pop_front() {
                ram.remove(&oldest);
            }
        }

        lru.push_back(url.clone());
        ram.insert(url, texture);
    }
}
