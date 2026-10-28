//! Structured yt-dlp format metadata, with no signed URLs or credentials retained.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MediaKind {
    Muxed,
    VideoOnly,
    AudioOnly,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MediaFormat {
    pub id: String,
    pub kind: MediaKind,
    pub container: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f32>,
    pub bitrate: Option<u64>,
    pub audio_bitrate: Option<u64>,
    pub audio_language: Option<String>,
    pub filesize: Option<u64>,
    pub hdr: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AvailableFormats {
    pub formats: Vec<MediaFormat>,
    pub live: bool,
}
#[derive(Deserialize)]
struct Probe {
    formats: Option<Vec<RawFormat>>,
    is_live: Option<bool>,
}
#[derive(Deserialize)]
struct RawFormat {
    format_id: Option<String>,
    ext: Option<String>,
    vcodec: Option<String>,
    acodec: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    fps: Option<f32>,
    tbr: Option<f64>,
    abr: Option<f64>,
    language: Option<String>,
    filesize: Option<u64>,
    filesize_approx: Option<u64>,
    dynamic_range: Option<String>,
    has_drm: Option<bool>,
}
fn codec(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty() && v != "none")
}
fn bitrate(value: Option<f64>) -> Option<u64> {
    value
        .filter(|v| v.is_finite() && *v >= 0.)
        .map(|v| (v * 1000.) as u64)
}
fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}
impl AvailableFormats {
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        let probe: Probe = serde_json::from_slice(bytes)
            .map_err(|_| anyhow::anyhow!("yt-dlp format JSON has an invalid shape"))?;
        let mut formats = Vec::new();
        for raw in probe
            .formats
            .context("yt-dlp response has no formats array")?
        {
            if raw.has_drm == Some(true) {
                continue;
            }
            let Some(id) = raw.format_id.filter(|id| safe_id(id)) else {
                continue;
            };
            let video_codec = codec(raw.vcodec);
            let audio_codec = codec(raw.acodec);
            let kind = match (video_codec.is_some(), audio_codec.is_some()) {
                (true, true) => MediaKind::Muxed,
                (true, false) => MediaKind::VideoOnly,
                (false, true) => MediaKind::AudioOnly,
                _ => continue, // Storyboards and unknown/non-media entries.
            };
            formats.push(MediaFormat {
                id,
                kind,
                container: raw.ext,
                video_codec,
                audio_codec,
                width: raw.width,
                height: raw.height,
                fps: raw.fps,
                bitrate: bitrate(raw.tbr),
                audio_bitrate: bitrate(raw.abr),
                audio_language: raw.language,
                filesize: raw.filesize.or(raw.filesize_approx),
                hdr: raw.dynamic_range.is_some_and(|v| {
                    !v.eq_ignore_ascii_case("SDR") && !v.eq_ignore_ascii_case("unknown")
                }),
            });
        }
        if formats.is_empty() {
            bail!("No playable formats returned (unavailable, DRM or unsupported response)");
        }
        Ok(Self {
            formats,
            live: probe.is_live.unwrap_or(false),
        })
    }
    pub fn exact(&self, id: &str) -> Result<&MediaFormat> {
        self.formats
            .iter()
            .find(|f| f.id == id)
            .context("Selected format is no longer available; reopen the media menu")
    }
    pub fn best_audio(&self, preferred: Option<&str>) -> Option<&MediaFormat> {
        self.formats
            .iter()
            .filter(|f| f.kind == MediaKind::AudioOnly)
            .max_by_key(|f| {
                (
                    preferred.is_some_and(|p| {
                        f.audio_codec
                            .as_deref()
                            .is_some_and(|c| codec_matches(c, p))
                    }),
                    f.audio_bitrate.or(f.bitrate).unwrap_or(0),
                )
            })
    }
    /// A requested height is a ceiling; codec preference never beats resolution.
    pub fn video(
        &self,
        requested: Option<u32>,
        maximum: Option<u32>,
        preferred: Option<&str>,
    ) -> Result<&MediaFormat> {
        let ceiling = match (requested, maximum) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        self.formats
            .iter()
            .filter(|f| f.kind != MediaKind::AudioOnly)
            .filter(|f| ceiling.is_none_or(|max| f.height.is_some_and(|h| h <= max)))
            .max_by_key(|f| {
                (
                    f.height.unwrap_or(0),
                    preferred.is_some_and(|p| {
                        f.video_codec
                            .as_deref()
                            .is_some_and(|c| codec_matches(c, p))
                    }),
                    (f.fps.unwrap_or(0.) * 100.) as u32,
                    f.bitrate.unwrap_or(0),
                )
            })
            .context("No available video format satisfies the quality ceiling")
    }
    /// Exact selection overrides defaults only for this playback; no transcoding.
    pub fn selector(&self, id: &str) -> Result<String> {
        let selected = self.exact(id)?;
        if selected.kind == MediaKind::VideoOnly {
            let audio = self
                .best_audio(None)
                .context("No audio stream is available for this video-only format")?;
            Ok(format!("{}+{}", selected.id, audio.id))
        } else {
            Ok(selected.id.clone())
        }
    }
}
fn codec_matches(codec: &str, preference: &str) -> bool {
    match preference.to_ascii_lowercase().as_str() {
        "av1" => codec.starts_with("av01") || codec == "av1",
        "vp9" => codec.starts_with("vp9") || codec.starts_with("vp09"),
        "h264" | "h.264" => codec.starts_with("avc") || codec == "h264",
        "aac" => codec.starts_with("mp4a") || codec == "aac",
        "opus" => codec == "opus",
        _ => false,
    }
}

/// Single current-video cache: bounded memory, TTL, serialized probes, no timer.
#[derive(Clone, Default)]
pub struct FormatResolver {
    current: Arc<Mutex<Option<CachedFormats>>>,
}
struct CachedFormats {
    video: String,
    loaded: Instant,
    value: AvailableFormats,
}
impl FormatResolver {
    pub async fn resolve(
        &self,
        video_id: &str,
        cancelled: CancellationToken,
    ) -> Result<AvailableFormats> {
        if cancelled.is_cancelled() {
            bail!("Format resolution cancelled");
        }
        if !safe_id(video_id) {
            bail!("Invalid video ID");
        }
        let mut current = tokio::select! {
            _ = cancelled.cancelled() => bail!("Format resolution cancelled"),
            guard = self.current.lock() => guard,
        };
        if let Some(cached) = current
            .as_ref()
            .filter(|c| c.video == video_id && c.loaded.elapsed() < Duration::from_secs(300))
        {
            return Ok(cached.value.clone());
        }
        *current = None;
        let mut command = tokio::process::Command::new("yt-dlp");
        command
            .kill_on_drop(true)
            .args([
                "--ignore-config",
                "--dump-single-json",
                "--skip-download",
                "--no-playlist",
                "--no-warnings",
                "--socket-timeout",
                "10",
                "--retries",
                "1",
                "--",
            ])
            .arg(format!("https://www.youtube.com/watch?v={video_id}"))
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let output = tokio::select! {
            _ = cancelled.cancelled() => bail!("Format resolution cancelled"),
            output = tokio::time::timeout(Duration::from_secs(45), command.output()) => output.context("Format resolution timed out after 45 seconds")?.context("Cannot launch yt-dlp; check PATH and permissions")?,
        };
        if !output.status.success() {
            bail!(
                "yt-dlp format resolution failed ({}); video may require login or be unavailable",
                output.status
            );
        }
        let value = AvailableFormats::from_json(&output.stdout)?;
        *current = Some(CachedFormats {
            video: video_id.into(),
            loaded: Instant::now(),
            value: value.clone(),
        });
        Ok(value)
    }
}
