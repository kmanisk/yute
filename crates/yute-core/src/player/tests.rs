use super::*;
use serde_json::{json, Value};
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::sync::atomic::AtomicU64;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, UnixListener};

static TEST_ID: AtomicU64 = AtomicU64::new(0);

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "yute-playback-{}-{}",
            std::process::id(),
            TEST_ID.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        Self(path)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn status_matching(
    rx: &mut Receiver<PlaybackStatus>,
    predicate: impl Fn(&PlaybackStatus) -> bool,
) -> PlaybackStatus {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let status = rx
                .recv()
                .await
                .expect("IPC status channel closed unexpectedly");
            if predicate(&status) {
                return status;
            }
        }
    })
    .await
    .expect("MPV status deadline exceeded")
}

#[tokio::test]
async fn stale_socket_reports_startup_failure() {
    let dir = TestDir::new();
    let path = dir.0.join("stale.sock");
    drop(std::os::unix::net::UnixListener::bind(&path).unwrap());
    assert!(path.exists());
    let (tx, mut rx) = channel(32);
    let running = Arc::new(AtomicBool::new(true));
    let _ipc =
        MpvIpc::start_with_timeout(path, tx, running.clone(), Duration::from_millis(150)).unwrap();
    let status = status_matching(&mut rx, |s| s.playback_error.is_some()).await;
    assert!(!status.is_alive);
    assert!(!running.load(Ordering::SeqCst));
    assert!(status.playback_error.unwrap().contains("startup timed out"));
}

#[tokio::test]
async fn delayed_ipc_startup_preserves_queued_load_and_observes_before_loading() {
    let dir = TestDir::new();
    let path = dir.0.join("delayed.sock");
    let server_path = path.clone();
    let server = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        let listener = UnixListener::bind(server_path).unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();
        for _ in 0..14 {
            let request: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            assert_eq!(request["command"][0], "observe_property");
        }
        let request: Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(
            request["command"],
            json!(["loadfile", "selected-video", "replace", -1, {"start":"0"}])
        );
        writer
            .write_all(b"{\"event\":\"file-loaded\"}\n")
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
    });
    let (tx, mut rx) = channel(32);
    let ipc = MpvIpc::start(path, tx, Arc::new(AtomicBool::new(true))).unwrap();
    ipc.send(IpcCommand::LoadFile("selected-video".into()))
        .unwrap();
    status_matching(&mut rx, |s| s.file_loaded).await;
    server.await.unwrap();
}

#[tokio::test]
async fn dropping_pending_ipc_cancels_obsolete_startup() {
    let dir = TestDir::new();
    let (tx, mut rx) = channel(32);
    let running = Arc::new(AtomicBool::new(true));
    let ipc = MpvIpc::start_with_timeout(
        dir.0.join("never.sock"),
        tx,
        running.clone(),
        Duration::from_millis(100),
    )
    .unwrap();
    tokio::task::yield_now().await;
    drop(ipc);
    tokio::time::sleep(Duration::from_millis(250)).await;
    assert!(
        rx.try_recv().is_err(),
        "obsolete startup must not publish a status"
    );
    assert!(
        running.load(Ordering::SeqCst),
        "obsolete task must not overwrite a later session's flag"
    );
}

#[tokio::test]
async fn resolver_error_is_reported_without_disconnecting_ipc() {
    let dir = TestDir::new();
    let path = dir.0.join("resolver.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();
        while let Some(line) = lines.next_line().await.unwrap() {
            let request: Value = serde_json::from_str(&line).unwrap();
            if request["command"][0] == "loadfile" {
                writer.write_all(b"{\"event\":\"end-file\",\"reason\":\"error\",\"file_error\":\"resolver failed\"}\n").await.unwrap();
                tokio::time::sleep(Duration::from_millis(100)).await;
                return;
            }
        }
    });
    let (tx, mut rx) = channel(32);
    let ipc = MpvIpc::start(path, tx, Arc::new(AtomicBool::new(true))).unwrap();
    ipc.send(IpcCommand::LoadFile("failed-video".into()))
        .unwrap();
    let status = status_matching(&mut rx, |s| s.playback_error.is_some()).await;
    assert!(status.is_alive);
    assert!(!status.file_loaded);
    assert!(status.playback_error.unwrap().contains("resolver failed"));
    server.await.unwrap();
    status_matching(&mut rx, |s| !s.is_alive).await;
    assert!(ipc.send(IpcCommand::LoadFile("next-video".into())).is_err());
}

struct NativePlayer {
    player: Player,
    dir: TestDir,
}

impl NativePlayer {
    fn new(extractor: Option<&str>) -> Self {
        let dir = TestDir::new();
        let path = dir.0.join("mpv.sock");
        let mut command = Command::new("mpv");
        command
            .args([
                "--no-config",
                "--load-scripts=no",
                "--vo=null",
                "--ao=null",
                "--idle=yes",
                "--keep-open=yes",
                "--no-terminal",
                "--start=7",
            ])
            .arg(format!("--input-ipc-server={}", path.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(script) = extractor {
            let extractor_path = dir.0.join("extractor");
            std::fs::write(&extractor_path, script).unwrap();
            std::fs::set_permissions(&extractor_path, std::fs::Permissions::from_mode(0o700))
                .unwrap();
            command.arg(format!(
                "--script-opts=ytdl_hook-ytdl_path={}",
                extractor_path.display()
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let child = command.spawn().expect("native test requires mpv");
        let (tx_status, rx_status) = channel(32);
        let mut player = Player {
            profile: MpvProfile { dir: dir.0.clone() },
            queue: PlayQueue::new(),
            current_video: None,
            status: PlaybackStatus::default(),
            is_playing: Arc::new(AtomicBool::new(false)),
            child_process: None,
            current_sock_path: None,
            ipc: None,
            tx_status,
            rx_status,
        };
        // Use exactly the production child-to-IPC handoff, with null outputs
        // and a generated local WAV rather than user config or credentials.
        let media = wav(&dir.0, "first");
        player
            .attach_window_process(child, path, media.to_str().unwrap())
            .unwrap();
        Self { player, dir }
    }

    async fn wait(&mut self, predicate: impl Fn(&PlaybackStatus) -> bool) -> PlaybackStatus {
        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                self.player.poll_updates();
                if predicate(&self.player.status) {
                    return self.player.status.clone();
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "native MPV status deadline exceeded: {:?}",
                self.player.status
            )
        })
    }
}

impl Drop for NativePlayer {
    fn drop(&mut self) {
        self.player.stop();
    }
}

fn wav(dir: &std::path::Path, name: &str) -> PathBuf {
    let path = dir.join(format!("{name}.wav"));
    let bytes = 8000u32 * 2 * 15;
    let mut data = Vec::new();
    data.extend(b"RIFF");
    data.extend((36 + bytes).to_le_bytes());
    data.extend(b"WAVEfmt ");
    data.extend(16u32.to_le_bytes());
    data.extend(1u16.to_le_bytes());
    data.extend(1u16.to_le_bytes());
    data.extend(8000u32.to_le_bytes());
    data.extend(16000u32.to_le_bytes());
    data.extend(2u16.to_le_bytes());
    data.extend(16u16.to_le_bytes());
    data.extend(b"data");
    data.extend(bytes.to_le_bytes());
    data.resize(44 + bytes as usize, 0);
    std::fs::write(&path, data).unwrap();
    path
}

#[tokio::test]
#[ignore = "requires installed mpv; uses local generated media and null outputs"]
async fn native_consecutive_videos_reset_resume_and_clean_socket() {
    let mut fixture = NativePlayer::new(None);
    let pid = fixture.player.child_process.as_ref().unwrap().id();
    // A second selection before IPC has connected must queue on this process,
    // rather than spawn another MPV and leave the old IPC startup task alive.
    let queued = wav(&fixture.dir.0, "queued");
    let video: VideoCard = serde_json::from_value(json!({"id":queued,"title":"queued","channel":"test","verified":false,"thumb_url":"","live":"None","badges":[]})).unwrap();
    fixture
        .player
        .play_video(video, &Config::default())
        .unwrap();
    assert_eq!(fixture.player.child_process.as_ref().unwrap().id(), pid);
    let loaded = fixture
        .wait(|s| s.file_loaded && s.title.contains("queued"))
        .await;
    assert!(
        loaded.time_pos < 2.0,
        "load options must override a global start setting"
    );
    let first = wav(&fixture.dir.0, "resumed");
    fixture
        .player
        .ipc
        .as_ref()
        .unwrap()
        .send(IpcCommand::LoadFileWithStart(
            first.to_string_lossy().into(),
            7.0,
        ))
        .unwrap();
    fixture
        .wait(|s| s.file_loaded && s.title.contains("resumed") && s.time_pos >= 6.5)
        .await;
    let second = wav(&fixture.dir.0, "second");
    let video: VideoCard = serde_json::from_value(json!({"id":second,"title":"second","channel":"test","verified":false,"thumb_url":"","live":"None","badges":[]})).unwrap();
    fixture
        .player
        .play_video(video, &Config::default())
        .unwrap();
    let status = fixture
        .wait(|s| s.file_loaded && s.title.contains("second"))
        .await;
    assert!(
        status.time_pos < 2.0,
        "previous file's resume must not carry over"
    );
    assert_eq!(
        fixture.player.child_process.as_ref().unwrap().id(),
        pid,
        "reuse the existing MPV process"
    );
    let path = fixture.player.current_sock_path.clone().unwrap();
    fixture.player.stop();
    assert!(!path.exists());
    assert!(!fixture.player.status.is_alive);
    assert!(
        !fixture.player.poll_updates(),
        "old session must not publish stale status"
    );
}

#[tokio::test]
#[ignore = "requires installed mpv; fake yt-dlp and loopback HTTP only"]
async fn native_resolver_failure_and_ytdlp_fallback_success() {
    let media_dir = TestDir::new();
    let media = wav(&media_dir.0, "fallback");
    let media_bytes = Arc::new(std::fs::read(&media).unwrap());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/video", listener.local_addr().unwrap());
    let stream_url = format!("http://{}/media.wav", listener.local_addr().unwrap());
    let http = tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            let media_bytes = Arc::clone(&media_bytes);
            tokio::spawn(async move {
                use tokio::io::AsyncReadExt;
                let mut buf = [0; 4096];
                let size = socket.read(&mut buf).await.unwrap_or(0);
                if buf[..size].starts_with(b"GET /media.wav ") {
                    let header = format!("HTTP/1.1 200 OK\r\nContent-Type: audio/wav\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", media_bytes.len());
                    let _ = socket.write_all(header.as_bytes()).await;
                    let _ = socket.write_all(&media_bytes).await;
                } else {
                    let _ = socket.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                }
            });
        }
    });
    let mut failure = NativePlayer::new(Some("#!/bin/sh\necho 'resolver failed' >&2\nexit 1\n"));
    failure.wait(|s| s.file_loaded).await;
    failure
        .player
        .ipc
        .as_ref()
        .unwrap()
        .send(IpcCommand::LoadFile(url.clone()))
        .unwrap();
    let error = failure.wait(|s| s.playback_error.is_some()).await;
    assert!(!error.file_loaded);
    let recovered = wav(&failure.dir.0, "recovered");
    failure
        .player
        .ipc
        .as_ref()
        .unwrap()
        .send(IpcCommand::LoadFile(recovered.to_string_lossy().into()))
        .unwrap();
    failure
        .wait(|s| s.file_loaded && s.title.contains("recovered"))
        .await;
    assert!(failure.player.status.playback_error.is_none());
    // A successful extractor can recover a URL that the native demuxer rejects.
    let payload = json!({"url":stream_url,"title":"fallback","ext":"wav"}).to_string();
    let script = format!("#!/bin/sh\nprintf '%s\\n' '{payload}'\n");
    let mut success = NativePlayer::new(Some(&script));
    success.wait(|s| s.file_loaded).await;
    success
        .player
        .ipc
        .as_ref()
        .unwrap()
        .send(IpcCommand::LoadFile(url))
        .unwrap();
    success
        .wait(|s| s.file_loaded && s.title == "fallback")
        .await;
    assert!(success.player.status.playback_error.is_none());
    http.abort();
}

#[test]
fn stalled_network_seek_recovery_reloads_once_at_target_with_format() {
    let command = super::ipc::seek_stall_reload_command(
        "https://www.youtube.com/watch?v=fixture",
        Some("bestvideo+bestaudio"),
        428.42,
    )
    .unwrap();
    assert_eq!(command["command"][0], "loadfile");
    assert_eq!(
        command["command"][1],
        "https://www.youtube.com/watch?v=fixture"
    );
    assert_eq!(command["command"][2], "replace");
    assert_eq!(command["command"][4]["start"], "428.42");
    assert_eq!(command["command"][4]["ytdl-format"], "bestvideo+bestaudio");
    assert!(super::ipc::seek_stall_reload_command("/tmp/local.mp4", None, 20.0).is_none());
}

#[tokio::test]
async fn absolute_seek_commands_use_seconds_in_json_ipc() {
    let dir = TestDir::new();
    let path = dir.0.join("seek.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        loop {
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            let value: Value = serde_json::from_str(&line).unwrap();
            if value["command"][0] == "seek" {
                assert_eq!(value["command"][1], 80.0);
                assert_eq!(value["command"][2], "absolute+keyframes");
                break;
            }
        }
    });
    let (tx, _) = channel(32);
    let ipc = MpvIpc::start(path, tx, Arc::new(AtomicBool::new(true))).unwrap();
    ipc.send(IpcCommand::Seek(80.0, false)).unwrap();
    tokio::time::timeout(Duration::from_secs(3), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn quality_reload_keeps_position_and_pause_and_sets_file_quality() {
    let dir = TestDir::new();
    let path = dir.0.join("quality.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();
        for _ in 0..14 {
            lines.next_line().await.unwrap().unwrap();
        }
        let request: Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(request["command"][0], "loadfile");
        assert_eq!(request["command"][4]["start"], "83");
        assert_eq!(request["command"][4]["ytdl-format"], "quality-fixture");
        assert_eq!(request["command"][4]["pause"], "yes");
        writer
            .write_all(b"{\"event\":\"file-loaded\"}\n")
            .await
            .unwrap();
        let pause: Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(pause["command"], json!(["set_property", "pause", true]));
    });
    let (tx, mut rx) = channel(32);
    let ipc = MpvIpc::start(path, tx, Arc::new(AtomicBool::new(true))).unwrap();
    ipc.send(IpcCommand::ChangeQuality(
        "quality-fixture".into(),
        "selected-video".into(),
        83.,
        true,
    ))
    .unwrap();
    status_matching(&mut rx, |s| s.file_loaded).await;
    server.await.unwrap();
}

#[tokio::test]
#[ignore = "requires installed mpv; local generated media and null outputs only"]
async fn native_quality_reload_and_cache_properties() {
    let mut fixture = NativePlayer::new(None);
    fixture.wait(|s| s.file_loaded).await;
    let media = wav(&fixture.dir.0, "quality-reloaded");
    fixture.player.send_command(IpcCommand::ChangeQuality(
        "bestvideo[height<=?720]+bestaudio/best".into(),
        media.to_string_lossy().into(),
        7.,
        true,
    ));
    fixture
        .wait(|s| {
            s.file_loaded && s.title.contains("quality-reloaded") && s.paused && s.time_pos >= 6.5
        })
        .await;
    fixture
        .player
        .send_command(IpcCommand::SetProperty("cache-secs", json!(40)));
    fixture.player.send_command(IpcCommand::SetProperty(
        "demuxer-max-bytes",
        json!(192 * 1024 * 1024),
    ));
    tokio::time::sleep(Duration::from_millis(100)).await;
    let stream =
        tokio::net::UnixStream::connect(fixture.player.current_sock_path.as_ref().unwrap())
            .await
            .unwrap();
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    for (property, expected) in [
        ("pause", json!(true)),
        (
            "ytdl-format",
            json!("bestvideo[height<=?720]+bestaudio/best"),
        ),
        ("cache-secs", json!(40.0)),
        ("demuxer-max-bytes", json!(192 * 1024 * 1024)),
    ] {
        writer
            .write_all(
                format!(
                    "{}\n",
                    json!({"command":["get_property",property],"request_id":99})
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        loop {
            let response: Value =
                serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
            if response["request_id"] == 99 {
                assert_eq!(response["error"], "success");
                assert_eq!(response["data"], expected, "property {property}");
                break;
            }
        }
    }
}
