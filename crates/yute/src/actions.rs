//! Typed content activation. Neither titles nor renderers choose playback.
use crate::backend::models::{Card, ChannelCard, LiveState, PlaylistCard, VideoCard};

#[derive(Clone, Debug)]
pub enum OpenAction {
    Play(VideoCard),
    OpenChannel(ChannelCard),
    OpenPlaylist(PlaylistCard),
    PlayMix(PlaylistCard),
    Unavailable(&'static str),
}

pub fn activation(card: &Card) -> OpenAction {
    match card {
        Card::Video(video) if video.live == LiveState::Upcoming => {
            OpenAction::Unavailable("This stream is upcoming and cannot be played yet")
        }
        // Reject malformed legacy cache entries rather than passing browse IDs to MPV.
        Card::Video(video) if video.id.starts_with("UC") && video.id.len() != 11 => {
            OpenAction::Unavailable("Cached item has an invalid video identity — refresh this feed")
        }
        Card::Video(video) if video.id.is_empty() => {
            OpenAction::Unavailable("Video identity unavailable — refresh this feed")
        }
        Card::Video(video) => OpenAction::Play(video.clone()),
        Card::Channel(channel) => OpenAction::OpenChannel(channel.clone()),
        Card::Playlist(playlist) if playlist.is_mix => OpenAction::PlayMix(playlist.clone()),
        Card::Playlist(playlist) => OpenAction::OpenPlaylist(playlist.clone()),
    }
}

pub fn channel_of(card: &Card) -> Option<ChannelCard> {
    match card {
        Card::Channel(channel) => Some(channel.clone()),
        Card::Video(video) => video.channel_id.as_ref().map(|id| ChannelCard {
            id: id.clone(),
            title: video.channel.clone(),
            handle: None,
            subscribers: None,
            avatar_url: None,
            verified: video.verified,
        }),
        Card::Playlist(_) => None,
    }
}

pub fn url(card: &Card) -> String {
    match card {
        Card::Channel(c) => format!("https://www.youtube.com/channel/{}", c.id),
        Card::Playlist(p) => format!(
            "https://www.youtube.com/playlist?list={}",
            p.id.trim_start_matches("VL")
        ),
        Card::Video(v) if v.id.starts_with('/') || v.id.starts_with('.') => v.id.clone(),
        Card::Video(v) => format!("https://www.youtube.com/watch?v={}", v.id),
    }
}

#[derive(Clone, Debug)]
pub enum AccountMutation {
    Dislike {
        video_id: String,
    },
    Subscribe {
        channel_id: String,
        subscribed: bool,
    },
    Like {
        video_id: String,
        liked: bool,
    },
}
