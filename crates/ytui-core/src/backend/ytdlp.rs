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

pub struct YtDlpBackend {
    cookies_path: Option<PathBuf>,
    cookies_from_browser: Option<String>,
}

impl YtDlpBackend {
    pub fn new(cookies_path: Option<PathBuf>, cookies_from_browser: Option<String>) -> Self {
        Self {
            cookies_path,
            cookies_from_browser,
        }
    }

    async fn run_ytdlp(&self, target: &str) -> Result<Value> {
        let mut cmd = Command::new("yt-dlp");
        cmd.arg("-J")
            .arg("--flat-playlist")
            .arg("--no-warnings")
            .arg("--playlist-items")
            .arg("1:30")
            .arg(target)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut cookie_set = false;
        if let Some(ref browser) = self.cookies_from_browser {
            if !browser.is_empty() {
                cmd.arg("--cookies-from-browser").arg(browser);
                cookie_set = true;
            }
        }

        if !cookie_set {
            if let Some(ref p) = self.cookies_path {
                if p.exists() {
                    let has_auth = std::fs::read_to_string(p)
                        .map(|s| s.contains("LOGIN_INFO") || s.contains("SAPISID") || s.contains("__Secure"))
                        .unwrap_or(false);
                    if has_auth {
                        cmd.arg("--cookies").arg(p);
                        cookie_set = true;
                    }
                }
            }
        }

        if !cookie_set {
            let home = std::env::var("HOME").unwrap_or_default();
            let brave_origin = PathBuf::from(format!("{home}/.config/BraveSoftware/Brave-Origin"));
            if brave_origin.exists() {
                cmd.arg("--cookies-from-browser").arg(format!("brave:{}", brave_origin.display()));
            }
        }

        let output = cmd.output().await.context("Failed to execute yt-dlp")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("yt-dlp error: {stderr}"));
        }

        let val: Value = serde_json::from_slice(&output.stdout)
            .context("Failed to parse yt-dlp JSON output")?;
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
            let id = item.get("id").and_then(|s| s.as_str()).unwrap_or("").to_string();
            let title = item.get("title").and_then(|s| s.as_str()).unwrap_or("").to_string();
            if id.is_empty() || title.is_empty() {
                continue;
            }

            let mut thumb_url = item.get("thumbnail")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();
            if thumb_url.is_empty() {
                if let Some(thumbs) = item.get("thumbnails").and_then(|t| t.as_array()) {
                    if let Some(first) = thumbs.first().and_then(|t| t.get("url")).and_then(|u| u.as_str()) {
                        thumb_url = first.to_string();
                    }
                }
            }
            if thumb_url.is_empty() {
                thumb_url = format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg");
            }

            let is_mix = id.starts_with("RD") || title.to_lowercase().contains("mix");
            let is_playlist = id.starts_with("PL") || id == "WL" || id == "LL"
                || is_mix
                || item.get("ie_key").and_then(|k| k.as_str()) == Some("YoutubeTab");

            if is_playlist {
                let default_chan = if is_mix { "YouTube" } else { "Playlist" };
                let channel = item.get("uploader")
                    .or_else(|| item.get("channel"))
                    .and_then(|s| s.as_str())
                    .unwrap_or(default_chan)
                    .to_string();
                let video_count = if is_mix { Some("Mix".to_string()) } else { None };
                cards.push(Card::Playlist(crate::backend::models::PlaylistCard {
                    id,
                    title,
                    channel,
                    video_count,
                    thumb_url,
                    is_mix,
                }));
                continue;
            }

            let channel = item.get("uploader")
                .or_else(|| item.get("channel"))
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let channel_id = item.get("channel_id").and_then(|s| s.as_str()).map(|s| s.to_string());
            let duration_sec = item.get("duration").and_then(|d| d.as_u64());
            let duration = duration_sec.map(|sec| {
                let mins = sec / 60;
                let s = sec % 60;
                format!("{mins}:{s:02}")
            });
            let views = item.get("view_count")
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
                published: None,
                thumb_url,
                progress_pct: None,
                live: LiveState::None,
                badges: Vec::new(),
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
        let val = self.run_ytdlp(":ytsubs").await?;
        let items = self.parse_entries(&val);
        Ok(FeedPage {
            chips: Vec::new(),
            items,
            continuation: None,
        })
    }

    async fn history(&self) -> Result<FeedPage> {
        let val = self.run_ytdlp(":ythistory").await?;
        let items = self.parse_entries(&val);
        Ok(FeedPage {
            chips: Vec::new(),
            items,
            continuation: None,
        })
    }

    async fn watch_later(&self) -> Result<FeedPage> {
        let val = self.run_ytdlp(":ytwatchlater").await?;
        let items = self.parse_entries(&val);
        Ok(FeedPage {
            chips: Vec::new(),
            items,
            continuation: None,
        })
    }

    async fn playlists(&self) -> Result<FeedPage> {
        let val = self.run_ytdlp("https://www.youtube.com/feed/playlists").await?;
        let items = self.parse_entries(&val);
        Ok(FeedPage {
            chips: Vec::new(),
            items,
            continuation: None,
        })
    }

    async fn playlist(&self, id: &str) -> Result<FeedPage> {
        let target = format!("https://www.youtube.com/playlist?list={id}");
        let val = self.run_ytdlp(&target).await?;
        let items = self.parse_entries(&val);
        Ok(FeedPage {
            chips: Vec::new(),
            items,
            continuation: None,
        })
    }

    async fn search(&self, q: &str, _filters: &SearchFilters, _token: Option<&str>) -> Result<FeedPage> {
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

        let id = val.get("id").and_then(|s| s.as_str()).unwrap_or(video_id).to_string();
        let title = val.get("title").and_then(|s| s.as_str()).unwrap_or("").to_string();
        let channel = val.get("uploader").and_then(|s| s.as_str()).unwrap_or("").to_string();
        let description = val.get("description").and_then(|s| s.as_str()).unwrap_or("").to_string();
        let duration_sec = val.get("duration").and_then(|d| d.as_u64());
        let duration = duration_sec.map(|sec| {
            let mins = sec / 60;
            let s = sec % 60;
            format!("{mins}:{s:02}")
        });
        let views = val.get("view_count").and_then(|v| v.as_u64()).map(|cnt| format!("{cnt} views"));
        let thumb_url = val.get("thumbnail")
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
            published: None,
            thumb_url,
            progress_pct: None,
            live: LiveState::None,
            badges: Vec::new(),
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
