//! Shared cancellable TV-code flow. UI events contain display data only.
use crate::{
    accounts::{Account, AccountStatus},
    backend::auth::AuthContext,
};
use anyhow::{bail, Result};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{sync::mpsc::UnboundedSender, time::Instant};
pub use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
pub enum LoginEvent {
    Code {
        user_code: String,
        verification_url: String,
        expires_in: u64,
    },
    Remaining(u64),
}
#[derive(Debug)]
pub struct NeedsLogin;
impl std::fmt::Display for NeedsLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Account needs re-login; use :account add")
    }
}
impl std::error::Error for NeedsLogin {}
#[derive(Debug, PartialEq, Eq)]
pub enum PollState {
    Pending,
    SlowDown,
    Expired,
    Denied,
    Failed,
}
pub fn poll_state(error: Option<&str>) -> PollState {
    match error {
        Some("authorization_pending") => PollState::Pending,
        Some("slow_down") => PollState::SlowDown,
        Some("expired_token" | "expired") => PollState::Expired,
        Some("access_denied") => PollState::Denied,
        _ => PollState::Failed,
    }
}

pub async fn device_login(
    path: PathBuf,
    language: (String, String),
    cancelled: CancellationToken,
    events: UnboundedSender<LoginEvent>,
) -> Result<Account> {
    let client = crate::http::client();
    let code = tokio::select! { _=cancelled.cancelled()=>bail!("Sign-in cancelled"), code=AuthContext::request_device_code(&client)=>code? };
    let _ = events.send(LoginEvent::Code {
        user_code: code.user_code.clone(),
        verification_url: code.verification_url.clone(),
        expires_in: code.expires_in,
    });
    let deadline = Instant::now() + Duration::from_secs(code.expires_in);
    let mut interval = Duration::from_secs(code.interval.max(1));
    let mut next_poll = Instant::now() + interval;
    let mut countdown = tokio::time::interval(Duration::from_secs(1));
    let token = loop {
        tokio::select! {
            _=cancelled.cancelled()=>bail!("Sign-in cancelled"),
            _=tokio::time::sleep_until(deadline)=>bail!("Device code expired; use :account add"),
            _=countdown.tick()=>{ let _=events.send(LoginEvent::Remaining(deadline.saturating_duration_since(Instant::now()).as_secs())); },
            _=tokio::time::sleep_until(next_poll)=>{
                let token=tokio::select! {
                    _=cancelled.cancelled()=>bail!("Sign-in cancelled"),
                    _=tokio::time::sleep_until(deadline)=>bail!("Device code expired"),
                    token=AuthContext::poll_device_response(&client,&code.device_code)=>token?,
                };
                if token.access_token.is_some() && token.refresh_token.is_some() { break token; }
                match poll_state(token.error.as_deref()) {
                    PollState::Pending=>(), PollState::SlowDown=>interval+=Duration::from_secs(5),
                    PollState::Expired=>bail!("Device code expired; use :account add"),
                    PollState::Denied=>bail!("Sign-in was denied"),PollState::Failed=>bail!("YouTube sign-in failed"),
                }
                next_poll=Instant::now()+interval;
            }
        }
    };
    let access = token
        .access_token
        .ok_or_else(|| anyhow::anyhow!("Missing access token"))?;
    let refresh = token
        .refresh_token
        .ok_or_else(|| anyhow::anyhow!("Missing refresh token"))?;
    let mut auth = AuthContext {
        access_token: Some(access.clone()),
        refresh_token: Some(refresh.clone()),
        hl: language.0,
        gl: language.1,
        token_expires_at: Some(now() + token.expires_in.unwrap_or(3600)),
        ..Default::default()
    };
    let profiles = tokio::select! { _=cancelled.cancelled()=>bail!("Sign-in cancelled"), result=crate::accounts::fetch_profiles(Arc::new(Mutex::new(auth.clone())))=>result.unwrap_or_default() };
    let profile = profiles.into_iter().next().unwrap_or_else(|| Account {
        id: format!(
            "oauth-{}",
            hex::encode(&Sha256::digest(refresh.as_bytes())[..12])
        ),
        name: "Signed in".into(),
        handle: String::new(),
        initial: "S".into(),
        channel_id: None,
        avatar_url: None,
        status: AccountStatus::Ok,
    });
    auth.auth_file_path = Some(path);
    auth.account_id = Some(profile.id.clone());
    auth.page_id = profile.channel_id.clone();
    if cancelled.is_cancelled() {
        bail!("Sign-in cancelled");
    }
    let saved = profile.clone();
    tokio::task::spawn_blocking(move || {
        auth.save_token_and_account(access, refresh, token.expires_in, saved.name, saved.handle)
    })
    .await??;
    Ok(profile)
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// No std mutex is held across network or credential I/O. Refresh is single-flight
/// per identity, with a skew and one explicit retry after HTTP 401.
pub async fn ensure_fresh(
    auth: &Arc<Mutex<AuthContext>>,
    client: &reqwest::Client,
    force: bool,
) -> Result<()> {
    let (lock, original) = {
        let a = auth.lock().unwrap();
        (a.refresh_lock.clone(), a.access_token.clone())
    };
    let _guard = lock.lock().await;
    let mut context = auth.lock().unwrap().clone();
    if context.needs_login {
        return Err(NeedsLogin.into());
    }
    if context.refresh_token.is_none() {
        return Ok(());
    }
    if force && context.access_token != original {
        return Ok(());
    }
    if !force && context.token_expires_at.is_some_and(|exp| exp > now() + 60) {
        return Ok(());
    }
    let result = context.refresh_access_token(client).await;
    {
        let mut active = auth.lock().unwrap();
        active.access_token = context.access_token.clone();
        active.refresh_token = context.refresh_token.clone();
        active.token_expires_at = context.token_expires_at;
        active.needs_login = context.needs_login;
    }
    if let Some(path) = context.auth_file_path.clone() {
        tokio::task::spawn_blocking(move || context.save_auth_file(&path)).await??;
    }
    result.map(|_| ())
}
#[cfg(test)]
mod tests {
    #[test]
    fn polling_errors_are_classified_without_server_descriptions() {
        use super::*;
        for (error, expected) in [
            ("authorization_pending", PollState::Pending),
            ("slow_down", PollState::SlowDown),
            ("expired_token", PollState::Expired),
            ("access_denied", PollState::Denied),
            ("unknown", PollState::Failed),
        ] {
            assert_eq!(poll_state(Some(error)), expected);
        }
    }
}
