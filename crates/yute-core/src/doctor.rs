//! Bounded, read-only diagnostics. Never format request errors, headers or bodies.
use crate::{backend::auth::AuthContext, config::Config};
use std::{path::Path, time::Duration};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warning,
    Failure,
}
#[derive(Clone, Debug)]
pub struct Check {
    pub name: &'static str,
    pub status: Status,
    pub detail: String,
}
impl Check {
    pub fn new(name: &'static str, status: Status, detail: impl Into<String>) -> Self {
        Self {
            name,
            status,
            detail: detail.into(),
        }
    }
}

/// Text originating outside the program must not inject terminal controls.
pub fn display_text(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).take(240).collect()
}

pub fn network_error(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        "request timed out after 10 seconds"
    } else if error.is_connect() {
        "connection failed (DNS, TCP or TLS); check connectivity and proxy settings"
    } else if error.is_decode() {
        "response could not be decoded"
    } else {
        "network request failed; no response body or URL logged"
    }
}

pub async fn tool_version(program: &'static str, argument: &str, required: bool) -> Check {
    let mut command = tokio::process::Command::new(program);
    command
        .arg(argument)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null());
    let status = if required {
        Status::Failure
    } else {
        Status::Warning
    };
    match tokio::time::timeout(Duration::from_secs(5), command.output()).await {
        Ok(Ok(output)) if output.status.success() => {
            let line = String::from_utf8_lossy(&output.stdout);
            Check::new(
                program,
                Status::Ok,
                display_text(line.lines().next().unwrap_or("version unavailable")),
            )
        }
        Ok(Ok(output)) => Check::new(
            program,
            status,
            format!(
                "version probe exited with {}; stderr suppressed",
                output.status
            ),
        ),
        Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
            Check::new(program, status, "executable missing from PATH")
        }
        Ok(Err(error)) => Check::new(
            program,
            status,
            format!("could not launch executable ({:?})", error.kind()),
        ),
        Err(_) => Check::new(program, status, "version probe timed out after 5 seconds"),
    }
}

fn directory(name: &'static str, path: &Path) -> Check {
    // Probe the nearest existing parent without creating persistent directories.
    let parent = path.ancestors().find(|p| p.exists());
    let writable = parent
        .filter(|p| p.is_dir())
        .is_some_and(|p| tempfile::tempfile_in(p).is_ok());
    Check::new(
        name,
        if writable {
            Status::Ok
        } else {
            Status::Failure
        },
        format!(
            "{} · {}",
            display_text(&path.to_string_lossy()),
            if writable {
                if path.exists() {
                    "writable"
                } else {
                    "not created; parent writable"
                }
            } else {
                "directory or parent unavailable/unwritable"
            }
        ),
    )
}

async fn reachability(name: &'static str, url: &str) -> Check {
    match crate::http::client().get(url).send().await {
        Ok(response) if response.status().is_success() => Check::new(
            name,
            Status::Ok,
            format!("HTTP {}", response.status().as_u16()),
        ),
        Ok(response) => Check::new(
            name,
            Status::Failure,
            format!("HTTP {}", response.status().as_u16()),
        ),
        Err(error) => Check::new(name, Status::Failure, network_error(&error)),
    }
}

pub async fn run(config: &Config) -> Vec<Check> {
    let (network, youtube, ytdlp, mpv, ffmpeg, ffprobe) = tokio::join!(
        reachability("network", "https://www.google.com/generate_204"),
        reachability("YouTube", "https://www.youtube.com/"),
        tool_version("yt-dlp", "--version", true),
        tool_version("mpv", "--version", true),
        tool_version("ffmpeg", "-version", false),
        tool_version("ffprobe", "-version", false),
    );
    let mut checks = vec![network, youtube, ytdlp, mpv, ffmpeg, ffprobe];
    let path = crate::credentials::path();
    let storage = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::task::spawn_blocking(move || {
            if path.exists() {
                crate::credentials::CredentialStore::open(path)?
                    .load()
                    .map(Some)
            } else {
                Ok(None)
            }
        }),
    )
    .await;
    let mut auth = AuthContext {
        client_name: config.backend.client_name.clone(),
        client_version: config.backend.client_version.clone(),
        hl: config.backend.hl.clone(),
        gl: config.backend.gl.clone(),
        ..Default::default()
    };
    match storage {
        Ok(Ok(Ok(Some(storage)))) => {
            checks.push(Check::new("credential store", Status::Ok, "existing encrypted store authenticated"));
            auth = crate::accounts::Accounts::from_storage(crate::credentials::path(), storage, &auth, config.general.start_anonymous || config.backend.auth == "guest").current_context();
        }
        Ok(Ok(Ok(None))) => checks.push(Check::new("credential store", Status::Warning, "no encrypted store; keyring not provisioned or tested")),
        Ok(Ok(Err(_))) => checks.push(Check::new("credential store", Status::Failure, "cannot unlock/authenticate store: keyring/key unavailable, wrong key, unsupported format or damaged file; store preserved")),
        Ok(Err(_)) => checks.push(Check::new("credential store", Status::Failure, "credential worker failed")),
        Err(_) => checks.push(Check::new("credential store", Status::Failure, "keyring/store probe timed out after 5 seconds; unlock desktop keyring")),
    }
    checks.push(Check::new(
        "InnerTube config",
        if auth.client_name.is_empty() || auth.client_version.is_empty() {
            Status::Failure
        } else {
            Status::Ok
        },
        format!(
            "clientName={} clientVersion={} hl={} gl={} API key={}",
            display_text(&auth.client_name),
            display_text(&auth.client_version),
            display_text(&auth.hl),
            display_text(&auth.gl),
            if config.api.key.is_empty() {
                "unset (current transport does not send one)"
            } else {
                "configured (current transport does not send one)"
            }
        ),
    ));
    checks.push(Check::new(
        "visitorData",
        Status::Ok,
        if auth.visitor_data.is_some() {
            "present (redacted)"
        } else {
            "absent; acquired from subsequent responses"
        },
    ));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (status, session) = if auth.needs_login {
        (Status::Failure, "needs re-login")
    } else if auth.access_token.is_some() {
        if auth.token_expires_at.is_some_and(|expiry| expiry <= now) {
            (
                Status::Warning,
                "access token expired; refresh required (doctor does not refresh)",
            )
        } else {
            (Status::Ok, "OAuth selected; checking server acceptance")
        }
    } else {
        (Status::Warning, "not signed in (anonymous session)")
    };
    checks.push(Check::new("session", status, session));
    // Direct probe avoids refresh, visitor persistence and other application writes.
    let probe = crate::http::client()
        .post("https://www.youtube.com/youtubei/v1/browse")
        .headers(auth.build_headers())
        .json(&serde_json::json!({"context": auth.build_client_context(), "browseId":"default"}))
        .send()
        .await;
    let check = match probe {
        Ok(response) if response.status() == reqwest::StatusCode::UNAUTHORIZED => Check::new(
            "InnerTube/session",
            Status::Failure,
            "HTTP 401: session rejected; refresh or re-login required",
        ),
        Ok(response) if !response.status().is_success() => Check::new(
            "InnerTube/session",
            Status::Failure,
            format!("HTTP {}: request rejected", response.status().as_u16()),
        ),
        Ok(response) => match response.json::<serde_json::Value>().await {
            Ok(body) if body.get("error").is_some() => Check::new(
                "InnerTube/session",
                Status::Failure,
                "API error in HTTP success response; body suppressed",
            ),
            Ok(body)
                if body
                    .pointer("/responseContext/mainAppWebResponseContext/loggedOut")
                    .and_then(|v| v.as_bool())
                    == Some(true)
                    && auth.access_token.is_some() =>
            {
                Check::new(
                    "InnerTube/session",
                    Status::Failure,
                    "server reports logged out; re-login required",
                )
            }
            Ok(_) => Check::new(
                "InnerTube/session",
                Status::Ok,
                if auth.access_token.is_some() {
                    "JSON request accepted; identity/feed access still needs manual QA"
                } else {
                    "anonymous JSON request accepted; no token to validate"
                },
            ),
            Err(error) => Check::new("InnerTube/session", Status::Failure, network_error(&error)),
        },
        Err(error) => Check::new("InnerTube/session", Status::Failure, network_error(&error)),
    };
    checks.push(check);
    checks.push(directory("config directory", &crate::config::config_dir()));
    checks.push(directory("cache directory", &crate::config::cache_dir()));
    checks.push(directory(
        "download directory",
        &crate::config::resolve_path("~/Videos/Yute"),
    ));
    checks
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_controls_are_removed_and_output_is_bounded() {
        assert_eq!(display_text("x\n\r\x1by"), "xy");
        assert_eq!(display_text(&"a".repeat(400)).len(), 240);
    }
    #[tokio::test]
    async fn connection_errors_never_include_the_request_url() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let error = crate::http::client()
            .get(format!("http://{address}/private?secret=fixture"))
            .send()
            .await
            .unwrap_err();
        assert!(!network_error(&error).contains("fixture"));
        assert!(network_error(&error).contains("connection failed"));
    }
    #[test]
    fn invalid_directory_is_reported() {
        let temp = tempfile::NamedTempFile::new().unwrap();
        assert_eq!(directory("test", temp.path()).status, Status::Failure);
    }
}
