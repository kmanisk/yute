pub mod ipc;
pub mod mpvconf;
pub mod queue;

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
    current_sock_path: Option<PathBuf>,
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
            current_sock_path: None,
            ipc: None,
            tx_status,
            rx_status,
        }
    }

    pub fn is_alive(&self) -> bool {
        self.child_process.is_some() && self.status.is_alive
    }

    pub fn play_video(
        &mut self,
        video: VideoCard,
        mode_override: Option<PlaybackMode>,
        cfg: &Config,
    ) -> Result<()> {
        let active_mode = mode_override.unwrap_or(self.mode);
        let target_url = if video.id.starts_with('/') || video.id.starts_with('.') {
            video.id.clone()
        } else {
            format!("https://www.youtube.com/watch?v={}", video.id)
        };

        self.current_video = Some(video);

        // If window player is already running, switch video via IPC loadfile replace
        if (active_mode == PlaybackMode::Window || active_mode == PlaybackMode::Auto)
            && self.is_alive()
        {
            if let Some(ref ipc) = self.ipc {
                ipc.send(IpcCommand::LoadFile(target_url));
                self.status.file_loaded = false;
                self.status.time_pos = 0.0;
                self.status.stall_error = false;
                return Ok(());
            }
        }

        self.stop();

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
        self.stop();

        let sock_path = generate_socket_path();
        if sock_path.exists() {
            let _ = std::fs::remove_file(&sock_path);
        }

        let cache_dir = if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(".cache/ytui")
        } else {
            PathBuf::from("/tmp/ytui")
        };
        let _ = std::fs::create_dir_all(&cache_dir);
        let mpv_log_file = cache_dir.join("mpv.log");

        let hwdec = if cfg.playback.hwdec.is_empty() || cfg.playback.hwdec == "auto-safe" {
            "no".to_string()
        } else {
            cfg.playback.hwdec.clone()
        };

        let mut cmd = Command::new("mpv");
        cmd.arg(format!("--input-ipc-server={}", sock_path.display()))
            .arg("--no-terminal")
            .arg("--no-input-terminal")
            .arg("--force-window=yes")
            .arg("--idle=no")
            .arg(format!("--log-file={}", mpv_log_file.display()))
            .arg(format!("--ytdl-format={}", cfg.playback.resolved_ytdl_format()))
            .arg(format!("--hwdec={hwdec}"))
            .arg("--vo=gpu")
            .arg("--cache=yes")
            .arg("--demuxer-max-bytes=64M")
            .arg("--demuxer-max-back-bytes=20M")
            .arg("--demuxer-readahead-secs=10")
            .arg("--ytdl=yes")
            .arg("--ytdl-raw-options-append=no-playlist=");

        if !cfg.playback.osd_bar {
            cmd.arg("--osd-bar=no");
        }

        if !cfg.playback.use_user_mpv_config {
            cmd.arg(format!("--config-dir={}", self.profile.dir.display()));
        }

        let cookies_path = crate::config::resolve_path(&cfg.backend.cookies_file);
        if cookies_path.exists() {
            let has_auth = std::fs::read_to_string(&cookies_path)
                .map(|s| s.contains("LOGIN_INFO") || s.contains("SAPISID") || s.contains("__Secure"))
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

        cmd.arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let child = cmd.spawn().context("Failed to spawn mpv window")?;
        self.child_process = Some(child);
        self.current_sock_path = Some(sock_path.clone());
        self.is_playing.store(true, Ordering::SeqCst);

        let ipc = MpvIpc::start(
            sock_path,
            self.tx_status.clone(),
            Arc::clone(&self.is_playing),
        )?;
        self.ipc = Some(ipc);

        Ok(())
    }

    fn start_inline_playback(&mut self, url: &str, cfg: &Config) -> Result<()> {
        self.stop();

        // Detect terminal graphic protocol flags
        let mut vo_arg = "--vo=sixel".to_string();
        let mut extra_vo = vec!["--vo-sixel-buffered=yes".to_string()];

        let term = std::env::var("TERM").unwrap_or_default();
        let term_prog = std::env::var("TERM_PROGRAM").unwrap_or_default();
        if term.contains("kitty") || term_prog.contains("kitty") || std::env::var("KITTY_PID").is_ok() {
            vo_arg = "--vo=kitty".to_string();
            extra_vo = vec!["--vo-kitty-use-shm=yes".to_string()];
        } else if term.contains("alacritty") {
            vo_arg = "--vo=tct".to_string();
            extra_vo = vec![];
        }

        let hwdec = if cfg.playback.hwdec.is_empty() || cfg.playback.hwdec == "auto-safe" {
            "no".to_string()
        } else {
            cfg.playback.hwdec.clone()
        };

        let mut cmd = Command::new("mpv");
        cmd.arg(&vo_arg)
            .arg(format!("--hwdec={hwdec}"))
            .arg("--cache=yes")
            .arg("--demuxer-max-bytes=100M")
            .arg("--demuxer-readahead-secs=15")
            .arg(format!(
                "--ytdl-format=bestvideo[height<={}]+bestaudio/best[height<={}]/best",
                cfg.inline.max_height, cfg.inline.max_height
            ))
            .arg("--ytdl=yes")
            .arg("--ytdl-raw-options-append=no-playlist=");

        for extra in &extra_vo {
            cmd.arg(extra);
        }

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

        cmd.arg("--force-window=no")
            .arg("--terminal=yes")
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .arg(url);

        // Terminal handoff: release raw mode so mpv controls stdio and receives hotkeys
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::event::PopKeyboardEnhancementFlags,
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        let _ = crossterm::terminal::disable_raw_mode();

        let mut child = cmd.spawn().context("Failed to run inline mpv")?;
        let _ = child.wait();

        // Restore terminal raw mode and alternate screen for ytui
        let _ = crossterm::terminal::enable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::EnterAlternateScreen,
            crossterm::cursor::Hide,
            crossterm::event::PushKeyboardEnhancementFlags(
                crossterm::event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | crossterm::event::KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            )
        );

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
            ipc.send(cmd);
        }
    }

    pub fn toggle_stats(&self) {
        self.send_command(IpcCommand::ToggleStats);
    }

    pub fn stop(&mut self) {
        if let Some(ref ipc) = self.ipc {
            ipc.send(IpcCommand::Stop);
        }
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
        self.ipc = None;
        self.is_playing.store(false, Ordering::SeqCst);
        self.status.is_alive = false;

        if let Some(sock) = self.current_sock_path.take() {
            let _ = std::fs::remove_file(sock);
        }
    }
}
