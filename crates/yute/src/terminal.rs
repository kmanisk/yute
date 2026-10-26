//! Terminal hints never read stdin; Crossterm remains its only owner.
use ratatui_image::picker::{Picker, ProtocolType};
use std::{path::PathBuf, time::Duration};
fn cache_path() -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    for key in ["TERM", "TERM_PROGRAM", "TERM_PROGRAM_VERSION", "TMUX"] {
        std::env::var(key).unwrap_or_default().hash(&mut hash);
    }
    yute_core::config::cache_dir().join(format!("terminal-{:016x}.json", hash.finish()))
}
pub fn initial_picker(mode: &str) -> Picker {
    let cells = std::fs::read(cache_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice::<(u16, u16)>(&bytes).ok())
        .filter(|(w, h)| *w > 0 && *h > 0)
        .unwrap_or((10, 20));
    #[allow(deprecated)]
    let mut picker = Picker::from_fontsize(cells.into());
    let term = std::env::var("TERM").unwrap_or_default();
    let program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    let kind = match mode {
        "sixel" => ProtocolType::Sixel,
        "kitty" => ProtocolType::Kitty,
        "halfblocks" | "none" => ProtocolType::Halfblocks,
        _ if std::env::var_os("KITTY_WINDOW_ID").is_some() || term.contains("kitty") => {
            ProtocolType::Kitty
        }
        _ if term.starts_with("foot") => ProtocolType::Sixel,
        _ if program == "WezTerm" => ProtocolType::Iterm2,
        _ => ProtocolType::Halfblocks,
    };
    picker.set_protocol_type(kind);
    picker
}
pub async fn cell_size() -> Option<(u16, u16)> {
    let query = tokio::task::spawn_blocking(crossterm::terminal::window_size);
    let size = tokio::time::timeout(Duration::from_millis(40), query)
        .await
        .ok()?
        .ok()?
        .ok()?;
    if size.columns == 0 || size.rows == 0 || size.width == 0 || size.height == 0 {
        return None;
    }
    Some((
        (size.width / size.columns).max(1),
        (size.height / size.rows).max(1),
    ))
}
pub fn remember_cells(width: u16, height: u16) {
    if let Ok(runtime) = tokio::runtime::Handle::try_current() {
        runtime.spawn_blocking(move || {
            if let Ok(bytes) = serde_json::to_vec(&(width, height)) {
                let path = cache_path();
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(path, bytes);
            }
        });
    }
}
