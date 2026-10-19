pub mod api;
pub mod auth;
pub mod innertube;
pub mod models;
pub mod parse;
pub mod rss;
pub mod ytdlp;

use async_trait::async_trait;
use models::{ChannelTab, ChatBatch, FeedPage, SearchFilters, WatchData};

#[async_trait]
pub trait Backend: Send + Sync {
    async fn home(&self, chip_token: Option<&str>) -> anyhow::Result<FeedPage>;
    async fn continue_feed(&self, token: &str) -> anyhow::Result<FeedPage>;
    async fn subscriptions(&self) -> anyhow::Result<FeedPage>;
    async fn history(&self) -> anyhow::Result<FeedPage>;
    async fn watch_later(&self) -> anyhow::Result<FeedPage>;
    async fn playlists(&self) -> anyhow::Result<FeedPage>;
    async fn playlist(&self, id: &str) -> anyhow::Result<FeedPage>;
    async fn search(&self, q: &str, filters: &SearchFilters, token: Option<&str>) -> anyhow::Result<FeedPage>;
    async fn suggest(&self, q: &str) -> anyhow::Result<Vec<String>>;
    async fn watch(&self, video_id: &str) -> anyhow::Result<WatchData>;
    async fn channel(&self, id: &str, tab: ChannelTab) -> anyhow::Result<FeedPage>;
    async fn live_chat(&self, token: &str) -> anyhow::Result<ChatBatch>;
    async fn mark_watched(&self, id: &str) -> anyhow::Result<()>;
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
    async fn home(&self, chip_token: Option<&str>) -> anyhow::Result<FeedPage> {
        let mut last_err = anyhow::anyhow!("No backends configured");
        let mut empty_res = None;
        for b in &self.backends {
            match b.home(chip_token).await {
                Ok(res) if !res.items.is_empty() => return Ok(res),
                Ok(res) => empty_res = Some(res),
                Err(e) => last_err = e,
            }
        }
        empty_res.map(Ok).unwrap_or(Err(last_err))
    }

    async fn continue_feed(&self, token: &str) -> anyhow::Result<FeedPage> {
        let mut last_err = anyhow::anyhow!("No backends configured");
        for b in &self.backends {
            match b.continue_feed(token).await {
                Ok(res) => return Ok(res),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    async fn subscriptions(&self) -> anyhow::Result<FeedPage> {
        let mut last_err = anyhow::anyhow!("No backends configured");
        let mut empty_res = None;
        for b in &self.backends {
            match b.subscriptions().await {
                Ok(res) if !res.items.is_empty() => return Ok(res),
                Ok(res) => empty_res = Some(res),
                Err(e) => last_err = e,
            }
        }
        empty_res.map(Ok).unwrap_or(Err(last_err))
    }

    async fn history(&self) -> anyhow::Result<FeedPage> {
        let mut last_err = anyhow::anyhow!("No backends configured");
        let mut empty_res = None;
        for b in &self.backends {
            match b.history().await {
                Ok(res) if !res.items.is_empty() => return Ok(res),
                Ok(res) => empty_res = Some(res),
                Err(e) => last_err = e,
            }
        }
        empty_res.map(Ok).unwrap_or(Err(last_err))
    }

    async fn watch_later(&self) -> anyhow::Result<FeedPage> {
        let mut last_err = anyhow::anyhow!("No backends configured");
        let mut empty_res = None;
        for b in &self.backends {
            match b.watch_later().await {
                Ok(res) if !res.items.is_empty() => return Ok(res),
                Ok(res) => empty_res = Some(res),
                Err(e) => last_err = e,
            }
        }
        empty_res.map(Ok).unwrap_or(Err(last_err))
    }

    async fn playlists(&self) -> anyhow::Result<FeedPage> {
        let mut last_err = anyhow::anyhow!("No backends configured");
        let mut empty_res = None;
        for b in &self.backends {
            match b.playlists().await {
                Ok(res) if !res.items.is_empty() => return Ok(res),
                Ok(res) => empty_res = Some(res),
                Err(e) => last_err = e,
            }
        }
        empty_res.map(Ok).unwrap_or(Err(last_err))
    }

    async fn playlist(&self, id: &str) -> anyhow::Result<FeedPage> {
        let mut last_err = anyhow::anyhow!("No backends configured");
        let mut empty_res = None;
        for b in &self.backends {
            match b.playlist(id).await {
                Ok(res) if !res.items.is_empty() => return Ok(res),
                Ok(res) => empty_res = Some(res),
                Err(e) => last_err = e,
            }
        }
        empty_res.map(Ok).unwrap_or(Err(last_err))
    }

    async fn search(&self, q: &str, filters: &SearchFilters, token: Option<&str>) -> anyhow::Result<FeedPage> {
        let mut last_err = anyhow::anyhow!("No backends configured");
        let mut empty_res = None;
        for b in &self.backends {
            match b.search(q, filters, token).await {
                Ok(res) if !res.items.is_empty() => return Ok(res),
                Ok(res) => empty_res = Some(res),
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

    async fn watch(&self, video_id: &str) -> anyhow::Result<WatchData> {
        let mut last_err = anyhow::anyhow!("No backends configured");
        for b in &self.backends {
            match b.watch(video_id).await {
                Ok(res) => return Ok(res),
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
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    async fn live_chat(&self, token: &str) -> anyhow::Result<ChatBatch> {
        let mut last_err = anyhow::anyhow!("No backends configured");
        for b in &self.backends {
            match b.live_chat(token).await {
                Ok(res) => return Ok(res),
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
}
