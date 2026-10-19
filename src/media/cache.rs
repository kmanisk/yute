use anyhow::Result;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

pub struct DiskCache {
    base_dir: PathBuf,
    max_bytes: u64,
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
        }
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
        fs::write(path, content)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn load_feed_cache(&self, key: &str, ttl_min: u64) -> Option<String> {
        let path = self.feeds_dir().join(format!("{key}.json"));
        if !path.exists() {
            return None;
        }

        if let Ok(meta) = fs::metadata(&path) {
            if let Ok(modified) = meta.modified() {
                if let Ok(age) = SystemTime::now().duration_since(modified) {
                    if age > Duration::from_secs(ttl_min * 60) {
                        return None;
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

        for entry in fs::read_dir(&thumbs)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let meta = entry.metadata()?;
                let bytes = meta.len();
                let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                total_bytes += bytes;
                files.push((path, bytes, mtime));
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
