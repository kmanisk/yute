use serde_json::json;
use yute::{
    actions::{activation, OpenAction},
    backend::{
        models::{Card, LiveState, PlaylistCard},
        parse::parse_card,
    },
};

fn video() -> Card {
    parse_card(&json!({"videoRenderer":{"videoId":"abcdefghijk","title":{"simpleText":"A mix of Rust tips"}}})).unwrap()
}
#[test]
fn video_short_live_and_upcoming_dispatch() {
    let mut card = video();
    assert!(matches!(activation(&card), OpenAction::Play(_)));
    if let Card::Video(v) = &mut card {
        v.badges.push("SHORTS".into());
    }
    assert!(matches!(activation(&card), OpenAction::Play(_)));
    if let Card::Video(v) = &mut card {
        v.live = LiveState::Live;
    }
    assert!(matches!(activation(&card), OpenAction::Play(_)));
    if let Card::Video(v) = &mut card {
        v.live = LiveState::Upcoming;
    }
    assert!(matches!(activation(&card), OpenAction::Unavailable(_)));
}
#[test]
fn channel_lockup_is_never_a_playable_video() {
    let card = parse_card(&json!({"lockupViewModel":{
        "contentId":"UCchannel", "contentType":"LOCKUP_CONTENT_TYPE_CHANNEL",
        "metadata":{"lockupMetadataViewModel":{"title":{"content":"Mix channel"}}},
        "rendererContext":{"commandContext":{"onTap":{"innertubeCommand":{"browseEndpoint":{"browseId":"UCchannel"}}}}}
    }})).unwrap();
    assert!(matches!(&card, Card::Channel(c) if c.id == "UCchannel"));
    assert!(matches!(activation(&card), OpenAction::OpenChannel(_)));
}
#[test]
fn mixes_have_an_explicit_playback_action() {
    let card = Card::Playlist(PlaylistCard {
        id: "RDabcdefghijk".into(),
        title: "Mix".into(),
        channel: "".into(),
        video_count: None,
        thumb_url: "".into(),
        is_mix: true,
        first_video_id: Some("abcdefghijk".into()),
    });
    assert!(matches!(activation(&card), OpenAction::PlayMix(_)));
}
#[test]
fn a_video_title_containing_mix_does_not_change_its_type() {
    let card = parse_card(&json!({"lockupViewModel":{"contentId":"abcdefghijk", "contentType":"LOCKUP_CONTENT_TYPE_VIDEO", "metadata":{"lockupMetadataViewModel":{"title":{"content":"Mix music in Rust"}}}}})).unwrap();
    assert!(matches!(card, Card::Video(_)));
}

#[test]
fn channel_parser_uses_only_advertised_tabs_and_active_content() {
    let raw = json!({"metadata":{"channelMetadataRenderer":{"title":"Fixture Channel","vanityChannelUrl":"https://www.youtube.com/@fixture"}},
    "contents":{"twoColumnBrowseResultsRenderer":{"tabs":[
        {"tabRenderer":{"title":"Videos","selected":true,"endpoint":{"browseEndpoint":{"browseId":"UCfixture","params":"server-videos"}},"content":{"videoRenderer":{"videoId":"abcdefghijk","title":{"simpleText":"Visible video"}}}}},
        {"tabRenderer":{"title":"Playlists","endpoint":{"browseEndpoint":{"browseId":"UCfixture","params":"server-playlists"}}}},
        {"tabRenderer":{"title":"Not a channel tab","endpoint":{"browseEndpoint":{"browseId":"other"}}}}
    ]}}});
    let page = yute::backend::parse::parse_channel_page(&raw, "UCfixture");
    assert_eq!(page.channel.title, "Fixture Channel");
    assert_eq!(page.channel.handle.as_deref(), Some("@fixture"));
    assert_eq!(page.tabs.len(), 2);
    assert_eq!(page.tabs[0].params.as_deref(), Some("server-videos"));
    assert_eq!(page.feed.items.len(), 1);
}
#[test]
fn search_channel_back_restores_position_filter_and_pages() {
    use yute::app::{App, ChannelState, DetailRoute, FilterKind, Screen};
    let mut app = App::new(
        yute::config::Config::default(),
        ratatui_image::picker::Picker::halfblocks(),
    );
    app.screen = Screen::Search;
    app.search_query = "bbs".into();
    app.selected_index = 7;
    app.scroll_row = 2;
    app.filter_kind = FilterKind::Channels;
    app.continuation_token = Some("search-next".into());
    app.items = vec![video(); 10];
    app.transition(Screen::ChannelView, true);
    app.detail_route = Some(DetailRoute::Channel(Box::new(ChannelState {
        channel: yute::backend::models::ChannelCard {
            id: "UCfixture".into(),
            title: "Channel".into(),
            handle: None,
            subscribers: None,
            avatar_url: None,
            verified: false,
        },
        tabs: vec![],
        active: 0,
        subscribed: None,
        pages: Default::default(),
    })));
    app.nav_back();
    assert_eq!(app.screen, Screen::Search);
    assert_eq!(app.search_query, "bbs");
    assert_eq!(app.selected_index, 7);
    assert_eq!(app.scroll_row, 2);
    assert_eq!(app.filter_kind, FilterKind::Channels);
    assert_eq!(app.continuation_token.as_deref(), Some("search-next"));
    assert_eq!(app.items.len(), 10);
}

#[test]
fn tv_channel_headers_are_parsed_without_inventing_tabs() {
    let raw = json!({"contents":{"tvBrowseRenderer":{"content":{"tvSurfaceContentRenderer":{
        "header":{"channelHeaderRenderer":{"title":{"simpleText":"TV Channel"},"subtitle":{"runs":[{"text":"@tvfixture"},{"text":" · "},{"text":"1.2M subscribers"}]}}},
        "content":{"videoRenderer":{"videoId":"abcdefghijk","title":{"simpleText":"Channel video"}}}
    }}}}});
    let page = yute::backend::parse::parse_channel_page(&raw, "UCfixture");
    assert_eq!(page.channel.title, "TV Channel");
    assert_eq!(page.channel.handle.as_deref(), Some("@tvfixture"));
    assert_eq!(
        page.channel.subscribers.as_deref(),
        Some("1.2M subscribers")
    );
    assert!(page.tabs.is_empty());
    assert_eq!(page.feed.items.len(), 1);
}

#[test]
fn explicit_video_types_win_over_ambiguous_id_prefixes() {
    for id in ["PLabcdefghi", "RDabcdefghi", "UCabcdefghi"] {
        let card=parse_card(&json!({"lockupViewModel":{"contentId":id,"contentType":"LOCKUP_CONTENT_TYPE_VIDEO","metadata":{"lockupMetadataViewModel":{"title":{"content":"Fixture video"}}}}})).unwrap();
        assert!(matches!(card, Card::Video(_)));
        assert!(matches!(activation(&card), OpenAction::Play(_)));
    }
}
