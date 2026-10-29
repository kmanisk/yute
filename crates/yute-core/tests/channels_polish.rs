use serde_json::json;
use yute_core::backend::innertube::parse_subscribed_channels;
#[test]
fn tv_subscribed_tabs_preserve_typed_channel_identity_and_avatar() {
    let page=parse_subscribed_channels(&json!({"contents":{"tvBrowseRenderer":{"tabs":[{"tabRenderer":{"title":"All subscriptions","endpoint":{"browseEndpoint":{"browseId":"FEsubscriptions"}}}},{"tabRenderer":{"title":"Fixture channel","endpoint":{"browseEndpoint":{"browseId":"UCfixture"}},"thumbnail":{"thumbnails":[{"url":"//yt3.ggpht.com/fixture=s88-c-k-no-rj"}]}}}]}}})).unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(
        matches!(&page.items[0],yute_core::backend::models::Card::Channel(c)if c.id=="UCfixture"&&c.avatar_url.as_deref()==Some("https://yt3.ggpht.com/fixture=s88-c-k-no-rj"))
    );
    let current = parse_subscribed_channels(&json!({
        "contents":{"tvBrowseRenderer":{"content":{"tvSecondaryNavRenderer":{"sections":[
            {"tvSecondaryNavSectionRenderer":{"tabs":[
                {"tabRenderer":{"title":"Fixture current channel","endpoint":{"browseEndpoint":{"browseId":"FEsubscriptions","params":"opaque-fixture-param"}},"thumbnail":{"thumbnails":[{"url":"//yt3.ggpht.com/current=s88-c-k-no-rj"}]}}}
            ]}}
        ]}}}}
    })).unwrap();
    assert!(matches!(
        &current.items[0],
        yute_core::backend::models::Card::Channel(c)
            if yute_core::backend::models::subscription_tab_params(&c.id) == Some("opaque-fixture-param")
                && c.title == "Fixture current channel"
                && c.avatar_url.as_deref() == Some("https://yt3.ggpht.com/current=s88-c-k-no-rj")
    ));
    assert!(parse_subscribed_channels(&json!({"unknown":[]})).is_err());
    assert!(parse_subscribed_channels(&json!({"loggedOut":true})).is_err());
    assert!(
        parse_subscribed_channels(&json!({"contents":{"emptyContentRenderer":{}}}))
            .unwrap()
            .items
            .is_empty()
    );
}
async fn saved_oauth_backend() -> yute_core::backend::innertube::InnerTubeBackend {
    use yute_core::{
        accounts::Accounts,
        backend::{auth::AuthContext, innertube::InnerTubeBackend},
        config::Config,
    };
    let config = Config::load_checked().unwrap();
    let auth = AuthContext::from_config(&config);
    let accounts = tokio::task::spawn_blocking(move || {
        Accounts::read_storage(&yute_core::credentials::path()).map(|storage| {
            Accounts::from_storage(yute_core::credentials::path(), storage, &auth, false)
        })
    })
    .await
    .unwrap()
    .unwrap();
    let mut context = accounts.current_context();
    assert!(
        context.has_oauth() && !context.needs_login,
        "Interactive TV OAuth required for this probe"
    );
    // Read-only QA must not persist rotated visitor state or tokens.
    context.auth_file_path = None;
    InnerTubeBackend::new(context)
}
#[tokio::test]
#[ignore = "read-only saved OAuth account probe; no identities or response bodies logged"]
async fn live_subscribed_channels_protocol() {
    use yute_core::backend::Backend;
    let backend = saved_oauth_backend().await;
    let page = backend.subscribed_channels(None).await.unwrap();
    assert!(page
        .items
        .iter()
        .all(|c| matches!(c, yute_core::backend::models::Card::Channel(_))));
    println!("Authenticated channel cards parsed: {}", page.items.len());
    if let Some(yute_core::backend::models::Card::Channel(channel)) = page.items.first() {
        if let Some(params) = yute_core::backend::models::subscription_tab_params(&channel.id) {
            let opened = backend
                .channel_page(
                    yute_core::backend::models::SUBSCRIPTIONS_BROWSE_ID,
                    Some(params),
                )
                .await
                .unwrap();
            println!(
                "Selected subscriptions tab feed items parsed: {}",
                opened.feed.items.len()
            );
        }
    }
    // Counts and pagination flags are the only account data emitted by this probe.
    for (name, result) in [
        ("Subscriptions", backend.subscriptions().await),
        ("History", backend.history().await),
        ("Watch Later", backend.watch_later().await),
        ("Liked", backend.playlist("LL").await),
        ("Playlists", backend.playlists().await),
    ] {
        assert!(
            result.is_ok(),
            "{name}: authenticated browse failed (details redacted)"
        );
        let page = result.unwrap();
        println!(
            "{name}: {} cards; continuation={}",
            page.items.len(),
            page.continuation.is_some()
        );
        if let Some(token) = &page.continuation {
            let next = backend.continue_feed(token).await;
            assert!(
                next.is_ok(),
                "{name}: continuation failed (details redacted)"
            );
            println!(
                "{name}: next page parsed: {} cards",
                next.unwrap().items.len()
            );
        }
        if name == "Playlists" {
            assert!(
                page.items
                    .iter()
                    .all(|card| matches!(card, yute_core::backend::models::Card::Playlist(_))),
                "Playlist library contains non-playlist identities"
            );
            if let Some(yute_core::backend::models::Card::Playlist(playlist)) = page.items.first() {
                let opened = backend.playlist(&playlist.id).await;
                assert!(
                    opened.is_ok(),
                    "Playlist contents failed (details redacted)"
                );
                println!(
                    "Playlist contents parsed: {} cards",
                    opened.unwrap().items.len()
                );
            }
        }
    }
}

#[tokio::test]
#[ignore = "read-only owned-playlist OAuth probe; no identities or response bodies logged"]
async fn live_owned_playlists_protocol() {
    use yute_core::backend::{models::Card, Backend};
    let backend = saved_oauth_backend().await;
    let page = backend.playlists().await;
    assert!(
        page.is_ok(),
        "Owned playlist browse failed (details redacted)"
    );
    let page = page.unwrap();
    assert!(
        page.items
            .iter()
            .all(|card| matches!(card, Card::Playlist(p) if !p.id.is_empty())),
        "Owned playlist response contains non-playlist identities"
    );
    println!(
        "Owned playlists: {} cards; continuation={}",
        page.items.len(),
        page.continuation.is_some()
    );
    if let Some(token) = &page.continuation {
        let next = backend.continue_feed(token).await;
        if let Err(error) = &next {
            assert!(
                error.is::<yute_core::backend::innertube::RepeatedAccountPage>(),
                "Owned playlist continuation failed (details redacted)"
            );
            println!("Owned playlist continuation: server replay detected; existing cards retained, explicit error returned");
        }
        if let Ok(next) = next {
            assert!(next
                .items
                .iter()
                .all(|card| matches!(card, Card::Playlist(p) if !p.id.is_empty())));
            assert!(
                next.items.is_empty()
                    || next
                        .items
                        .iter()
                        .any(|card| !page.items.iter().any(|first| first.id() == card.id())),
                "Owned playlist continuation repeated the first page"
            );
            println!("Owned playlists next page: {} cards", next.items.len());
        }
    }
    if let Some(Card::Playlist(playlist)) = page.items.first() {
        let contents = backend.playlist(&playlist.id).await;
        assert!(
            contents.is_ok(),
            "Playlist contents failed (details redacted)"
        );
        println!("Playlist contents: {} cards", contents.unwrap().items.len());
    }
}
