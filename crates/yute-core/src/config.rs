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
    pub thumbnails: ThumbnailConfig,
    #[serde(default)]
    pub local: LocalConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub text: TextConfig,
    #[serde(default)]
    pub downloads: DownloadConfig,
}

impl Config {
    pub fn strip_emoji(&self) -> bool {
        self.ui.strip_emoji || self.general.strip_emoji
    }

    pub fn icons(&self) -> &str {
        self.ui.icons.as_deref().unwrap_or(&self.general.icons)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UiConfig {
    #[serde(default)]
    pub icons: Option<String>,
    #[serde(default)]
    pub strip_emoji: bool,
    #[serde(default = "auto_mode")]
    pub action_bar: String,
    #[serde(default)]
    pub sidebar_channels: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default)]
    pub start_anonymous: bool,
    pub graphics: String,
    pub icons: String,
    pub theme: String,
    #[serde(default = "dark_theme")]
    pub theme_dark: String,
    #[serde(default = "light_theme")]
    pub theme_light: String,
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
            icons: "auto".to_string(),
            theme: "terminal".to_string(),
            theme_dark: dark_theme(),
            theme_light: light_theme(),
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
    0
}

fn default_show_shorts() -> bool {
    true
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            columns: 0,
            min_card_width: 30,
            show_shorts: true,
            sidebar_expanded: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct PlaybackConfig {
    pub default_quality: String,
    pub preferred_codec: String,
    pub preferred_audio_codec: String,
    pub audio_language: String,
    pub subtitle_language: String,
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
            default_quality: "best".into(),
            preferred_codec: "auto".into(),
            preferred_audio_codec: "auto".into(),
            audio_language: "auto".into(),
            subtitle_language: "auto".into(),
            use_user_mpv_config: false,
            ytdl_format: "bestvideo[height<=1080]+bestaudio/best[height<=1080]".to_string(),
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
        if self.max_resolution == "manual" {
            return self.ytdl_format.clone();
        }
        let maximum = self
            .max_resolution
            .trim_end_matches('p')
            .parse::<u32>()
            .ok();
        let requested = self
            .default_quality
            .trim_end_matches('p')
            .parse::<u32>()
            .ok();
        let height = match (maximum, requested) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let filter = height.map(|h| format!("[height<={h}]")).unwrap_or_default();
        let audio = match self.preferred_audio_codec.as_str() {
            "opus" => "bestaudio[acodec=opus]/bestaudio",
            "aac" => "bestaudio[acodec^=mp4a]/bestaudio",
            _ => "bestaudio",
        };
        let base = if audio == "bestaudio" {
            format!("bestvideo{filter}+bestaudio/best{filter}")
        } else {
            format!("bestvideo{filter}+({audio})/best{filter}")
        };
        let codec = match self.preferred_codec.as_str() {
            "av1" => Some("av01"),
            "vp9" => Some("vp9"),
            "h264" => Some("avc"),
            _ => None,
        };
        codec
            .map(|codec| format!("bestvideo{filter}[vcodec^={codec}]+bestaudio/{base}"))
            .unwrap_or(base)
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
#[serde(default)]
pub struct ThumbnailConfig {
    pub cache_mb: u64,
    pub prefetch_pages: usize,
}
impl Default for ThumbnailConfig {
    fn default() -> Self {
        Self {
            cache_mb: 256,
            prefetch_pages: 1,
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
            thumbnails: ThumbnailConfig::default(),
            local: LocalConfig::default(),
            ui: UiConfig::default(),
            text: TextConfig::default(),
            downloads: DownloadConfig::default(),
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
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("yute")
    } else if let Some(home) = std::env::var_os("HOME") {
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
        Self::load_checked().unwrap_or_default()
    }

    /// Diagnostic callers must not mistake malformed configuration for defaults.
    pub fn load_checked() -> anyhow::Result<Self> {
        let dir = config_dir();
        let preferred = dir.join("config.toml");
        let path = if preferred.exists() {
            preferred
        } else {
            resolve_path("~/.config/ytui/config.toml")
        };
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)
            .map_err(|_| anyhow::anyhow!("Cannot read config.toml; check file permissions"))?;
        let mut cfg = Self::parse_checked(&content)?;
        if cfg.backend.auth == "cookies" {
            cfg.backend.auth = "tv_oauth".into();
        }
        if cfg.backend.cookies_file == "~/.config/ytui/cookies.txt"
            && dir.join("cookies.txt").exists()
        {
            cfg.backend.cookies_file = "~/.config/yute/cookies.txt".into();
        }
        Ok(cfg)
    }
    pub fn parse_checked(content: &str) -> anyhow::Result<Self> {
        toml::from_str(content).map_err(|_| anyhow::anyhow!("Invalid config.toml: malformed TOML, invalid field types or missing required fields (values redacted)"))
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

fn auto_mode() -> String {
    "auto".into()
}
fn dark_theme() -> String {
    "gruvbox-dark".into()
}
fn light_theme() -> String {
    "gruvbox-light".into()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct TextConfig {
    pub emoji: String,
}
impl Default for TextConfig {
    fn default() -> Self {
        Self { emoji: auto_mode() }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DownloadConfig {
    pub directory: String,
    pub video_quality: String,
    pub audio_mode: String,
    pub playlist_subdirectories: bool,
    pub write_metadata: bool,
    pub write_thumbnail: bool,
    pub write_subtitles: bool,
    pub keep_partial: bool,
}
impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            directory: "~/Videos/Yute".into(),
            video_quality: "1080p".into(),
            audio_mode: "best".into(),
            playlist_subdirectories: true,
            write_metadata: false,
            write_thumbnail: false,
            write_subtitles: false,
            keep_partial: true,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            icons: None,
            strip_emoji: false,
            action_bar: auto_mode(),
            sidebar_channels: false,
        }
    }
}
