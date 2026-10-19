use anyhow::Result;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc::{channel, Sender};

#[derive(Clone, Debug, Default)]
pub struct PlaybackStatus {
    pub title: String,
    pub time_pos: f64,
    pub duration: f64,
    pub paused: bool,
    pub volume: f64,
    pub speed: f64,
    pub chapter: i64,
    pub fullscreen: bool,
    pub is_alive: bool,
}

#[allow(dead_code)]
pub enum IpcCommand {
    LoadFile(String),
    TogglePause,
    Seek(f64, bool), // (secs, is_relative)
    SetVolume(f64),
    AdjustVolume(f64),
    SetSpeed(f64),
    AdjustSpeed(f64),
    CycleSubtitle,
    ToggleFullscreen,
    Stop,
}

pub struct MpvIpc {
    tx_cmd: Sender<IpcCommand>,
}

impl MpvIpc {
    pub fn socket_path() -> PathBuf {
        if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
            PathBuf::from(runtime_dir).join("ytui-mpv.sock")
        } else {
            PathBuf::from("/tmp/ytui-mpv.sock")
        }
    }

    pub fn start(
        tx_status: Sender<PlaybackStatus>,
        running_flag: Arc<AtomicBool>,
    ) -> Result<Self> {
        let (tx_cmd, mut rx_cmd) = channel::<IpcCommand>(32);
        let sock_path = Self::socket_path();

        tokio::spawn(async move {
            let mut stream = None;
            for _ in 0..25 {
                if sock_path.exists() {
                    if let Ok(s) = UnixStream::connect(&sock_path).await {
                        stream = Some(s);
                        break;
                    }
                }
                tokio::time::sleep(Duration::from_millis(80)).await;
            }

            let Some(stream) = stream else {
                running_flag.store(false, Ordering::SeqCst);
                return;
            };

            let (reader, mut writer) = stream.into_split();
            let mut lines = BufReader::new(reader).lines();

            // Setup observe_property commands
            let observe_props = [
                (1, "time-pos"),
                (2, "duration"),
                (3, "pause"),
                (4, "volume"),
                (5, "speed"),
                (6, "chapter"),
                (7, "media-title"),
                (8, "fullscreen"),
            ];

            for (id, prop) in observe_props {
                let req = json!({
                    "command": ["observe_property", id, prop]
                });
                let _ = writer.write_all(format!("{req}\n").as_bytes()).await;
            }

            let mut status = PlaybackStatus {
                volume: 100.0,
                speed: 1.0,
                is_alive: true,
                ..Default::default()
            };

            loop {
                tokio::select! {
                    cmd_opt = rx_cmd.recv() => {
                        let Some(cmd) = cmd_opt else { break; };
                        let cmd_json = match cmd {
                            IpcCommand::LoadFile(url) => json!({ "command": ["loadfile", url, "replace"] }),
                            IpcCommand::TogglePause => json!({ "command": ["cycle", "pause"] }),
                            IpcCommand::Seek(secs, true) => json!({ "command": ["seek", secs, "relative"] }),
                            IpcCommand::Seek(pct, false) => json!({ "command": ["seek", pct, "absolute-percent"] }),
                            IpcCommand::SetVolume(v) => json!({ "command": ["set_property", "volume", v] }),
                            IpcCommand::AdjustVolume(dv) => json!({ "command": ["add", "volume", dv] }),
                            IpcCommand::SetSpeed(s) => json!({ "command": ["set_property", "speed", s] }),
                            IpcCommand::AdjustSpeed(ds) => json!({ "command": ["add", "speed", ds] }),
                            IpcCommand::CycleSubtitle => json!({ "command": ["cycle", "sub-visibility"] }),
                            IpcCommand::ToggleFullscreen => json!({ "command": ["cycle", "fullscreen"] }),
                            IpcCommand::Stop => json!({ "command": ["stop"] }),
                        };
                        let _ = writer.write_all(format!("{cmd_json}\n").as_bytes()).await;
                    }

                    line_res = lines.next_line() => {
                        match line_res {
                            Ok(Some(line)) => {
                                if let Ok(val) = serde_json::from_str::<Value>(&line) {
                                    if let Some(event) = val.get("event").and_then(|e| e.as_str()) {
                                        if event == "property-change" {
                                            if let Some(name) = val.get("name").and_then(|n| n.as_str()) {
                                                let data = val.get("data");
                                                match name {
                                                    "time-pos" => status.time_pos = data.and_then(|d| d.as_f64()).unwrap_or(status.time_pos),
                                                    "duration" => status.duration = data.and_then(|d| d.as_f64()).unwrap_or(status.duration),
                                                    "pause" => status.paused = data.and_then(|d| d.as_bool()).unwrap_or(status.paused),
                                                    "volume" => status.volume = data.and_then(|d| d.as_f64()).unwrap_or(status.volume),
                                                    "speed" => status.speed = data.and_then(|d| d.as_f64()).unwrap_or(status.speed),
                                                    "chapter" => status.chapter = data.and_then(|d| d.as_i64()).unwrap_or(status.chapter),
                                                    "media-title" => status.title = data.and_then(|d| d.as_str()).unwrap_or("").to_string(),
                                                    "fullscreen" => status.fullscreen = data.and_then(|d| d.as_bool()).unwrap_or(status.fullscreen),
                                                    _ => {}
                                                }
                                                let _ = tx_status.send(status.clone()).await;
                                            }
                                        } else if event == "end-file" {
                                            status.is_alive = false;
                                            let _ = tx_status.send(status.clone()).await;
                                        }
                                    }
                                }
                            }
                            _ => {
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
