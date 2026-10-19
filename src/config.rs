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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeneralConfig {
    pub graphics: String,
    pub icons: String,
    pub theme: String,
    pub mouse: bool,
    pub stop_mpv_on_quit: bool,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            graphics: "auto".to_string(),
            icons: "nerd".to_string(),
            theme: "youtube-dark".to_string(),
            mouse: false,
            stop_mpv_on_quit: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackendConfig {
    pub order: Vec<String>,
    pub cookies_file: String,
    pub cookies_from_browser: String,
    pub client_version: String,
    pub hl: String,
    pub gl: String,
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            order: vec!["innertube".to_string(), "ytdlp".to_string()],
            cookies_file: "~/.config/ytui/cookies.txt".to_string(),
            cookies_from_browser: String::new(),
            client_version: String::new(),
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
    pub min_card_width: u16,
    pub show_shorts: bool,
    pub sidebar_expanded: bool,
}

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            min_card_width: 38,
            show_shorts: false,
            sidebar_expanded: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaybackConfig {
    pub default_mode: String,
    pub use_user_mpv_config: bool,
    pub ytdl_format: String,
    pub mark_watched: bool,
    pub autoplay: bool,
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            default_mode: "window".to_string(),
            use_user_mpv_config: false,
            ytdl_format: "bestvideo[height<=1080]+bestaudio/best".to_string(),
            mark_watched: true,
            autoplay: true,
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
