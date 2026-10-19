use crate::backend::auth::AuthContext;
use crate::backend::models::{
    ChannelTab, ChatBatch, ChatMessage, FeedPage, SearchFilters, WatchData,
};
use crate::backend::parse::{extract_text, parse_feed_page, parse_watch_data};
use crate::backend::Backend;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

const INNERTUBE_BASE: &str = "https://www.youtube.com/youtubei/v1";

pub struct InnerTubeBackend {
    client: Client,
    pub auth: AuthContext,
}

impl InnerTubeBackend {
    pub fn new(auth: AuthContext) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self { client, auth }
    }

    async fn post_endpoint(&self, endpoint: &str, body: Value) -> Result<Value> {
        let url = format!("{INNERTUBE_BASE}/{endpoint}");
        let headers = self.auth.build_headers();

        let resp = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .with_context(|| format!("InnerTube request to {endpoint} failed"))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(anyhow!("InnerTube returned HTTP {status} for {endpoint}"));
        }

        let json_val: Value = resp
            .json()
            .await
            .with_context(|| format!("Failed to parse JSON response from {endpoint}"))?;

        Ok(json_val)
    }

    fn make_body(&self, extra: Value) -> Value {
        let mut base = json!({
            "context": self.auth.build_client_context()
        });

        if let (Some(base_map), Some(extra_map)) = (base.as_object_mut(), extra.as_object()) {
            for (k, v) in extra_map {
                base_map.insert(k.clone(), v.clone());
            }
        }
        base
    }
}

#[async_trait]
impl Backend for InnerTubeBackend {
    async fn home(&self, chip_token: Option<&str>) -> Result<FeedPage> {
        let body = if let Some(token) = chip_token {
            self.make_body(json!({
                "continuation": token
            }))
        } else {
            self.make_body(json!({
                "browseId": "FEwhat_to_watch"
            }))
        };

        let val = self.post_endpoint("browse", body).await?;
        let page = parse_feed_page(&val);

        if page.items.is_empty() && chip_token.is_none() {
            return Err(anyhow!("InnerTube home feed returned 0 items, falling back to recommendation extractor"));
        }

        Ok(page)
    }

    async fn continue_feed(&self, token: &str) -> Result<FeedPage> {
        let body = self.make_body(json!({
            "continuation": token
        }));
        let val = self.post_endpoint("browse", body).await?;
        let page = parse_feed_page(&val);
        Ok(page)
    }

    async fn subscriptions(&self) -> Result<FeedPage> {
        let body = self.make_body(json!({
            "browseId": "FEsubscriptions"
        }));
        let val = self.post_endpoint("browse", body).await?;
        let page = parse_feed_page(&val);
        Ok(page)
    }

    async fn history(&self) -> Result<FeedPage> {
        let body = self.make_body(json!({
            "browseId": "FEhistory"
        }));
        let val = self.post_endpoint("browse", body).await?;
        let page = parse_feed_page(&val);
        Ok(page)
    }

    async fn watch_later(&self) -> Result<FeedPage> {
        let body = self.make_body(json!({
            "browseId": "VLWL"
        }));
        let val = self.post_endpoint("browse", body).await?;
        let page = parse_feed_page(&val);
        Ok(page)
    }

    async fn playlists(&self) -> Result<FeedPage> {
        let body = self.make_body(json!({
            "browseId": "FElibrary"
        }));
        let val = self.post_endpoint("browse", body).await?;
        let page = parse_feed_page(&val);
        Ok(page)
    }

    async fn playlist(&self, id: &str) -> Result<FeedPage> {
        let browse_id = if id.starts_with("VL") {
            id.to_string()
        } else {
            format!("VL{id}")
        };
        let body = self.make_body(json!({
            "browseId": browse_id
        }));
        let val = self.post_endpoint("browse", body).await?;
        let page = parse_feed_page(&val);
        Ok(page)
    }

    async fn search(&self, q: &str, _filters: &SearchFilters, token: Option<&str>) -> Result<FeedPage> {
        let body = if let Some(tok) = token {
            self.make_body(json!({
                "continuation": tok
            }))
        } else {
            self.make_body(json!({
                "query": q
            }))
        };

        let val = self.post_endpoint("search", body).await?;
        let page = parse_feed_page(&val);
        Ok(page)
    }

    async fn suggest(&self, q: &str) -> Result<Vec<String>> {
        let url = format!(
            "https://suggestqueries-clients6.youtube.com/complete/search?client=youtube&ds=yt&q={}",
            urlencoding::encode(q)
        );

        let resp = self.client.get(&url).send().await?;
        let text = resp.text().await?;

        // Format is typically: window.google.ac.h(["query",[["sugg1",0,[...]],["sugg2",0]]])
        // Or pure JSON if client=youtube-reduced
        let mut results = Vec::new();
        if let Some(start) = text.find('(') {
            if let Some(end) = text.rfind(')') {
                let json_slice = &text[start + 1..end];
                if let Ok(v) = serde_json::from_str::<Value>(json_slice) {
                    if let Some(suggestions) = v.get(1).and_then(|s| s.as_array()) {
                        for item in suggestions {
                            if let Some(s) = item.get(0).and_then(|t| t.as_str()) {
                                results.push(s.to_string());
                            }
                        }
                    }
                }
            }
        }
        Ok(results)
    }

    async fn watch(&self, video_id: &str) -> Result<WatchData> {
        let body = self.make_body(json!({
            "videoId": video_id
        }));
        let val = self.post_endpoint("next", body).await?;
        parse_watch_data(&val)
    }

    async fn channel(&self, id: &str, tab: ChannelTab) -> Result<FeedPage> {
        let tab_param = match tab {
            ChannelTab::Videos => "videos",
            ChannelTab::Live => "streams",
            ChannelTab::Playlists => "playlists",
        };
        let body = self.make_body(json!({
            "browseId": id,
            "params": tab_param
        }));
        let val = self.post_endpoint("browse", body).await?;
        Ok(parse_feed_page(&val))
    }

    async fn live_chat(&self, token: &str) -> Result<ChatBatch> {
        let body = self.make_body(json!({
            "continuation": token
        }));
        let val = self.post_endpoint("live_chat/get_live_chat", body).await?;

        let mut messages = Vec::new();
        let mut next_token = None;
        let mut poll_interval_ms = 3000;

        if let Some(actions) = val.pointer("/continuationContents/liveChatContinuation/actions").and_then(|a| a.as_array()) {
            for act in actions {
                if let Some(renderer) = act.pointer("/addChatItemAction/item/liveChatTextMessageRenderer") {
                    let id = renderer.get("id").and_then(|s| s.as_str()).unwrap_or("").to_string();
                    let author = extract_text(renderer.get("authorName").unwrap_or(&Value::Null));
                    let message = extract_text(renderer.get("message").unwrap_or(&Value::Null));
                    let timestamp_usec = renderer.get("timestampUsec")
                        .and_then(|s| s.as_str())
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                    messages.push(ChatMessage {
                        id,
                        author,
                        author_badges: Vec::new(),
                        message,
                        timestamp_usec,
                    });
                }
            }
        }

        if let Some(cont) = val.pointer("/continuationContents/liveChatContinuation/continuations/0/timedContinuationData") {
            if let Some(tok) = cont.get("continuation").and_then(|s| s.as_str()) {
                next_token = Some(tok.to_string());
            }
            if let Some(ms) = cont.get("timeoutMs").and_then(|t| t.as_u64()) {
                poll_interval_ms = ms;
            }
        }

        Ok(ChatBatch {
            messages,
            next_token,
            poll_interval_ms,
        })
    }

    async fn mark_watched(&self, _id: &str) -> Result<()> {
        // Handled via mpv's ytdl-raw-options mark-watched
        Ok(())
    }

    async fn account_info(&self) -> Result<(String, String)> {
        let body = self.make_body(json!({}));
        let val = self.post_endpoint("account/account_menu", body).await?;

        let header = val.pointer("/actions/0/openPopupAction/popup/multiPageMenuRenderer/header/activeAccountHeaderRenderer");
        if let Some(h) = header {
            let name = h
                .pointer("/accountName/simpleText")
                .and_then(|v| v.as_str())
                .or_else(|| h.pointer("/accountName/runs/0/text").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string();
            let handle = h
                .pointer("/channelHandle/simpleText")
                .and_then(|v| v.as_str())
                .or_else(|| h.pointer("/channelHandle/runs/0/text").and_then(|v| v.as_str()))
                .unwrap_or("")
                .to_string();
            if !name.is_empty() || !handle.is_empty() {
                return Ok((name, handle));
            }
        }

        Err(anyhow!("No active account found in account menu"))
    }
}
