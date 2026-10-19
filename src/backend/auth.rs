use anyhow::Result;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, COOKIE, ORIGIN, USER_AGENT};
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const DEFAULT_USER_AGENT: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36";
const YOUTUBE_ORIGIN: &str = "https://www.youtube.com";

#[derive(Clone, Debug)]
pub struct AuthContext {
    pub cookies: HashMap<String, String>,
    pub sapisid: Option<String>,
    pub cookie_header: Option<String>,
    pub user_agent: String,
    pub visitor_data: Option<String>,
    pub client_version: String,
    pub hl: String,
    pub gl: String,
}

impl Default for AuthContext {
    fn default() -> Self {
        Self {
            cookies: HashMap::new(),
            sapisid: None,
            cookie_header: None,
            user_agent: DEFAULT_USER_AGENT.to_string(),
            visitor_data: None,
            client_version: "2.20241018.01.00".to_string(),
            hl: "en".to_string(),
            gl: "IN".to_string(),
        }
    }
}

impl AuthContext {
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let mut ctx = Self::default();
        if !path.exists() {
            Self::try_auto_extract_brave_cookies(path);
        }

        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                ctx.parse_netscape_cookies(&content);
            }
        }

        // Auto-refresh from Brave if cookies are missing authenticated YouTube tokens
        if ctx.sapisid.is_none() || !ctx.cookies.contains_key("LOGIN_INFO") {
            Self::try_auto_extract_brave_cookies(path);
            if path.exists() {
                if let Ok(content) = fs::read_to_string(path) {
                    ctx.parse_netscape_cookies(&content);
                }
            }
        }

        Ok(ctx)
    }

    fn try_auto_extract_brave_cookies(target_path: &Path) {
        let home = std::env::var("HOME").unwrap_or_default();
        let candidates = [
            format!("{home}/.config/BraveSoftware/Brave-Origin"),
            format!("{home}/.config/BraveSoftware/Brave-Browser"),
        ];

        for cand in candidates {
            let cookie_db = format!("{cand}/Default/Cookies");
            if Path::new(&cookie_db).exists() {
                if let Some(parent) = target_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let status = std::process::Command::new("yt-dlp")
                    .arg("--cookies-from-browser")
                    .arg(format!("brave:{cand}"))
                    .arg("--cookies")
                    .arg(target_path)
                    .arg("--playlist-items")
                    .arg("0")
                    .arg("https://www.youtube.com")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status();

                if let Ok(st) = status {
                    if st.success() && target_path.exists() {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            let _ = fs::set_permissions(target_path, fs::Permissions::from_mode(0o600));
                        }
                        break;
                    }
                }
            }
        }
    }

    pub fn parse_netscape_cookies(&mut self, text: &str) {
        let mut yt_cookies = HashMap::new();
        let mut google_cookies = HashMap::new();

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 7 {
                let domain = parts[0].to_lowercase();
                let name = parts[5].trim().to_string();
                let value = parts[6].trim().to_string();

                if domain.contains("youtube.com") {
                    if name == "SAPISID" || name == "__Secure-3PAPISID" {
                        if self.sapisid.is_none() || name == "SAPISID" {
                            self.sapisid = Some(value.clone());
                        }
                    }
                    yt_cookies.insert(name, value);
                } else if domain == ".google.com" || domain == "google.com" || domain == ".google.co.in" {
                    if (name == "SAPISID" || name == "__Secure-3PAPISID") && self.sapisid.is_none() {
                        self.sapisid = Some(value.clone());
                    }
                    google_cookies.insert(name, value);
                }
            }
        }

        // YouTube domain cookies override general Google cookies
        self.cookies = google_cookies;
        for (k, v) in yt_cookies {
            self.cookies.insert(k, v);
        }

        if !self.cookies.is_empty() {
            let header_str = self
                .cookies
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("; ");
            self.cookie_header = Some(header_str);
        }
    }

    pub fn generate_sapisid_hash(&self) -> Option<String> {
        let sapisid = self.sapisid.as_ref()?;
        let now_sec = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let payload = format!("{} {} {}", now_sec, sapisid, YOUTUBE_ORIGIN);
        let mut hasher = Sha1::new();
        hasher.update(payload.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        Some(format!("SAPISIDHASH {}_{}", now_sec, hash))
    }

    pub fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(DEFAULT_USER_AGENT));
        headers.insert(ORIGIN, HeaderValue::from_static(YOUTUBE_ORIGIN));
        headers.insert("X-Origin", HeaderValue::from_static(YOUTUBE_ORIGIN));
        headers.insert("X-Goog-AuthUser", HeaderValue::from_static("0"));
        headers.insert("X-YouTube-Client-Name", HeaderValue::from_static("1"));

        if let Ok(val) = HeaderValue::from_str(&self.client_version) {
            headers.insert("X-YouTube-Client-Version", val);
        }

        if let Some(ref cookie_str) = self.cookie_header {
            if let Ok(val) = HeaderValue::from_str(cookie_str) {
                headers.insert(COOKIE, val);
            }
        }

        if let Some(auth_hash) = self.generate_sapisid_hash() {
            if let Ok(val) = HeaderValue::from_str(&auth_hash) {
                headers.insert(AUTHORIZATION, val);
            }
        }

        headers
    }

    pub fn build_client_context(&self) -> serde_json::Value {
        let mut client_obj = serde_json::json!({
            "clientName": "WEB",
            "clientVersion": self.client_version,
            "hl": self.hl,
            "gl": self.gl,
            "utcOffsetMinutes": 330,
        });

        if let Some(ref visitor) = self.visitor_data {
            client_obj["visitorData"] = serde_json::Value::String(visitor.clone());
        }

        serde_json::json!({
            "client": client_obj,
            "user": {
                "lockedSafetyMode": false
            }
        })
    }
}
