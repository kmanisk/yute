use crate::backend::models::{FeedPage, WatchData};
use crossterm::event::KeyEvent;

#[allow(dead_code)]
pub enum AppEvent {
    Input(KeyEvent),
    Resize(u16, u16),
    Tick,
    FeedLoaded(u64, anyhow::Result<FeedPage>),
    MixLoaded(u64, anyhow::Result<FeedPage>),
    CardMetadataLoaded(u64, String, Option<crate::backend::models::VideoCard>),
    FeedContinued(
        u64,
        crate::app::Screen,
        String,
        String,
        anyhow::Result<FeedPage>,
    ),
    MutationFinished(u64, crate::actions::AccountMutation, anyhow::Result<()>),
    ChannelLoaded(
        u64,
        String,
        anyhow::Result<crate::backend::models::ChannelPage>,
    ),
    WatchDataLoaded(String, anyhow::Result<WatchData>),
    SuggestionsLoaded(anyhow::Result<Vec<String>>),
    AccountProfiles(u64, anyhow::Result<Vec<yute_core::accounts::Account>>),
    AccountLoaded(u64, anyhow::Result<(String, String)>),
    CredentialsLoaded(u64, anyhow::Result<yute_core::accounts::Accounts>),
    AccountRemoved(String, anyhow::Result<()>),
    LoginDisplay(u64, yute_core::oauth::LoginEvent),
    LoginStatus(u64, String),
    LoginSucceeded(u64, String, String),
    CommentsLoaded(
        String,
        anyhow::Result<crate::backend::models::CommentsBatch>,
    ),
    StatusMessage(String),
    MenuPlaylists(u64, anyhow::Result<FeedPage>),
    PlaylistSaved(u64, anyhow::Result<()>),
    DownloadFinished(u64, bool),
    CommentUpdated(u64, String, bool, anyhow::Result<()>),
}
