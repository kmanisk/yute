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
    pub playback: PlaybackConfig,
    #[serde(default)]
    pub inline: InlineConfig,
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
    "tv_oauth".to_string()
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
            cookies_file: "~/.config/ytui/cookies.txt".to_string(),
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

#[derive(Clone, Debug, Serialize, Deserialize)]
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
pub struct PlaybackConfig {
    pub default_mode: String,
    pub use_user_mpv_config: bool,
    pub ytdl_format: String,
    #[serde(default = "default_max_resolution")]
    pub max_resolution: String,
    #[serde(default = "default_hwdec")]
    pub hwdec: String,
    #[serde(default = "default_osd_bar")]
    pub osd_bar: bool,
    pub mark_watched: bool,
    pub autoplay: bool,
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
            default_mode: "window".to_string(),
            use_user_mpv_config: false,
            ytdl_format: "bestvideo[height<=?1080]+bestaudio/best".to_string(),
            max_resolution: "1080p".to_string(),
            hwdec: "auto-safe".to_string(),
            osd_bar: false,
            mark_watched: true,
            autoplay: true,
        }
    }
}

impl PlaybackConfig {
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
pub struct InlineConfig {
    pub max_height: u32,
    pub extra_args: Vec<String>,
}

impl Default for InlineConfig {
    fn default() -> Self {
        Self {
            max_height: 480,
            extra_args: Vec::new(),
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
            playback: PlaybackConfig::default(),
            inline: InlineConfig::default(),
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
        PathBuf::from(home).join(".config/ytui")
    } else {
        PathBuf::from(".config/ytui")
    }
}

pub fn cache_dir() -> PathBuf {
    if let Some(xdg) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(xdg).join("ytui")
    } else if let Some(home) = std::env::var_os("HOME") {
        PathBuf::from(home).join(".cache/ytui")
    } else {
        PathBuf::from(".cache/ytui")
    }
}

impl Config {
    pub fn load() -> Self {
        let dir = config_dir();
        let path = dir.join("config.toml");
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(cfg) = toml::from_str::<Config>(&content) {
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
