//! Account identity and isolated sessions shared by all frontends.
use crate::backend::{
    auth::{AuthContext, AuthStorage},
    models::FeedPage,
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub const ANONYMOUS: &str = "anonymous";
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountStatus {
    #[default]
    Ok,
    NeedsLogin,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub handle: String,
    pub initial: String,
    pub channel_id: Option<String>,
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub status: AccountStatus,
}
impl Account {
    fn anonymous() -> Self {
        Self {
            id: ANONYMOUS.into(),
            name: "Anonymous".into(),
            handle: String::new(),
            initial: "A".into(),
            channel_id: None,
            avatar_url: None,
            status: AccountStatus::Ok,
        }
    }
}

pub struct Accounts {
    pub profiles: Vec<Account>,
    sessions: HashMap<String, AuthContext>,
    active: String,
    storage: AuthStorage,
    path: PathBuf,
    feeds: HashMap<(String, String), FeedPage>,
    revision: Arc<std::sync::atomic::AtomicU64>,
    write_lock: Arc<Mutex<()>>,
}
impl Accounts {
    pub fn load(path: &Path, current: &AuthContext, anonymous: bool) -> Self {
        let storage = Self::read_storage(path).unwrap_or_default();
        Self::from_storage(path.to_owned(), storage, current, anonymous)
    }
    pub fn read_storage(path: &Path) -> Result<AuthStorage> {
        let raw = std::fs::read_to_string(path)?;
        let bytes = if raw.starts_with("YTUI_ENC_V1:") {
            crate::backend::auth::decrypt_auth_data(&raw)?
        } else {
            raw.into_bytes()
        };
        Ok(serde_json::from_slice(&bytes)?)
    }
    pub fn from_storage(
        path: PathBuf,
        storage: AuthStorage,
        current: &AuthContext,
        anonymous: bool,
    ) -> Self {
        let mut profiles = vec![Account::anonymous()];
        let mut sessions = HashMap::from([(
            ANONYMOUS.into(),
            AuthContext {
                hl: current.hl.clone(),
                gl: current.gl.clone(),
                ..Default::default()
            },
        )]);
        let mut saved: Vec<_> = storage.accounts.iter().collect();
        saved.sort_by_key(|(id, _)| *id);
        for (id, account) in saved {
            let name = if account.name.is_empty() {
                id.clone()
            } else {
                account.name.clone()
            };
            profiles.push(Account {
                id: id.clone(),
                initial: name
                    .chars()
                    .next()
                    .unwrap_or('A')
                    .to_uppercase()
                    .to_string(),
                name,
                handle: account.handle.clone(),
                channel_id: None,
                avatar_url: account.avatar_url.clone(),
                status: if account.needs_login || account.refresh_token.is_empty() {
                    AccountStatus::NeedsLogin
                } else {
                    AccountStatus::Ok
                },
            });
            sessions.insert(
                id.clone(),
                AuthContext {
                    access_token: Some(account.access_token.clone()),
                    refresh_token: Some(account.refresh_token.clone()),
                    token_expires_at: Some(account.expires_at),
                    visitor_data: account.visitor_data.clone().or_else(|| {
                        if storage.active_account.as_deref() == Some(id.as_str()) {
                            storage.visitor_data.clone()
                        } else {
                            None
                        }
                    }),
                    cookies: Default::default(),
                    cookie_header: None,
                    sapisid: None,
                    page_id: account.page_id.clone(),
                    account_id: Some(id.clone()),
                    needs_login: account.needs_login,
                    auth_file_path: Some(path.clone()),
                    ..current.clone()
                },
            );
        }
        // Imported cookie sessions are selectable without inventing OAuth tokens.
        if current.cookie_header.is_some() && profiles.len() == 1 {
            profiles.push(Account {
                id: "cookies".into(),
                name: "Cookie session".into(),
                handle: String::new(),
                initial: "C".into(),
                channel_id: None,
                avatar_url: None,
                status: AccountStatus::Ok,
            });
            sessions.insert("cookies".into(), current.clone());
        }
        let active = if anonymous {
            ANONYMOUS.into()
        } else {
            storage
                .active_account
                .as_ref()
                .filter(|id| sessions.contains_key(*id))
                .cloned()
                .or_else(|| profiles.get(1).map(|a| a.id.clone()))
                .unwrap_or_else(|| ANONYMOUS.into())
        };
        Self {
            profiles,
            sessions,
            active,
            storage,
            path,
            feeds: HashMap::new(),
            revision: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            write_lock: Arc::new(Mutex::new(())),
        }
    }
    pub fn current_context(&self) -> AuthContext {
        self.sessions[&self.active].clone()
    }
    pub fn update_profile(&mut self, name: &str, handle: &str) {
        let index = self.active_index();
        if let Some(profile) = self.profiles.get_mut(index) {
            if profile.id != ANONYMOUS {
                if !name.is_empty() {
                    profile.name = name.into();
                }
                if !handle.is_empty() {
                    profile.handle = handle.into();
                }
            }
        }
    }
    pub fn import_profiles(&mut self, profiles: Vec<Account>, current: &AuthContext) {
        for profile in profiles {
            let id = profile.id.clone();
            if let Some(old) = self.profiles.iter_mut().find(|a| a.id == id) {
                *old = profile.clone();
            } else {
                self.profiles.push(profile.clone());
            }
            let mut session = current.clone();
            session.page_id = profile.channel_id.clone();
            session.account_id = Some(id.clone());
            session.cancelled = tokio_util::sync::CancellationToken::new();
            self.sessions.insert(id.clone(), session);
            if let (Some(access), Some(refresh)) = (&current.access_token, &current.refresh_token) {
                self.storage.accounts.insert(
                    id,
                    crate::backend::auth::AuthAccount {
                        access_token: access.clone(),
                        refresh_token: refresh.clone(),
                        expires_at: current.token_expires_at.unwrap_or(0),
                        name: profile.name,
                        handle: profile.handle,
                        avatar_url: profile.avatar_url,
                        page_id: profile.channel_id,
                        visitor_data: current.visitor_data.clone(),
                        needs_login: false,
                    },
                );
            }
        }
    }
    pub fn active_id(&self) -> &str {
        &self.active
    }
    pub fn active_index(&self) -> usize {
        self.profiles
            .iter()
            .position(|p| p.id == self.active)
            .unwrap_or(0)
    }
    pub fn resolve(&self, input: &str) -> Result<usize> {
        if matches!(input, "anon" | "anonymous") {
            return Ok(0);
        }
        if let Ok(index) = input.parse::<usize>() {
            if index > 0 && index <= self.profiles.len() {
                return Ok(index - 1);
            }
        }
        let text = input.to_lowercase();
        let exact = self.profiles.iter().position(|p| {
            [&p.id, &p.name, &p.handle]
                .iter()
                .any(|s| s.to_lowercase() == text)
        });
        if let Some(index) = exact {
            return Ok(index);
        }
        let matches: Vec<_> = self
            .profiles
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                [&p.id, &p.name, &p.handle]
                    .iter()
                    .any(|s| !s.is_empty() && s.to_lowercase().starts_with(&text))
            })
            .collect();
        if matches.len() == 1 {
            return Ok(matches[0].0);
        }
        bail!("Account not found or prefix is ambiguous")
    }
    pub fn activate(
        &mut self,
        index: usize,
        current: &Arc<Mutex<AuthContext>>,
    ) -> Result<Arc<Mutex<AuthContext>>> {
        let Some(profile) = self.profiles.get(index) else {
            bail!("Account not found");
        };
        let id = profile.id.clone();
        {
            let old = current.lock().unwrap();
            old.cancelled.cancel();
            if self.active != ANONYMOUS {
                self.sessions.insert(self.active.clone(), old.clone());
            }
        }
        let mut next = self
            .sessions
            .get(&id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Session unavailable"))?;
        next.cancelled = tokio_util::sync::CancellationToken::new();
        self.active = id;
        self.storage.active_account = if self.active == ANONYMOUS {
            None
        } else {
            Some(self.active.clone())
        };
        Ok(Arc::new(Mutex::new(next)))
    }
    pub fn remember_feed(&mut self, route: &str, page: FeedPage) {
        self.feeds.insert((self.active.clone(), route.into()), page);
    }
    pub fn cached_feed(&self, route: &str) -> Option<FeedPage> {
        self.feeds
            .get(&(self.active.clone(), route.into()))
            .cloned()
    }
    pub fn cache_namespace(&self) -> String {
        hex::encode(&Sha256::digest(self.active.as_bytes())[..12])
    }
    pub fn remove(&mut self, index: usize) -> Result<()> {
        let profile = self
            .profiles
            .get(index)
            .ok_or_else(|| anyhow::anyhow!("Account not found"))?;
        if profile.id == ANONYMOUS {
            bail!("Anonymous cannot be removed");
        }
        if profile.id == self.active {
            bail!("Switch to another account before removing this one");
        }
        let id = profile.id.clone();
        self.storage.accounts.remove(&id);
        self.persist()?;
        self.profiles.remove(index);
        self.sessions.remove(&id);
        self.feeds.retain(|(account, _), _| *account != id);
        Ok(())
    }
    pub fn persist_background(&self) {
        let path = self.path.clone();
        let revision = self.revision.clone();
        let generation = revision.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
        let lock = self.write_lock.clone();
        let mut storage = self.storage.clone();
        for (id, session) in &self.sessions {
            if let Some(saved) = storage.accounts.get_mut(id) {
                if let Some(token) = &session.access_token {
                    saved.access_token = token.clone();
                }
                if let Some(token) = &session.refresh_token {
                    saved.refresh_token = token.clone();
                }
                saved.expires_at = session.token_expires_at.unwrap_or(saved.expires_at);
                saved.visitor_data = session.visitor_data.clone();
            }
        }
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn_blocking(move || -> Result<()> {
                let _guard = lock.lock().unwrap();
                if revision.load(std::sync::atomic::Ordering::SeqCst) != generation {
                    return Ok(());
                }
                let encrypted =
                    crate::backend::auth::encrypt_auth_data(&serde_json::to_vec(&storage)?)?;
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                #[cfg(unix)]
                {
                    use std::{fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt};
                    let mut file = OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .mode(0o600)
                        .open(path)?;
                    file.write_all(encrypted.as_bytes())?;
                }
                #[cfg(not(unix))]
                std::fs::write(path, encrypted)?;
                Ok(())
            });
        }
    }
    pub fn persist(&self) -> Result<()> {
        self.revision
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let _guard = self.write_lock.lock().unwrap();
        let json = serde_json::to_vec(&self.storage)?;
        let encrypted = crate::backend::auth::encrypt_auth_data(&json)?;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        #[cfg(unix)]
        {
            use std::{fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt};
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&self.path)?;
            file.write_all(encrypted.as_bytes())?;
        }
        #[cfg(not(unix))]
        std::fs::write(&self.path, encrypted)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn switching_isolates_identity_cache_and_cancels_old_requests() {
        let auth = Arc::new(Mutex::new(AuthContext::default()));
        let mut storage = AuthStorage::default();
        storage.accounts.insert(
            "alice".into(),
            crate::backend::auth::AuthAccount {
                access_token: "fixture".into(),
                refresh_token: "refresh-fixture".into(),
                expires_at: u64::MAX,
                name: "Alice".into(),
                handle: "@alice".into(),
                ..Default::default()
            },
        );
        let mut accounts =
            Accounts::from_storage(PathBuf::new(), storage, &auth.lock().unwrap(), true);
        assert_eq!(accounts.resolve("Al").unwrap(), 1);
        let previous = auth.lock().unwrap().cancelled.clone();
        accounts.remember_feed("home", FeedPage::default());
        let signed = accounts.activate(1, &auth).unwrap();
        assert!(previous.is_cancelled());
        assert!(accounts.cached_feed("home").is_none());
        assert_eq!(
            signed.lock().unwrap().access_token.as_deref(),
            Some("fixture")
        );
        let anon = accounts.activate(0, &signed).unwrap();
        assert!(anon.lock().unwrap().access_token.is_none());
        assert!(accounts.cached_feed("home").is_some());
    }
}

/// MediaServiceCore V2 accountReadMask includes the owner and brand identities.
pub async fn fetch_profiles(auth: Arc<Mutex<AuthContext>>) -> Result<Vec<Account>> {
    let context = auth.lock().unwrap().build_client_context();
    let backend = crate::backend::innertube::InnerTubeBackend::from_arc(auth);
    let value = backend.post_endpoint("account/accounts_list",serde_json::json!({"context":context,"accountReadMask":{"returnOwner":true,"returnBrandAccounts":true,"returnPersonaAccounts":false}})).await?;
    Ok(parse_profiles(&value))
}
pub fn parse_profiles(value: &serde_json::Value) -> Vec<Account> {
    fn visit(value: &serde_json::Value, out: &mut Vec<Account>) {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(item) = map.get("accountItem") {
                    if item.get("isDisabled").and_then(|v| v.as_bool()) != Some(true) {
                        let text = |key: &str| {
                            item.get(key)
                                .map(crate::backend::parse::extract_text)
                                .unwrap_or_default()
                        };
                        let name = text("accountName");
                        let handle = text("channelHandle");
                        let page = item
                            .pointer(
                                "/serviceEndpoint/selectActiveIdentityEndpoint/supportedTokens",
                            )
                            .and_then(|v| v.as_array())
                            .and_then(|tokens| {
                                tokens.iter().find_map(|v| {
                                    v.pointer("/pageIdToken/pageId").and_then(|v| v.as_str())
                                })
                            })
                            .map(str::to_owned);
                        let id = page.clone().unwrap_or_else(|| handle.clone());
                        if !id.is_empty() && !out.iter().any(|a| a.id == id) {
                            out.push(Account {
                                id,
                                initial: name
                                    .chars()
                                    .next()
                                    .unwrap_or('A')
                                    .to_uppercase()
                                    .to_string(),
                                name,
                                handle,
                                channel_id: page,
                                avatar_url: item
                                    .pointer("/accountPhoto/thumbnails/0/url")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_owned),
                                status: AccountStatus::Ok,
                            });
                        }
                    }
                }
                for v in map.values() {
                    visit(v, out);
                }
            }
            serde_json::Value::Array(values) => {
                for v in values {
                    visit(v, out);
                }
            }
            _ => (),
        }
    }
    let mut profiles = Vec::new();
    visit(value, &mut profiles);
    profiles
}

#[cfg(test)]
mod profile_tests {
    #[test]
    fn profiles_parse_owner_and_brand_page_tokens_without_duplicates() {
        let value = serde_json::json!({"contents":[{"accountItem":{"accountName":{"simpleText":"Brand"},"channelHandle":{"simpleText":"@brand"},"accountPhoto":{"thumbnails":[{"url":"https://example.invalid/avatar.jpg"}]},"serviceEndpoint":{"selectActiveIdentityEndpoint":{"supportedTokens":[{"pageIdToken":{"pageId":"brand-page"}}]}}}},{"accountItem":{"isDisabled":true,"channelHandle":{"simpleText":"@disabled"}}}]});
        let profiles = super::parse_profiles(&value);
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].id, "brand-page");
        assert_eq!(profiles[0].channel_id.as_deref(), Some("brand-page"));
        assert!(profiles[0].avatar_url.is_some());
    }
}

#[derive(Debug)]
pub struct AccountCancelled;
impl std::fmt::Display for AccountCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Account request cancelled")
    }
}
impl std::error::Error for AccountCancelled {}
