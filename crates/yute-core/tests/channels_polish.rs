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
#[tokio::test]
#[ignore = "read-only saved OAuth account probe; no identities or response bodies logged"]
async fn live_subscribed_channels_protocol() {
    use yute_core::{
        accounts::Accounts,
        backend::{auth::AuthContext, innertube::InnerTubeBackend, Backend},
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
    let context = accounts.current_context();
    assert!(
        context.has_oauth() && !context.needs_login,
        "Interactive TV OAuth required for this probe"
    );
    let backend = InnerTubeBackend::new(context);
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
}
