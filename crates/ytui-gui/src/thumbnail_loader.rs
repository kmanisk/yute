use gtk4::gdk;
use gtk4::gio;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::rc::Rc;
use ytui_core::media::thumbs::cache_key_for_url;

#[derive(Clone)]
pub struct ThumbnailLoader {
    cache_dir: PathBuf,
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
        let cache_dir = ytui_core::config::cache_dir().join("thumbs");
        let _ = std::fs::create_dir_all(&cache_dir);

        Self {
            cache_dir,
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

        let key = cache_key_for_url(url);
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
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(8))
                    .build()
                    .unwrap_or_default();

                if let Ok(resp) = client.get(&url_task).send().await {
                    if let Ok(bytes) = resp.bytes().await {
                        let _ = std::fs::write(&path_task, &bytes);
                        return true;
                    }
                }
                false
            },
            move |success| {
                if success {
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
