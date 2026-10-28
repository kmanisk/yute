use crate::backend::models::{
    Card, ChannelTab, ChatBatch, FeedPage, LiveState, SearchFilters, VideoCard, WatchData,
};
use crate::backend::Backend;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Flat playlists expose approximate upload timestamps when requested. Do not
/// resolve every video just to populate a card's age.
fn published_text(item: &Value, now: u64) -> Option<String> {
    if let Some(timestamp) = item.get("timestamp").and_then(Value::as_u64) {
        if timestamp > now {
            return None;
        }
        let age = now - timestamp;
        let (count, unit) = if age < 60 {
            return Some("Just now".into());
        } else if age < 3600 {
            (age / 60, "minute")
        } else if age < 86400 {
            (age / 3600, "hour")
        } else if age < 30 * 86400 {
            (age / 86400, "day")
        } else if age < 365 * 86400 {
            (age / (30 * 86400), "month")
        } else {
            (age / (365 * 86400), "year")
        };
        return Some(format!(
            "{count} {unit}{} ago",
            if count == 1 { "" } else { "s" }
        ));
    }
    item.get("upload_date")
        .and_then(Value::as_str)
        .filter(|date| date.len() == 8 && date.bytes().all(|c| c.is_ascii_digit()))
        .map(|date| format!("{}-{}-{}", &date[..4], &date[4..6], &date[6..]))
}

/// Public extraction fallback; never receives authenticated session material.
pub struct YtDlpBackend;

impl YtDlpBackend {
    pub fn new(_legacy_cookies: Option<PathBuf>, _legacy_browser: Option<String>) -> Self {
        Self
    }

    async fn run_ytdlp(&self, target: &str) -> Result<Value> {
        let mut cmd = Command::new("yt-dlp");
        cmd.kill_on_drop(true)
            .arg("--ignore-config")
            .arg("-J")
            .arg("--flat-playlist")
            .arg("--no-warnings")
            .arg("--extractor-args")
            .arg("youtubetab:approximate_date")
            .arg("--playlist-items")
            .arg("1:30")
            .arg(target)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = tokio::time::timeout(std::time::Duration::from_secs(45), cmd.output())
            .await
            .map_err(|_| anyhow!("yt-dlp request timed out after 45 seconds"))?
            .context("Failed to execute yt-dlp; check PATH and executable permissions")?;
        if !output.status.success() {
            return Err(anyhow!(
                "yt-dlp failed ({}); resolver stderr suppressed to protect session data",
                output.status
            ));
        }

        let val: Value =
            serde_json::from_slice(&output.stdout).context("Failed to parse yt-dlp JSON output")?;
        Ok(val)
    }

    fn parse_entries(&self, val: &Value) -> Vec<Card> {
        let mut cards = Vec::new();
        let entries = val.get("entries").and_then(|e| e.as_array());

        let list = match entries {
            Some(arr) => arr.as_slice(),
            None => std::slice::from_ref(val),
        };

        for item in list {
            let id = item
                .get("id")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let title = item
                .get("title")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() || title.is_empty() {
                continue;
            }

            let mut thumb_url = item
                .get("thumbnail")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            if thumb_url.is_empty() {
                if let Some(thumbs) = item.get("thumbnails").and_then(|t| t.as_array()) {
                    if let Some(first) = thumbs
                        .first()
                        .and_then(|t| t.get("url"))
                        .and_then(|u| u.as_str())
                    {
                        thumb_url = first.to_string();
                    }
                }
            }
            if thumb_url.is_empty() {
                thumb_url = format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg");
            }

            let entity_url = item
                .get("url")
                .or_else(|| item.get("webpage_url"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let is_channel = item.get("ie_key").and_then(Value::as_str) == Some("YoutubeTab")
                && (id.starts_with("UC")
                    || entity_url.contains("/channel/")
                    || entity_url.contains("youtube.com/@"));
            if is_channel {
                cards.push(Card::Channel(crate::backend::models::ChannelCard {
                    id,
                    title,
                    handle: item
                        .get("uploader_id")
                        .and_then(Value::as_str)
                        .filter(|h| h.starts_with('@'))
                        .map(str::to_string),
                    subscribers: item
                        .get("channel_follower_count")
                        .and_then(Value::as_u64)
                        .map(|n| format!("{n} subscribers")),
                    avatar_url: Some(thumb_url).filter(|url| !url.contains("i.ytimg.com/vi/")),
                    verified: false,
                }));
                continue;
            }
            let extractor = item.get("ie_key").and_then(Value::as_str).unwrap_or("");
            let is_playlist = item.get("_type").and_then(Value::as_str) == Some("playlist")
                || matches!(extractor, "YoutubeTab" | "YoutubePlaylist")
                || extractor.is_empty()
                    && ((id.len() > 11 && (id.starts_with("PL") || id.starts_with("RD")))
                        || id == "WL"
                        || id == "LL");
            let is_mix = is_playlist && id.starts_with("RD");

            if is_playlist {
                let default_chan = if is_mix { "YouTube" } else { "Playlist" };
                let channel = item
                    .get("uploader")
                    .or_else(|| item.get("channel"))
                    .and_then(|s| s.as_str())
                    .unwrap_or(default_chan)
                    .to_string();
                let video_count = if is_mix {
                    Some("Mix".to_string())
                } else {
                    None
                };
                cards.push(Card::Playlist(crate::backend::models::PlaylistCard {
                    id,
                    title,
                    channel,
                    video_count,
                    thumb_url,
                    is_mix,
                    first_video_id: None,
                }));
                continue;
            }

            let channel = item
                .get("uploader")
                .or_else(|| item.get("channel"))
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let channel_id = item
                .get("channel_id")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            let duration_sec = item.get("duration").and_then(|d| d.as_u64());
            let duration = duration_sec.map(|sec| {
                let mins = sec / 60;
                let s = sec % 60;
                format!("{mins}:{s:02}")
            });
            let views = item
                .get("view_count")
                .and_then(|v| v.as_u64())
                .map(|cnt| format!("{cnt} views"));

            cards.push(Card::Video(VideoCard {
                id,
                title,
                channel,
                channel_id,
                verified: false,
                duration,
                views,
                published: published_text(item, now_seconds()),
                thumb_url,
                progress_pct: None,
                live: LiveState::None,
                badges: Vec::new(),
                resume_time_sec: None,
            }));
        }

        cards
    }
}

#[async_trait]
impl Backend for YtDlpBackend {
    async fn home(&self, _chip_token: Option<&str>) -> Result<FeedPage> {
        let val = self.run_ytdlp(":ytrec").await?;
        let items = self.parse_entries(&val);
        Ok(FeedPage {
            chips: Vec::new(),
            items,
            continuation: None,
        })
    }

    async fn continue_feed(&self, _token: &str) -> Result<FeedPage> {
        Err(anyhow!("Pagination not supported in yt-dlp fallback"))
    }

    async fn subscriptions(&self) -> Result<FeedPage> {
        anyhow::bail!("Account feeds require TV OAuth; sign in with :account add")
    }

    async fn history(&self) -> Result<FeedPage> {
        anyhow::bail!("Account feeds require TV OAuth; sign in with :account add")
    }

    async fn watch_later(&self) -> Result<FeedPage> {
        anyhow::bail!("Account feeds require TV OAuth; sign in with :account add")
    }

    async fn playlists(&self) -> Result<FeedPage> {
        anyhow::bail!("Account feeds require TV OAuth; sign in with :account add")
    }

    async fn playlist(&self, id: &str) -> Result<FeedPage> {
        if matches!(id, "LL" | "WL" | "VLLL" | "VLWL") {
            anyhow::bail!("Account feeds require TV OAuth; sign in with :account add");
        }
        let target = if let Some(seed) = crate::backend::models::mix_seed_video_id(id) {
            format!("https://www.youtube.com/watch?v={seed}&list={id}")
        } else {
            format!("https://www.youtube.com/playlist?list={id}")
        };
        let val = self.run_ytdlp(&target).await?;
        let items = self.parse_entries(&val);
        Ok(FeedPage {
            chips: Vec::new(),
            items,
            continuation: None,
        })
    }

    async fn search(
        &self,
        q: &str,
        _filters: &SearchFilters,
        _token: Option<&str>,
    ) -> Result<FeedPage> {
        let target = format!("ytsearch30:{q}");
        let val = self.run_ytdlp(&target).await?;
        let items = self.parse_entries(&val);
        Ok(FeedPage {
            chips: Vec::new(),
            items,
            continuation: None,
        })
    }

    async fn suggest(&self, _q: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    async fn watch(&self, video_id: &str) -> Result<WatchData> {
        let target = format!("https://www.youtube.com/watch?v={video_id}");
        let val = self.run_ytdlp(&target).await?;

        let id = val
            .get("id")
            .and_then(|s| s.as_str())
            .unwrap_or(video_id)
            .to_string();
        let title = val
            .get("title")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let channel = val
            .get("uploader")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let description = val
            .get("description")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let duration_sec = val.get("duration").and_then(|d| d.as_u64());
        let duration = duration_sec.map(|sec| {
            let mins = sec / 60;
            let s = sec % 60;
            format!("{mins}:{s:02}")
        });
        let views = val
            .get("view_count")
            .and_then(|v| v.as_u64())
            .map(|cnt| format!("{cnt} views"));
        let thumb_url = val
            .get("thumbnail")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();

        let video = VideoCard {
            id,
            title,
            channel,
            channel_id: None,
            verified: false,
            duration,
            views,
            published: published_text(&val, now_seconds()),
            thumb_url,
            progress_pct: None,
            live: LiveState::None,
            badges: Vec::new(),
            resume_time_sec: None,
        };

        Ok(WatchData {
            video,
            description,
            chapters: Vec::new(),
            up_next: Vec::new(),
            chat_token: None,
            likes: None,
            dislikes: None,
            rating: None,
            is_liked: None,
            is_disliked: None,
            is_subscribed: None,
            comments_token: None,
            comments: Vec::new(),
            comment_create_params: None,
            description_links: Vec::new(),
        })
    }

    async fn channel(&self, id: &str, _tab: ChannelTab) -> Result<FeedPage> {
        let target = format!("https://www.youtube.com/channel/{id}/videos");
        let val = self.run_ytdlp(&target).await?;
        let items = self.parse_entries(&val);
        Ok(FeedPage {
            chips: Vec::new(),
            items,
            continuation: None,
        })
    }

    async fn live_chat(&self, _token: &str) -> Result<ChatBatch> {
        Err(anyhow!("Live chat not supported by yt-dlp fallback"))
    }

    async fn mark_watched(&self, _id: &str) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flat_upload_dates_are_kept_without_video_resolution() {
        assert_eq!(
            published_text(&serde_json::json!({"timestamp":1000}), 1000 + 2 * 86400).as_deref(),
            Some("2 days ago")
        );
        assert_eq!(
            published_text(&serde_json::json!({"timestamp":1000}), 1060).as_deref(),
            Some("1 minute ago")
        );
        assert_eq!(
            published_text(&serde_json::json!({"timestamp":2000}), 1000),
            None
        );
        assert_eq!(
            published_text(&serde_json::json!({"upload_date":"20261020"}), 1000).as_deref(),
            Some("2026-10-20")
        );
        assert_eq!(published_text(&serde_json::json!({}), 1000), None);
        let backend = YtDlpBackend::new(None, None);
        let cards=backend.parse_entries(&serde_json::json!({"entries":[{"id":"abcdefghijk","title":"Video","ie_key":"Youtube","timestamp":now_seconds()-3*86400}]}));
        assert!(matches!(&cards[0],Card::Video(v) if v.published.as_deref()==Some("3 days ago")));
    }
    #[test]
    fn extractor_entity_types_override_titles_and_prefixes() {
        let backend = YtDlpBackend::new(None, None);
        let result=backend.parse_entries(&serde_json::json!({"entries":[
            {"id":"RDabcdefghi","title":"Mix of videos","ie_key":"Youtube","_type":"url"},
            {"id":"UCfixture","title":"Channel","ie_key":"YoutubeTab","url":"https://www.youtube.com/channel/UCfixture"},
            {"id":"PLfixturelong","title":"Playlist","ie_key":"YoutubeTab"}
        ]}));
        assert!(matches!(&result[0], Card::Video(_)));
        assert!(matches!(&result[1], Card::Channel(_)));
        assert!(matches!(&result[2], Card::Playlist(_)));
    }
}
