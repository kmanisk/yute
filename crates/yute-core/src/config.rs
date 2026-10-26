use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub backend: BackendConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub grid: GridConfig,
    #[serde(default)]
    pub content: ContentConfig,
    #[serde(default)]
    pub playback: PlaybackConfig,
    #[serde(default)]
    pub chat: ChatConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub local: LocalConfig,
    #[serde(default)]
    pub ui: UiConfig,
}

impl Config {
    pub fn strip_emoji(&self) -> bool {
        self.ui.strip_emoji || self.general.strip_emoji
    }

    pub fn icons(&self) -> &str {
        self.ui.icons.as_deref().unwrap_or(&self.general.icons)
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default)]
    pub icons: Option<String>,
    #[serde(default)]
    pub strip_emoji: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default)]
    pub start_anonymous: bool,
    pub graphics: String,
    pub icons: String,
    pub theme: String,
    pub mouse: bool,
    pub stop_mpv_on_quit: bool,
    #[serde(default)]
    pub strip_emoji: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            start_anonymous: false,
            graphics: "auto".to_string(),
            icons: "ascii".to_string(),
            theme: "youtube-dark".to_string(),
            mouse: false,
            stop_mpv_on_quit: true,
            strip_emoji: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackendConfig {
    #[serde(default = "default_auth")]
    pub auth: String,
    #[serde(default = "default_client")]
    pub client: String,
    pub order: Vec<String>,
    pub cookies_file: String,
    pub cookies_from_browser: String,
    #[serde(default = "default_client_name")]
    pub client_name: String,
    #[serde(default = "default_client_version")]
    pub client_version: String,
    pub hl: String,
    pub gl: String,
}

fn default_auth() -> String {
    "guest".to_string()
}

fn default_client() -> String {
    "tv".to_string()
}

fn default_client_name() -> String {
    "TVHTML5".to_string()
}

fn default_client_version() -> String {
    "7.20260901.15.00".to_string()
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            auth: default_auth(),
            client: default_client(),
            order: vec!["innertube".to_string(), "ytdlp".to_string()],
            cookies_file: "~/.config/yute/cookies.txt".to_string(),
            cookies_from_browser: String::new(),
            client_name: default_client_name(),
            client_version: default_client_version(),
            hl: "en".to_string(),
            gl: "IN".to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ApiConfig {
    pub key: String,
    pub oauth_client_id: String,
    pub oauth_client_secret: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShortsVisibility {
    #[default]
    Hide,
    Show,
    Never,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ContentConfig {
    #[serde(default)]
    pub shorts: ShortsVisibility,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct GridConfig {
    #[serde(default = "default_columns")]
    pub columns: u16,
    pub min_card_width: u16,
    #[serde(default = "default_show_shorts")]
    pub show_shorts: bool,
    pub sidebar_expanded: bool,
}

fn default_columns() -> u16 {
    3
}

fn default_show_shorts() -> bool {
    true
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            columns: 3,
            min_card_width: 26,
            show_shorts: true,
            sidebar_expanded: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackConfig {
    pub use_user_mpv_config: bool,
    pub ytdl_format: String,
    #[serde(default = "default_max_resolution")]
    pub max_resolution: String,
    #[serde(default = "default_hwdec")]
    pub hwdec: String,
    #[serde(default = "default_osd_bar")]
    pub osd_bar: bool,
    #[serde(default = "default_stream_cache")]
    pub stream_cache_seconds: u16,
    pub mark_watched: bool,
    pub autoplay: bool,
}

fn default_stream_cache() -> u16 {
    20
}

fn default_max_resolution() -> String {
    "1080p".to_string()
}

fn default_hwdec() -> String {
    "auto-safe".to_string()
}

fn default_osd_bar() -> bool {
    false
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            use_user_mpv_config: false,
            ytdl_format: "bestvideo[height<=?1080]+bestaudio/best".to_string(),
            max_resolution: "1080p".to_string(),
            hwdec: "auto-safe".to_string(),
            osd_bar: false,
            stream_cache_seconds: default_stream_cache(),
            mark_watched: true,
            autoplay: true,
        }
    }
}

impl PlaybackConfig {
    /// Bounded memory budget paired with the selected readahead horizon.
    pub fn stream_cache_mb(&self) -> u16 {
        match self.stream_cache_seconds {
            0..=5 => 32,
            6..=10 => 64,
            11..=20 => 128,
            21..=40 => 192,
            _ => 256,
        }
    }

    pub fn resolved_ytdl_format(&self) -> String {
        match self.max_resolution.as_str() {
            "480p" => "bestvideo[height<=?480]+bestaudio/best[height<=?480]/best".to_string(),
            "720p" => "bestvideo[height<=?720]+bestaudio/best[height<=?720]/best".to_string(),
            "1080p" => "bestvideo[height<=?1080]+bestaudio/best[height<=?1080]/best".to_string(),
            "1440p" => "bestvideo[height<=?1440]+bestaudio/best[height<=?1440]/best".to_string(),
            "2160p" | "4k" => "bestvideo[height<=?2160]+bestaudio/best".to_string(),
            "best" | "max" => "bestvideo+bestaudio/best".to_string(),
            _ => {
                if !self.ytdl_format.is_empty() {
                    self.ytdl_format.clone()
                } else {
                    "bestvideo[height<=?1080]+bestaudio/best[height<=?1080]/best".to_string()
                }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatConfig {
    pub mode: String,
}

impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            mode: "script".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CacheConfig {
    pub max_mb: u64,
    pub home_ttl_min: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_mb: 200,
            home_ttl_min: 15,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LocalConfig {
    pub dirs: Vec<String>,
}

impl Default for LocalConfig {
    fn default() -> Self {
        Self {
            dirs: vec!["~/Videos".to_string()],
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            backend: BackendConfig::default(),
            api: ApiConfig::default(),
            grid: GridConfig::default(),
            content: ContentConfig::default(),
            playback: PlaybackConfig::default(),
            chat: ChatConfig::default(),
            cache: CacheConfig::default(),
            local: LocalConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

pub fn resolve_path(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(p)
}

pub fn config_dir() -> PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".config/yute")
    } else {
        PathBuf::from(".config/yute")
    }
}

pub fn cache_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(xdg).join("yute")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".cache/yute")
    } else {
        PathBuf::from(".cache/yute")
    }
}

impl Config {
    pub fn load() -> Self {
        let dir = config_dir();
        let preferred = dir.join("config.toml");
        let path = if preferred.exists() {
            preferred
        } else {
            resolve_path("~/.config/ytui/config.toml")
        };
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut cfg) = toml::from_str::<Config>(&content) {
                    if cfg.backend.cookies_file == "~/.config/ytui/cookies.txt"
                        && dir.join("cookies.txt").exists()
                    {
                        cfg.backend.cookies_file = "~/.config/yute/cookies.txt".into();
                    }
                    return cfg;
                }
            }
        }
        Config::default()
    }

    #[allow(dead_code)]
    pub fn save(&self) -> anyhow::Result<()> {
        let dir = config_dir();
        fs::create_dir_all(&dir)?;
        let path = dir.join("config.toml");
        let toml_str = toml::to_string_pretty(self)?;
        fs::write(path, toml_str)?;
        Ok(())
    }
}

/// Copy legacy settings without overwriting any Yute files. Encryption stays compatible.
pub fn migrate_legacy_config() -> std::io::Result<()> {
    let Some(home) = std::env::var_os("HOME") else {
        return Ok(());
    };
    migrate_config_from(&PathBuf::from(home).join(".config/ytui"), &config_dir())
}
fn migrate_config_from(old: &std::path::Path, new: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    if !old.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(new)?;
    fs::set_permissions(new, fs::Permissions::from_mode(0o700))?;
    for name in ["config.toml", "active_account.json", "theme.toml"] {
        let source = old.join(name);
        let dest = new.join(name);
        if source.is_file() && !dest.exists() {
            use std::io::Write;
            let data = fs::read(source)?;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(dest)?;
            file.write_all(&data)?;
        }
    }
    copy_legacy_profile(&old.join("mpv"), &new.join("mpv"))?;
    Ok(())
}
fn copy_legacy_profile(old: &std::path::Path, new: &std::path::Path) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    if !old.is_dir() {
        return Ok(());
    }
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(new)?;
    for entry in fs::read_dir(old)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let dest = new.join(entry.file_name());
        if kind.is_dir() {
            copy_legacy_profile(&entry.path(), &dest)?;
        } else if kind.is_file() && !dest.exists() {
            let data = fs::read(entry.path())?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(dest)?;
            file.write_all(&data)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod migration_tests {
    #[test]
    fn copies_private_legacy_files_without_overwriting_new_settings() {
        use super::*;
        let root = std::env::temp_dir().join(format!("yute-migration-{}", std::process::id()));
        let old = root.join("legacy");
        let new = root.join("current");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&new).unwrap();
        fs::write(old.join("auth.json"), "fixture-encrypted-data").unwrap();
        fs::write(old.join("config.toml"), "legacy").unwrap();
        fs::write(new.join("config.toml"), "current").unwrap();
        migrate_config_from(&old, &new).unwrap();
        assert_eq!(
            fs::read_to_string(new.join("config.toml")).unwrap(),
            "current"
        );
        assert!(
            !new.join("auth.json").exists(),
            "credentials migrate only through the encrypted store"
        );
        assert!(old.join("auth.json").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
