use anyhow::{Context, Result};
use libmpv2::Mpv;
use std::sync::{Arc, Mutex};
use ytui_core::backend::models::VideoCard;
use ytui_core::backend::sponsorblock::{SponsorBlockClient, SponsorBlockTracker};
use ytui_core::config::Config;
use ytui_core::player::ipc::PlaybackStatus;

#[allow(dead_code)]
pub struct EmbeddedPlayer {
    pub mpv: Arc<Mpv>,
    pub current_video: Arc<Mutex<Option<VideoCard>>>,
    pub status: Arc<Mutex<PlaybackStatus>>,
    pub sponsor_tracker: Arc<Mutex<Option<SponsorBlockTracker>>>,
    pub sponsor_client: SponsorBlockClient,
    pub config: Config,
}

impl EmbeddedPlayer {
    pub fn new(cfg: &Config) -> Result<Self> {
        let max_res = cfg.playback.resolved_ytdl_format();
        let hwdec = if cfg.playback.hwdec.is_empty() {
            "auto-safe"
        } else {
            &cfg.playback.hwdec
        };

        let cookies_path = if !cfg.backend.cookies_file.is_empty() {
            Some(ytui_core::config::resolve_path(&cfg.backend.cookies_file))
        } else {
            None
        };

        let mpv = Mpv::with_initializer(|init| {
            init.set_option("vo", "libmpv")?;
            init.set_option("hwdec", hwdec)?;
            init.set_option("hr-seek", "no")?;
            init.set_option("cache", "yes")?;
            init.set_option("demuxer-max-bytes", "256MiB")?;
            init.set_option("demuxer-max-back-bytes", "128MiB")?;
            init.set_option("demuxer-readahead-secs", "45")?;
            init.set_option("demuxer-seekable-cache", "yes")?;
            init.set_option("ytdl-format", max_res.as_str())?;
            init.set_option("ytdl-raw-options-append", "no-playlist=")?;
            init.set_option("ytdl-raw-options-append", "concurrent-fragments=4")?;
            if let Some(ref cp) = cookies_path {
                if cp.exists() {
                    let cookie_arg = format!("cookies={}", cp.display());
                    init.set_option("ytdl-raw-options-append", cookie_arg.as_str())?;
                    if cfg.playback.mark_watched {
                        init.set_option("ytdl-raw-options-append", "mark-watched=")?;
                    }
                }
            }
            init.set_option("sub-auto", "fuzzy")?;
            init.set_option("sub-visibility", "yes")?;
            init.set_option("slang", "en,eng,hi,hin")?;
            init.set_option("keep-open", "yes")?;
            init.set_option("idle", "yes")?;
            Ok(())
        })
        .context("Failed to initialize libmpv")?;

        let player = Self {
            mpv: Arc::new(mpv),
            current_video: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(PlaybackStatus::default())),
            sponsor_tracker: Arc::new(Mutex::new(None)),
            sponsor_client: SponsorBlockClient::default(),
            config: cfg.clone(),
        };

        Ok(player)
    }

    pub fn load_video(&self, video: VideoCard, start_sec: Option<f64>) -> Result<()> {
        let vid_id = video.id.clone();
        let target_url = if video.id.starts_with('/') || video.id.starts_with('.') {
            video.id.clone()
        } else {
            format!("https://www.youtube.com/watch?v={}", video.id)
        };

        let start = start_sec.or_else(|| video.resolved_resume_sec().map(|s| s as f64));

        if let Some(st) = start {
            self.mpv.command(
                "loadfile",
                &[&target_url, "replace", "-1", &format!("start={st}")],
            )?;
        } else {
            self.mpv.command("loadfile", &[&target_url, "replace"])?;
        }

        {
            let mut guard = self.current_video.lock().unwrap();
            *guard = Some(video);
        }

        {
            let mut st = self.status.lock().unwrap();
            st.file_loaded = false;
            st.time_pos = start.unwrap_or(0.0);
            st.paused = false;
            st.is_alive = true;
        }

        // Fetch SponsorBlock segments asynchronously
        let client = self.sponsor_client.clone();
        let tracker_ref = Arc::clone(&self.sponsor_tracker);
        tokio::spawn(async move {
            if let Ok(segments) = client.fetch_segments(&vid_id).await {
                let mut guard = tracker_ref.lock().unwrap();
                *guard = Some(SponsorBlockTracker::new(&vid_id, segments));
            }
        });

        Ok(())
    }

    pub fn play(&self) -> Result<()> {
        self.mpv.set_property("pause", false)?;
        let mut st = self.status.lock().unwrap();
        st.paused = false;
        Ok(())
    }

    pub fn pause(&self) -> Result<()> {
        self.mpv.set_property("pause", true)?;
        let mut st = self.status.lock().unwrap();
        st.paused = true;
        Ok(())
    }

    pub fn toggle_pause(&self) -> Result<()> {
        let is_paused: bool = self.mpv.get_property("pause").unwrap_or(false);
        self.mpv.set_property("pause", !is_paused)?;
        let mut st = self.status.lock().unwrap();
        st.paused = !is_paused;
        Ok(())
    }

    pub fn seek(&self, seconds: f64, relative: bool) -> Result<()> {
        if relative {
            self.mpv
                .command("seek", &[&format!("{seconds}"), "relative"])?;
        } else {
            self.mpv
                .command("seek", &[&format!("{seconds}"), "absolute", "exact"])?;
        }

        if let Ok(time_pos) = self.mpv.get_property::<f64>("time-pos") {
            if let Some(ref mut trk) = *self.sponsor_tracker.lock().unwrap() {
                trk.on_manual_seek(time_pos);
            }
        }
        Ok(())
    }

    pub fn set_volume(&self, volume: f64) -> Result<()> {
        let clamped = volume.clamp(0.0, 100.0);
        self.mpv.set_property("volume", clamped)?;
        let mut st = self.status.lock().unwrap();
        st.volume = clamped;
        Ok(())
    }

    pub fn set_speed(&self, speed: f64) -> Result<()> {
        let clamped = speed.clamp(0.25, 3.0);
        self.mpv.set_property("speed", clamped)?;
        let mut st = self.status.lock().unwrap();
        st.speed = clamped;
        Ok(())
    }

    pub fn cycle_subtitles(&self) -> Result<()> {
        self.mpv.command("cycle", &["sub-visibility"])?;
        Ok(())
    }

    pub fn poll_status(&self) -> PlaybackStatus {
        let time_pos: f64 = self.mpv.get_property("time-pos").unwrap_or(0.0);
        let duration: f64 = self.mpv.get_property("duration").unwrap_or(0.0);
        let paused: bool = self.mpv.get_property("pause").unwrap_or(false);
        let volume: f64 = self.mpv.get_property("volume").unwrap_or(100.0);
        let speed: f64 = self.mpv.get_property("speed").unwrap_or(1.0);
        let paused_for_cache: bool = self.mpv.get_property("paused-for-cache").unwrap_or(false);
        let eof: bool = self.mpv.get_property("eof-reached").unwrap_or(false);

        // Check SponsorBlock automatically during playback
        if !paused && time_pos > 0.0 {
            let mut tracker_guard = self.sponsor_tracker.lock().unwrap();
            if let Some(ref mut trk) = *tracker_guard {
                if let Some((target_seek, notice)) = trk.check_position(time_pos) {
                    tracing::info!("{notice}");
                    let _ = self
                        .mpv
                        .command("seek", &[&format!("{target_seek}"), "absolute", "exact"]);
                }
            }
        }

        let mut st = self.status.lock().unwrap();
        st.time_pos = time_pos;
        st.duration = duration;
        st.paused = paused;
        st.volume = volume;
        st.speed = speed;
        st.paused_for_cache = paused_for_cache;
        if duration > 0.0 {
            st.file_loaded = true;
        }
        if eof {
            st.core_idle = true;
        }
        st.clone()
    }
}
