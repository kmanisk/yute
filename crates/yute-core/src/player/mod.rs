pub mod ipc;
pub mod mpvconf;
pub mod queue;
pub mod tracking;
pub mod traits;

use crate::backend::models::VideoCard;
use crate::config::Config;
use anyhow::{Context, Result};
use ipc::{generate_socket_path, IpcCommand, MpvIpc, PlaybackStatus};
use mpvconf::MpvProfile;
use queue::PlayQueue;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{channel, Receiver, Sender};

pub struct Player {
    pub profile: MpvProfile,
    pub queue: PlayQueue,
    pub current_video: Option<VideoCard>,
    pub status: PlaybackStatus,
    pub is_playing: Arc<AtomicBool>,
    child_process: Option<Child>,
    current_sock_path: Option<PathBuf>,
    ipc: Option<MpvIpc>,
    tx_status: Sender<PlaybackStatus>,
    rx_status: Receiver<PlaybackStatus>,
}

impl Player {
    pub fn new(_cfg: &Config) -> Self {
        let (tx_status, rx_status) = channel::<PlaybackStatus>(32);
        let profile = MpvProfile::new();

        Self {
            profile,
            queue: PlayQueue::new(),
            current_video: None,
            status: PlaybackStatus::default(),
            is_playing: Arc::new(AtomicBool::new(false)),
            child_process: None,
            current_sock_path: None,
            ipc: None,
            tx_status,
            rx_status,
        }
    }

    pub fn has_session(&self) -> bool {
        self.child_process.is_some()
    }

    pub fn is_alive(&self) -> bool {
        self.child_process.is_some()
            && self.status.is_alive
            && self.is_playing.load(Ordering::SeqCst)
    }

    pub fn current_video_id(&self) -> Option<&str> {
        self.current_video.as_ref().map(|v| v.id.as_str())
    }

    pub fn play_video(&mut self, video: VideoCard, cfg: &Config) -> Result<()> {
        self.poll_updates();
        let target_url = if video.id.starts_with('/') || video.id.starts_with('.') {
            video.id.clone()
        } else {
            format!("https://www.youtube.com/watch?v={}", video.id)
        };

        self.current_video = Some(video);

        let resume_sec = self
            .current_video
            .as_ref()
            .and_then(|v| v.resolved_resume_sec());

        // If window player is already running, switch video via IPC loadfile replace
        if self.is_alive() {
            if let Some(ref ipc) = self.ipc {
                if ipc
                    .send(IpcCommand::LoadFileWithStart(
                        target_url.clone(),
                        resume_sec.unwrap_or(0) as f64,
                    ))
                    .is_ok()
                {
                    self.status.file_loaded = false;
                    self.status.time_pos = resume_sec.unwrap_or(0) as f64;
                    self.status.duration = 0.0;
                    self.status.cache_duration = 0.0;
                    self.status.stall_error = false;
                    self.status.playback_error = None;
                    return Ok(());
                }
            }
        }

        self.stop();

        self.start_window_playback(&target_url, cfg)?;

        Ok(())
    }

    fn start_window_playback(&mut self, url: &str, cfg: &Config) -> Result<()> {
        self.stop();
        self.profile
            .setup_default_configs(None, Some(&cfg.playback.hwdec))?;

        let sock_path = generate_socket_path();
        if sock_path.exists() {
            let _ = std::fs::remove_file(&sock_path);
        }

        let cache_dir = if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(".cache/yute")
        } else {
            PathBuf::from("/tmp/yute")
        };
        let _ = std::fs::create_dir_all(&cache_dir);
        let mpv_log_file = cache_dir.join("mpv.log");

        let hwdec = if cfg.playback.hwdec.is_empty() {
            "auto-safe".to_string()
        } else {
            cfg.playback.hwdec.clone()
        };

        let mut cmd = Command::new("mpv");
        cmd.arg(format!("--input-ipc-server={}", sock_path.display()))
            .arg("--no-terminal")
            .arg("--no-input-terminal")
            .arg("--force-window=yes")
            .arg("--idle=yes")
            .arg("--keep-open=yes")
            .arg(format!("--log-file={}", mpv_log_file.display()))
            .arg("--ytdl=yes")
            .arg("--ytdl-raw-options-append=no-playlist=");

        if cfg.playback.use_user_mpv_config {
            // User MPV mode: Let user's ~/.config/mpv/mpv.conf and input.conf control vo, hwdec, shaders, scripts
        } else {
            // Dedicated yute profile mode
            cmd.arg(format!("--config-dir={}", self.profile.dir.display()))
                .arg(format!(
                    "--ytdl-format={}",
                    cfg.playback.resolved_ytdl_format()
                ))
                .arg(format!("--hwdec={hwdec}"))
                .arg("--vo=gpu-next")
                .arg("--cache=yes")
                .arg(format!(
                    "--demuxer-max-bytes={}MiB",
                    cfg.playback.stream_cache_mb()
                ))
                .arg("--demuxer-max-back-bytes=32MiB")
                .arg(format!(
                    "--cache-secs={}",
                    cfg.playback.stream_cache_seconds.clamp(5, 60)
                ))
                .arg(format!(
                    "--demuxer-readahead-secs={}",
                    cfg.playback.stream_cache_seconds.clamp(5, 60)
                ))
                .arg("--cache-pause-wait=1")
                .arg("--demuxer-seekable-cache=yes")
                .arg("--hr-seek=no");

            if !cfg.playback.osd_bar {
                cmd.arg("--osd-bar=no");
            }
        }

        let cookies_path = crate::config::resolve_path(&cfg.backend.cookies_file);
        if cfg.backend.auth != "guest" && cookies_path.exists() {
            let has_auth = std::fs::read_to_string(&cookies_path)
                .map(|s| {
                    s.contains("LOGIN_INFO") || s.contains("SAPISID") || s.contains("__Secure")
                })
                .unwrap_or(false);
            if has_auth {
                cmd.arg(format!(
                    "--ytdl-raw-options-append=cookies={}",
                    cookies_path.display()
                ));
            }
            if cfg.playback.mark_watched {
                cmd.arg("--ytdl-raw-options-append=mark-watched=");
            }
        }

        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }

        // Start idle, subscribe to IPC events, then load. A fast initial file
        // can otherwise emit file-loaded before the IPC client connects.
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = cmd.spawn().context("Failed to spawn mpv window")?;
        self.attach_window_process(child, sock_path, url)
    }

    fn attach_window_process(&mut self, child: Child, sock_path: PathBuf, url: &str) -> Result<()> {
        self.child_process = Some(child);
        self.current_sock_path = Some(sock_path.clone());
        // Each launch owns its status channel and running flag. A terminating
        // IPC task must not overwrite a subsequent player's state.
        let (tx, rx) = channel(32);
        self.tx_status = tx;
        self.rx_status = rx;
        self.is_playing = Arc::new(AtomicBool::new(true));
        self.status = PlaybackStatus {
            is_alive: true,
            ..Default::default()
        };

        let ipc = MpvIpc::start(
            sock_path,
            self.tx_status.clone(),
            Arc::clone(&self.is_playing),
        )?;
        let start = self
            .current_video
            .as_ref()
            .and_then(VideoCard::resolved_resume_sec)
            .unwrap_or(0);
        if let Err(error) = ipc.send(IpcCommand::LoadFileWithStart(url.to_string(), start as f64)) {
            self.stop();
            return Err(error);
        }
        self.ipc = Some(ipc);

        Ok(())
    }

    pub fn poll_updates(&mut self) -> bool {
        let mut updated = false;
        while let Ok(st) = self.rx_status.try_recv() {
            self.status = st;
            updated = true;
        }

        if let Some(ref mut child) = self.child_process {
            if let Ok(Some(exit)) = child.try_wait() {
                if !exit.success() && self.status.playback_error.is_none() {
                    self.status.playback_error = Some(format!("MPV exited with {exit}"));
                }
                self.ipc = None;
                self.child_process = None;
                self.is_playing.store(false, Ordering::SeqCst);
                self.status.is_alive = false;
                if let Some(sock) = self.current_sock_path.take() {
                    let _ = std::fs::remove_file(sock);
                }
                updated = true;
            }
        }

        updated
    }

    pub fn send_command(&self, cmd: IpcCommand) {
        if let Some(ref ipc) = self.ipc {
            let _ = ipc.send(cmd);
        }
    }

    pub fn toggle_stats(&self) {
        self.send_command(IpcCommand::ToggleStats);
    }

    pub fn stop_playback(&mut self) {
        if let Some(ref ipc) = self.ipc {
            let _ = ipc.send(IpcCommand::Stop);
        }
        self.current_video = None;
        self.status.file_loaded = false;
        self.status.time_pos = 0.0;
        self.status.paused = true;
    }

    pub fn stop(&mut self) {
        self.ipc = None;
        if let Some(ref mut child) = self.child_process {
            #[cfg(unix)]
            {
                let pid = child.id();
                let _ = std::process::Command::new("kill")
                    .arg("-15")
                    .arg(format!("-{}", pid))
                    .status();
                std::thread::sleep(std::time::Duration::from_millis(50));
                let _ = std::process::Command::new("kill")
                    .arg("-9")
                    .arg(format!("-{}", pid))
                    .status();
            }
            let _ = child.kill();
            let _ = child.wait();
            self.child_process = None;
        }
        self.is_playing.store(false, Ordering::SeqCst);
        self.status = PlaybackStatus::default();
        while self.rx_status.try_recv().is_ok() {}

        if let Some(sock) = self.current_sock_path.take() {
            let _ = std::fs::remove_file(sock);
        }
    }
}

#[cfg(test)]
mod tests;
