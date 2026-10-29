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
    #[serde(default)]
    pub resume_time_sec: Option<u64>,
}

impl VideoCard {
    pub fn resolved_resume_sec(&self) -> Option<u64> {
        if let Some(sec) = self.resume_time_sec {
            if sec > 5 {
                return Some(sec);
            }
        }
        if let Some(pct) = self.progress_pct {
            if pct > 2 && pct < 95 {
                if let Some(ref dur) = self.duration {
                    if let Some(total_sec) = parse_duration_to_seconds(dur) {
                        let sec = (total_sec as f64 * (pct as f64 / 100.0)) as u64;
                        if sec > 5 {
                            return Some(sec);
                        }
                    }
                }
            }
        }
        None
    }
}

pub fn parse_duration_to_seconds(s: &str) -> Option<u64> {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        1 => parts[0].trim().parse::<u64>().ok(),
        2 => {
            let mins = parts[0].trim().parse::<u64>().ok()?;
            let secs = parts[1].trim().parse::<u64>().ok()?;
            Some(mins * 60 + secs)
        }
        3 => {
            let hours = parts[0].trim().parse::<u64>().ok()?;
            let mins = parts[1].trim().parse::<u64>().ok()?;
            let secs = parts[2].trim().parse::<u64>().ok()?;
            Some(hours * 3600 + mins * 60 + secs)
        }
        _ => None,
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaylistCard {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub video_count: Option<String>,
    pub thumb_url: String,
    pub is_mix: bool,
    #[serde(default)]
    pub first_video_id: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
}

/// Only the canonical RD + video-id form exposes a seed directly. Other Mix
/// identities are opaque and must be resolved by YouTube using their playlist index.
pub fn mix_seed_video_id(id: &str) -> Option<&str> {
    id.strip_prefix("RD").filter(|seed| {
        seed.len() == 11
            && seed
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    })
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

/// Current TV subscriptions expose channel tabs as FEsubscriptions + opaque params
/// instead of a UC browse id. Keep that endpoint typed without pretending it is a
/// normal YouTube channel identity.
pub const SUBSCRIPTION_TAB_PREFIX: &str = "yute:subscription-tab:";
pub const SUBSCRIPTIONS_BROWSE_ID: &str = "FEsubscriptions";

pub fn subscription_tab_card_id(params: &str) -> String {
    format!("{SUBSCRIPTION_TAB_PREFIX}{params}")
}

pub fn subscription_tab_params(id: &str) -> Option<&str> {
    id.strip_prefix(SUBSCRIPTION_TAB_PREFIX)
        .filter(|p| !p.is_empty())
}

pub fn youtube_channel_id(id: &str) -> Option<&str> {
    (id.starts_with("UC") && id.len() == 24).then_some(id)
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommentMediaKind {
    Image,
    Gif,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommentMedia {
    pub url: String,
    pub kind: CommentMediaKind,
    pub width: Option<u32>,
    pub height: Option<u32>,
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
    #[serde(default)]
    pub media: Vec<CommentMedia>,
    #[serde(default)]
    pub is_liked: bool,
    #[serde(default)]
    pub is_disliked: bool,
    /// Continuation for the focused comment detail. Current TV responses put
    /// reaction/reply actions there rather than on the list item itself.
    #[serde(default, skip_serializing)]
    pub detail_token: Option<String>,
    #[serde(default, skip_serializing)]
    pub like_action: Option<String>,
    #[serde(default, skip_serializing)]
    pub unlike_action: Option<String>,
    #[serde(default, skip_serializing)]
    pub dislike_action: Option<String>,
    #[serde(default, skip_serializing)]
    pub undislike_action: Option<String>,
    #[serde(default, skip_serializing)]
    pub reply_params: Option<String>,
    #[serde(default, skip_serializing)]
    pub replies_token: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CommentsBatch {
    pub comments: Vec<CommentItem>,
    pub continuation: Option<String>,
    pub total_count: Option<String>,
    #[serde(default, skip_serializing)]
    pub create_params: Option<String>,
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
    #[serde(default, skip_serializing)]
    pub comment_create_params: Option<String>,
    #[serde(default)]
    pub description_links: Vec<(String, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelTab {
    Videos,
    Live,
    Playlists,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HomeCategory {
    Shorts,
    Live,
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

/// Tabs are advertised by YouTube; params are opaque and belong to this channel.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelLink {
    pub title: String,
    pub params: Option<String>,
    pub selected: bool,
}

#[derive(Clone, Debug)]
pub struct ChannelPage {
    pub channel: ChannelCard,
    pub tabs: Vec<ChannelLink>,
    pub subscribed: Option<bool>,
    pub feed: FeedPage,
}
