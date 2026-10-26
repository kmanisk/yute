use crate::backend::models::{Card, FeedPage, LiveState, VideoCard};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

pub fn scan_local_videos(dirs: &[String]) -> FeedPage {
    let extensions = ["mp4", "mkv", "webm", "mov", "avi"];
    let mut files: Vec<(PathBuf, u64, SystemTime)> = Vec::new();

    let mut dirs = dirs.to_vec();
    let downloads = "~/Videos/Yute".to_string();
    if !dirs.contains(&downloads) {
        dirs.push(downloads);
    }
    for dir_str in &dirs {
        let dir_path = crate::config::resolve_path(dir_str);
        if !dir_path.exists() {
            continue;
        }

        if let Ok(entries) = fs::read_dir(dir_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                        if extensions.iter().any(|&e| e.eq_ignore_ascii_case(ext)) {
                            let meta = entry.metadata().ok();
                            let len = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                            let mtime = meta
                                .as_ref()
                                .and_then(|m| m.modified().ok())
                                .unwrap_or(SystemTime::UNIX_EPOCH);
                            files.push((path, len, mtime));
                        }
                    }
                }
            }
        }
    }

    // Sort by mtime descending
    files.sort_by(|a, b| b.2.cmp(&a.2));

    let mut items = Vec::new();
    for (path, size, _) in files {
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("video")
            .to_string();
        let size_mb = size / (1024 * 1024);
        let path_str = path.to_string_lossy().to_string();

        items.push(Card::Video(VideoCard {
            id: path_str,
            title: filename,
            channel: format!("{size_mb} MB"),
            channel_id: None,
            verified: false,
            duration: None,
            views: None,
            published: None,
            thumb_url: String::new(),
            progress_pct: None,
            live: LiveState::None,
            badges: vec!["LOCAL".to_string()],
            resume_time_sec: None,
        }));
    }

    FeedPage {
        chips: Vec::new(),
        items,
        continuation: None,
    }
}
