//! Opt-in measurements contain static operation names, durations and counts only.
use std::sync::OnceLock;
use std::time::{Duration, Instant};
static ENABLED: OnceLock<bool> = OnceLock::new();
static START: OnceLock<Instant> = OnceLock::new();
pub fn enabled() -> bool {
    *ENABLED.get_or_init(|| std::env::var("YUTE_TRACE").as_deref() == Ok("1"))
}
pub fn start() {
    let _ = START.set(Instant::now());
}
pub fn since_start(event: &'static str) {
    if let Some(start) = START.get() {
        duration(event, start.elapsed(), 0);
    }
}
pub fn duration(event: &'static str, duration: Duration, count: usize) {
    if enabled() {
        tracing::info!(target:"yute_perf",metric=event,elapsed_ms=duration.as_secs_f64()*1000.,count);
    }
}
