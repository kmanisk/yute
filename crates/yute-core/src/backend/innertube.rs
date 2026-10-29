use crate::backend::auth::AuthContext;
use crate::backend::models::{
    ChannelTab, ChatBatch, ChatMessage, CommentsBatch, FeedPage, SearchFilters, WatchData,
};
use crate::backend::parse::{extract_text, parse_comments, parse_feed_page, parse_watch_data};
use crate::backend::Backend;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

const INNERTUBE_BASE: &str = "https://www.youtube.com/youtubei/v1";

pub struct InnerTubeBackend {
    client: Client,
    pub auth: Arc<Mutex<AuthContext>>,
    history_actions: Mutex<std::collections::HashMap<String, String>>,
    history_tokens: Mutex<std::collections::HashSet<String>>,
}

impl InnerTubeBackend {
    pub fn new(auth: AuthContext) -> Self {
        Self {
            client: crate::http::client(),
            history_actions: Default::default(),
            history_tokens: Default::default(),
            auth: Arc::new(Mutex::new(auth)),
        }
    }

    pub fn from_arc(auth: Arc<Mutex<AuthContext>>) -> Self {
        Self {
            client: crate::http::client(),
            history_actions: Default::default(),
            history_tokens: Default::default(),
            auth,
        }
    }

    pub async fn post_endpoint(&self, endpoint: &str, body: Value) -> Result<Value> {
        use tracing::Instrument;
        let operation = match endpoint {
            "browse" => "browse",
            "next" => "next",
            "search" => "search",
            _ => "other",
        };
        let cancelled = self.auth.lock().unwrap().cancelled.clone();
        let request = self.post_endpoint_inner(endpoint, body);
        let started = std::time::Instant::now();
        let result = async {
            tokio::select! {
                _ = cancelled.cancelled() => Err(crate::accounts::AccountCancelled.into()),
                result = tokio::time::timeout(std::time::Duration::from_secs(30), request) => result.map_err(|_| anyhow!("InnerTube operation timed out; check network and unlock the credential keyring"))?,
            }
        }
        .instrument(tracing::info_span!(target:"yute_perf","innertube_request",operation))
        .await;
        if crate::trace::enabled() {
            crate::trace::duration(
                match operation {
                    "browse" => "innertube_browse",
                    "next" => "innertube_next",
                    "search" => "innertube_search",
                    _ => "innertube_other",
                },
                started.elapsed(),
                1,
            );
            tracing::info!(target:"yute_perf", operation, success=result.is_ok(), "request_completed");
        }
        result
    }
    async fn post_endpoint_inner(&self, endpoint: &str, body: Value) -> Result<Value> {
        let url = format!("{INNERTUBE_BASE}/{endpoint}");
        crate::oauth::ensure_fresh(&self.auth, &self.client, false).await?;
        let mut retry = false;
        let resp = loop {
            let headers = self.auth.lock().unwrap().build_headers();
            let response = self
                .client
                .post(&url)
                .headers(headers)
                .json(&body)
                .send()
                .await
                .with_context(|| format!("InnerTube request to {endpoint} failed"))?;
            if response.status() == reqwest::StatusCode::UNAUTHORIZED
                && !retry
                && self.auth.lock().unwrap().refresh_token.is_some()
            {
                retry = true;
                crate::oauth::ensure_fresh(&self.auth, &self.client, true).await?;
                continue;
            }
            break response;
        };

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            let mut auth = self.auth.lock().unwrap();
            if auth.has_oauth() {
                auth.needs_login = true;
                auth.access_token = None;
                return Err(crate::oauth::NeedsLogin.into());
            }
        }
        if !status.is_success() {
            return Err(anyhow!("InnerTube returned HTTP {status} for {endpoint}"));
        }

        let json_val: Value = resp
            .json()
            .await
            .with_context(|| format!("Failed to parse JSON response from {endpoint}"))?;

        if let Some(error) = json_val.get("error") {
            let code = error.get("code").and_then(Value::as_u64).unwrap_or(0);
            anyhow::bail!("InnerTube rejected {endpoint} request (code {code})");
        }

        if self.auth.lock().unwrap().has_oauth()
            && (crate::backend::parse::find_json_key(&json_val, "loggedOut")
                .and_then(Value::as_bool)
                == Some(true)
                || crate::backend::parse::find_json_key(&json_val, "isSignedIn")
                    .and_then(Value::as_bool)
                    == Some(false))
        {
            let mut auth = self.auth.lock().unwrap();
            auth.needs_login = true;
            auth.access_token = None;
            return Err(crate::oauth::NeedsLogin.into());
        }

        // Capture and persist visitorData from responseContext
        if let Some(vis) = json_val
            .pointer("/responseContext/visitorData")
            .and_then(|v| v.as_str())
        {
            let mut guard = self.auth.lock().unwrap();
            if guard.visitor_data.as_deref() != Some(vis) {
                guard.visitor_data = Some(vis.into());
                if let Some(path) = guard.auth_file_path.clone() {
                    let context = guard.clone();
                    tokio::task::spawn_blocking(move || context.save_auth_file(&path));
                }
            }
        }

        Ok(json_val)
    }

    async fn reel_sequence(&self, token: &str) -> Result<FeedPage> {
        use futures_util::{stream, StreamExt};
        let value = self
            .post_endpoint(
                "reel/reel_watch_sequence",
                self.make_body(json!({"sequenceParams":token})),
            )
            .await?;
        let entries = value
            .get("entries")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
        let results = stream::iter(entries.into_iter().take(10))
            .map(|entry| async move {
                let endpoint = entry.pointer("/command/reelWatchEndpoint").context("Shorts sequence entry has no watch endpoint")?;
                let id = endpoint.get("videoId").and_then(Value::as_str).context("Shorts sequence entry has no video ID")?;
                let params = endpoint.get("params").and_then(Value::as_str).context("Shorts sequence entry has no request parameters")?;
                let value = self
                    .post_endpoint(
                        "reel/reel_item_watch",
                        self.make_body(
                            json!({"playerRequest":{"videoId":id},"params":params,"disablePlayerResponse":true}),
                        ),
                    )
                    .await?;
                crate::backend::parse::parse_reel_card(&value, Some(id)).context("Shorts item response contains no playable card")
            })
            .buffered(2)
            .collect::<Vec<_>>()
            .await;
        let items = results.into_iter().collect::<Result<Vec<_>>>()?;
        let continuation = value
            .get("continuation")
            .and_then(serde_json::Value::as_str)
            .map(|token| format!("yute:reel:{token}"));
        Ok(FeedPage {
            items,
            chips: Vec::new(),
            continuation,
        })
    }

    fn require_account(&self) -> Result<()> {
        let auth = self.auth.lock().unwrap();
        if auth.needs_login {
            return Err(crate::oauth::NeedsLogin.into());
        }
        if !auth.has_oauth() {
            anyhow::bail!("Sign in with :account add");
        }
        Ok(())
    }

    async fn account_feed(&self, browse_id: &str) -> Result<FeedPage> {
        self.require_account()?;
        let value = self
            .post_endpoint("browse", self.make_body(json!({"browseId": browse_id})))
            .await?;
        if browse_id == "FEhistory" {
            self.capture_history_actions(&value);
        }
        self.read_account_feed(&value)
    }
    fn capture_history_actions(&self, value: &Value) {
        fn visit(v: &Value, map: &mut std::collections::HashMap<String, String>) {
            if let Some(id) = v.get("videoId").and_then(Value::as_str) {
                if let Some(token) =
                    crate::backend::parse::find_json_key(v, "feedbackToken").and_then(Value::as_str)
                {
                    map.insert(id.into(), token.into());
                }
            }
            match v {
                Value::Object(m) => {
                    for v in m.values() {
                        visit(v, map)
                    }
                }
                Value::Array(a) => {
                    for v in a {
                        visit(v, map)
                    }
                }
                _ => (),
            }
        }
        visit(value, &mut self.history_actions.lock().unwrap());
        if let Some(token) = crate::backend::parse::parse_continuation(value) {
            self.history_tokens.lock().unwrap().insert(token);
        }
    }

    fn read_account_feed(&self, value: &Value) -> Result<FeedPage> {
        let result = parse_account_feed(value);
        if result
            .as_ref()
            .is_err_and(|error| error.is::<crate::oauth::NeedsLogin>())
        {
            let mut auth = self.auth.lock().unwrap();
            auth.needs_login = true;
            auth.access_token = None;
        }
        result
    }

    pub fn make_body(&self, extra: Value) -> Value {
        let client_ctx = self.auth.lock().unwrap().build_client_context();
        let mut base = json!({
            "context": client_ctx
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
    fn is_anonymous(&self) -> bool {
        let auth = self.auth.lock().unwrap();
        !auth.has_oauth() || auth.needs_login
    }
    async fn subscribed_channels(&self, token: Option<&str>) -> Result<FeedPage> {
        self.require_account()?;
        let extra = if let Some(token) = token {
            json!({"continuation":token})
        } else {
            json!({"browseId":"FEsubscriptions"})
        };
        let value = self.post_endpoint("browse", self.make_body(extra)).await?;
        let mut page = parse_subscribed_channels(&value)?;
        page.continuation = page.continuation.map(|s| format!("yute:channels:{s}"));
        Ok(page)
    }
    async fn remove_from_playlist(&self, playlist: &str, video: &str) -> Result<()> {
        self.require_account()?;
        let response=self.post_endpoint("browse/edit_playlist",self.make_body(json!({"playlistId":playlist.trim_start_matches("VL"),"actions":[{"action":"ACTION_REMOVE_VIDEO_BY_VIDEO_ID","removedVideoId":video}]}))).await?;
        if response.get("status").and_then(Value::as_str) == Some("STATUS_SUCCEEDED") {
            Ok(())
        } else {
            acknowledge_mutation(&response)
        }
    }
    fn can_remove_history(&self, video: &str) -> bool {
        self.history_actions.lock().unwrap().contains_key(video)
    }
    async fn remove_from_history(&self, video: &str) -> Result<()> {
        self.require_account()?;
        let token = self
            .history_actions
            .lock()
            .unwrap()
            .get(video)
            .cloned()
            .context("Reload History to obtain the server removal action")?;
        let response = self
            .post_endpoint(
                "feedback",
                self.make_body(json!({"feedbackTokens":[token]})),
            )
            .await?;
        acknowledge_mutation(&response)
    }
    async fn home(&self, chip_token: Option<&str>) -> Result<FeedPage> {
        let body = if let Some(token) = chip_token {
            self.make_body(json!({
                "continuation": token
            }))
        } else {
            self.make_body(json!({
                "browseId": if self.auth.lock().unwrap().client_name == "TVHTML5" { "default" } else { "FEwhat_to_watch" }
            }))
        };

        let val = self.post_endpoint("browse", body).await?;
        let page = parse_feed_page(&val);
        // An empty/nudged TV Home lets BackendChain try the existing real
        // recommendations fallback. A generic search is not a Home feed.
        Ok(page)
    }

    async fn home_category(
        &self,
        category: crate::backend::models::HomeCategory,
    ) -> Result<FeedPage> {
        if category == crate::backend::models::HomeCategory::Live {
            let value = self
                .post_endpoint(
                    "browse",
                    self.make_body(json!({"browseId":"FEtopics_live"})),
                )
                .await?;
            return Ok(parse_feed_page(&value));
        }
        let value=self.post_endpoint("reel/reel_item_watch",self.make_body(json!({"disablePlayerResponse":true,"inputType":"REEL_WATCH_INPUT_TYPE_SEEDLESS","params":"CA8="}))).await?;
        let first = crate::backend::parse::parse_reel_card(&value, None);
        let mut page = if let Some(token) = value
            .get("sequenceContinuation")
            .and_then(serde_json::Value::as_str)
        {
            self.reel_sequence(token).await?
        } else {
            FeedPage::default()
        };
        if let Some(first) = first {
            page.items.insert(0, first);
        }
        let mut ids = std::collections::HashSet::new();
        page.items.retain(|item| ids.insert(item.id().to_string()));
        Ok(page)
    }

    async fn continue_feed(&self, token: &str) -> Result<FeedPage> {
        if let Some(token) = token.strip_prefix("yute:account:") {
            self.require_account()?;
            let value = self
                .post_endpoint("browse", self.make_body(json!({"continuation":token})))
                .await?;
            if self.history_tokens.lock().unwrap().contains(token) {
                self.capture_history_actions(&value);
            }
            return self.read_account_feed(&value);
        }
        if let Some(token) = token.strip_prefix("yute:reel:") {
            return self.reel_sequence(token).await;
        }
        let body = self.make_body(json!({
            "continuation": token
        }));
        let val = self.post_endpoint("browse", body).await?;
        let page = parse_feed_page(&val);
        Ok(page)
    }

    async fn subscriptions(&self) -> Result<FeedPage> {
        self.account_feed("FEsubscriptions").await
    }

    async fn history(&self) -> Result<FeedPage> {
        self.account_feed("FEhistory").await
    }

    async fn watch_later(&self) -> Result<FeedPage> {
        self.account_feed("VLWL").await
    }

    async fn playlist_destinations(&self, video: &str) -> Result<FeedPage> {
        self.require_account()?;
        let response = self
            .post_endpoint(
                "playlist/get_add_to_playlist",
                self.make_body(json!({"videoIds":[video]})),
            )
            .await?;
        Ok(crate::backend::parse::parse_playlist_destinations(
            &response,
        ))
    }
    async fn save_to_playlist(&self, playlist: &str, video: &str) -> Result<()> {
        self.require_account()?;
        let response=self.post_endpoint("browse/edit_playlist",self.make_body(json!({"playlistId":playlist.trim_start_matches("VL"),"actions":[{"action":"ACTION_ADD_VIDEO","addedVideoId":video}]}))).await?;
        if response.get("status").and_then(Value::as_str) == Some("STATUS_SUCCEEDED")
            || crate::backend::parse::find_json_key(&response, "playlistEditVideoAddedResultData")
                .is_some()
        {
            Ok(())
        } else {
            anyhow::bail!("YouTube did not confirm this playlist update")
        }
    }
    async fn playlists(&self) -> Result<FeedPage> {
        self.account_feed("FElibrary").await
    }

    async fn playlist(&self, id: &str) -> Result<FeedPage> {
        if matches!(id, "LL" | "WL" | "VLLL" | "VLWL") {
            return self
                .account_feed(&format!("VL{}", id.trim_start_matches("VL")))
                .await;
        }
        if id.starts_with("RD") {
            let mut body = self.make_body(json!({"playlistId":id,"playlistIndex":0}));
            if let Some(seed) = crate::backend::models::mix_seed_video_id(id) {
                body["videoId"] = seed.into();
            }
            let value = self.post_endpoint("next", body).await?;
            let page = crate::backend::parse::parse_mix_page(&value, id);
            if page.items.is_empty() {
                anyhow::bail!("YouTube did not return playable items for this Mix");
            }
            return Ok(page);
        }
        let browse_id = if id.starts_with("VL") {
            id.to_string()
        } else {
            format!("VL{id}")
        };
        let body = self.make_body(json!({
            "browseId": browse_id
        }));
        let val = self.post_endpoint("browse", body).await?;
        if self.auth.lock().unwrap().has_oauth() {
            self.read_account_feed(&val)
        } else {
            Ok(parse_feed_page(&val))
        }
    }

    async fn search(
        &self,
        q: &str,
        _filters: &SearchFilters,
        token: Option<&str>,
    ) -> Result<FeedPage> {
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
        let mut page = parse_feed_page(&val);

        if token.is_none() {
            if let Some(ref cont_token) = page.continuation {
                let cont_body = self.make_body(json!({
                    "continuation": cont_token
                }));
                if let Ok(next_val) = self.post_endpoint("search", cont_body).await {
                    let next_page = parse_feed_page(&next_val);
                    page.items.extend(next_page.items);
                    page.continuation = next_page.continuation;
                }
            }
        }

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

    async fn video_metadata(&self, video_id: &str) -> Result<crate::backend::models::VideoCard> {
        let value = self
            .post_endpoint("next", self.make_body(json!({"videoId":video_id})))
            .await?;
        let mut video = parse_watch_data(&value)?.video;
        if video.id.is_empty() {
            video.id = video_id.to_string();
        }
        Ok(video)
    }

    async fn watch(&self, video_id: &str) -> Result<WatchData> {
        let body = self.make_body(json!({
            "videoId": video_id
        }));
        let val = self.post_endpoint("next", body).await?;
        let mut watch_data = parse_watch_data(&val)?;

        if watch_data.video.id.is_empty() {
            watch_data.video.id = video_id.to_string();
        }

        // Return the initial metadata promptly. The presentation requests comments
        // separately; optional third-party vote lookups must not block Watch opening.
        watch_data.up_next.retain(|card| card.id() != video_id);
        Ok(watch_data)
    }

    async fn channel(&self, id: &str, tab: ChannelTab) -> Result<FeedPage> {
        let tab_param = match tab {
            ChannelTab::Videos => "EgZ2aWRlb3PyBgQKAjoA",
            ChannelTab::Live => "EgdzdHJlYW1z8gYECgJ6AA",
            ChannelTab::Playlists => "EglwbGF5bGlzdHPyBgQKAkIA",
        };
        let body = self.make_body(json!({
            "browseId": id,
            "params": tab_param
        }));
        let val = self.post_endpoint("browse", body).await?;
        Ok(parse_feed_page(&val))
    }

    async fn channel_page(
        &self,
        id: &str,
        params: Option<&str>,
    ) -> Result<crate::backend::models::ChannelPage> {
        let mut extra = json!({ "browseId": id });
        if let Some(params) = params {
            extra["params"] = json!(params);
        }
        let value = self.post_endpoint("browse", self.make_body(extra)).await?;
        let mut page = crate::backend::parse::parse_channel_page(&value, id);
        if id == crate::backend::models::SUBSCRIPTIONS_BROWSE_ID && params.is_some() {
            // These are sibling subscribed channels, not tabs of the selected channel.
            page.tabs.clear();
        }
        Ok(page)
    }

    async fn live_chat(&self, token: &str) -> Result<ChatBatch> {
        let body = self.make_body(json!({
            "continuation": token
        }));
        let val = self.post_endpoint("live_chat/get_live_chat", body).await?;

        let mut messages = Vec::new();
        let mut next_token = None;
        let mut poll_interval_ms = 3000;

        if let Some(actions) = val
            .pointer("/continuationContents/liveChatContinuation/actions")
            .and_then(|a| a.as_array())
        {
            for act in actions {
                if let Some(renderer) =
                    act.pointer("/addChatItemAction/item/liveChatTextMessageRenderer")
                {
                    let id = renderer
                        .get("id")
                        .and_then(|s| s.as_str())
                        .unwrap_or("")
                        .to_string();
                    let author = extract_text(renderer.get("authorName").unwrap_or(&Value::Null));
                    let message = extract_text(renderer.get("message").unwrap_or(&Value::Null));
                    let timestamp_usec = renderer
                        .get("timestampUsec")
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

        if let Some(cont) = val.pointer(
            "/continuationContents/liveChatContinuation/continuations/0/timedContinuationData",
        ) {
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

    async fn ping_playback(&self, id: &str, cpn: &str) -> Result<()> {
        let token = {
            let guard = self.auth.lock().unwrap();
            guard.access_token.clone()
        };

        if let Some(tok) = token {
            let playback_url = format!(
                "https://www.youtube.com/api/stats/playback?ns=yt&ver=2&docid={id}&cpn={cpn}"
            );
            let response = self
                .client
                .get(&playback_url)
                .header("Authorization", format!("Bearer {}", tok.expose()))
                .header("User-Agent", "Mozilla/5.0")
                .send()
                .await
                .map_err(|_| anyhow!("Playback tracking network request failed"))?;
            if !response.status().is_success() {
                anyhow::bail!(
                    "Playback tracking returned HTTP {}",
                    response.status().as_u16()
                );
            }
        }

        Ok(())
    }

    async fn ping_watchtime(
        &self,
        id: &str,
        cpn: &str,
        st: f64,
        et: f64,
        cmt: f64,
        is_final: bool,
    ) -> Result<()> {
        let token = {
            let guard = self.auth.lock().unwrap();
            guard.access_token.clone()
        };

        if let Some(tok) = token {
            let final_flag = if is_final { "&final=1" } else { "" };
            let watchtime_url = format!(
                "https://www.youtube.com/api/stats/watchtime?ns=yt&ver=2&docid={id}&cpn={cpn}&st={st:.1}&et={et:.1}&cmt={cmt:.1}{final_flag}"
            );
            let response = self
                .client
                .get(&watchtime_url)
                .header("Authorization", format!("Bearer {}", tok.expose()))
                .header("User-Agent", "Mozilla/5.0")
                .send()
                .await
                .map_err(|_| anyhow!("Playback tracking network request failed"))?;
            if !response.status().is_success() {
                anyhow::bail!(
                    "Playback tracking returned HTTP {}",
                    response.status().as_u16()
                );
            }
        }

        Ok(())
    }

    async fn mark_watched(&self, id: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let cpn = format!("{:016x}", now);
        self.ping_playback(id, &cpn).await?;
        self.ping_watchtime(id, &cpn, 0.0, 30.0, 30.0, true).await?;
        Ok(())
    }

    async fn account_info(&self) -> Result<(String, String)> {
        self.require_account()?;
        // Try TVHTML5 accounts_list
        let tv_body = self.make_body(json!({
            "accountReadMask": {
                "returnOwner": true,
                "returnBrandAccounts": true,
                "returnPersonaAccounts": false
            }
        }));
        if let Ok(val) = self.post_endpoint("account/accounts_list", tv_body).await {
            if let Some(actions) = val.get("actions").and_then(|a| a.as_array()) {
                for act in actions {
                    if let Some(items) = act.pointer("/getMultiPageMenuAction/menu/multiPageMenuRenderer/sections/0/accountSectionRenderer/items").and_then(|i| i.as_array()) {
                        for item in items {
                            if let Some(acc) = item.get("accountItem") {
                                let name = acc.pointer("/accountName/simpleText").and_then(|s| s.as_str()).unwrap_or("");
                                let handle = acc.pointer("/channelHandle/simpleText").and_then(|s| s.as_str()).unwrap_or("");
                                if !name.is_empty() {
                                    return Ok((name.to_string(), handle.to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Fallback to account_menu
        let body = self.make_body(json!({}));
        let val = self.post_endpoint("account/account_menu", body).await?;

        let header = val.pointer("/actions/0/openPopupAction/popup/multiPageMenuRenderer/header/activeAccountHeaderRenderer");
        if let Some(h) = header {
            let name = h
                .pointer("/accountName/simpleText")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    h.pointer("/accountName/runs/0/text")
                        .and_then(|v| v.as_str())
                })
                .unwrap_or("")
                .to_string();
            let handle = h
                .pointer("/channelHandle/simpleText")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    h.pointer("/channelHandle/runs/0/text")
                        .and_then(|v| v.as_str())
                })
                .unwrap_or("")
                .to_string();
            if !name.is_empty() || !handle.is_empty() {
                return Ok((name, handle));
            }
        }

        Err(anyhow!("No active account found in account menu"))
    }

    async fn like_video(&self, video_id: &str) -> Result<bool> {
        self.require_account()?;
        let body = self.make_body(json!({
            "target": {
                "videoId": video_id
            }
        }));
        let resp = self.post_endpoint("like/like", body).await?;
        if let Some(mutations) = resp
            .pointer("/frameworkUpdates/entityBatchUpdate/mutations")
            .and_then(|m| m.as_array())
        {
            for m in mutations {
                if let Some(status) = m
                    .pointer("/payload/likeStatusEntity/likeStatus")
                    .and_then(|s| s.as_str())
                {
                    if status == "LIKE" {
                        return Ok(true);
                    }
                    anyhow::bail!("YouTube did not confirm the liked state");
                }
            }
        }
        if resp.get("responseContext").is_some() {
            return Ok(true);
        }
        anyhow::bail!("YouTube did not acknowledge the rating update")
    }

    async fn dislike_video(&self, video_id: &str) -> Result<bool> {
        self.require_account()?;
        let body = self.make_body(json!({
            "target": {
                "videoId": video_id
            }
        }));
        let resp = self.post_endpoint("like/dislike", body).await?;
        if let Some(mutations) = resp
            .pointer("/frameworkUpdates/entityBatchUpdate/mutations")
            .and_then(|m| m.as_array())
        {
            for m in mutations {
                if let Some(status) = m
                    .pointer("/payload/likeStatusEntity/likeStatus")
                    .and_then(|s| s.as_str())
                {
                    if status == "DISLIKE" {
                        return Ok(true);
                    }
                    anyhow::bail!("YouTube did not confirm the disliked state");
                }
            }
        }
        if resp.get("responseContext").is_some() {
            return Ok(true);
        }
        anyhow::bail!("YouTube did not acknowledge the rating update")
    }

    async fn remove_rating(&self, video_id: &str) -> Result<()> {
        self.require_account()?;
        let body = self.make_body(json!({
            "target": {
                "videoId": video_id
            }
        }));
        let response = self
            .post_endpoint(rating_remove_endpoint(false), body)
            .await?;
        acknowledge_mutation(&response)
    }

    async fn remove_dislike(&self, video_id: &str) -> Result<()> {
        self.require_account()?;
        let body = self.make_body(json!({
            "target": {
                "videoId": video_id
            }
        }));
        let response = self
            .post_endpoint(rating_remove_endpoint(true), body)
            .await?;
        acknowledge_mutation(&response)
    }

    async fn subscribe_channel(&self, channel_id: &str) -> Result<()> {
        self.require_account()?;
        let body = self.make_body(json!({
            "channelIds": [channel_id]
        }));
        let response = self.post_endpoint("subscription/subscribe", body).await?;
        acknowledge_mutation(&response)
    }

    async fn unsubscribe_channel(&self, channel_id: &str) -> Result<()> {
        self.require_account()?;
        let body = self.make_body(json!({
            "channelIds": [channel_id]
        }));
        let response = self.post_endpoint("subscription/unsubscribe", body).await?;
        acknowledge_mutation(&response)
    }

    async fn fetch_ryd_votes(&self, video_id: &str) -> Result<(u64, u64, f64)> {
        let url = format!("https://returnyoutubedislikeapi.com/votes?videoId={video_id}");
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("RYD returned HTTP {}", resp.status()));
        }
        let val: Value = resp.json().await?;
        let likes = val.get("likes").and_then(|v| v.as_u64()).unwrap_or(0);
        let dislikes = val.get("dislikes").and_then(|v| v.as_u64()).unwrap_or(0);
        let rating = val.get("rating").and_then(|v| v.as_f64()).unwrap_or(0.0);
        Ok((likes, dislikes, rating))
    }

    async fn comment_action(&self, action: &str) -> Result<()> {
        self.require_account()?;
        let response = self
            .post_endpoint(
                "comment/perform_comment_action",
                self.make_body(json!({"actions":[action]})),
            )
            .await?;
        if response.get("responseContext").is_none() {
            anyhow::bail!("YouTube did not acknowledge the comment reaction");
        }
        Ok(())
    }
    async fn react_comment(&self, detail_token: &str, dislike: bool) -> Result<()> {
        self.require_account()?;
        let detail = self
            .post_endpoint(
                "next",
                self.make_body(json!({"continuation": detail_token})),
            )
            .await?;
        let action = comment_detail_reaction_action(&detail, dislike)
            .context("YouTube did not expose this comment reaction")?;
        self.comment_action(&action).await
    }
    async fn submit_comment(&self, params: &str, text: &str, reply: bool) -> Result<()> {
        self.require_account()?;
        if text.trim().is_empty() || text.chars().count() > 10000 {
            anyhow::bail!("Comment must contain 1 to 10000 characters");
        }
        let (endpoint, field) = if reply {
            ("comment/create_comment_reply", "createReplyParams")
        } else {
            ("comment/create_comment", "createCommentParams")
        };
        let mut body = self.make_body(json!({"commentText":text}));
        body[field] = params.into();
        let response = self.post_endpoint(endpoint, body).await?;
        if crate::backend::parse::find_json_key(&response, "commentRenderer").is_none()
            && crate::backend::parse::find_json_key(&response, "commentEntityPayload").is_none()
            && crate::backend::parse::find_json_key(&response, "createCommentAction").is_none()
            && crate::backend::parse::find_json_key(&response, "createCommentReplyAction").is_none()
        {
            anyhow::bail!("YouTube did not confirm the posted comment");
        }
        Ok(())
    }

    async fn fetch_comments(&self, continuation: &str) -> Result<CommentsBatch> {
        let body = self.make_body(json!({
            "continuation": continuation
        }));
        let val = self.post_endpoint("next", body).await?;
        Ok(parse_comments(&val))
    }
}

fn rating_remove_endpoint(disliked: bool) -> &'static str {
    if disliked {
        "like/removedislike"
    } else {
        "like/removelike"
    }
}

fn comment_detail_reaction_action(value: &Value, dislike: bool) -> Option<String> {
    fn active_comment(value: &Value) -> Option<&Value> {
        if let Some(comment) = value.get("commentRenderer") {
            if comment
                .pointer("/actionButtons/commentActionButtonsRenderer")
                .is_some()
            {
                return Some(comment);
            }
        }
        match value {
            Value::Object(map) => map.values().find_map(active_comment),
            Value::Array(items) => items.iter().find_map(active_comment),
            _ => None,
        }
    }
    fn action(endpoint: Option<&Value>) -> Option<String> {
        endpoint
            .and_then(|value| {
                crate::backend::parse::find_json_key(value, "performCommentActionEndpoint")
            })
            .and_then(|value| value.get("action"))
            .and_then(Value::as_str)
            .map(str::to_string)
    }
    let comment = active_comment(value)?;
    let button = comment
        .pointer("/actionButtons/commentActionButtonsRenderer")?
        .get(if dislike {
            "dislikeButton"
        } else {
            "likeButton"
        })?;
    let renderer = crate::backend::parse::find_json_key(button, "toggleButtonRenderer")?;
    let toggled = if dislike {
        renderer
            .get("isToggled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    } else {
        comment
            .get("isLiked")
            .and_then(Value::as_bool)
            .or_else(|| renderer.get("isToggled").and_then(Value::as_bool))
            .unwrap_or(false)
    };
    let key = if toggled {
        "toggledServiceEndpoint"
    } else {
        "defaultServiceEndpoint"
    };
    action(renderer.get(key))
}

/// Empty account pages need an explicit empty renderer/container. Unknown or
/// signed-out responses are failures, never evidence of an empty library.
fn acknowledge_mutation(response: &Value) -> Result<()> {
    if response.get("responseContext").is_some() && response.get("error").is_none() {
        Ok(())
    } else {
        anyhow::bail!("YouTube did not acknowledge this account update")
    }
}

fn parse_account_feed(value: &Value) -> Result<FeedPage> {
    use crate::backend::parse::find_json_key;
    if find_json_key(value, "loggedOut").and_then(Value::as_bool) == Some(true)
        || find_json_key(value, "isSignedIn").and_then(Value::as_bool) == Some(false)
    {
        return Err(crate::oauth::NeedsLogin.into());
    }
    if find_json_key(value, "alertRenderer").is_some()
        || find_json_key(value, "backgroundPromoRenderer").is_some()
    {
        anyhow::bail!("YouTube rejected this account feed; use :account add or retry");
    }
    let mut page = parse_feed_page(value);
    page.continuation = page
        .continuation
        .map(|token| format!("yute:account:{token}"));
    if !page.items.is_empty() || page.continuation.is_some() {
        return Ok(page);
    }
    let explicit_empty = ["emptyContentRenderer"]
        .iter()
        .any(|key| find_json_key(value, key).is_some())
        || [
            "gridRenderer",
            "richGridRenderer",
            "playlistVideoListRenderer",
            "horizontalListRenderer",
        ]
        .iter()
        .any(|key| {
            find_json_key(value, key)
                .and_then(|v| v.get("items").or_else(|| v.get("contents")))
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty)
        });
    if explicit_empty {
        Ok(page)
    } else {
        anyhow::bail!("YouTube returned an unrecognized account feed; press r to retry")
    }
}

#[cfg(test)]
mod account_feed_tests {
    use super::*;
    #[test]
    fn account_responses_distinguish_empty_error_and_tv_continuation() {
        assert!(parse_account_feed(&json!({"responseContext":{}})).is_err());
        assert!(parse_account_feed(&json!({"responseContext":{"mainAppWebResponseContext":{"loggedOut":true}},"contents":{"messageRenderer":{"text":{"simpleText":"Sign in"}}}})).is_err());
        assert!(parse_account_feed(&json!({"alertRenderer":{"type":"ERROR"}})).is_err());
        assert!(
            parse_account_feed(&json!({"contents":{"gridRenderer":{"items":[]}}}))
                .unwrap()
                .items
                .is_empty()
        );
        let response = json!({"continuationContents":{"gridContinuation":{"items":[{"gridVideoRenderer":{"videoId":"abcdefghijk","title":{"simpleText":"Subscribed video"}}}],"continuations":[{"nextContinuationData":{"continuation":"next-subs-page"}}]}}});
        let page = parse_account_feed(&response).unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(
            page.continuation.as_deref(),
            Some("yute:account:next-subs-page")
        );
    }
    #[test]
    fn rating_removal_uses_distinct_like_and_dislike_endpoints() {
        assert_eq!(rating_remove_endpoint(false), "like/removelike");
        assert_eq!(rating_remove_endpoint(true), "like/removedislike");
    }

    #[test]
    fn comment_detail_reactions_follow_active_toggle_endpoints() {
        let value = json!({
            "commentRenderer": {
                "isLiked": true,
                "actionButtons": {"commentActionButtonsRenderer": {
                    "likeButton": {"toggleButtonRenderer": {
                        "isToggled": true,
                        "defaultServiceEndpoint": {"performCommentActionEndpoint": {"action": "like"}},
                        "toggledServiceEndpoint": {"performCommentActionEndpoint": {"action": "unlike"}}
                    }},
                    "dislikeButton": {"toggleButtonRenderer": {
                        "isToggled": false,
                        "defaultServiceEndpoint": {"performCommentActionEndpoint": {"action": "dislike"}},
                        "toggledServiceEndpoint": {"performCommentActionEndpoint": {"action": "undislike"}}
                    }}
                }}
            }
        });
        assert_eq!(
            comment_detail_reaction_action(&value, false).as_deref(),
            Some("unlike")
        );
        assert_eq!(
            comment_detail_reaction_action(&value, true).as_deref(),
            Some("dislike")
        );
    }

    #[tokio::test]
    async fn legacy_session_cannot_fetch_account_libraries() {
        let auth = AuthContext {
            needs_login: true,
            cookie_header: Some("legacy".into()),
            ..Default::default()
        };
        let backend = InnerTubeBackend::new(auth);
        assert!(backend
            .subscriptions()
            .await
            .unwrap_err()
            .is::<crate::oauth::NeedsLogin>());
        assert!(backend.history().await.is_err());
        assert!(backend.watch_later().await.is_err());
        assert!(backend.playlists().await.is_err());
        assert!(backend.playlist("LL").await.is_err());
    }
}

/// TV subscriptions expose channel tabs, independently of Home video shelves.
pub fn parse_subscribed_channels(value: &Value) -> Result<FeedPage> {
    use crate::backend::models::{
        subscription_tab_card_id, Card, ChannelCard, SUBSCRIPTIONS_BROWSE_ID,
    };
    if crate::backend::parse::find_json_key(value, "loggedOut").and_then(Value::as_bool)
        == Some(true)
    {
        return Err(crate::oauth::NeedsLogin.into());
    }
    let mut items = vec![];
    let mut ids = std::collections::HashSet::new();
    fn visit(v: &Value, out: &mut Vec<Card>, ids: &mut std::collections::HashSet<String>) {
        if let Some(Card::Channel(c)) = crate::backend::parse::parse_card(v) {
            if ids.insert(c.id.clone()) {
                out.push(Card::Channel(c));
            }
        }
        if let Some(tab) = v.get("tabRenderer") {
            let endpoint = tab
                .get("endpoint")
                .or_else(|| tab.get("navigationEndpoint"));
            let browse = endpoint.and_then(|e| e.get("browseEndpoint"));
            let browse_id = browse
                .and_then(|e| e.get("browseId"))
                .and_then(Value::as_str);
            let params = browse.and_then(|e| e.get("params")).and_then(Value::as_str);
            let title = tab
                .get("title")
                .map(crate::backend::parse::extract_text)
                .unwrap_or_default();
            let avatar = crate::backend::parse::extract_best_thumbnail(tab);

            // Older TV responses advertised a normal UC endpoint. Current TV
            // responses advertise subscribed channels as FEsubscriptions tabs with
            // per-channel opaque params. Both are real browse endpoints.
            let identity = browse_id
                .filter(|id| id.starts_with("UC"))
                .map(str::to_owned)
                .or_else(|| {
                    (browse_id == Some(SUBSCRIPTIONS_BROWSE_ID)
                        && params.is_some()
                        && !title.is_empty()
                        && !avatar.is_empty())
                    .then(|| subscription_tab_card_id(params.unwrap_or_default()))
                });
            if let Some(id) = identity {
                if ids.insert(id.clone()) {
                    out.push(Card::Channel(ChannelCard {
                        id,
                        title,
                        handle: None,
                        subscribers: None,
                        avatar_url: (!avatar.is_empty()).then_some(avatar),
                        verified: false,
                    }));
                }
            }
        }
        match v {
            Value::Object(m) => {
                for child in m.values() {
                    visit(child, out, ids);
                }
            }
            Value::Array(a) => {
                for child in a {
                    visit(child, out, ids);
                }
            }
            _ => {}
        }
    }
    visit(value, &mut items, &mut ids);
    let continuation = crate::backend::parse::parse_continuation(value);
    if items.is_empty()
        && crate::backend::parse::find_json_key(value, "emptyContentRenderer").is_none()
    {
        return Err(anyhow!(
            "YouTube did not return recognizable subscribed channel tabs"
        ));
    }
    Ok(FeedPage {
        items,
        chips: vec![],
        continuation,
    })
}
