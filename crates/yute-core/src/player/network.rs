//! Direct MPV transport policy. Recovery is owned by the existing IPC loop.
use super::ipc::PlaybackStatus;
use serde::{Deserialize, Serialize};
use std::time::Duration;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    #[default]
    Auto,
    Mpv,
    Proxy,
}
impl Engine {
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s {
            "auto" => Ok(Self::Auto),
            "mpv" => Ok(Self::Mpv),
            "proxy" => Ok(Self::Proxy),
            _ => anyhow::bail!("Network engine must be auto, mpv or proxy"),
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    #[default]
    Direct,
    Buffering,
    Refreshing,
    Proxy,
    Failed,
}
#[derive(Default)]
pub struct Recovery {
    attempted: bool,
    stalled: Option<Duration>,
    healthy: Option<(Duration, f64)>,
    previous: Option<f64>,
    pub phase: Phase,
}
impl Recovery {
    pub fn proxy_started(&mut self) {
        self.phase = Phase::Proxy;
    }
    pub fn observe(
        &mut self,
        now: Duration,
        s: &PlaybackStatus,
        network: bool,
        engine: Engine,
    ) -> bool {
        let delta = self.previous.map(|p| s.time_pos - p).unwrap_or(0.);
        self.previous = Some(s.time_pos);
        if !network || !s.is_alive {
            self.stalled = None;
            self.healthy = None;
            self.phase = Phase::Direct;
            return false;
        }
        // MPV reports pause/end-file transiently while a replacement stream is
        // loading. Preserve the recovery phase until the replacement is actually
        // loaded; otherwise Proxy is incorrectly reset to Direct mid-switch.
        if !s.file_loaded {
            return false;
        }
        if s.paused {
            self.stalled = None;
            self.healthy = None;
            self.phase = if engine == Engine::Proxy {
                Phase::Proxy
            } else {
                Phase::Direct
            };
            return false;
        }
        let healthy = s.file_loaded
            && !s.paused_for_cache
            && !s.seeking
            && !s.core_idle
            && (0.05..=5.).contains(&delta);
        if healthy {
            self.stalled = None;
            let (since, pos) = *self.healthy.get_or_insert((now, s.time_pos));
            if now.saturating_sub(since) >= Duration::from_secs(2) && s.time_pos - pos >= 2. {
                self.attempted = false;
                self.phase = if engine == Engine::Proxy {
                    Phase::Proxy
                } else {
                    Phase::Direct
                };
            }
            return false;
        }
        self.healthy = None;
        let waiting = (s.paused_for_cache && s.cache_speed.is_none_or(|rate| rate <= 1024.))
            || s.seeking
            || s.playback_error.is_some()
            || (s.file_loaded && !s.idle_active && s.core_idle);
        if !waiting {
            return false;
        }
        let since = *self.stalled.get_or_insert(now);
        let elapsed = now.saturating_sub(since);
        if self.phase != Phase::Refreshing {
            self.phase = Phase::Buffering;
        }
        if elapsed >= Duration::from_secs(4) && !self.attempted && engine == Engine::Auto {
            self.attempted = true;
            // Auto performs one fresh anonymous resolution as part of switching
            // to the finite-range transport at the current position. Avoid an
            // intermediate MPV loadfile refresh: live testing showed that it can
            // restart near zero and incorrectly look healthy while losing the seek.
            self.phase = Phase::Refreshing;
            return true;
        }
        if elapsed >= Duration::from_secs(20) {
            self.phase = Phase::Failed;
        }
        false
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StreamSummary {
    pub host: Option<String>,
    pub query_keys: Vec<String>,
}
impl StreamSummary {
    pub fn from_url(raw: &str) -> Self {
        let Ok(url) = reqwest::Url::parse(raw) else {
            return Self::default();
        };
        if !matches!(url.scheme(), "http" | "https") {
            return Self::default();
        }
        let mut keys = url
            .query_pairs()
            .map(|(k, _)| {
                k.chars()
                    .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
                    .take(64)
                    .collect::<String>()
            })
            .collect::<Vec<_>>();
        keys.sort();
        keys.dedup();
        Self {
            host: url.host_str().map(Into::into),
            query_keys: keys,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn stalled() -> PlaybackStatus {
        PlaybackStatus {
            is_alive: true,
            file_loaded: true,
            paused_for_cache: true,
            cache_speed: Some(0.),
            time_pos: 42.,
            ..Default::default()
        }
    }
    #[test]
    fn recovery_is_bounded_until_two_seconds_of_healthy_progress() {
        let mut r = Recovery::default();
        let mut s = stalled();
        let mut tries = 0;
        for n in 0..30 {
            tries += usize::from(r.observe(Duration::from_secs(n), &s, true, Engine::Auto));
        }
        assert_eq!(tries, 1);
        assert_eq!(r.phase, Phase::Failed);
        s.paused_for_cache = false;
        for n in 30..34 {
            s.time_pos += 1.;
            assert!(!r.observe(Duration::from_secs(n), &s, true, Engine::Auto));
        }
        s.paused_for_cache = true;
        for n in 34..40 {
            tries += usize::from(r.observe(Duration::from_secs(n), &s, true, Engine::Auto));
        }
        assert_eq!(tries, 2);
    }
    #[test]
    fn auto_requests_one_proxy_switch_after_four_seconds() {
        let mut r = Recovery::default();
        let s = stalled();
        let mut switches = 0;
        for n in 0..20 {
            switches += usize::from(r.observe(Duration::from_secs(n), &s, true, Engine::Auto));
        }
        assert_eq!(switches, 1);
        r.proxy_started();
        assert_eq!(r.phase, Phase::Proxy);
    }
    #[test]
    fn local_and_forced_direct_do_not_reload() {
        for (network, engine) in [(false, Engine::Auto), (true, Engine::Mpv)] {
            let mut r = Recovery::default();
            for n in 0..30 {
                assert!(!r.observe(Duration::from_secs(n), &stalled(), network, engine));
            }
        }
    }

    #[test]
    fn replacement_loading_preserves_proxy_phase() {
        let mut r = Recovery::default();
        r.proxy_started();
        let s = PlaybackStatus {
            is_alive: true,
            file_loaded: false,
            paused: true,
            ..Default::default()
        };
        assert!(!r.observe(Duration::from_secs(6), &s, true, Engine::Proxy));
        assert_eq!(r.phase, Phase::Proxy);
    }
    #[test]
    fn url_summary_never_exposes_signed_values() {
        let s = StreamSummary::from_url(
            "https://r.googlevideo.com/videoplayback?sig=PRIVATE&ip=PRIVATE&expire=PRIVATE",
        );
        let text = serde_json::to_string(&s).unwrap();
        assert!(!text.contains("PRIVATE"));
        assert_eq!(s.query_keys, vec!["expire", "ip", "sig"]);
    }
}
