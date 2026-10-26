use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, COOKIE, ORIGIN, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn get_machine_encryption_key() -> [u8; 32] {
    let machine_id = fs::read_to_string("/etc/machine-id")
        .unwrap_or_else(|_| "ytui-machine-bound-salt-id".to_string());
    let user = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let mut hasher = Sha256::new();
    hasher.update(b"ytui-secret-key-salt-v1:");
    hasher.update(machine_id.trim().as_bytes());
    hasher.update(b":");
    hasher.update(user.as_bytes());
    let res = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&res);
    key
}

pub fn encrypt_auth_data(plaintext: &[u8]) -> Result<String> {
    let key_bytes = get_machine_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).map_err(|e| anyhow!("Cipher error: {e}"))?;

    let mut nonce_bytes = [0u8; 12];
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let now_bytes = now.to_le_bytes();
    let pid_bytes = pid.to_le_bytes();
    nonce_bytes[..8].copy_from_slice(&now_bytes[..8]);
    nonce_bytes[8..12].copy_from_slice(&pid_bytes[..4]);
    let nonce = Nonce::from(nonce_bytes);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| anyhow!("Encryption error: {e}"))?;

    let mut payload = Vec::with_capacity(12 + ciphertext.len());
    payload.extend_from_slice(&nonce_bytes);
    payload.extend_from_slice(&ciphertext);

    Ok(format!("YTUI_ENC_V1:{}", hex::encode(payload)))
}

pub fn decrypt_auth_data(encrypted_str: &str) -> Result<Vec<u8>> {
    let hex_part = encrypted_str.trim_start_matches("YTUI_ENC_V1:").trim();
    let payload = hex::decode(hex_part).map_err(|e| anyhow!("Hex decode error: {e}"))?;
    if payload.len() < 12 {
        return Err(anyhow!("Encrypted payload too short"));
    }

    let (nonce_slice, ciphertext) = payload.split_at(12);
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(nonce_slice);
    let key_bytes = get_machine_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key_bytes).map_err(|e| anyhow!("Cipher error: {e}"))?;
    let nonce = Nonce::from(nonce_bytes);

    let plaintext = cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|e| anyhow!("Decryption error: {e}"))?;

    Ok(plaintext)
}

pub const OAUTH_CLIENT_ID: &str =
    "861556708454-d6dlm3lh05idd8npek18k6be8ba3oc68.apps.googleusercontent.com";
pub const OAUTH_CLIENT_SECRET: &str = "SboVhoG9s0rNafixCSGGKXAT";
pub const OAUTH_SCOPE: &str =
    "http://gdata.youtube.com https://www.googleapis.com/auth/youtube-paid-content";
pub const OAUTH_GRANT_TYPE_DEVICE: &str = "http://oauth.net/grant_type/device/1.0";
pub const OAUTH_GRANT_TYPE_REFRESH: &str = "refresh_token";
pub const OAUTH_MODEL_NAME: &str = "ytlr::";
pub const OAUTH_DEVICE_CODE_URL: &str = "https://www.youtube.com/o/oauth2/device/code";
pub const OAUTH_TOKEN_URL: &str = "https://www.youtube.com/o/oauth2/token";

pub const TV_USER_AGENT: &str =
    "Mozilla/5.0 (ChromiumStylePlatform) Cobalt/25.lts.30.1034943-gold (unlike Gecko), Unknown_TV_Unknown_0/Unknown (Unknown, Unknown)";
pub const TV_ORIGIN: &str = "https://www.youtube.com";
pub const TV_REFERER: &str = "https://www.youtube.com/tv";
pub const DEFAULT_CLIENT_VERSION: &str = "7.20260901.15.00";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AuthAccount {
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub page_id: Option<String>,
    #[serde(default)]
    pub visitor_data: Option<String>,
    #[serde(default)]
    pub needs_login: bool,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub handle: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AuthStorage {
    #[serde(default)]
    pub accounts: HashMap<String, AuthAccount>,
    #[serde(default)]
    pub active_account: Option<String>,
    #[serde(default)]
    pub visitor_data: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_url: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceTokenResponse {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub token_type: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AuthContext {
    pub cookies: HashMap<String, String>,
    pub sapisid: Option<String>,
    pub cookie_header: Option<String>,
    pub user_agent: String,
    pub visitor_data: Option<String>,
    pub client_name: String,
    pub client_version: String,
    pub hl: String,
    pub gl: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub token_expires_at: Option<u64>,
    pub auth_file_path: Option<PathBuf>,
    pub page_id: Option<String>,
    pub account_id: Option<String>,
    pub needs_login: bool,
    pub refresh_lock: std::sync::Arc<tokio::sync::Mutex<()>>,
    pub cancelled: tokio_util::sync::CancellationToken,
}

impl Default for AuthContext {
    fn default() -> Self {
        Self {
            cookies: HashMap::new(),
            sapisid: None,
            cookie_header: None,
            user_agent: TV_USER_AGENT.to_string(),
            visitor_data: None,
            client_name: "TVHTML5".to_string(),
            client_version: DEFAULT_CLIENT_VERSION.to_string(),
            hl: "en".to_string(),
            gl: "IN".to_string(),
            access_token: None,
            refresh_token: None,
            token_expires_at: None,
            auth_file_path: None,
            page_id: None,
            account_id: None,
            needs_login: false,
            refresh_lock: std::sync::Arc::new(tokio::sync::Mutex::new(())),
            cancelled: tokio_util::sync::CancellationToken::new(),
        }
    }
}

impl AuthContext {
    pub fn load_auth(&mut self, auth_path: &Path, cookies_path: &Path) -> Result<()> {
        self.load_auth_with_mode(auth_path, cookies_path, "tv_oauth")
    }

    pub fn load_auth_with_mode(
        &mut self,
        auth_path: &Path,
        cookies_path: &Path,
        auth_mode: &str,
    ) -> Result<()> {
        self.auth_file_path = Some(auth_path.to_path_buf());
        match auth_mode {
            "guest" => {
                self.auth_file_path = None;
                self.visitor_data = None;
                self.page_id = None;
                self.account_id = None;
                self.needs_login = false;
                self.sapisid = None;
                self.token_expires_at = None;
                self.access_token = None;
                self.refresh_token = None;
                self.cookies.clear();
                self.cookie_header = None;
            }
            "cookies" => {
                let _ = self.load_cookies_from_path(cookies_path);
            }
            _ => {
                if auth_path.exists() {
                    let _ = self.load_auth_file(auth_path);
                }
                if self.access_token.is_none() {
                    let _ = self.load_cookies_from_path(cookies_path);
                }
            }
        }
        Ok(())
    }

    pub fn load_auth_file(&mut self, path: &Path) -> Result<()> {
        let content = fs::read_to_string(path)?;
        let (storage, was_plain) = if content.starts_with("YTUI_ENC_V1:") {
            let decrypted = decrypt_auth_data(&content)?;
            let parsed: AuthStorage = serde_json::from_slice(&decrypted)?;
            (parsed, false)
        } else {
            let parsed: AuthStorage = serde_json::from_str(&content)?;
            (parsed, true)
        };
        if let Some(ref vis) = storage.visitor_data {
            self.visitor_data = Some(vis.clone());
        }
        let active_key = storage.active_account.as_deref().unwrap_or("default");
        if let Some(acc) = storage.accounts.get(active_key) {
            self.access_token = Some(acc.access_token.clone());
            self.refresh_token = Some(acc.refresh_token.clone());
            self.token_expires_at = Some(acc.expires_at);
            self.account_id = Some(active_key.to_string());
            self.page_id = acc.page_id.clone();
            self.needs_login = acc.needs_login;
        } else if let Some((_, acc)) = storage.accounts.iter().next() {
            self.access_token = Some(acc.access_token.clone());
            self.refresh_token = Some(acc.refresh_token.clone());
            self.token_expires_at = Some(acc.expires_at);
        }
        if was_plain {
            let _ = self.save_auth_file(path);
        }
        Ok(())
    }

    pub fn save_auth_file(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut storage = if path.exists() {
            fs::read_to_string(path)
                .ok()
                .and_then(|s| {
                    if s.starts_with("YTUI_ENC_V1:") {
                        decrypt_auth_data(&s)
                            .ok()
                            .and_then(|b| serde_json::from_slice::<AuthStorage>(&b).ok())
                    } else {
                        serde_json::from_str::<AuthStorage>(&s).ok()
                    }
                })
                .unwrap_or_default()
        } else {
            AuthStorage::default()
        };

        if let Some(ref vis) = self.visitor_data {
            storage.visitor_data = Some(vis.clone());
        }

        if let (Some(ref acc_token), Some(ref ref_token)) =
            (&self.access_token, &self.refresh_token)
        {
            let active_key = self
                .account_id
                .clone()
                .or_else(|| storage.active_account.clone())
                .unwrap_or_else(|| "default".to_string());
            let exp = self.token_expires_at.unwrap_or_else(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    + 3600
            });
            storage.accounts.insert(
                active_key.clone(),
                AuthAccount {
                    access_token: acc_token.clone(),
                    refresh_token: ref_token.clone(),
                    expires_at: exp,
                    name: storage
                        .accounts
                        .get(&active_key)
                        .map(|a| a.name.clone())
                        .unwrap_or_default(),
                    handle: storage
                        .accounts
                        .get(&active_key)
                        .map(|a| a.handle.clone())
                        .unwrap_or_default(),
                    page_id: self.page_id.clone(),
                    visitor_data: self.visitor_data.clone(),
                    needs_login: self.needs_login,
                    avatar_url: storage
                        .accounts
                        .get(&active_key)
                        .and_then(|a| a.avatar_url.clone()),
                },
            );
            if storage.active_account.is_none() {
                storage.active_account = Some(active_key);
            }
        }

        if self.needs_login {
            if let Some(account) = self
                .account_id
                .as_ref()
                .and_then(|id| storage.accounts.get_mut(id))
            {
                account.needs_login = true;
                account.access_token.clear();
            }
        }
        let json_bytes = serde_json::to_vec_pretty(&storage)?;
        let enc_str = encrypt_auth_data(&json_bytes)?;
        fs::write(path, enc_str)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn save_token_and_account(
        &mut self,
        access_token: String,
        refresh_token: String,
        expires_in: Option<u64>,
        name: String,
        handle: String,
    ) -> Result<()> {
        let path = match &self.auth_file_path {
            Some(p) => p.clone(),
            None => crate::config::resolve_path("~/.config/yute/auth.json"),
        };
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut storage = if path.exists() {
            fs::read_to_string(&path)
                .ok()
                .and_then(|s| {
                    if s.starts_with("YTUI_ENC_V1:") {
                        decrypt_auth_data(&s)
                            .ok()
                            .and_then(|b| serde_json::from_slice::<AuthStorage>(&b).ok())
                    } else {
                        serde_json::from_str::<AuthStorage>(&s).ok()
                    }
                })
                .unwrap_or_default()
        } else {
            AuthStorage::default()
        };

        if let Some(ref vis) = self.visitor_data {
            storage.visitor_data = Some(vis.clone());
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let exp = expires_in.map(|sec| now + sec).unwrap_or(now + 3600);

        self.access_token = Some(access_token.clone());
        self.refresh_token = Some(refresh_token.clone());
        self.token_expires_at = Some(exp);

        let active_key = if let Some(id) = self.account_id.clone() {
            id
        } else if !handle.is_empty() {
            handle.clone()
        } else {
            storage
                .active_account
                .clone()
                .unwrap_or_else(|| "default".to_string())
        };

        storage.accounts.insert(
            active_key.clone(),
            AuthAccount {
                access_token,
                refresh_token,
                expires_at: exp,
                name,
                handle,
                page_id: self.page_id.clone(),
                visitor_data: self.visitor_data.clone(),
                ..Default::default()
            },
        );
        storage.active_account = Some(active_key);

        let json_bytes = serde_json::to_vec_pretty(&storage)?;
        let enc_str = encrypt_auth_data(&json_bytes)?;
        fs::write(&path, enc_str)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    pub fn load_from_file(path: &Path) -> Result<Self> {
        let mut ctx = Self::default();
        ctx.load_cookies_from_path(path)?;
        Ok(ctx)
    }

    pub fn load_cookies_from_path(&mut self, path: &Path) -> Result<()> {
        if !path.exists() {
            Self::try_auto_extract_brave_cookies(path);
        }

        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                self.parse_netscape_cookies(&content);
            }
        }

        if self.cookies.is_empty() {
            Self::try_auto_extract_brave_cookies(path);
            if path.exists() {
                if let Ok(content) = fs::read_to_string(path) {
                    self.parse_netscape_cookies(&content);
                }
            }
        }

        Ok(())
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
                            let _ =
                                fs::set_permissions(target_path, fs::Permissions::from_mode(0o600));
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
            let line = line.strip_prefix("#HttpOnly_").unwrap_or(line);
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
                } else if domain == ".google.com"
                    || domain == "google.com"
                    || domain == ".google.co.in"
                {
                    if (name == "SAPISID" || name == "__Secure-3PAPISID") && self.sapisid.is_none()
                    {
                        self.sapisid = Some(value.clone());
                    }
                    google_cookies.insert(name, value);
                }
            }
        }

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

        let payload = format!("{} {} {}", now_sec, sapisid, TV_ORIGIN);
        let mut hasher = Sha1::new();
        hasher.update(payload.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        Some(format!("SAPISIDHASH {}_{}", now_sec, hash))
    }

    pub async fn request_device_code(client: &reqwest::Client) -> Result<DeviceCodeResponse> {
        let payload = serde_json::json!({
            "client_id": OAUTH_CLIENT_ID,
            "device_id": "yute-cachyos",
            "model_name": OAUTH_MODEL_NAME,
            "scope": OAUTH_SCOPE,
        });

        let resp = client
            .post(OAUTH_DEVICE_CODE_URL)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let res: DeviceCodeResponse = resp.json().await?;
        Ok(res)
    }

    pub async fn poll_device_response(
        client: &reqwest::Client,
        device_code: &str,
    ) -> Result<DeviceTokenResponse> {
        let payload = serde_json::json!({
            "code": device_code,
            "client_id": OAUTH_CLIENT_ID,
            "client_secret": OAUTH_CLIENT_SECRET,
            "grant_type": OAUTH_GRANT_TYPE_DEVICE,
        });

        let resp = client
            .post(OAUTH_TOKEN_URL)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let tok: DeviceTokenResponse = resp.json().await?;
        Ok(tok)
    }
    pub async fn poll_device_token(
        client: &reqwest::Client,
        device_code: &str,
    ) -> Result<Option<DeviceTokenResponse>> {
        let token = Self::poll_device_response(client, device_code).await?;
        if token.access_token.is_some() {
            Ok(Some(token))
        } else if token.error.as_deref() == Some("authorization_pending") {
            Ok(None)
        } else {
            Err(anyhow!("TV sign-in pending or unavailable"))
        }
    }

    pub async fn refresh_access_token(&mut self, client: &reqwest::Client) -> Result<bool> {
        let ref_tok = match &self.refresh_token {
            Some(t) => t.clone(),
            None => return Ok(false),
        };

        let payload = serde_json::json!({
            "refresh_token": ref_tok,
            "client_id": OAUTH_CLIENT_ID,
            "client_secret": OAUTH_CLIENT_SECRET,
            "grant_type": OAUTH_GRANT_TYPE_REFRESH,
        });

        let resp = client
            .post(OAUTH_TOKEN_URL)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let tok: DeviceTokenResponse = resp.json().await?;
        if let Some(new_access) = tok.access_token {
            self.access_token = Some(new_access);
            let exp_in = tok.expires_in.unwrap_or(3600);
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            self.token_expires_at = Some(now + exp_in);
            if let Some(refresh) = tok.refresh_token {
                self.refresh_token = Some(refresh);
            }
            self.needs_login = false;
            Ok(true)
        } else if matches!(
            tok.error.as_deref(),
            Some("invalid_grant" | "access_denied")
        ) {
            self.needs_login = true;
            self.access_token = None;
            Err(crate::oauth::NeedsLogin.into())
        } else {
            Err(anyhow!("Token refresh failed"))
        }
    }

    pub fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(TV_USER_AGENT));
        headers.insert(ORIGIN, HeaderValue::from_static(TV_ORIGIN));
        headers.insert(REFERER, HeaderValue::from_static(TV_REFERER));
        headers.insert("X-Origin", HeaderValue::from_static(TV_ORIGIN));

        let client_name_id = if self.client_name == "TVHTML5" {
            "7"
        } else {
            "1"
        };
        headers.insert(
            "X-YouTube-Client-Name",
            HeaderValue::from_static(client_name_id),
        );

        if let Ok(val) = HeaderValue::from_str(&self.client_version) {
            headers.insert("X-YouTube-Client-Version", val);
        }

        if let Some(ref visitor) = self.visitor_data {
            if let Ok(val) = HeaderValue::from_str(visitor) {
                headers.insert("X-Goog-Visitor-Id", val);
            }
        }

        if let Some(page) = self
            .page_id
            .as_deref()
            .and_then(|p| HeaderValue::from_str(p).ok())
        {
            headers.insert("X-Goog-Pageid", page);
        }

        if let Some(ref token) = self.access_token {
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {token}")) {
                headers.insert(AUTHORIZATION, val);
            }
        } else {
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
        }

        headers
    }

    pub fn build_client_context(&self) -> serde_json::Value {
        let is_tv = self.client_name == "TVHTML5";
        let mut client_obj = serde_json::json!({
            "clientName": self.client_name,
            "clientVersion": self.client_version,
            "hl": self.hl,
            "gl": self.gl,
            "utcOffsetMinutes": 330,
        });

        if is_tv {
            client_obj["tvAppInfo"] = serde_json::json!({
                "appQuality": "TV_APP_QUALITY_FULL_ANIMATION",
                "zylonLeftNav": true
            });
        }

        if let Some(ref visitor) = self.visitor_data {
            client_obj["visitorData"] = serde_json::Value::String(visitor.clone());
        }

        serde_json::json!({
            "client": client_obj,
            "user": {
                "enableSafetyMode": false,
                "lockedSafetyMode": false
            }
        })
    }
}
