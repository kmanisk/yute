use anyhow::{bail, Result};
use async_trait::async_trait;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use yute_core::backend::{models::*, Backend, BackendChain};

struct ProbeBackend {
    fail: bool,
    calls: Arc<AtomicUsize>,
}
impl ProbeBackend {
    fn feed(&self) -> Result<FeedPage> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            bail!("authoritative OAuth error");
        }
        Ok(FeedPage::default())
    }
}
#[async_trait]
impl Backend for ProbeBackend {
    async fn home(&self, _: Option<&str>) -> Result<FeedPage> {
        self.feed()
    }
    async fn continue_feed(&self, _: &str) -> Result<FeedPage> {
        self.feed()
    }
    async fn subscriptions(&self) -> Result<FeedPage> {
        self.feed()
    }
    async fn history(&self) -> Result<FeedPage> {
        self.feed()
    }
    async fn watch_later(&self) -> Result<FeedPage> {
        self.feed()
    }
    async fn playlists(&self) -> Result<FeedPage> {
        self.feed()
    }
    async fn playlist(&self, _: &str) -> Result<FeedPage> {
        self.feed()
    }
    async fn search(&self, _: &str, _: &SearchFilters, _: Option<&str>) -> Result<FeedPage> {
        self.feed()
    }
    async fn suggest(&self, _: &str) -> Result<Vec<String>> {
        bail!("unused")
    }
    async fn watch(&self, _: &str) -> Result<WatchData> {
        bail!("unused")
    }
    async fn channel(&self, _: &str, _: ChannelTab) -> Result<FeedPage> {
        self.feed()
    }
    async fn live_chat(&self, _: &str) -> Result<ChatBatch> {
        bail!("unused")
    }
    async fn mark_watched(&self, _: &str) -> Result<()> {
        self.feed().map(|_| ())
    }
    async fn account_info(&self) -> Result<(String, String)> {
        self.feed().map(|_| ("fallback".into(), String::new()))
    }
    async fn like_video(&self, _: &str) -> Result<bool> {
        self.feed().map(|_| true)
    }
    async fn subscribe_channel(&self, _: &str) -> Result<()> {
        self.feed().map(|_| ())
    }
}
fn chain(fail: bool) -> (BackendChain, Arc<AtomicUsize>) {
    let fallback = Arc::new(AtomicUsize::new(0));
    (
        BackendChain::new(vec![
            Box::new(ProbeBackend {
                fail,
                calls: Default::default(),
            }),
            Box::new(ProbeBackend {
                fail: false,
                calls: fallback.clone(),
            }),
        ]),
        fallback,
    )
}
#[tokio::test]
async fn account_errors_cannot_be_masked_by_successful_empty_fallback() {
    let (chain, fallback) = chain(true);
    for response in [
        chain.subscriptions().await,
        chain.history().await,
        chain.watch_later().await,
        chain.playlists().await,
        chain.playlist("VLLL").await,
        chain.playlist("private-id").await,
        chain.continue_feed("yute:account:opaque").await,
    ] {
        assert_eq!(
            response.unwrap_err().to_string(),
            "authoritative OAuth error"
        );
    }
    assert_eq!(
        chain.like_video("fixture").await.unwrap_err().to_string(),
        "authoritative OAuth error"
    );
    assert_eq!(
        chain
            .subscribe_channel("UCfixture")
            .await
            .unwrap_err()
            .to_string(),
        "authoritative OAuth error"
    );
    assert_eq!(
        chain.account_info().await.unwrap_err().to_string(),
        "authoritative OAuth error"
    );
    assert_eq!(
        chain.mark_watched("fixture").await.unwrap_err().to_string(),
        "authoritative OAuth error"
    );
    assert_eq!(fallback.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn successful_empty_account_result_is_authoritative() {
    let (chain, fallback) = chain(false);
    assert!(chain.subscriptions().await.unwrap().items.is_empty());
    assert_eq!(fallback.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn public_fallback_remains_available() {
    let (chain, fallback) = chain(true);
    assert!(chain.home(None).await.unwrap().items.is_empty());
    assert_eq!(fallback.load(Ordering::SeqCst), 1);
}
