//! Query decisions and cached capabilities. terminal_input is the sole reader.
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
    let cells = report()
        .cells
        .or_else(|| {
            std::fs::read(cache_path())
                .ok()
                .and_then(|bytes| serde_json::from_slice::<(u16, u16)>(&bytes).ok())
                .filter(|(w, h)| *w > 0 && *h > 0)
        })
        .unwrap_or((10, 20));
    #[allow(deprecated)]
    let mut picker = Picker::from_fontsize(cells.into());
    let kind = graphics_kind(mode, &report(), multiplexed());
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

static REPORT: std::sync::OnceLock<std::sync::Mutex<crate::terminal_input::Report>> =
    std::sync::OnceLock::new();
pub fn report() -> crate::terminal_input::Report {
    REPORT.get_or_init(Default::default).lock().unwrap().clone()
}
pub fn remember_report(report: crate::terminal_input::Report) {
    *REPORT.get_or_init(Default::default).lock().unwrap() = report;
}
pub fn background_light() -> Option<bool> {
    report().light.or_else(|| {
        std::env::var("COLORFGBG")
            .ok()
            .and_then(|v| v.rsplit(';').next()?.parse::<u8>().ok())
            .map(|n| n == 7 || n >= 15)
    })
}
pub fn multiplexed() -> bool {
    std::env::var_os("TMUX").is_some() || std::env::var_os("ZELLIJ").is_some()
}
pub fn graphics_kind(mode: &str, r: &crate::terminal_input::Report, mux: bool) -> ProtocolType {
    if mux {
        return ProtocolType::Halfblocks;
    }
    let mode = std::env::var("YUTE_GRAPHICS").unwrap_or_else(|_| mode.into());
    match mode.as_str() {
        "kitty" => ProtocolType::Kitty,
        "sixel" => ProtocolType::Sixel,
        "iterm2" => ProtocolType::Iterm2,
        "auto" if r.kitty => ProtocolType::Kitty,
        "auto" if r.sixel => ProtocolType::Sixel,
        "auto"
            if r.iterm2
                && matches!(
                    std::env::var("TERM_PROGRAM").as_deref(),
                    Ok("iTerm.app" | "WezTerm")
                ) =>
        {
            ProtocolType::Iterm2
        }
        _ => ProtocolType::Halfblocks,
    }
}
pub fn query() {
    use std::io::Write;
    if multiplexed() {
        return;
    }
    let mut out = std::io::stdout();
    let _=out.write_all(b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[c\x1b]11;?\x1b\\\x1b[?996n\x1b[?2031h\x1b[16t\x1b]1337;ReportCellSize\x07");
    let _ = out.flush();
}
