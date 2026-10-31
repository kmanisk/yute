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
    let filename = format!("yute-mpv-{pid}-{counter}.sock");
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join(filename)
    } else {
        std::env::temp_dir().join(filename)
    }
}

#[cfg(test)]
fn log_ipc_event(_msg: &str) {}

#[cfg(not(test))]
fn log_ipc_event(msg: &str) {
    if let Some(home) = std::env::var_os("HOME") {
        let _ = home;
        let dir = crate::config::cache_dir();
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
    pub seeking: bool,
    pub percent_pos: f64,
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
    pub cache_speed: Option<f64>,
    pub cache_time: Option<f64>,
    pub cache_buffering_state: Option<f64>,
    pub video_codec: Option<String>,
    pub video_height: Option<u32>,
    pub stream: super::network::StreamSummary,
    pub recovery: super::network::Phase,
    pub proxy_active: bool,
    pub playback_error: Option<String>,
}

#[allow(dead_code)]
pub enum IpcCommand {
    LoadFile(String),
    LoadFileWithStart(String, f64),
    ChangeQuality(String, String, f64, bool),
    SetProperty(&'static str, Value),
    TogglePause,
    ToggleMute,
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
    task: tokio::task::JoinHandle<()>,
}

impl MpvIpc {
    pub fn start(
        sock_path: PathBuf,
        tx_status: Sender<PlaybackStatus>,
        running_flag: Arc<AtomicBool>,
    ) -> Result<Self> {
        Self::start_with_timeout(sock_path, tx_status, running_flag, Duration::from_secs(10))
    }

    pub(super) fn start_with_timeout(
        sock_path: PathBuf,
        tx_status: Sender<PlaybackStatus>,
        running_flag: Arc<AtomicBool>,
        timeout: Duration,
    ) -> Result<Self> {
        Self::start_configured_timeout(
            sock_path,
            tx_status,
            running_flag,
            timeout,
            super::network::Engine::Auto,
        )
    }
    pub fn start_with_engine(
        sock_path: PathBuf,
        tx_status: Sender<PlaybackStatus>,
        running_flag: Arc<AtomicBool>,
        engine: super::network::Engine,
    ) -> Result<Self> {
        Self::start_configured_timeout(
            sock_path,
            tx_status,
            running_flag,
            Duration::from_secs(10),
            engine,
        )
    }
    fn start_configured_timeout(
        sock_path: PathBuf,
        tx_status: Sender<PlaybackStatus>,
        running_flag: Arc<AtomicBool>,
        timeout: Duration,
        engine: super::network::Engine,
    ) -> Result<Self> {
        let (tx_cmd, mut rx_cmd) = channel::<IpcCommand>(64);
        let path_clone = sock_path.clone();

        let task = tokio::spawn(async move {
            let mut stream = None;
            let deadline = tokio::time::Instant::now() + timeout;
            while tokio::time::Instant::now() < deadline {
                if let Ok(Ok(s)) =
                    tokio::time::timeout_at(deadline, UnixStream::connect(&path_clone)).await
                {
                    stream = Some(s);
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            let Some(stream) = stream else {
                log_ipc_event("Failed to connect to mpv socket before startup deadline");
                let _ = tx_status
                    .send(PlaybackStatus {
                        playback_error: Some("MPV IPC startup timed out".into()),
                        ..Default::default()
                    })
                    .await;
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
                (13, "seeking"),
                (14, "percent-pos"),
                (15, "cache-speed"),
                (16, "demuxer-cache-time"),
                (17, "cache-buffering-state"),
                (18, "video-codec"),
                (19, "video-params"),
                (20, "stream-path"),
            ];

            for (id, prop) in observe_props {
                let req = json!({
                    "command": ["observe_property", id, prop]
                });
                let _ = writer.write_all(format!("{req}\n").as_bytes()).await;
            }
            let _ = writer.flush().await;

            let mut pause_after_load = false;
            let mut status = PlaybackStatus {
                volume: 100.0,
                speed: 1.0,
                is_alive: true,
                ..Default::default()
            };
            let _ = tx_status.send(status.clone()).await;

            let mut watchdog_ticker = tokio::time::interval(Duration::from_secs(1));
            let mut current_media: Option<(String, Option<String>)> = None;
            let mut active_proxy: Option<super::proxy::Session> = None;
            let mut selector = String::from("bestvideo+bestaudio/best");
            let mut recovery = super::network::Recovery::default();
            let mut requested_seek_target: Option<f64> = None;
            let recovery_clock = std::time::Instant::now();

            loop {
                tokio::select! {
                    _ = watchdog_ticker.tick() => {
                        let network=current_media.as_ref().is_some_and(|(url,_)|url.starts_with("http://")||url.starts_with("https://"));
                        if let Some(target)=requested_seek_target {
                            if status.file_loaded && !status.seeking && !status.paused_for_cache && !status.core_idle && (status.time_pos-target).abs() <= 5.0 {
                                requested_seek_target=None;
                            }
                        }
                        if recovery.observe(recovery_clock.elapsed(),&status,network,if active_proxy.is_some(){super::network::Engine::Proxy}else{engine}) {
                          if let Some((url,format))=&current_media {
                            if let Ok(id)=super::diagnostics::video_id(url) {
                              match super::proxy::Session::youtube(id,format.clone().unwrap_or_else(||selector.clone())) {
                                Ok(session)=>{
                                  let target=requested_seek_target.unwrap_or(status.time_pos);
                                  let command=proxy_load(&session,target,status.paused);
                                  active_proxy=Some(session);
                                  recovery.proxy_started();
                                  status.proxy_active=true;
                                  status.file_loaded=false;
                                  status.playback_error=None;
                                  let _=writer.write_all(format!("{command}\n").as_bytes()).await;
                                  let _=writer.flush().await;
                                  log_ipc_event(&format!("Recovery: switched once to fresh finite-range transport at {:.2}s",target));
                                },
                                Err(_)=>{status.playback_error=Some("Cannot start playback range transport".into());}
                              }
                            }
                          }
                        }
                        if status.recovery!=recovery.phase {status.recovery=recovery.phase;status.stall_error=recovery.phase==super::network::Phase::Failed;let _=tx_status.send(status.clone()).await;}
                    }

                    cmd_opt = rx_cmd.recv() => {
                        let Some(cmd) = cmd_opt else { break; };
                        let is_load = matches!(&cmd, IpcCommand::LoadFile(_) | IpcCommand::LoadFileWithStart(_, _) | IpcCommand::ChangeQuality(_,_,_,_));
                        let mut cmd_json = match cmd {
                            IpcCommand::LoadFile(url) => {
                                pause_after_load = false;
                                status.playback_error = None;
                                status.duration = 0.0;
                                status.file_loaded = false;
                                status.time_pos = 0.0;
                                status.cache_duration = 0.0;
                                status.stall_error = false;
                                recovery=super::network::Recovery::default();
                                status.paused=pause_after_load;
                                status.cache_speed=None;status.cache_time=None;status.cache_buffering_state=None;status.video_codec=None;status.video_height=None;status.stream=Default::default();status.recovery=super::network::Phase::Direct;
                                current_media = Some((url.clone(), None));
                                requested_seek_target=None;
                                log_ipc_event("Command: loadfile replace (source redacted)");
                                json!({ "command": ["loadfile", url, "replace", -1, {"start": "0"}], "request_id": 1 })
                            }
                            IpcCommand::LoadFileWithStart(url, start) => {
                                pause_after_load = false;
                                status.playback_error = None;
                                status.duration = 0.0;
                                status.file_loaded = false;
                                status.time_pos = start;
                                status.cache_duration = 0.0;
                                status.stall_error = false;
                                recovery=super::network::Recovery::default();
                                status.paused=pause_after_load;
                                status.cache_speed=None;status.cache_time=None;status.cache_buffering_state=None;status.video_codec=None;status.video_height=None;status.stream=Default::default();status.recovery=super::network::Phase::Direct;
                                current_media = Some((url.clone(), None));
                                requested_seek_target=Some(start.max(0.0));
                                log_ipc_event("Command: loadfile replace at requested position (source redacted)");
                                json!({ "command": ["loadfile", url, "replace", -1, {"start": start.to_string()}], "request_id": 1 })
                            }
                            IpcCommand::SetProperty(name, value) => {if name=="ytdl-format" {if let Some(f)=value.as_str(){selector=f.to_string();}} json!({"command":["set_property",name,value]})},
                            IpcCommand::ChangeQuality(format, url, start, paused) => {
                                pause_after_load = paused;
                                status.file_loaded = false;
                                status.playback_error = None;
                                status.stall_error = false;
                                status.time_pos = start;
                                recovery=super::network::Recovery::default();
                                status.paused=pause_after_load;
                                status.cache_speed=None;status.cache_time=None;status.cache_buffering_state=None;status.video_codec=None;status.video_height=None;status.stream=Default::default();status.recovery=super::network::Phase::Direct;
                                current_media = Some((url.clone(), Some(format.clone())));
                                requested_seek_target=Some(start.max(0.0));
                                json!({"command":["loadfile",url,"replace",-1,{"start":start.to_string(),"ytdl-format":format,"pause":if paused {"yes"} else {"no"}}],"request_id":1})
                            }
                            IpcCommand::ToggleMute => json!({ "command": ["cycle", "mute"] }),
                            IpcCommand::TogglePause => json!({ "command": ["cycle", "pause"] }),
                            IpcCommand::Play => json!({ "command": ["set_property", "pause", false] }),
                            IpcCommand::Pause => json!({ "command": ["set_property", "pause", true] }),
                            IpcCommand::Seek(secs, true) => {
                                requested_seek_target=Some((status.time_pos+secs).max(0.0));
                                json!({ "command": ["seek", secs, "relative"] })
                            },
                            IpcCommand::Seek(secs, false) => {
                                requested_seek_target=Some(secs.max(0.0));
                                json!({ "command": ["seek", secs, "absolute+keyframes"] })
                            },
                            IpcCommand::SetVolume(v) => json!({ "command": ["set_property", "volume", v] }),
                            IpcCommand::AdjustVolume(dv) => json!({ "command": ["add", "volume", dv] }),
                            IpcCommand::SetSpeed(s) => json!({ "command": ["set_property", "speed", s] }),
                            IpcCommand::AdjustSpeed(ds) => json!({ "command": ["add", "speed", ds] }),
                            IpcCommand::CycleSubtitle => json!({ "command": ["cycle", "sub-visibility"] }),
                            IpcCommand::ToggleFullscreen => json!({ "command": ["cycle", "fullscreen"] }),
                            IpcCommand::ToggleStats => json!({ "command": ["script-binding", "stats/display-stats-toggle"] }),
                            IpcCommand::Stop => json!({ "command": ["stop"] }),
                        };
                        if is_load {
                          active_proxy=None;status.proxy_active=false;
                          if engine==super::network::Engine::Proxy {
                            if let Some((url,format))=&current_media {
                              if let Ok(id)=super::diagnostics::video_id(url) {
                                match super::proxy::Session::youtube(id,format.clone().unwrap_or_else(||selector.clone())) {
                                  Ok(session)=>{cmd_json=proxy_load(&session,status.time_pos,status.paused);active_proxy=Some(session);status.proxy_active=true;},
                                  Err(_)=>{status.playback_error=Some("Cannot start playback range transport".into());let _=tx_status.send(status.clone()).await;continue;}
                                }
                              }
                            }
                          }
                        }
                        if matches!(cmd_json["command"][0].as_str(),Some("stop")){active_proxy=None;status.proxy_active=false;}
                        if writer.write_all(format!("{cmd_json}\n").as_bytes()).await.is_err()
                            || writer.flush().await.is_err() {
                            status.playback_error = Some("MPV IPC command write failed".into());
                            break;
                        }
                        if is_load { let _ = tx_status.send(status.clone()).await; }
                    }

                    line_res = lines.next_line() => {
                        match line_res {
                            Ok(Some(line)) => {
                                if let Ok(val) = serde_json::from_str::<Value>(&line) {
                                    if val.get("request_id").and_then(Value::as_u64) == Some(1) {
                                        if let Some(error) = val.get("error").and_then(Value::as_str).filter(|e| *e != "success") {
                                            status.playback_error = Some(format!("MPV loadfile failed: {error}"));
                                            let _ = tx_status.send(status.clone()).await;
                                        }
                                    }
                                    if let Some(event) = val.get("event").and_then(|e| e.as_str()) {
                                        match event {
                                            "file-loaded" => {
                                                status.playback_error = None;
                                                log_ipc_event("Event: file-loaded");
                                                status.file_loaded = true;
                                                status.stall_error = false;
                                                                                let unpause = json!({ "command": ["set_property", "pause", pause_after_load] });
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
                                                        "seeking" => status.seeking = data.and_then(|d| d.as_bool()).unwrap_or(false),
                                                        "percent-pos" => status.percent_pos = data.and_then(|d| d.as_f64()).unwrap_or(status.percent_pos),
                                                        "core-idle" => status.core_idle = data.and_then(|d| d.as_bool()).unwrap_or(false),
                                                        "idle-active" => status.idle_active = data.and_then(|d| d.as_bool()).unwrap_or(false),
                                                        "volume" => status.volume = data.and_then(|d| d.as_f64()).unwrap_or(status.volume),
                                                        "speed" => status.speed = data.and_then(|d| d.as_f64()).unwrap_or(status.speed),
                                                        "chapter" => status.chapter = data.and_then(|d| d.as_i64()).unwrap_or(status.chapter),
                                                        "media-title" => status.title = data.and_then(|d| d.as_str()).unwrap_or("").to_string(),
                                                        "fullscreen" => status.fullscreen = data.and_then(|d| d.as_bool()).unwrap_or(status.fullscreen),
                                                        "cache-speed"=>status.cache_speed=data.and_then(Value::as_f64),
                                                        "demuxer-cache-time"=>status.cache_time=data.and_then(Value::as_f64),
                                                        "cache-buffering-state"=>status.cache_buffering_state=data.and_then(Value::as_f64),
                                                        "video-codec"=>status.video_codec=data.and_then(Value::as_str).map(|s|s.chars().filter(|c|!c.is_control()).take(40).collect()),
                                                        "video-params"=>status.video_height=data.and_then(|d|d.get("h")).and_then(Value::as_u64).and_then(|n|u32::try_from(n).ok()),
                                                        "stream-path"=>status.stream=data.and_then(Value::as_str).map(super::network::StreamSummary::from_url).unwrap_or_default(),
                                                        "demuxer-cache-duration" => status.cache_duration = data.and_then(|d| d.as_f64()).unwrap_or(0.0),
                                                        _ => {}
                                                    }
                                                    let _ = tx_status.send(status.clone()).await;
                                                }
                                            }
                                            "end-file" => {
                                                if val.get("reason").and_then(Value::as_str) == Some("error") {
                                                    let error = val.get("file_error").and_then(Value::as_str).unwrap_or("stream resolution or media loading failed");
                                                    status.playback_error = Some(format!("MPV could not load media: {error}"));
                                                }
                                                log_ipc_event("Event: end-file");
                                                status.file_loaded = false;
                                                if status.playback_error.is_none(){status.time_pos=0.;status.paused=true;}
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

        Ok(Self { tx_cmd, task })
    }

    pub fn send(&self, cmd: IpcCommand) -> Result<()> {
        self.tx_cmd
            .try_send(cmd)
            .map_err(|e| anyhow::anyhow!("MPV IPC command queue unavailable: {e}"))
    }
}

impl Drop for MpvIpc {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn proxy_load(session: &super::proxy::Session, start: f64, paused: bool) -> serde_json::Value {
    json!({"command":["loadfile",session.manifest(),"replace",-1,{"start":start.to_string(),"ytdl":"no","demuxer":"edl","pause":if paused{"yes"}else{"no"}}],"request_id":1})
}
