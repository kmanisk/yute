use crate::backend::models::{
    Card, ChannelTab, ChatBatch, FeedPage, LiveState, SearchFilters, VideoCard, WatchData,
};
use crate::backend::Backend;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

const YOUTUBE_API_BASE: &str = "https://www.googleapis.com/youtube/v3";
#[allow(dead_code)]
const GOOGLE_OAUTH_DEVICE_CODE: &str = "https://oauth2.googleapis.com/device/code";
#[allow(dead_code)]
const GOOGLE_OAUTH_TOKEN: &str = "https://oauth2.googleapis.com/token";

#[allow(dead_code)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OAuthTokenFile {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at_sec: u64,
}

#[allow(dead_code)]
pub struct ApiBackend {
    client: Client,
    api_key: Option<String>,
    oauth_client_id: Option<String>,
    oauth_client_secret: Option<String>,
    token_file_path: PathBuf,
    quota_used: AtomicU64,
}

impl ApiBackend {
    pub fn new(
        api_key: Option<String>,
        client_id: Option<String>,
        client_secret: Option<String>,
        token_path: PathBuf,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            client,
            api_key,
            oauth_client_id: client_id,
            oauth_client_secret: client_secret,
            token_file_path: token_path,
            quota_used: AtomicU64::new(0),
        }
    }

    pub fn get_quota_used(&self) -> u64 {
        self.quota_used.load(Ordering::Relaxed)
    }

    fn record_quota(&self, units: u64) {
        self.quota_used.fetch_add(units, Ordering::Relaxed);
    }

    fn read_saved_token(&self) -> Option<OAuthTokenFile> {
        if self.token_file_path.exists() {
            if let Ok(data) = fs::read_to_string(&self.token_file_path) {
                return serde_json::from_str(&data).ok();
            }
        }
        None
    }

    async fn get_auth_param(&self) -> (Option<String>, Option<String>) {
        if let Some(token) = self.read_saved_token() {
            return (None, Some(token.access_token));
        }
        (self.api_key.clone(), None)
    }

    async fn api_get(
        &self,
        endpoint: &str,
        query_params: &[(&str, &str)],
        quota_cost: u64,
    ) -> Result<Value> {
        let (key_opt, token_opt) = self.get_auth_param().await;
        let url = format!("{YOUTUBE_API_BASE}/{endpoint}");
        let mut req = self.client.get(&url);

        if let Some(ref token) = token_opt {
            req = req.bearer_auth(token);
        }

        let mut all_params = query_params.to_vec();
        let key_str;
        if let Some(ref k) = key_opt {
            key_str = k.clone();
            all_params.push(("key", &key_str));
        }

        req = req.query(&all_params);

        let resp = req
            .send()
            .await
            .with_context(|| format!("API request to {endpoint} failed"))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(anyhow!("YouTube API returned HTTP {status} for {endpoint}"));
        }

        self.record_quota(quota_cost);
        let val: Value = resp.json().await.context("Failed to parse API JSON")?;
        Ok(val)
    }
}

#[async_trait]
impl Backend for ApiBackend {
    async fn home(&self, _chip_token: Option<&str>) -> Result<FeedPage> {
        Err(anyhow!("Home feed not supported by official YouTube API"))
    }

    async fn continue_feed(&self, _token: &str) -> Result<FeedPage> {
        Err(anyhow!("Continuation not supported in API fallback"))
    }

    async fn subscriptions(&self) -> Result<FeedPage> {
        // Requires OAuth subscriptions.list
        let val = self
            .api_get(
                "subscriptions",
                &[("part", "snippet"), ("mine", "true"), ("maxResults", "50")],
                1,
            )
            .await?;

        let mut items = Vec::new();
        if let Some(list) = val.get("items").and_then(|i| i.as_array()) {
            for item in list {
                let snippet = item.get("snippet");
                let ch_id = snippet
                    .and_then(|s| s.get("resourceId"))
                    .and_then(|r| r.get("channelId"))
                    .and_then(|id| id.as_str())
                    .unwrap_or("")
                    .to_string();
                let title = snippet
                    .and_then(|s| s.get("title"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                let avatar = snippet
                    .and_then(|s| s.get("thumbnails"))
                    .and_then(|t| t.get("default"))
                    .and_then(|d| d.get("url"))
                    .and_then(|u| u.as_str())
                    .map(|s| s.to_string());
                items.push(Card::Channel(crate::backend::models::ChannelCard {
                    id: ch_id,
                    title,
                    handle: None,
                    subscribers: None,
                    avatar_url: avatar,
                    verified: false,
                }));
            }
        }

        Ok(FeedPage {
            chips: Vec::new(),
            items,
            continuation: val
                .get("nextPageToken")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string()),
        })
    }

    async fn history(&self) -> Result<FeedPage> {
        Err(anyhow!("Watch history was removed from YouTube API"))
    }

    async fn watch_later(&self) -> Result<FeedPage> {
        Err(anyhow!("Watch later not accessible through YouTube API"))
    }

    async fn playlists(&self) -> Result<FeedPage> {
        let val = self
            .api_get(
                "playlists",
                &[
                    ("part", "snippet,contentDetails"),
                    ("mine", "true"),
                    ("maxResults", "50"),
                ],
                1,
            )
            .await?;

        let mut items = Vec::new();
        if let Some(list) = val.get("items").and_then(|i| i.as_array()) {
            for item in list {
                let id = item
                    .get("id")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();
                let snippet = item.get("snippet");
                let title = snippet
                    .and_then(|s| s.get("title"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                let channel = snippet
                    .and_then(|s| s.get("channelTitle"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();
                let thumb_url = snippet
                    .and_then(|s| s.get("thumbnails"))
                    .and_then(|t| t.get("medium"))
                    .and_then(|m| m.get("url"))
                    .and_then(|u| u.as_str())
                    .unwrap_or("")
                    .to_string();
                let video_count = item
                    .get("contentDetails")
                    .and_then(|c| c.get("itemCount"))
                    .map(|cnt| cnt.to_string());

                items.push(Card::Playlist(crate::backend::models::PlaylistCard {
                    id,
                    title,
                    channel,
                    video_count,
                    thumb_url,
                    is_mix: false,
                    first_video_id: None,
                }));
            }
        }

        Ok(FeedPage {
            chips: Vec::new(),
            items,
            continuation: val
                .get("nextPageToken")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string()),
        })
    }

    async fn playlist(&self, id: &str) -> Result<FeedPage> {
        let val = self
            .api_get(
                "playlistItems",
                &[
                    ("part", "snippet,contentDetails"),
                    ("playlistId", id),
                    ("maxResults", "50"),
                ],
                1,
            )
            .await?;

        let mut items = Vec::new();
        if let Some(list) = val.get("items").and_then(|i| i.as_array()) {
            for item in list {
                let snippet = item.get("snippet");
                let vid_id = snippet
                    .and_then(|s| s.get("resourceId"))
                    .and_then(|r| r.get("videoId"))
                    .and_then(|id| id.as_str())
                    .unwrap_or("")
                    .to_string();
                let title = snippet
                    .and_then(|s| s.get("title"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                let channel = snippet
                    .and_then(|s| s.get("videoOwnerChannelTitle"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();
                let thumb_url = snippet
                    .and_then(|s| s.get("thumbnails"))
                    .and_then(|t| t.get("medium"))
                    .and_then(|m| m.get("url"))
                    .and_then(|u| u.as_str())
                    .unwrap_or("")
                    .to_string();

                if !vid_id.is_empty() {
                    items.push(Card::Video(VideoCard {
                        id: vid_id,
                        title,
                        channel,
                        channel_id: None,
                        verified: false,
                        duration: None,
                        views: None,
                        published: None,
                        thumb_url,
                        progress_pct: None,
                        live: LiveState::None,
                        badges: Vec::new(),
                        resume_time_sec: None,
                    }));
                }
            }
        }

        Ok(FeedPage {
            chips: Vec::new(),
            items,
            continuation: val
                .get("nextPageToken")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string()),
        })
    }

    async fn search(
        &self,
        q: &str,
        _filters: &SearchFilters,
        token: Option<&str>,
    ) -> Result<FeedPage> {
        let mut params = vec![
            ("part", "snippet"),
            ("q", q),
            ("maxResults", "25"),
            ("type", "video"),
        ];
        if let Some(tok) = token {
            params.push(("pageToken", tok));
        }

        let val = self.api_get("search", &params, 100).await?;
        let mut items = Vec::new();

        if let Some(list) = val.get("items").and_then(|i| i.as_array()) {
            for item in list {
                let vid_id = item
                    .get("id")
                    .and_then(|id| id.get("videoId"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let snippet = item.get("snippet");
                let title = snippet
                    .and_then(|s| s.get("title"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                let channel = snippet
                    .and_then(|s| s.get("channelTitle"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();
                let thumb_url = snippet
                    .and_then(|s| s.get("thumbnails"))
                    .and_then(|t| t.get("medium"))
                    .and_then(|m| m.get("url"))
                    .and_then(|u| u.as_str())
                    .unwrap_or("")
                    .to_string();

                if !vid_id.is_empty() {
                    items.push(Card::Video(VideoCard {
                        id: vid_id,
                        title,
                        channel,
                        channel_id: None,
                        verified: false,
                        duration: None,
                        views: None,
                        published: None,
                        thumb_url,
                        progress_pct: None,
                        live: LiveState::None,
                        badges: Vec::new(),
                        resume_time_sec: None,
                    }));
                }
            }
        }

        Ok(FeedPage {
            chips: Vec::new(),
            items,
            continuation: val
                .get("nextPageToken")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string()),
        })
    }

    async fn suggest(&self, _q: &str) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    async fn watch(&self, video_id: &str) -> Result<WatchData> {
        let val = self
            .api_get(
                "videos",
                &[
                    ("part", "snippet,contentDetails,statistics"),
                    ("id", video_id),
                ],
                1,
            )
            .await?;

        let first = val
            .get("items")
            .and_then(|i| i.as_array())
            .and_then(|arr| arr.first())
            .ok_or_else(|| anyhow!("Video not found"))?;
        let snippet = first.get("snippet");
        let title = snippet
            .and_then(|s| s.get("title"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        let channel = snippet
            .and_then(|s| s.get("channelTitle"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();
        let description = snippet
            .and_then(|s| s.get("description"))
            .and_then(|d| d.as_str())
            .unwrap_or("")
            .to_string();
        let thumb_url = snippet
            .and_then(|s| s.get("thumbnails"))
            .and_then(|t| t.get("medium"))
            .and_then(|m| m.get("url"))
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        let views = first
            .get("statistics")
            .and_then(|s| s.get("viewCount"))
            .and_then(|v| v.as_str())
            .map(|cnt| format!("{cnt} views"));

        let video = VideoCard {
            id: video_id.to_string(),
            title,
            channel,
            channel_id: None,
            verified: false,
            duration: None,
            views,
            published: None,
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
        })
    }

    async fn channel(&self, id: &str, _tab: ChannelTab) -> Result<FeedPage> {
        let val = self
            .api_get("channels", &[("part", "contentDetails"), ("id", id)], 1)
            .await?;

        let uploads_id = val
            .pointer("/items/0/contentDetails/relatedPlaylists/uploads")
            .and_then(|s| s.as_str())
            .ok_or_else(|| anyhow!("Uploads playlist not found for channel"))?;

        self.playlist(uploads_id).await
    }

    async fn live_chat(&self, _token: &str) -> Result<ChatBatch> {
        Err(anyhow!("Official API live chat requires active liveChatId"))
    }

    async fn mark_watched(&self, _id: &str) -> Result<()> {
        Ok(())
    }
}
