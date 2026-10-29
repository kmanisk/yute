use crate::backend::models::{Card, FeedPage, LiveState, VideoCard};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Scan downloaded/local media without touching the network. The configured
/// yt-dlp download directory is always first; extra local directories are opt-in.
pub fn scan_local_videos(dirs: &[String], download_dir: &str) -> FeedPage {
    let extensions = ["mp4", "mkv", "webm", "mov", "avi", "m4v"];
    let mut files: Vec<(PathBuf, u64, SystemTime)> = Vec::new();
    let mut roots = Vec::with_capacity(dirs.len() + 1);
    roots.push(download_dir.to_string());
    roots.extend_from_slice(dirs);
    let mut seen_roots = HashSet::new();

    fn visit(
        path: &Path,
        extensions: &[&str],
        depth: usize,
        out: &mut Vec<(PathBuf, u64, SystemTime)>,
    ) {
        // Playlist downloads commonly create one subdirectory. A small bounded
        // recursion keeps Local deterministic and avoids walking an entire home tree.
        if depth > 4 {
            return;
        }
        let Ok(entries) = fs::read_dir(path) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, extensions, depth + 1, out);
                continue;
            }
            if !path.is_file() {
                continue;
            }
            let Some(ext) = path.extension().and_then(|s| s.to_str()) else {
                continue;
            };
            if !extensions
                .iter()
                .any(|candidate| candidate.eq_ignore_ascii_case(ext))
            {
                continue;
            }
            let meta = entry.metadata().ok();
            let len = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let mtime = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            out.push((path, len, mtime));
        }
    }

    for root in roots {
        let path = crate::config::resolve_path(&root);
        if !seen_roots.insert(path.clone()) || !path.is_dir() {
            continue;
        }
        visit(&path, &extensions, 0, &mut files);
    }

    files.sort_by_key(|a| std::cmp::Reverse(a.2));
    let items = files
        .into_iter()
        .map(|(path, size, _)| {
            let filename = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("video")
                .to_string();
            let size_mb = size / (1024 * 1024);
            Card::Video(VideoCard {
                id: path.to_string_lossy().to_string(),
                title: filename,
                channel: format!("{size_mb} MB · offline"),
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
            })
        })
        .collect();

    FeedPage {
        chips: Vec::new(),
        items,
        continuation: None,
    }
}
