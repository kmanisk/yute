use crate::backend::models::{FeedPage, WatchData};
use crossterm::event::KeyEvent;

#[allow(dead_code)]
pub enum AppEvent {
    Input(KeyEvent),
    Resize(u16, u16),
    Tick,
    FeedLoaded(anyhow::Result<FeedPage>),
    FeedContinued(anyhow::Result<FeedPage>),
    WatchDataLoaded(anyhow::Result<WatchData>),
    SuggestionsLoaded(anyhow::Result<Vec<String>>),
    AccountLoaded(anyhow::Result<(String, String)>),
    CommentsLoaded(anyhow::Result<crate::backend::models::CommentsBatch>),
    StatusMessage(String),
}
