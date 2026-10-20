use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc::{channel, Sender};

static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn generate_socket_path() -> PathBuf {
    let pid = std::process::id();
    let counter = SOCKET_COUNTER.fetch_add(1, Ordering::SeqCst);
    let filename = format!("ytui-mpv-{pid}-{counter}.sock");
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join(filename)
    } else {
        std::env::temp_dir().join(filename)
    }
}

fn log_ipc_event(msg: &str) {
    if let Some(home) = std::env::var_os("HOME") {
        let dir = PathBuf::from(home).join(".cache/ytui");
        let _ = std::fs::create_dir_all(&dir);
        let log_file = dir.join("mpv-events.log");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)
        {
            use std::io::Write;
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let _ = writeln!(f, "[{ts}] {msg}");
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct PlaybackStatus {
    pub title: String,
    pub time_pos: f64,
    pub duration: f64,
    pub paused: bool,
    pub paused_for_cache: bool,
    pub core_idle: bool,
    pub idle_active: bool,
    pub volume: f64,
    pub speed: f64,
    pub chapter: i64,
    pub fullscreen: bool,
    pub is_alive: bool,
    pub file_loaded: bool,
    pub stall_error: bool,
    pub cache_duration: f64,
}

#[allow(dead_code)]
pub enum IpcCommand {
    LoadFile(String),
    LoadFileWithStart(String, f64),
    TogglePause,
    Play,
    Pause,
    Seek(f64, bool), // (secs, is_relative)
    SetVolume(f64),
    AdjustVolume(f64),
    SetSpeed(f64),
    AdjustSpeed(f64),
    CycleSubtitle,
    ToggleFullscreen,
    ToggleStats,
    Stop,
}

pub struct MpvIpc {
    tx_cmd: Sender<IpcCommand>,
}

impl MpvIpc {
    pub fn start(
        sock_path: PathBuf,
        tx_status: Sender<PlaybackStatus>,
        running_flag: Arc<AtomicBool>,
    ) -> Result<Self> {
        let (tx_cmd, mut rx_cmd) = channel::<IpcCommand>(64);
        let path_clone = sock_path.clone();

        tokio::spawn(async move {
            let mut stream = None;
            for _ in 0..100 {
                if path_clone.exists() {
                    if let Ok(s) = UnixStream::connect(&path_clone).await {
                        stream = Some(s);
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            let Some(stream) = stream else {
                log_ipc_event("Failed to connect to mpv socket within 10s");
                running_flag.store(false, Ordering::SeqCst);
                return;
            };

            log_ipc_event(&format!(
                "Connected to mpv IPC socket: {}",
                path_clone.display()
            ));

            let (reader, mut writer) = stream.into_split();
            let mut lines = BufReader::new(reader).lines();

            // Observe property commands
            let observe_props = [
                (1, "time-pos"),
                (2, "duration"),
                (3, "pause"),
                (4, "volume"),
                (5, "speed"),
                (6, "chapter"),
                (7, "media-title"),
                (8, "fullscreen"),
                (9, "paused-for-cache"),
                (10, "core-idle"),
                (11, "idle-active"),
                (12, "demuxer-cache-duration"),
            ];

            for (id, prop) in observe_props {
                let req = json!({
                    "command": ["observe_property", id, prop]
                });
                let _ = writer.write_all(format!("{req}\n").as_bytes()).await;
            }
            let _ = writer.flush().await;

            let mut status = PlaybackStatus {
                volume: 100.0,
                speed: 1.0,
                is_alive: true,
                ..Default::default()
            };

            let mut watchdog_ticker = tokio::time::interval(Duration::from_secs(1));
            let mut last_adv_time = std::time::Instant::now();
            let mut prev_time_pos = -1.0f64;

            loop {
                tokio::select! {
                    _ = watchdog_ticker.tick() => {
                        if status.file_loaded && !status.paused && status.is_alive {
                            if (status.time_pos - prev_time_pos).abs() > 0.05 {
                                prev_time_pos = status.time_pos;
                                last_adv_time = std::time::Instant::now();
                                if status.stall_error {
                                    status.stall_error = false;
                                    let _ = tx_status.send(status.clone()).await;
                                }
                            } else if last_adv_time.elapsed().as_secs() >= 15 && !status.stall_error {
                                status.stall_error = true;
                                log_ipc_event(&format!(
                                    "WATCHDOG: time-pos has not advanced for 15s (stuck at {:.2}s), reporting stall error",
                                    status.time_pos
                                ));
                                let _ = tx_status.send(status.clone()).await;
                            }
                        } else {
                            last_adv_time = std::time::Instant::now();
                        }
                    }

                    cmd_opt = rx_cmd.recv() => {
                        let Some(cmd) = cmd_opt else { break; };
                        let cmd_json = match cmd {
                            IpcCommand::LoadFile(url) => {
                                status.file_loaded = false;
                                status.time_pos = 0.0;
                                status.cache_duration = 0.0;
                                status.stall_error = false;
                                prev_time_pos = -1.0;
                                last_adv_time = std::time::Instant::now();
                                log_ipc_event(&format!("Command: loadfile replace {url}"));
                                json!({ "command": ["loadfile", url, "replace"] })
                            }
                            IpcCommand::LoadFileWithStart(url, start) => {
                                status.file_loaded = false;
                                status.time_pos = start;
                                status.cache_duration = 0.0;
                                status.stall_error = false;
                                prev_time_pos = -1.0;
                                last_adv_time = std::time::Instant::now();
                                log_ipc_event(&format!("Command: loadfile replace {url} start={start}"));
                                json!({ "command": ["loadfile", url, "replace", -1, format!("start={start}")] })
                            }
                            IpcCommand::TogglePause => json!({ "command": ["cycle", "pause"] }),
                            IpcCommand::Play => json!({ "command": ["set_property", "pause", false] }),
                            IpcCommand::Pause => json!({ "command": ["set_property", "pause", true] }),
                            IpcCommand::Seek(secs, true) => json!({ "command": ["seek", secs, "relative"] }),
                            IpcCommand::Seek(pct, false) => json!({ "command": ["seek", pct, "absolute-percent"] }),
                            IpcCommand::SetVolume(v) => json!({ "command": ["set_property", "volume", v] }),
                            IpcCommand::AdjustVolume(dv) => json!({ "command": ["add", "volume", dv] }),
                            IpcCommand::SetSpeed(s) => json!({ "command": ["set_property", "speed", s] }),
                            IpcCommand::AdjustSpeed(ds) => json!({ "command": ["add", "speed", ds] }),
                            IpcCommand::CycleSubtitle => json!({ "command": ["cycle", "sub-visibility"] }),
                            IpcCommand::ToggleFullscreen => json!({ "command": ["cycle", "fullscreen"] }),
                            IpcCommand::ToggleStats => json!({ "command": ["script-binding", "stats/display-stats-toggle"] }),
                            IpcCommand::Stop => json!({ "command": ["stop"] }),
                        };
                        let _ = writer.write_all(format!("{cmd_json}\n").as_bytes()).await;
                        let _ = writer.flush().await;
                    }

                    line_res = lines.next_line() => {
                        match line_res {
                            Ok(Some(line)) => {
                                if let Ok(val) = serde_json::from_str::<Value>(&line) {
                                    if let Some(event) = val.get("event").and_then(|e| e.as_str()) {
                                        match event {
                                            "file-loaded" => {
                                                log_ipc_event("Event: file-loaded (sending pause=false unpause handshake)");
                                                status.file_loaded = true;
                                                status.stall_error = false;
                                                prev_time_pos = -1.0;
                                                last_adv_time = std::time::Instant::now();
                                                let unpause = json!({ "command": ["set_property", "pause", false] });
                                                let _ = writer.write_all(format!("{unpause}\n").as_bytes()).await;
                                                let _ = writer.flush().await;
                                                let _ = tx_status.send(status.clone()).await;
                                            }
                                            "property-change" => {
                                                if let Some(name) = val.get("name").and_then(|n| n.as_str()) {
                                                    let data = val.get("data");
                                                    match name {
                                                        "time-pos" => status.time_pos = data.and_then(|d| d.as_f64()).unwrap_or(status.time_pos),
                                                        "duration" => status.duration = data.and_then(|d| d.as_f64()).unwrap_or(status.duration),
                                                        "pause" => status.paused = data.and_then(|d| d.as_bool()).unwrap_or(status.paused),
                                                        "paused-for-cache" => status.paused_for_cache = data.and_then(|d| d.as_bool()).unwrap_or(false),
                                                        "core-idle" => status.core_idle = data.and_then(|d| d.as_bool()).unwrap_or(false),
                                                        "idle-active" => status.idle_active = data.and_then(|d| d.as_bool()).unwrap_or(false),
                                                        "volume" => status.volume = data.and_then(|d| d.as_f64()).unwrap_or(status.volume),
                                                        "speed" => status.speed = data.and_then(|d| d.as_f64()).unwrap_or(status.speed),
                                                        "chapter" => status.chapter = data.and_then(|d| d.as_i64()).unwrap_or(status.chapter),
                                                        "media-title" => status.title = data.and_then(|d| d.as_str()).unwrap_or("").to_string(),
                                                        "fullscreen" => status.fullscreen = data.and_then(|d| d.as_bool()).unwrap_or(status.fullscreen),
                                                        "demuxer-cache-duration" => status.cache_duration = data.and_then(|d| d.as_f64()).unwrap_or(0.0),
                                                        _ => {}
                                                    }
                                                    let _ = tx_status.send(status.clone()).await;
                                                }
                                            }
                                            "end-file" => {
                                                log_ipc_event("Event: end-file");
                                                status.file_loaded = false;
                                                status.time_pos = 0.0;
                                                status.paused = true;
                                                let _ = tx_status.send(status.clone()).await;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                log_ipc_event("EOF from mpv IPC socket");
                                break;
                            }
                            Err(e) => {
                                log_ipc_event(&format!("Error reading IPC socket: {e}"));
                                break;
                            }
                        }
                    }
                }
            }

            status.is_alive = false;
            let _ = tx_status.send(status).await;
            running_flag.store(false, Ordering::SeqCst);
        });

        Ok(Self { tx_cmd })
    }

    pub fn send(&self, cmd: IpcCommand) {
        let _ = self.tx_cmd.try_send(cmd);
    }
}
