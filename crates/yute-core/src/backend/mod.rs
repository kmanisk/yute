pub mod api;
pub mod auth;
pub mod innertube;
pub mod models;
pub mod parse;
pub mod rss;
pub mod sponsorblock;
pub mod ytdlp;

use async_trait::async_trait;
use models::{ChannelTab, ChatBatch, CommentsBatch, FeedPage, SearchFilters, WatchData};

#[async_trait]
pub trait Backend: Send + Sync {
    fn is_anonymous(&self) -> bool {
        false
    }
    async fn home(&self, chip_token: Option<&str>) -> anyhow::Result<FeedPage>;
    async fn home_category(&self, _category: models::HomeCategory) -> anyhow::Result<FeedPage> {
        anyhow::bail!("This YouTube section is unavailable from the current backend")
    }
    async fn continue_feed(&self, token: &str) -> anyhow::Result<FeedPage>;
    async fn subscriptions(&self) -> anyhow::Result<FeedPage>;
    async fn history(&self) -> anyhow::Result<FeedPage>;
    async fn watch_later(&self) -> anyhow::Result<FeedPage>;
    async fn playlists(&self) -> anyhow::Result<FeedPage>;
    async fn playlist(&self, id: &str) -> anyhow::Result<FeedPage>;
    async fn playlist_destinations(&self, _video: &str) -> anyhow::Result<FeedPage> {
        anyhow::bail!("Playlist destinations unavailable")
    }
    async fn save_to_playlist(&self, _playlist: &str, _video: &str) -> anyhow::Result<()> {
        anyhow::bail!("Saving playlists is unavailable from this backend")
    }
    async fn search(
        &self,
        q: &str,
        filters: &SearchFilters,
        token: Option<&str>,
    ) -> anyhow::Result<FeedPage>;
    async fn suggest(&self, q: &str) -> anyhow::Result<Vec<String>>;
    async fn watch(&self, video_id: &str) -> anyhow::Result<WatchData>;
    /// Lightweight card metadata; never resolves playback or fetches comments/votes.
    async fn video_metadata(&self, _video_id: &str) -> anyhow::Result<models::VideoCard> {
        anyhow::bail!("Card metadata is unavailable from this backend")
    }
    async fn channel(&self, id: &str, tab: ChannelTab) -> anyhow::Result<FeedPage>;
    async fn channel_page(
        &self,
        _id: &str,
        _params: Option<&str>,
    ) -> anyhow::Result<models::ChannelPage> {
        anyhow::bail!("Channel details are unavailable from this backend")
    }

    async fn live_chat(&self, token: &str) -> anyhow::Result<ChatBatch>;
    async fn mark_watched(&self, id: &str) -> anyhow::Result<()>;
    async fn ping_playback(&self, _id: &str, _cpn: &str) -> anyhow::Result<()> {
        Ok(())
    }
    async fn ping_watchtime(
        &self,
        _id: &str,
        _cpn: &str,
        _st: f64,
        _et: f64,
        _cmt: f64,
        _is_final: bool,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    async fn account_info(&self) -> anyhow::Result<(String, String)> {
        anyhow::bail!("Account info not supported by this backend")
    }
    async fn like_video(&self, _video_id: &str) -> anyhow::Result<bool> {
        anyhow::bail!("Like not supported by this backend")
    }
    async fn dislike_video(&self, _video_id: &str) -> anyhow::Result<bool> {
        anyhow::bail!("Dislike not supported by this backend")
    }
    async fn remove_rating(&self, _video_id: &str) -> anyhow::Result<()> {
        anyhow::bail!("Remove rating not supported by this backend")
    }
    async fn subscribe_channel(&self, _channel_id: &str) -> anyhow::Result<()> {
        anyhow::bail!("Subscribe not supported by this backend")
    }
    async fn unsubscribe_channel(&self, _channel_id: &str) -> anyhow::Result<()> {
        anyhow::bail!("Unsubscribe not supported by this backend")
    }
    async fn fetch_ryd_votes(&self, _video_id: &str) -> anyhow::Result<(u64, u64, f64)> {
        anyhow::bail!("RYD not supported by this backend")
    }
    async fn comment_action(&self, _action: &str) -> anyhow::Result<()> {
        anyhow::bail!("Comment reactions are unavailable from this client")
    }
    async fn submit_comment(&self, _params: &str, _text: &str, _reply: bool) -> anyhow::Result<()> {
        anyhow::bail!("Comment posting is unavailable from this client")
    }
    async fn fetch_comments(&self, _continuation: &str) -> anyhow::Result<CommentsBatch> {
        anyhow::bail!("Comments not supported by this backend")
    }
}

pub struct BackendChain {
    backends: Vec<Box<dyn Backend>>,
}

impl BackendChain {
    pub fn new(backends: Vec<Box<dyn Backend>>) -> Self {
        Self { backends }
    }
}

#[async_trait]
impl Backend for BackendChain {
    fn is_anonymous(&self) -> bool {
        self.backends.first().is_none_or(|b| b.is_anonymous())
    }
    async fn home(&self, chip_token: Option<&str>) -> anyhow::Result<FeedPage> {
        let mut last_err = anyhow::anyhow!("No backends configured");
        let mut empty_res = None;
        for b in &self.backends {
            match b.home(chip_token).await {
                Ok(res) if !res.items.is_empty() => return Ok(res),
                Ok(res) => empty_res = Some(res),
                Err(e) if e.is::<crate::accounts::AccountCancelled>() => return Err(e),
                Err(e) => last_err = e,
            }
        }
        empty_res.map(Ok).unwrap_or(Err(last_err))
    }

    async fn home_category(&self, category: models::HomeCategory) -> anyhow::Result<FeedPage> {
        let mut error = anyhow::anyhow!("This YouTube section is unavailable");
        for backend in &self.backends {
            match backend.home_category(category).await {
                Ok(page) if !page.items.is_empty() => return Ok(page),
                Err(e) if e.is::<crate::accounts::AccountCancelled>() => return Err(e),
                Err(e) => error = e,
                _ => {}
            }
        }
        Err(error)
    }

    async fn continue_feed(&self, token: &str) -> anyhow::Result<FeedPage> {
        let mut last_err = anyhow::anyhow!("No backends configured");
        for b in &self.backends {
            match b.continue_feed(token).await {
                Ok(res) => return Ok(res),
                Err(e) if e.is::<crate::accounts::AccountCancelled>() => return Err(e),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    async fn subscriptions(&self) -> anyhow::Result<FeedPage> {
        if self.is_anonymous() {
            return Ok(FeedPage::default());
        }
        let mut last_err = anyhow::anyhow!("No backends configured");
        let mut empty_res = None;
        for b in &self.backends {
            match b.subscriptions().await {
                Ok(res) if !res.items.is_empty() => return Ok(res),
                Ok(res) => empty_res = Some(res),
                Err(e) if e.is::<crate::accounts::AccountCancelled>() => return Err(e),
                Err(e) => last_err = e,
            }
        }
        empty_res.map(Ok).unwrap_or(Err(last_err))
    }

    async fn history(&self) -> anyhow::Result<FeedPage> {
        if self.is_anonymous() {
            return Ok(FeedPage::default());
        }
        let mut last_err = anyhow::anyhow!("No backends configured");
        let mut empty_res = None;
        for b in &self.backends {
            match b.history().await {
                Ok(res) if !res.items.is_empty() => return Ok(res),
                Ok(res) => empty_res = Some(res),
                Err(e) if e.is::<crate::accounts::AccountCancelled>() => return Err(e),
                Err(e) => last_err = e,
            }
        }
        empty_res.map(Ok).unwrap_or(Err(last_err))
    }

    async fn watch_later(&self) -> anyhow::Result<FeedPage> {
        if self.is_anonymous() {
            return Ok(FeedPage::default());
        }
        let mut last_err = anyhow::anyhow!("No backends configured");
        let mut empty_res = None;
        for b in &self.backends {
            match b.watch_later().await {
                Ok(res) if !res.items.is_empty() => return Ok(res),
                Ok(res) => empty_res = Some(res),
                Err(e) if e.is::<crate::accounts::AccountCancelled>() => return Err(e),
                Err(e) => last_err = e,
            }
        }
        empty_res.map(Ok).unwrap_or(Err(last_err))
    }

    async fn playlist_destinations(&self, video: &str) -> anyhow::Result<FeedPage> {
        self.backends
            .first()
            .ok_or_else(|| anyhow::anyhow!("No backend"))?
            .playlist_destinations(video)
            .await
    }
    async fn save_to_playlist(&self, playlist: &str, video: &str) -> anyhow::Result<()> {
        self.backends
            .first()
            .ok_or_else(|| anyhow::anyhow!("No backend"))?
            .save_to_playlist(playlist, video)
            .await
    }
    async fn playlists(&self) -> anyhow::Result<FeedPage> {
        if self.is_anonymous() {
            return Ok(FeedPage::default());
        }
        let mut last_err = anyhow::anyhow!("No backends configured");
        let mut empty_res = None;
        for b in &self.backends {
            match b.playlists().await {
                Ok(res) if !res.items.is_empty() => return Ok(res),
                Ok(res) => empty_res = Some(res),
                Err(e) if e.is::<crate::accounts::AccountCancelled>() => return Err(e),
                Err(e) => last_err = e,
            }
        }
        empty_res.map(Ok).unwrap_or(Err(last_err))
    }

    async fn playlist(&self, id: &str) -> anyhow::Result<FeedPage> {
        if self.is_anonymous() && matches!(id, "LL" | "WL" | "VLLL" | "VLWL") {
            return Ok(FeedPage::default());
        }
        let mut last_err = anyhow::anyhow!("No backends configured");
        let mut empty_res = None;
        for b in &self.backends {
            match b.playlist(id).await {
                Ok(res) if !res.items.is_empty() => return Ok(res),
                Ok(res) => empty_res = Some(res),
                Err(e) if e.is::<crate::accounts::AccountCancelled>() => return Err(e),
                Err(e) => last_err = e,
            }
        }
        empty_res.map(Ok).unwrap_or(Err(last_err))
    }

    async fn search(
        &self,
        q: &str,
        filters: &SearchFilters,
        token: Option<&str>,
    ) -> anyhow::Result<FeedPage> {
        let mut last_err = anyhow::anyhow!("No backends configured");
        let mut empty_res = None;
        for b in &self.backends {
            match b.search(q, filters, token).await {
                Ok(res) if !res.items.is_empty() => return Ok(res),
                Ok(res) => empty_res = Some(res),
                Err(e) if e.is::<crate::accounts::AccountCancelled>() => return Err(e),
                Err(e) => last_err = e,
            }
        }
        empty_res.map(Ok).unwrap_or(Err(last_err))
    }

    async fn suggest(&self, q: &str) -> anyhow::Result<Vec<String>> {
        for b in &self.backends {
            if let Ok(res) = b.suggest(q).await {
                return Ok(res);
            }
        }
        Ok(Vec::new())
    }

    async fn video_metadata(&self, video_id: &str) -> anyhow::Result<models::VideoCard> {
        for backend in &self.backends {
            if let Ok(video) = backend.video_metadata(video_id).await {
                return Ok(video);
            }
        }
        anyhow::bail!("Card metadata is unavailable")
    }

    async fn watch(&self, video_id: &str) -> anyhow::Result<WatchData> {
        let mut last_err = anyhow::anyhow!("No backends configured");
        for b in &self.backends {
            match b.watch(video_id).await {
                Ok(res) => return Ok(res),
                Err(e) if e.is::<crate::accounts::AccountCancelled>() => return Err(e),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    async fn channel(&self, id: &str, tab: ChannelTab) -> anyhow::Result<FeedPage> {
        let mut last_err = anyhow::anyhow!("No backends configured");
        for b in &self.backends {
            match b.channel(id, tab).await {
                Ok(res) => return Ok(res),
                Err(e) if e.is::<crate::accounts::AccountCancelled>() => return Err(e),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    async fn channel_page(
        &self,
        id: &str,
        params: Option<&str>,
    ) -> anyhow::Result<models::ChannelPage> {
        let mut error = anyhow::anyhow!("No channel backend configured");
        for backend in &self.backends {
            match backend.channel_page(id, params).await {
                Ok(page) => return Ok(page),
                Err(e) if e.is::<crate::accounts::AccountCancelled>() => return Err(e),
                Err(e) => error = e,
            }
        }
        Err(error)
    }

    async fn live_chat(&self, token: &str) -> anyhow::Result<ChatBatch> {
        let mut last_err = anyhow::anyhow!("No backends configured");
        for b in &self.backends {
            match b.live_chat(token).await {
                Ok(res) => return Ok(res),
                Err(e) if e.is::<crate::accounts::AccountCancelled>() => return Err(e),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    async fn mark_watched(&self, id: &str) -> anyhow::Result<()> {
        for b in &self.backends {
            if let Ok(()) = b.mark_watched(id).await {
                return Ok(());
            }
        }
        Ok(())
    }

    async fn ping_playback(&self, id: &str, cpn: &str) -> anyhow::Result<()> {
        for b in &self.backends {
            let _ = b.ping_playback(id, cpn).await;
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
    ) -> anyhow::Result<()> {
        for b in &self.backends {
            let _ = b.ping_watchtime(id, cpn, st, et, cmt, is_final).await;
        }
        Ok(())
    }

    async fn account_info(&self) -> anyhow::Result<(String, String)> {
        for b in &self.backends {
            if let Ok(info) = b.account_info().await {
                return Ok(info);
            }
        }
        anyhow::bail!("No backend could provide account info")
    }

    async fn like_video(&self, video_id: &str) -> anyhow::Result<bool> {
        for b in &self.backends {
            if let Ok(res) = b.like_video(video_id).await {
                return Ok(res);
            }
        }
        anyhow::bail!("Failed to like video")
    }

    async fn dislike_video(&self, video_id: &str) -> anyhow::Result<bool> {
        for b in &self.backends {
            if let Ok(res) = b.dislike_video(video_id).await {
                return Ok(res);
            }
        }
        anyhow::bail!("Failed to dislike video")
    }

    async fn remove_rating(&self, video_id: &str) -> anyhow::Result<()> {
        for b in &self.backends {
            if let Ok(()) = b.remove_rating(video_id).await {
                return Ok(());
            }
        }
        anyhow::bail!("Failed to remove rating")
    }

    async fn subscribe_channel(&self, channel_id: &str) -> anyhow::Result<()> {
        for b in &self.backends {
            if let Ok(()) = b.subscribe_channel(channel_id).await {
                return Ok(());
            }
        }
        anyhow::bail!("Failed to subscribe to channel")
    }

    async fn unsubscribe_channel(&self, channel_id: &str) -> anyhow::Result<()> {
        for b in &self.backends {
            if let Ok(()) = b.unsubscribe_channel(channel_id).await {
                return Ok(());
            }
        }
        anyhow::bail!("Failed to unsubscribe from channel")
    }

    async fn fetch_ryd_votes(&self, video_id: &str) -> anyhow::Result<(u64, u64, f64)> {
        for b in &self.backends {
            if let Ok(res) = b.fetch_ryd_votes(video_id).await {
                return Ok(res);
            }
        }
        anyhow::bail!("Failed to fetch RYD votes")
    }

    async fn comment_action(&self, action: &str) -> anyhow::Result<()> {
        let mut error = anyhow::anyhow!("Comment reactions unavailable");
        for backend in &self.backends {
            match backend.comment_action(action).await {
                Ok(()) => return Ok(()),
                Err(e) if e.is::<crate::accounts::AccountCancelled>() => return Err(e),
                Err(e) => error = e,
            }
        }
        Err(error)
    }
    async fn submit_comment(&self, params: &str, text: &str, reply: bool) -> anyhow::Result<()> {
        let mut error = anyhow::anyhow!("Comment posting unavailable");
        for backend in &self.backends {
            match backend.submit_comment(params, text, reply).await {
                Ok(()) => return Ok(()),
                Err(e) if e.is::<crate::accounts::AccountCancelled>() => return Err(e),
                Err(e) => error = e,
            }
        }
        Err(error)
    }
    async fn fetch_comments(&self, continuation: &str) -> anyhow::Result<CommentsBatch> {
        for b in &self.backends {
            if let Ok(res) = b.fetch_comments(continuation).await {
                return Ok(res);
            }
        }
        anyhow::bail!("Failed to fetch comments")
    }
}
