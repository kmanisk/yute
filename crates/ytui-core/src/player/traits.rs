use crate::backend::models::VideoCard;
use crate::player::ipc::PlaybackStatus;
use anyhow::Result;

#[derive(Clone, Debug, PartialEq)]
pub enum PlayerEvent {
    Position(f64),
    Duration(f64),
    Pause(bool),
    Buffering(bool),
    FileLoaded,
    EndOfFile,
    TrackListChanged,
    Error(String),
}

pub trait PlayerBackend: Send {
    fn load_video(&mut self, video: VideoCard, start_sec: Option<f64>) -> Result<()>;
    fn play(&mut self) -> Result<()>;
    fn pause(&mut self) -> Result<()>;
    fn toggle_pause(&mut self) -> Result<()>;
    fn seek(&mut self, seconds: f64, relative: bool) -> Result<()>;
    fn set_volume(&mut self, volume: f64) -> Result<()>;
    fn set_speed(&mut self, speed: f64) -> Result<()>;
    fn cycle_subtitles(&mut self) -> Result<()>;
    fn status(&self) -> PlaybackStatus;
    fn stop(&mut self) -> Result<()>;
}
