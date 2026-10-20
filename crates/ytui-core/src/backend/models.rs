use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum LiveState {
    None,
    Live,
    Upcoming,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VideoCard {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub channel_id: Option<String>,
    pub verified: bool,
    pub duration: Option<String>,
    pub views: Option<String>,
    pub published: Option<String>,
    pub thumb_url: String,
    pub progress_pct: Option<u8>,
    pub live: LiveState,
    pub badges: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaylistCard {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub video_count: Option<String>,
    pub thumb_url: String,
    pub is_mix: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelCard {
    pub id: String,
    pub title: String,
    pub handle: Option<String>,
    pub subscribers: Option<String>,
    pub avatar_url: Option<String>,
    pub verified: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Card {
    Video(VideoCard),
    Playlist(PlaylistCard),
    Channel(ChannelCard),
}

impl Card {
    #[allow(dead_code)]
    pub fn id(&self) -> &str {
        match self {
            Card::Video(v) => &v.id,
            Card::Playlist(p) => &p.id,
            Card::Channel(c) => &c.id,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            Card::Video(v) => &v.title,
            Card::Playlist(p) => &p.title,
            Card::Channel(c) => &c.title,
        }
    }

    pub fn thumb_url(&self) -> &str {
        match self {
            Card::Video(v) => &v.thumb_url,
            Card::Playlist(p) => &p.thumb_url,
            Card::Channel(c) => c.avatar_url.as_deref().unwrap_or(""),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chip {
    pub title: String,
    pub token: Option<String>,
    pub active: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FeedPage {
    pub chips: Vec<Chip>,
    pub items: Vec<Card>,
    pub continuation: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommentItem {
    pub id: String,
    pub author: String,
    pub author_thumb: Option<String>,
    pub text: String,
    pub published: Option<String>,
    pub likes: Option<String>,
    pub reply_count: Option<u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CommentsBatch {
    pub comments: Vec<CommentItem>,
    pub continuation: Option<String>,
    pub total_count: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatchData {
    pub video: VideoCard,
    pub description: String,
    pub chapters: Vec<(u32, String)>,
    pub up_next: Vec<Card>,
    pub chat_token: Option<String>,
    pub likes: Option<String>,
    pub dislikes: Option<String>,
    pub rating: Option<f64>,
    pub is_liked: Option<bool>,
    pub is_disliked: Option<bool>,
    pub is_subscribed: Option<bool>,
    pub comments_token: Option<String>,
    pub comments: Vec<CommentItem>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelTab {
    Videos,
    Live,
    Playlists,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SearchFilters {
    pub filter_type: Option<String>,
    pub duration: Option<String>,
    pub upload_date: Option<String>,
    pub sort_by: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub author: String,
    pub author_badges: Vec<String>,
    pub message: String,
    pub timestamp_usec: u64,
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ChatBatch {
    pub messages: Vec<ChatMessage>,
    pub next_token: Option<String>,
    pub poll_interval_ms: u64,
}
