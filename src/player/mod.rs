pub mod ipc;
pub mod mpvconf;
pub mod queue;

use crate::backend::models::VideoCard;
use crate::config::Config;
use anyhow::{Context, Result};
use ipc::{IpcCommand, MpvIpc, PlaybackStatus};
use mpvconf::MpvProfile;
use queue::PlayQueue;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{channel, Receiver, Sender};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackMode {
    Window,
    Inline,
    Auto,
}

pub struct Player {
    pub mode: PlaybackMode,
    pub profile: MpvProfile,
    pub queue: PlayQueue,
    pub current_video: Option<VideoCard>,
    pub status: PlaybackStatus,
    pub is_playing: Arc<AtomicBool>,
    child_process: Option<Child>,
    ipc: Option<MpvIpc>,
    tx_status: Sender<PlaybackStatus>,
    rx_status: Receiver<PlaybackStatus>,
}

impl Player {
    pub fn new(cfg: &Config) -> Self {
        let (tx_status, rx_status) = channel::<PlaybackStatus>(32);
        let mode = match cfg.playback.default_mode.as_str() {
            "inline" => PlaybackMode::Inline,
            "auto" => PlaybackMode::Auto,
            _ => PlaybackMode::Window,
        };

        let profile = MpvProfile::new();
        let cookies_path = if !cfg.backend.cookies_file.is_empty() {
            Some(crate::config::resolve_path(&cfg.backend.cookies_file))
        } else {
            None
        };
        let _ = profile.setup_default_configs(cookies_path.as_deref());

        Self {
            mode,
            profile,
            queue: PlayQueue::new(),
            current_video: None,
            status: PlaybackStatus::default(),
            is_playing: Arc::new(AtomicBool::new(false)),
            child_process: None,
            ipc: None,
            tx_status,
            rx_status,
        }
    }

    pub fn play_video(
        &mut self,
        video: VideoCard,
        mode_override: Option<PlaybackMode>,
        cfg: &Config,
    ) -> Result<()> {
        let active_mode = mode_override.unwrap_or(self.mode);
        let target_url = if video.id.starts_with('/') || video.id.starts_with('.') {
            // Local file
            video.id.clone()
        } else {
            format!("https://www.youtube.com/watch?v={}", video.id)
        };

        self.current_video = Some(video);

        match active_mode {
            PlaybackMode::Window | PlaybackMode::Auto => {
                self.start_window_playback(&target_url, cfg)?;
            }
            PlaybackMode::Inline => {
                self.start_inline_playback(&target_url, cfg)?;
            }
        }

        Ok(())
    }

    fn start_window_playback(&mut self, url: &str, cfg: &Config) -> Result<()> {
        let sock_path = MpvIpc::socket_path();
        if sock_path.exists() {
            let _ = std::fs::remove_file(&sock_path);
        }

        let mut cmd = Command::new("mpv");
        cmd.arg(format!("--input-ipc-server={}", sock_path.display()))
            .arg("--force-window=immediate")
            .arg(format!("--ytdl-format={}", cfg.playback.resolved_ytdl_format()))
            .arg(format!("--hwdec={}", cfg.playback.hwdec))
            .arg("--vo=gpu")
            .arg("--cache=yes")
            .arg("--demuxer-max-bytes=150M")
            .arg("--demuxer-max-back-bytes=50M")
            .arg("--demuxer-readahead-secs=20");

        if !cfg.playback.osd_bar {
            cmd.arg("--osd-bar=no");
        }

        if !cfg.playback.use_user_mpv_config {
            cmd.arg(format!("--config-dir={}", self.profile.dir.display()));
        }

        let cookies_path = crate::config::resolve_path(&cfg.backend.cookies_file);
        if cookies_path.exists() {
            cmd.arg(format!(
                "--ytdl-raw-options-append=cookies={}",
                cookies_path.display()
            ));
            if cfg.playback.mark_watched {
                cmd.arg("--ytdl-raw-options-append=mark-watched=");
            }
        }

        cmd.arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = cmd.spawn().context("Failed to spawn mpv window")?;
        self.child_process = Some(child);
        self.is_playing.store(true, Ordering::SeqCst);

        let ipc = MpvIpc::start(self.tx_status.clone(), Arc::clone(&self.is_playing))?;
        self.ipc = Some(ipc);

        Ok(())
    }

    fn start_inline_playback(&mut self, url: &str, cfg: &Config) -> Result<()> {
        // Detect protocol flags
        let mut vo_arg = "--vo=sixel".to_string();
        let mut extra_vo = "--vo-sixel-buffered=yes".to_string();

        let term = std::env::var("TERM").unwrap_or_default();
        if term.contains("kitty") || std::env::var("KITTY_PID").is_ok() {
            vo_arg = "--vo=kitty".to_string();
            extra_vo = "--vo-kitty-use-shm=yes".to_string();
        }

        let mut cmd = Command::new("mpv");
        cmd.arg(&vo_arg)
            .arg(&extra_vo)
            .arg(format!(
                "--ytdl-format=bestvideo[height<={}]+bestaudio/best[height<={}]",
                cfg.inline.max_height, cfg.inline.max_height
            ));

        for extra in &cfg.inline.extra_args {
            cmd.arg(extra);
        }

        let cookies_path = crate::config::resolve_path(&cfg.backend.cookies_file);
        if cookies_path.exists() {
            cmd.arg(format!(
                "--ytdl-raw-options-append=cookies={}",
                cookies_path.display()
            ));
        }

        cmd.arg(url);

        // Terminal handoff: mpv inherits terminal stdio
        let mut child = cmd.spawn().context("Failed to run inline mpv")?;
        let _ = child.wait();

        Ok(())
    }

    pub fn poll_updates(&mut self) -> bool {
        let mut updated = false;
        while let Ok(st) = self.rx_status.try_recv() {
            self.status = st;
            updated = true;
        }

        if let Some(ref mut child) = self.child_process {
            if let Ok(Some(_exit)) = child.try_wait() {
                self.child_process = None;
                self.is_playing.store(false, Ordering::SeqCst);
                self.status.is_alive = false;
                updated = true;
            }
        }

        updated
    }

    pub fn send_command(&self, cmd: IpcCommand) {
        if let Some(ref ipc) = self.ipc {
            ipc.send(cmd);
        }
    }

    pub fn stop(&mut self) {
        if let Some(ref ipc) = self.ipc {
            ipc.send(IpcCommand::Stop);
        }
        if let Some(ref mut child) = self.child_process {
            let _ = child.kill();
            self.child_process = None;
        }
        self.is_playing.store(false, Ordering::SeqCst);
        self.status.is_alive = false;
    }
}
