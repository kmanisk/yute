//! Muted diagnostics run the real Player/IPC/resolution path, with null outputs.
use super::{
    ipc::IpcCommand,
    network::{Phase, StreamSummary},
    Player,
};
use crate::{
    backend::models::{LiveState, VideoCard},
    config::Config,
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sample {
    pub elapsed_ms: u64,
    pub time_pos: f64,
    pub paused_for_cache: bool,
    pub cache_speed: Option<f64>,
    pub demuxer_cache_time: Option<f64>,
    pub demuxer_cache_duration: f64,
    pub cache_buffering_state: Option<f64>,
    pub seeking: bool,
    pub core_idle: bool,
    pub recovery: Phase,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Seek {
    pub round: u8,
    pub target: String,
    pub success: bool,
    pub resume_ms: Option<u64>,
    pub confirmed_ms: Option<u64>,
    pub samples: Vec<Sample>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub engine: String,
    pub codec: Option<String>,
    pub height: Option<u32>,
    pub cache_seconds: u16,
    pub stream: StreamSummary,
    pub seeks: Vec<Seek>,
    pub error: Option<String>,
}
struct Session(Player);
impl Drop for Session {
    fn drop(&mut self) {
        self.0.stop();
    }
}
pub fn video_id(input: &str) -> anyhow::Result<String> {
    let id = if input.len() == 11 && !input.contains('/') {
        input.to_string()
    } else {
        let u = reqwest::Url::parse(input)
            .map_err(|_| anyhow::anyhow!("Expected a YouTube video ID or URL"))?;
        let host = u.host_str().unwrap_or("");
        if host == "youtu.be" {
            u.path().trim_matches('/').to_string()
        } else if host == "youtube.com" || host.ends_with(".youtube.com") {
            u.query_pairs()
                .find(|(k, _)| k == "v")
                .map(|(_, v)| v.into_owned())
                .or_else(|| {
                    u.path()
                        .strip_prefix("/shorts/")
                        .map(|s| s.trim_matches('/').into())
                })
                .unwrap_or_default()
        } else {
            String::new()
        }
    };
    if id.len() != 11
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
    {
        anyhow::bail!("Expected an 11-character YouTube video ID");
    }
    Ok(id)
}
pub async fn seek_test(
    input: &str,
    config: &Config,
    rounds: u8,
    threshold: Duration,
) -> anyhow::Result<Report> {
    let id = video_id(input)?;
    let mut cfg = config.clone();
    cfg.playback.mark_watched = false;
    cfg.playback.autoplay = false;
    let mut session = Session(Player::diagnostic(&cfg));
    let player = &mut session.0;
    let video = VideoCard {
        id,
        title: "Seek diagnostic".into(),
        channel: String::new(),
        channel_id: None,
        verified: false,
        duration: None,
        views: None,
        published: None,
        thumb_url: String::new(),
        progress_pct: None,
        live: LiveState::None,
        badges: vec![],
        resume_time_sec: None,
    };
    player.play_video(video, &cfg)?;
    let mut report = Report {
        engine: cfg.playback.network_engine.clone(),
        codec: None,
        height: None,
        cache_seconds: cfg.playback.stream_cache_seconds,
        stream: Default::default(),
        seeks: vec![],
        error: None,
    };
    let startup = Instant::now();
    while startup.elapsed() < Duration::from_secs(45) {
        player.poll_updates();
        if player.status.file_loaded && player.status.duration > 0. {
            break;
        }
        if !player.has_session() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    if !player.status.file_loaded || player.status.duration <= 0. {
        report.error=Some("MPV did not load a timed video within 45s; check anonymous extraction, CDN reachability and tool versions".into());
        return Ok(report);
    }
    for round in 0..rounds.clamp(1, 5) {
        for (label, fraction) in [
            ("start", 0.),
            ("60%", 0.6),
            ("20%", 0.2),
            ("90%", 0.9),
            ("10%", 0.1),
        ] {
            let target = player.status.duration * fraction;
            player.send_command(IpcCommand::Seek(target, false));
            let start = Instant::now();
            let mut row = Seek {
                round: round + 1,
                target: label.into(),
                success: false,
                resume_ms: None,
                confirmed_ms: None,
                samples: vec![],
            };
            let mut first = None;
            let mut first_ms = None;
            while start.elapsed() < threshold {
                tokio::time::sleep(Duration::from_millis(250)).await;
                player.poll_updates();
                let s = &player.status;
                row.samples.push(Sample {
                    elapsed_ms: start.elapsed().as_millis() as u64,
                    time_pos: s.time_pos,
                    paused_for_cache: s.paused_for_cache,
                    cache_speed: s.cache_speed,
                    demuxer_cache_time: s.cache_time,
                    demuxer_cache_duration: s.cache_duration,
                    cache_buffering_state: s.cache_buffering_state,
                    seeking: s.seeking,
                    core_idle: s.core_idle,
                    recovery: s.recovery,
                });
                report.stream = s.stream.clone();
                report.codec = s.video_codec.clone();
                report.height = s.video_height;
                if s.file_loaded
                    && !s.seeking
                    && !s.paused_for_cache
                    && !s.core_idle
                    && (s.time_pos - target).abs() < 10.
                {
                    let initial = *first.get_or_insert(s.time_pos);
                    first_ms.get_or_insert(start.elapsed().as_millis() as u64);
                    if s.time_pos - initial >= 2. {
                        row.success = true;
                        row.resume_ms = first_ms;
                        row.confirmed_ms = Some(start.elapsed().as_millis() as u64);
                        break;
                    }
                } else {
                    first = None;
                    first_ms = None;
                }
                if !player.has_session() {
                    break;
                }
            }
            report.seeks.push(row);
            if !player.has_session() {
                break;
            }
        }
        if !player.has_session() {
            break;
        }
    }
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_only_video_identity_and_never_echoes_untrusted_urls() {
        assert_eq!(video_id("g1qNCcxFWRo").unwrap(), "g1qNCcxFWRo");
        assert_eq!(
            video_id("https://youtu.be/g1qNCcxFWRo?token=PRIVATE").unwrap(),
            "g1qNCcxFWRo"
        );
        let e = video_id("https://evil.test/path?secret=PRIVATE")
            .unwrap_err()
            .to_string();
        assert!(!e.contains("PRIVATE"));
    }
}
