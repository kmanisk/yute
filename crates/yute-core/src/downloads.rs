//! Argument-only, anonymous yt-dlp jobs. No account secrets enter this process.
use crate::config::{resolve_path, Config};
use anyhow::{bail, Result};
use std::process::Stdio;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc::UnboundedSender,
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Video(Option<u32>),
    Exact(String),
    Audio(String),
}
/// Parse a command before starting a job. Invalid quality must never mean best.
pub fn parse_mode(input: &str, config: &Config) -> Result<(bool, Mode)> {
    let mut words = input.split_whitespace().collect::<Vec<_>>();
    let playlist = words.first() == Some(&"playlist");
    if playlist {
        words.remove(0);
    }
    let mode = match words.as_slice() {
        ["video"] => Mode::Video(parse_quality(&config.downloads.video_quality)?),
        ["video", quality] => Mode::Video(parse_quality(quality)?),
        ["audio"] => Mode::Audio(config.downloads.audio_mode.clone()),
        ["audio", kind] => Mode::Audio((*kind).into()),
        _ => bail!(
            "Usage: :download [playlist] video [720p|best] | audio [best|opus|aac|mp3|flac|m4a]"
        ),
    };
    if let Mode::Audio(kind) = &mode {
        if !matches!(
            kind.as_str(),
            "best" | "opus" | "aac" | "mp3" | "flac" | "m4a"
        ) {
            bail!("Unknown audio mode; choose best, opus, aac, mp3, flac or m4a");
        }
    }
    Ok((playlist, mode))
}
fn parse_quality(value: &str) -> Result<Option<u32>> {
    if value == "best" {
        return Ok(None);
    }
    let height = value.strip_suffix('p').unwrap_or(value).parse::<u32>();
    match height {
        Ok(height @ 1..=8640) => Ok(Some(height)),
        _ => bail!("Invalid video quality; use a resolution such as 720p or best"),
    }
}
#[derive(Clone, Debug)]
pub struct Request {
    pub url: String,
    pub playlist: bool,
    pub mode: Mode,
    pub ffmpeg: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Progress {
    pub item: Option<u32>,
    pub total: Option<u32>,
    pub percent: String,
    pub completed: u32,
    pub failed: u32,
}
pub fn video_selector(height: Option<u32>) -> String {
    height
        .map(|n| format!("bestvideo[height<={n}]+bestaudio/best[height<={n}]"))
        .unwrap_or_else(|| "bestvideo+bestaudio/best".into())
}
pub fn command(request: &Request, config: &Config) -> Result<tokio::process::Command> {
    let mut cmd = tokio::process::Command::new("yt-dlp");
    cmd.kill_on_drop(true).args([
        "--ignore-config",
        "--js-runtimes",
        "node",
        "--no-simulate",
        "--restrict-filenames",
        "--newline",
        "--progress",
        "--no-warnings",
        "--no-overwrites",
        "--socket-timeout",
        "10",
        "--retries",
        "2",
    ]);
    let selector = match &request.mode {
        Mode::Video(h) => video_selector(*h),
        Mode::Exact(s) => s.clone(),
        Mode::Audio(kind) => match kind.as_str() {
            "opus" => "bestaudio[acodec=opus]/bestaudio".into(),
            "aac" => "bestaudio[acodec^=mp4a]/bestaudio".into(),
            "best" => "bestaudio".into(),
            "mp3" | "flac" | "m4a" => {
                if !request.ffmpeg {
                    bail!("ffmpeg is required for audio conversion");
                }
                cmd.args(["--extract-audio", "--audio-format", kind]);
                if kind == "mp3" {
                    cmd.args(["--audio-quality", "320K"]);
                }
                "bestaudio/best".into()
            }
            _ => bail!("Unknown audio download mode"),
        },
    };
    cmd.arg("--format")
        .arg(selector)
        .arg("--paths")
        .arg(resolve_path(&config.downloads.directory));
    if request.playlist {
        cmd.args(["--yes-playlist", "--ignore-errors"]);
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| resolve_path("~/.local/state"))
            .join("yute");
        use sha2::{Digest, Sha256};
        let archive = hex::encode(Sha256::digest(
            format!("{}:{:?}", request.url, request.mode).as_bytes(),
        ));
        cmd.arg("--download-archive")
            .arg(state.join(format!("download-archive-{}.txt", &archive[..16])));
        cmd.args(["--output",if config.downloads.playlist_subdirectories{"%(playlist_title|Playlist).120B/%(playlist_index)02d - %(title).160B [%(id)s].%(ext)s"}else{"%(playlist_index)02d - %(title).160B [%(id)s].%(ext)s"}]);
    } else {
        cmd.args([
            "--no-playlist",
            "--output",
            "%(title).160B [%(id)s].%(ext)s",
        ]);
    }
    if !config.downloads.keep_partial {
        cmd.arg("--no-part");
    }
    if config.downloads.write_metadata {
        cmd.arg("--write-info-json");
    }
    if config.downloads.write_thumbnail {
        cmd.arg("--write-thumbnail");
    }
    if config.downloads.write_subtitles {
        cmd.arg("--write-subs");
    }
    cmd.args([
        "--progress-template",
        "download:YUTE_PROGRESS:%(info.playlist_index)s:%(info.n_entries)s:%(progress)j",
        "--print",
        "after_move:YUTE_DONE",
    ]);
    cmd.arg("--")
        .arg(&request.url)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    Ok(cmd)
}
pub async fn run(
    request: Request,
    config: Config,
    updates: UnboundedSender<Progress>,
) -> Result<Progress> {
    tokio::fs::create_dir_all(resolve_path(&config.downloads.directory)).await?;
    if request.playlist {
        let state = std::env::var_os("XDG_STATE_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| resolve_path("~/.local/state"))
            .join("yute");
        tokio::fs::create_dir_all(state).await?;
    }
    run_command(command(&request, &config)?, updates).await
}
/// Shared process layer also permits deterministic fake-process tests.
pub async fn run_command(
    mut command: tokio::process::Command,
    updates: UnboundedSender<Progress>,
) -> Result<Progress> {
    command
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|_| anyhow::anyhow!("Cannot start yt-dlp; run yute doctor"))?;
    let mut out = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut err = BufReader::new(child.stderr.take().unwrap()).lines();
    let (mut stdout_open, mut stderr_open) = (true, true);
    let mut progress = Progress::default();
    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
    while stdout_open || stderr_open {
        let line = tokio::select! {line=out.next_line(),if stdout_open=>{let line=line?;if line.is_none(){stdout_open=false;}line},line=err.next_line(),if stderr_open=>{let line=line?;if line.is_none(){stderr_open=false;}line}};
        if let Some(line) = line {
            update_progress(&mut progress, &line);
            if last.elapsed() >= std::time::Duration::from_millis(500) {
                let _ = updates.send(progress.clone());
                last = std::time::Instant::now();
            }
        }
    }
    let status = child.wait().await?;
    if !status.success() && progress.completed == 0 {
        bail!("yt-dlp exited with {}; check access, network, disk space and ffmpeg (private stderr suppressed)",status);
    }
    Ok(progress)
}
pub fn update_progress(progress: &mut Progress, line: &str) {
    if line == "YUTE_DONE" {
        progress.completed += 1;
    }
    if line.starts_with("ERROR:") {
        progress.failed += 1;
    }
    if let Some(body) = line.strip_prefix("YUTE_PROGRESS:") {
        let mut parts = body.splitn(3, ':');
        progress.item = parts.next().and_then(|v| v.parse().ok());
        progress.total = parts.next().and_then(|v| v.parse().ok());
        if let Some(value) = parts
            .next()
            .and_then(|v| serde_json::from_str::<serde_json::Value>(v).ok())
        {
            let downloaded = value["downloaded_bytes"].as_f64();
            let total = value["total_bytes"]
                .as_f64()
                .or_else(|| value["total_bytes_estimate"].as_f64());
            if let (Some(n), Some(d)) = (downloaded, total) {
                if d > 0. {
                    progress.percent = format!("{:.0}%", (100. * n / d).clamp(0., 100.));
                }
            }
        }
    }
}
