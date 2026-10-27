use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

#[derive(Clone)]
pub struct DiskCache {
    base_dir: PathBuf,
    max_bytes: u64,
    pruning: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl DiskCache {
    pub fn new(max_mb: u64) -> Self {
        let base_dir = crate::config::cache_dir();
        let _ = fs::create_dir_all(base_dir.join("feeds"));
        let _ = fs::create_dir_all(base_dir.join("thumbs"));
        let _ = fs::create_dir_all(base_dir.join("debug"));

        Self {
            base_dir,
            max_bytes: max_mb * 1024 * 1024,
            pruning: Default::default(),
        }
    }

    pub fn new_in(base_dir: PathBuf, max_mb: u64) -> Self {
        let _ = fs::create_dir_all(base_dir.join("thumbs"));
        let _ = fs::create_dir_all(base_dir.join("feeds"));
        Self {
            base_dir,
            max_bytes: max_mb * 1024 * 1024,
            pruning: Default::default(),
        }
    }
    pub fn prune_background(&self) {
        use std::sync::atomic::Ordering;
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            if self.pruning.swap(true, Ordering::SeqCst) {
                return;
            }
            let cache = self.clone();
            runtime.spawn_blocking(move || {
                let _ = cache.prune_thumbnails();
                cache.pruning.store(false, Ordering::SeqCst);
            });
        }
    }
    pub fn with_limit(&self, max_mb: u64) -> Self {
        Self {
            base_dir: self.base_dir.clone(),
            max_bytes: max_mb.clamp(16, 2048) * 1024 * 1024,
            pruning: self.pruning.clone(),
        }
    }
    pub fn encoded_dir(&self) -> PathBuf {
        self.thumbs_dir().join("encoded")
    }
    pub fn feeds_dir(&self) -> PathBuf {
        self.base_dir.join("feeds")
    }

    pub fn thumbs_dir(&self) -> PathBuf {
        self.base_dir.join("thumbs")
    }

    pub fn debug_dir(&self) -> PathBuf {
        self.base_dir.join("debug")
    }

    #[allow(dead_code)]
    pub fn save_feed_cache(&self, key: &str, content: &str) -> Result<()> {
        let path = self.feeds_dir().join(format!("{key}.json"));
        crate::credentials::atomic_write(&path, content.as_bytes())?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn load_feed_cache(&self, key: &str, ttl_sec: u64) -> Option<String> {
        let path = self.feeds_dir().join(format!("{key}.json"));
        if !path.exists() {
            return None;
        }

        if ttl_sec > 0 {
            if let Ok(meta) = fs::metadata(&path) {
                if let Ok(modified) = meta.modified() {
                    if let Ok(elapsed) = modified.elapsed() {
                        if elapsed > Duration::from_secs(ttl_sec) {
                            return None;
                        }
                    }
                }
            }
        }

        fs::read_to_string(path).ok()
    }

    pub fn prune_thumbnails(&self) -> Result<()> {
        let thumbs = self.thumbs_dir();
        if !thumbs.exists() {
            return Ok(());
        }

        let mut files = Vec::new();
        let mut total_bytes = 0u64;

        for directory in [thumbs, self.encoded_dir()] {
            let Ok(entries) = fs::read_dir(directory) else {
                continue;
            };
            for entry in entries {
                let entry = entry?;
                let path = entry.path();
                if path.is_file()
                    && matches!(
                        path.extension().and_then(|s| s.to_str()),
                        Some("jpg" | "cover")
                    )
                {
                    let meta = entry.metadata()?;
                    let bytes = meta.len();
                    let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    total_bytes += bytes;
                    files.push((path, bytes, mtime));
                }
            }
        }

        if total_bytes > self.max_bytes {
            // Sort oldest first
            files.sort_by_key(|(_, _, mtime)| *mtime);
            for (path, bytes, _) in files {
                if total_bytes <= self.max_bytes {
                    break;
                }
                let _ = fs::remove_file(&path);
                total_bytes = total_bytes.saturating_sub(bytes);
            }
        }

        Ok(())
    }

    pub fn prune_feeds(&self, max_age_days: u64) -> Result<()> {
        let feeds = self.feeds_dir();
        if !feeds.exists() {
            return Ok(());
        }
        let cutoff = SystemTime::now() - Duration::from_secs(max_age_days * 86400);
        for entry in fs::read_dir(&feeds)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        if mtime < cutoff {
                            let _ = fs::remove_file(path);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        if self.base_dir.exists() {
            let _ = fs::remove_dir_all(&self.base_dir);
            let _ = fs::create_dir_all(self.feeds_dir());
            let _ = fs::create_dir_all(self.thumbs_dir());
            let _ = fs::create_dir_all(self.debug_dir());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_and_encoded_caches_share_access_order_and_size_budget() {
        let directory = tempfile::tempdir().unwrap();
        let cache = DiskCache::new_in(directory.path().to_owned(), 1);
        fs::create_dir_all(cache.encoded_dir()).unwrap();
        let files = [
            cache.thumbs_dir().join("old.jpg"),
            cache.encoded_dir().join("middle.cover"),
            cache.encoded_dir().join("recent.cover"),
        ];
        for (index, path) in files.iter().enumerate() {
            fs::write(path, vec![0; 600_000]).unwrap();
            fs::OpenOptions::new()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(index as u64))
                .unwrap();
        }
        cache.prune_thumbnails().unwrap();
        assert!(!files[0].exists());
        assert!(!files[1].exists());
        assert!(files[2].exists());
    }
}
