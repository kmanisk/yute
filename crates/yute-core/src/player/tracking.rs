use std::time::Instant;

#[derive(Clone, Debug)]
pub struct WatchTracker {
    pub video_id: String,
    pub cpn: String,
    pub last_ping_pos: f64,
    pub last_ping_time: Instant,
    pub playback_ping_sent: bool,
}

impl WatchTracker {
    pub fn new(video_id: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let cpn = format!("{:016x}", now);
        Self {
            video_id: video_id.to_string(),
            cpn,
            last_ping_pos: 0.0,
            last_ping_time: Instant::now(),
            playback_ping_sent: false,
        }
    }

    pub fn should_ping_watchtime(&self, current_pos: f64) -> bool {
        if current_pos <= 0.0 {
            return false;
        }
        let pos_diff = current_pos - self.last_ping_pos;
        let time_elapsed = self.last_ping_time.elapsed().as_secs();
        pos_diff >= 25.0 || (pos_diff > 2.0 && time_elapsed >= 30)
    }

    pub fn record_ping(&mut self, current_pos: f64) -> (f64, f64, f64) {
        let st = self.last_ping_pos;
        let et = current_pos;
        let cmt = current_pos;
        self.last_ping_pos = current_pos;
        self.last_ping_time = Instant::now();
        (st, et, cmt)
    }
}
