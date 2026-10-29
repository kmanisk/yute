use crate::secret::SecretString;
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, COOKIE, ORIGIN, REFERER, USER_AGENT};
use serde::{Deserialize, Serialize};
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
    crate::credentials::CredentialStore::open(crate::credentials::path())?.encrypt(plaintext)
}
pub fn decrypt_auth_data(raw: &str) -> Result<Vec<u8>> {
    if raw.starts_with("YUTE_ENC_V2:") {
        Ok(
            crate::credentials::CredentialStore::open(crate::credentials::path())?
                .decrypt(raw)?
                .to_vec(),
        )
    } else {
        decrypt_legacy_auth_data(raw)
    }
}

pub(crate) fn decrypt_legacy_auth_data(encrypted_str: &str) -> Result<Vec<u8>> {
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
    "https://www.googleapis.com/auth/youtube https://www.googleapis.com/auth/youtube-paid-content";
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
    pub visitor_data: Option<SecretString>,
    #[serde(default)]
    pub needs_login: bool,
    pub access_token: SecretString,
    pub refresh_token: SecretString,
    pub expires_at: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub handle: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AuthStorage {
    #[serde(default)]
    pub cookie_jars: HashMap<String, SecretString>,
    #[serde(default)]
    pub accounts: HashMap<String, AuthAccount>,
    #[serde(default)]
    pub active_account: Option<String>,
    #[serde(default)]
    pub visitor_data: Option<SecretString>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: SecretString,
    pub user_code: String,
    pub verification_url: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DeviceTokenResponse {
    pub access_token: Option<SecretString>,
    pub refresh_token: Option<SecretString>,
    pub expires_in: Option<u64>,
    pub token_type: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AuthContext {
    pub cookies: HashMap<String, SecretString>,
    pub sapisid: Option<SecretString>,
    pub cookie_header: Option<SecretString>,
    pub user_agent: String,
    pub visitor_data: Option<SecretString>,
    pub client_name: String,
    pub client_version: String,
    pub hl: String,
    pub gl: String,
    pub access_token: Option<SecretString>,
    pub refresh_token: Option<SecretString>,
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
    /// Shared frontend configuration. OAuth always uses a coherent TV context.
    pub fn from_config(config: &crate::config::Config) -> Self {
        let mut auth = Self {
            hl: config.backend.hl.clone(),
            gl: config.backend.gl.clone(),
            ..Default::default()
        };
        if config.backend.client == "web" && config.backend.auth == "guest" {
            auth.client_name = "WEB".into();
            auth.client_version = "2.20240101.00.00".into();
        } else {
            if !config.backend.client_name.is_empty() {
                auth.client_name = config.backend.client_name.clone();
            }
            if !config.backend.client_version.is_empty() {
                auth.client_version = config.backend.client_version.clone();
            }
        }
        auth
    }
    fn client_identity(&self) -> (&str, &str) {
        if self.has_oauth() && self.client_name != "TVHTML5" {
            ("TVHTML5", DEFAULT_CLIENT_VERSION)
        } else {
            (&self.client_name, &self.client_version)
        }
    }
    pub fn has_oauth(&self) -> bool {
        self.access_token.as_ref().is_some_and(|t| !t.is_empty())
            || self.refresh_token.as_ref().is_some_and(|t| !t.is_empty())
    }

    pub fn load_auth(&mut self, auth_path: &Path, cookies_path: &Path) -> Result<()> {
        self.load_auth_with_mode(auth_path, cookies_path, "tv_oauth")
    }

    pub fn load_auth_with_mode(
        &mut self,
        auth_path: &Path,
        cookies_path: &Path,
        auth_mode: &str,
    ) -> Result<()> {
        // Legacy material stays on disk/in the encrypted store, but is never sent.
        self.cookies.clear();
        self.cookie_header = None;
        self.sapisid = None;
        self.auth_file_path = Some(auth_path.to_path_buf());
        if auth_mode == "guest" {
            self.auth_file_path = None;
            self.visitor_data = None;
            self.page_id = None;
            self.account_id = None;
            self.needs_login = false;
            self.token_expires_at = None;
            self.access_token = None;
            self.refresh_token = None;
            return Ok(());
        }
        if auth_path.exists() || crate::credentials::path().exists() {
            self.load_auth_file(auth_path)?;
        }
        if self.access_token.is_none() && self.refresh_token.is_none() {
            self.needs_login = cookies_path.exists() || auth_mode == "cookies";
        }
        Ok(())
    }

    pub fn load_auth_file(&mut self, path: &Path) -> Result<()> {
        let storage = crate::credentials::migrate(
            &[
                path.to_owned(),
                crate::config::resolve_path("~/.config/ytui/auth.json"),
            ],
            &[],
        )?;
        self.auth_file_path = Some(crate::credentials::path());
        if let Some(id) = storage
            .active_account
            .as_ref()
            .or_else(|| storage.accounts.keys().next())
        {
            if let Some(account) = storage.accounts.get(id) {
                self.account_id = Some(id.clone());
                self.page_id = account.page_id.clone();
                self.needs_login = account.needs_login;
                self.access_token = if account.needs_login {
                    None
                } else {
                    Some(account.access_token.clone())
                };
                self.refresh_token = Some(account.refresh_token.clone());
                self.token_expires_at = Some(account.expires_at);
                self.visitor_data = account.visitor_data.clone().or(storage.visitor_data);
            }
        }
        Ok(())
    }
    pub fn save_auth_file(&self, _path: &Path) -> Result<()> {
        crate::credentials::update(|storage| {
            let Some(id) = self
                .account_id
                .as_ref()
                .or(storage.active_account.as_ref())
                .cloned()
            else {
                return Ok(());
            };
            let account = storage.accounts.entry(id).or_default();
            if let Some(token) = &self.access_token {
                account.access_token = token.clone();
            }
            if let Some(token) = &self.refresh_token {
                account.refresh_token = token.clone();
            }
            account.expires_at = self.token_expires_at.unwrap_or(account.expires_at);
            account.page_id = self.page_id.clone();
            account.visitor_data = self.visitor_data.clone();
            account.needs_login = self.needs_login;
            if self.needs_login {
                account.access_token = SecretString::default();
            }
            Ok(())
        })
    }

    pub fn save_token_and_account(
        &mut self,
        access_token: impl Into<SecretString>,
        refresh_token: impl Into<SecretString>,
        expires_in: Option<u64>,
        name: String,
        handle: String,
    ) -> Result<()> {
        let access_token = access_token.into();
        let refresh_token = refresh_token.into();
        let id = self.account_id.clone().unwrap_or_else(|| {
            if handle.is_empty() {
                "default".into()
            } else {
                handle.clone()
            }
        });
        let expires_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + expires_in.unwrap_or(3600);
        crate::credentials::update(|storage| {
            let avatar = storage.accounts.get(&id).and_then(|a| a.avatar_url.clone());
            storage.accounts.insert(
                id.clone(),
                AuthAccount {
                    access_token: access_token.clone(),
                    refresh_token: refresh_token.clone(),
                    expires_at,
                    name,
                    handle,
                    avatar_url: avatar,
                    page_id: self.page_id.clone(),
                    visitor_data: self.visitor_data.clone(),
                    needs_login: false,
                },
            );
            storage.active_account = Some(id.clone());
            Ok(())
        })?;
        self.access_token = Some(access_token);
        self.refresh_token = Some(refresh_token);
        self.token_expires_at = Some(expires_at);
        self.account_id = Some(id);
        self.auth_file_path = Some(crate::credentials::path());
        self.needs_login = false;
        Ok(())
    }

    pub fn load_from_file(path: &Path) -> Result<Self> {
        let mut ctx = Self::default();
        ctx.load_cookies_from_path(path)?;
        Ok(ctx)
    }

    pub fn load_cookies_from_path(&mut self, path: &Path) -> Result<()> {
        let storage = crate::credentials::migrate(
            &[],
            &[
                path.to_owned(),
                crate::config::resolve_path("~/.config/ytui/cookies.txt"),
            ],
        )?;
        self.needs_login = !storage.cookie_jars.is_empty();
        Ok(())
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
                let value: SecretString = parts[6].trim().into();

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
                .map(|(k, v)| format!("{k}={}", v.expose()))
                .collect::<Vec<_>>()
                .join("; ");
            self.cookie_header = Some(header_str.into());
        }
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

        let (name, version) = self.client_identity();
        let client_name_id = if name == "TVHTML5" { "7" } else { "1" };
        headers.insert(
            "X-YouTube-Client-Name",
            HeaderValue::from_static(client_name_id),
        );

        if let Ok(val) = HeaderValue::from_str(version) {
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
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", token.expose())) {
                headers.insert(AUTHORIZATION, val);
            }
        }

        for key in [
            AUTHORIZATION,
            COOKIE,
            reqwest::header::HeaderName::from_static("x-goog-visitor-id"),
        ] {
            if let Some(value) = headers.get_mut(key) {
                value.set_sensitive(true);
            }
        }
        headers
    }

    pub fn build_client_context(&self) -> serde_json::Value {
        let (name, version) = self.client_identity();
        let is_tv = name == "TVHTML5";
        let mut client_obj = serde_json::json!({
            "clientName": name,
            "clientVersion": version,
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
            client_obj["visitorData"] = serde_json::Value::String(visitor.expose().to_owned());
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

impl std::fmt::Debug for DeviceCodeResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeviceCodeResponse([REDACTED])")
    }
}
impl std::fmt::Debug for DeviceTokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DeviceTokenResponse([REDACTED])")
    }
}

#[cfg(test)]
mod oauth_scope_tests {
    use super::OAUTH_SCOPE;

    #[test]
    fn device_login_requests_youtube_write_scope_without_legacy_gdata_scope() {
        assert!(OAUTH_SCOPE.contains("https://www.googleapis.com/auth/youtube"));
        assert!(OAUTH_SCOPE.contains("https://www.googleapis.com/auth/youtube-paid-content"));
        assert!(!OAUTH_SCOPE.contains("gdata.youtube.com"));
    }
}
