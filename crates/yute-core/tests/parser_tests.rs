use serde_json::Value;
use yute_core::backend::models::{Card, LiveState};
use yute_core::backend::parse::{parse_feed_page, parse_watch_data};

#[test]
fn test_parse_home_fixture() {
    let raw = include_str!("fixtures/home.json");
    let val: Value = serde_json::from_str(raw).expect("Valid JSON");

    let page = parse_feed_page(&val);
    assert_eq!(page.chips.len(), 3);
    assert_eq!(page.chips[0].title, "All");
    assert!(page.chips[0].active);
    assert_eq!(page.chips[1].title, "Gaming");
    assert_eq!(page.chips[1].token.as_deref(), Some("token_gaming_123"));

    assert_eq!(page.items.len(), 2);
    match &page.items[0] {
        Card::Video(v) => {
            assert_eq!(v.id, "vid_sample_1");
            assert_eq!(v.title, "Linux Gaming on CachyOS with RTX 5050");
            assert_eq!(v.channel, "Arch Linux Crew");
            assert_eq!(v.duration.as_deref(), Some("14:20"));
            assert_eq!(v.views.as_deref(), Some("140K views"));
            assert_eq!(v.progress_pct, Some(45));
            assert_eq!(v.live, LiveState::None);
        }
        _ => panic!("Expected Video card"),
    }

    match &page.items[1] {
        Card::Video(v) => {
            assert_eq!(v.id, "vid_sample_2");
            assert_eq!(v.live, LiveState::Live);
        }
        _ => panic!("Expected Live Video card"),
    }

    assert_eq!(
        page.continuation.as_deref(),
        Some("continuation_home_page_2")
    );
}

#[test]
fn test_parse_search_fixture() {
    let raw = include_str!("fixtures/search.json");
    let val: Value = serde_json::from_str(raw).expect("Valid JSON");

    let page = parse_feed_page(&val);
    assert_eq!(page.items.len(), 1);
    match &page.items[0] {
        Card::Video(v) => {
            assert_eq!(v.id, "search_result_1");
            assert_eq!(v.title, "Ultimate Ratatui TUI Guide");
            assert_eq!(v.channel, "Rust Systems");
            assert_eq!(v.duration.as_deref(), Some("28:15"));
        }
        _ => panic!("Expected Video card"),
    }
}

#[test]
fn test_parse_watch_fixture() {
    let raw = include_str!("fixtures/watch.json");
    let val: Value = serde_json::from_str(raw).expect("Valid JSON");

    let watch = parse_watch_data(&val).expect("Watch data parsed");
    assert_eq!(watch.video.id, "watch_sample_vid");
    assert_eq!(
        watch.video.title,
        "Building a High-Performance YouTube TUI in Rust"
    );
    assert_eq!(watch.video.channel, "Antigravity Engineering");
    assert_eq!(watch.video.duration.as_deref(), Some("20:45"));
    assert!(watch
        .description
        .contains("Deep dive into building an interactive YouTube TUI"));
}

#[test]
fn test_auth_cookie_domain_filtering() {
    use yute_core::backend::auth::AuthContext;
    let mut auth = AuthContext::default();
    let netscape_content = "\
# Netscape HTTP Cookie File
.sdcard.org\tTRUE\t/\tTRUE\t123456\ttoken\tbad_sdcard_val
.testufo.com\tTRUE\t/\tFALSE\t123456\tsession\tbad_ufo_val
.youtube.com\tTRUE\t/\tTRUE\t123456\tSAPISID\tvalid_sapisid_123
.youtube.com\tTRUE\t/\tTRUE\t123456\tLOGIN_INFO\tvalid_login_info
.google.com\tTRUE\t/\tTRUE\t123456\tSID\tvalid_sid
";
    auth.parse_netscape_cookies(netscape_content);
    assert_eq!(auth.sapisid.as_deref(), Some("valid_sapisid_123"));
    assert!(auth.cookies.contains_key("SAPISID"));
    assert!(auth.cookies.contains_key("LOGIN_INFO"));
    assert!(auth.cookies.contains_key("SID"));
    assert!(!auth.cookies.contains_key("bad_sdcard_val"));
    assert!(!auth.cookies.contains_key("token"));
    assert!(!auth.cookies.contains_key("session"));
    let hdr = auth.cookie_header.unwrap_or_default();
    assert!(hdr.contains("SAPISID=valid_sapisid_123"));
    assert!(!hdr.contains("bad_sdcard_val"));
}

#[test]
fn test_parse_lockup_view_model() {
    let raw = serde_json::json!({
        "lockupViewModel": {
            "contentId": "PL1234567890",
            "contentType": "LOCKUP_CONTENT_TYPE_PLAYLIST",
            "metadata": {
                "lockupMetadataViewModel": {
                    "title": { "content": "Rust Mastery Playlist" },
                    "metadataRows": [
                        {
                            "metadataParts": [
                                { "text": { "content": "Rust Channel" } }
                            ]
                        },
                        {
                            "metadataParts": [
                                { "text": { "content": "45 videos" } }
                            ]
                        }
                    ]
                }
            }
        }
    });

    let page = parse_feed_page(&raw);
    assert_eq!(page.items.len(), 1);
    match &page.items[0] {
        Card::Playlist(p) => {
            assert_eq!(p.id, "PL1234567890");
            assert_eq!(p.title, "Rust Mastery Playlist");
            assert_eq!(p.channel, "Rust Channel");
            assert_eq!(p.video_count.as_deref(), Some("45 videos"));
            assert!(!p.is_mix);
        }
        _ => panic!("Expected Playlist card"),
    }
}

#[test]
fn test_parse_lockup_view_model_mix() {
    let raw = serde_json::json!({
        "lockupViewModel": {
            "contentId": "RDn5wLZyvEukM",
            "contentType": "LOCKUP_CONTENT_TYPE_PLAYLIST",
            "metadata": {
                "lockupMetadataViewModel": {
                    "title": { "content": "Mix - Instrumental Focus" },
                    "metadataRows": [
                        {
                            "metadataParts": [
                                { "text": { "content": "Playlist" } }
                            ]
                        },
                        {
                            "metadataParts": [
                                { "text": { "content": "PLAYLIST" } }
                            ]
                        }
                    ]
                }
            }
        }
    });

    let page = parse_feed_page(&raw);
    assert_eq!(page.items.len(), 1);
    match &page.items[0] {
        Card::Playlist(p) => {
            assert_eq!(p.id, "RDn5wLZyvEukM");
            assert_eq!(p.title, "Mix - Instrumental Focus");
            assert_eq!(p.channel, "YouTube");
            assert_eq!(p.video_count.as_deref(), Some("Mix"));
            assert!(p.is_mix);
        }
        _ => panic!("Expected Mix Playlist card"),
    }
}

#[test]
fn test_parse_shorts_lockup_view_model() {
    let raw = serde_json::json!({
        "shortsLockupViewModel": {
            "entityId": "shorts-shelf-item-cvNvCGYAbPc",
            "accessibilityText": "CachyOS vs Bazzite, 168 thousand views - play Short",
            "overlayMetadata": {
                "primaryText": {
                    "content": "CachyOS vs Bazzite"
                },
                "secondaryText": {
                    "content": "168K views"
                }
            },
            "onTap": {
                "innertubeCommand": {
                    "reelWatchEndpoint": {
                        "videoId": "cvNvCGYAbPc"
                    }
                }
            }
        }
    });

    let page = parse_feed_page(&raw);
    assert_eq!(page.items.len(), 1);
    match &page.items[0] {
        Card::Video(v) => {
            assert_eq!(v.id, "cvNvCGYAbPc");
            assert_eq!(v.title, "CachyOS vs Bazzite");
            assert_eq!(v.views.as_deref(), Some("168K views"));
            assert!(v.badges.iter().any(|b| b == "SHORTS"));
        }
        _ => panic!("Expected Video card for short"),
    }
}

#[test]
fn test_parse_reel_item_renderer() {
    let raw = serde_json::json!({
        "reelItemRenderer": {
            "videoId": "short123456",
            "headline": {
                "simpleText": "Linux Terminal Tricks"
            },
            "viewCountText": {
                "simpleText": "520K views"
            },
            "shortBylineText": {
                "simpleText": "Arch Mastery"
            }
        }
    });

    let page = parse_feed_page(&raw);
    assert_eq!(page.items.len(), 1);
    match &page.items[0] {
        Card::Video(v) => {
            assert_eq!(v.id, "short123456");
            assert_eq!(v.title, "Linux Terminal Tricks");
            assert_eq!(v.channel, "Arch Mastery");
            assert_eq!(v.views.as_deref(), Some("520K views"));
            assert!(v.badges.iter().any(|b| b == "SHORTS"));
        }
        _ => panic!("Expected Video card for reel item"),
    }
}

#[test]
fn test_parse_lockup_view_model_duration_and_nested_metadata() {
    let raw = serde_json::json!({
        "lockupViewModel": {
            "contentId": "izGwDsrQ1eQ",
            "contentType": "LOCKUP_CONTENT_TYPE_VIDEO",
            "contentImage": {
                "thumbnailViewModel": {
                    "overlays": [
                        {
                            "thumbnailBottomOverlayViewModel": {
                                "badges": [
                                    {
                                        "thumbnailBadgeViewModel": {
                                            "text": "5:01"
                                        }
                                    }
                                ]
                            }
                        }
                    ]
                }
            },
            "metadata": {
                "lockupMetadataViewModel": {
                    "title": { "content": "George Michael - Careless Whisper" },
                    "metadata": {
                        "contentMetadataViewModel": {
                            "metadataRows": [
                                {
                                    "metadataParts": [
                                        { "text": { "content": "George Michael" } }
                                    ]
                                },
                                {
                                    "metadataParts": [
                                        { "text": { "content": "450M views" } },
                                        { "text": { "content": "10y ago" } }
                                    ]
                                }
                            ]
                        }
                    }
                }
            }
        }
    });

    let page = parse_feed_page(&raw);
    assert_eq!(page.items.len(), 1);
    match &page.items[0] {
        Card::Video(v) => {
            assert_eq!(v.id, "izGwDsrQ1eQ");
            assert_eq!(v.title, "George Michael - Careless Whisper");
            assert_eq!(v.channel, "George Michael");
            assert_eq!(v.duration.as_deref(), Some("5:01"));
            assert_eq!(v.views.as_deref(), Some("450M views"));
            assert_eq!(v.published.as_deref(), Some("10y ago"));
        }
        _ => panic!("Expected Video card with parsed duration and metadata"),
    }
}

#[test]
fn test_parse_tile_renderer_video_and_playlist() {
    let raw = serde_json::json!({
        "contents": [
            {
                "tileRenderer": {
                    "contentType": "TILE_CONTENT_TYPE_VIDEO",
                    "header": {
                        "tileHeaderRenderer": {
                            "thumbnail": {
                                "thumbnails": [
                                    { "url": "https://i.ytimg.com/vi/abc123/hqdefault.jpg" }
                                ]
                            },
                            "badgeText": {
                                "simpleText": "12:34"
                            }
                        }
                    },
                    "metadata": {
                        "tileMetadataRenderer": {
                            "title": { "simpleText": "CachyOS Performance Guide" },
                            "userName": { "simpleText": "Linux Gaming" }
                        }
                    },
                    "onSelectCommand": {
                        "watchEndpoint": {
                            "videoId": "abc123"
                        }
                    }
                }
            },
            {
                "tileRenderer": {
                    "contentType": "TILE_CONTENT_TYPE_PLAYLIST",
                    "header": {
                        "tileHeaderRenderer": {
                            "thumbnail": {
                                "thumbnails": [
                                    { "url": "https://i.ytimg.com/vi/mix456/hqdefault.jpg" }
                                ]
                            }
                        }
                    },
                    "metadata": {
                        "tileMetadataRenderer": {
                            "title": { "simpleText": "My Mix" },
                            "userName": { "simpleText": "YouTube" }
                        }
                    },
                    "onSelectCommand": {
                        "watchPlaylistEndpoint": {
                            "playlistId": "RDCLAK5uy_mix"
                        }
                    }
                }
            }
        ]
    });

    let page = parse_feed_page(&raw);
    assert_eq!(page.items.len(), 2);
    match &page.items[0] {
        Card::Video(v) => {
            assert_eq!(v.id, "abc123");
            assert_eq!(v.title, "CachyOS Performance Guide");
            assert_eq!(v.channel, "Linux Gaming");
            assert_eq!(v.duration.as_deref(), Some("12:34"));
        }
        _ => panic!("Expected Video card"),
    }
    match &page.items[1] {
        Card::Playlist(p) => {
            assert_eq!(p.id, "RDCLAK5uy_mix");
            assert_eq!(p.title, "My Mix");
            assert!(p.is_mix);
        }
        _ => panic!("Expected Mix Playlist card"),
    }
}

#[test]
fn test_parse_comments() {
    use yute_core::backend::parse::parse_comments;

    let json_val = serde_json::json!({
        "continuationContents": {
            "itemSectionContinuation": {
                "contents": [
                    {
                        "commentThreadRenderer": {
                            "comment": {
                                "commentRenderer": {
                                    "commentId": "comment_123",
                                    "authorText": { "simpleText": "@archlinux" },
                                    "contentText": { "runs": [{ "text": "This TUI is blazing fast!" }] },
                                    "publishedTimeText": { "simpleText": "1 day ago" },
                                    "voteCount": { "simpleText": "128" },
                                    "replyCount": 5
                                }
                            }
                        }
                    }
                ],
                "continuations": [
                    {
                        "nextContinuationData": {
                            "continuation": "continuation_comments_page_2"
                        }
                    }
                ]
            }
        }
    });

    let batch = parse_comments(&json_val);
    assert_eq!(batch.comments.len(), 1);
    assert_eq!(batch.comments[0].id, "comment_123");
    assert_eq!(batch.comments[0].author, "@archlinux");
    assert_eq!(batch.comments[0].text, "This TUI is blazing fast!");
    assert_eq!(batch.comments[0].published.as_deref(), Some("1 day ago"));
    assert_eq!(batch.comments[0].likes.as_deref(), Some("128"));
    assert_eq!(batch.comments[0].reply_count, Some(5));
    assert_eq!(
        batch.continuation.as_deref(),
        Some("continuation_comments_page_2")
    );
}

#[test]
fn test_resume_time_calculation_and_resolution() {
    use yute_core::backend::models::VideoCard;

    let mut vid = VideoCard {
        id: "test1".to_string(),
        title: "Test".to_string(),
        channel: "Chan".to_string(),
        channel_id: None,
        verified: false,
        duration: Some("10:00".to_string()),
        views: None,
        published: None,
        thumb_url: "".to_string(),
        progress_pct: Some(50),
        live: LiveState::None,
        badges: vec![],
        resume_time_sec: None,
    };

    // 50% of 10:00 (600s) = 300s
    assert_eq!(vid.resolved_resume_sec(), Some(300));

    // Explicit resume_time_sec overrides progress_pct calculation
    vid.resume_time_sec = Some(420);
    assert_eq!(vid.resolved_resume_sec(), Some(420));
}

#[test]
fn test_parse_structured_description_content_renderer() {
    let raw = serde_json::json!({
        "engagementPanels": [
            {
                "engagementPanelSectionListRenderer": {
                    "panelIdentifier": "engagement-panel-structured-description",
                    "content": {
                        "structuredDescriptionContentRenderer": {
                            "items": [
                                {
                                    "videoDescriptionHeaderRenderer": {
                                        "title": { "runs": [{ "text": "Header" }] }
                                    }
                                },
                                {
                                    "expandableVideoDescriptionBodyRenderer": {
                                        "descriptionBodyText": {
                                            "runs": [
                                                { "text": "Detailed tracklist and links\nhttps://example.com" }
                                            ]
                                        }
                                    }
                                }
                            ]
                        }
                    }
                }
            }
        ]
    });

    let watch = parse_watch_data(&raw).expect("watch data parsed");
    assert_eq!(
        watch.description,
        "Detailed tracklist and links\nhttps://example.com"
    );
}

#[test]
fn test_shorts_badge_detection() {
    use yute_core::backend::parse::parse_video_renderer;

    let raw = serde_json::json!({
        "videoId": "short123",
        "title": { "runs": [{ "text": "Crazy Quick Short" }] },
        "navigationEndpoint": {
            "reelWatchEndpoint": {
                "videoId": "short123"
            }
        }
    });

    let card = parse_video_renderer(&raw).expect("parsed video renderer");
    assert!(card.badges.contains(&"SHORTS".to_string()));
}

#[test]
fn test_sponsorblock_tracker_skipping_and_cooldown() {
    use yute_core::backend::sponsorblock::{SponsorBlockTracker, SponsorSegment};

    let segments = vec![
        SponsorSegment {
            category: "sponsor".to_string(),
            action_type: "skip".to_string(),
            segment: (10.0, 30.0),
            uuid: "uuid-sponsor-1".to_string(),
        },
        SponsorSegment {
            category: "selfpromo".to_string(),
            action_type: "skip".to_string(),
            segment: (60.0, 75.0),
            uuid: "uuid-promo-2".to_string(),
        },
    ];

    let mut tracker = SponsorBlockTracker::new("vid_test", segments);

    // 1. Before segment: no skip
    assert_eq!(tracker.check_position(5.0), None);

    // 2. Enters segment (10.0): triggers skip
    let skip1 = tracker.check_position(10.5);
    assert!(skip1.is_some());
    let (target, msg) = skip1.unwrap();
    assert!((target - 30.1).abs() < 1e-4);
    assert!(msg.contains("Skipped sponsor"));

    // 3. Position still reported in segment during decoder lag: NO repeat skip because UUID is marked
    assert_eq!(tracker.check_position(11.0), None);

    // 4. Position at 31.0: outside segment, no skip
    assert_eq!(tracker.check_position(31.0), None);

    // 5. Seek back before first segment: clears UUID tracking
    tracker.on_manual_seek(5.0);
    // When playing forward and entering segment again, it can skip
    let skip_again = tracker.check_position(10.2);
    assert!(skip_again.is_some());
}

#[test]
fn protocol_relative_channel_avatars_are_https() {
    let card = yute_core::backend::parse::parse_card(&serde_json::json!({
        "lockupViewModel": {
            "contentId":"UCfixture", "contentType":"LOCKUP_CONTENT_TYPE_CHANNEL",
            "metadata":{"lockupMetadataViewModel":{"title":{"content":"Channel"}}},
            "contentImage":{"thumbnailViewModel":{"image":{"sources":[{"url":"//yt3.ggpht.com/avatar", "width":160, "height":160}]}}}
        }
    })).unwrap();
    let yute_core::backend::models::Card::Channel(channel) = card else {
        panic!("expected channel")
    };
    assert_eq!(
        channel.avatar_url.as_deref(),
        Some("https://yt3.ggpht.com/avatar")
    );
}

#[test]
fn mix_pages_only_include_videos_from_the_requested_mix() {
    use yute_core::backend::{models::mix_seed_video_id, parse::parse_mix_page};
    let tile = |id: &str, playlist: &str| {
        serde_json::json!({"tileRenderer":{
            "contentType":"TILE_CONTENT_TYPE_VIDEO",
            "onSelectCommand":{"watchEndpoint":{"videoId":id,"playlistId":playlist}},
            "metadata":{"tileMetadataRenderer":{"title":{"simpleText":"Song"}}}
        }})
    };
    let value = serde_json::json!({"contents":[tile("abcdefghijk","RDabcdefghijk"),tile("other-video","RDdifferent"),tile("next-video1","RDabcdefghijk"),tile("abcdefghijk","RDabcdefghijk")]});
    let page = parse_mix_page(&value, "RDabcdefghijk");
    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].id(), "abcdefghijk");
    assert_eq!(page.items[1].id(), "next-video1");
    assert_eq!(mix_seed_video_id("RDabcdefghijk"), Some("abcdefghijk"));
    assert!(mix_seed_video_id("RDGMopaque-playlist").is_none());
}

#[test]
fn comments_keep_authors_unicode_and_real_action_params() {
    use serde_json::json;
    let raw = json!({"contents":[{"commentRenderer":{
        "commentId":"c1","authorText":{"simpleText":"Reader"},
        "authorEndpoint":{"browseEndpoint":{"canonicalBaseUrl":"/@reader"}},
        "contentText":{"runs":[{"text":"Great हिन्दी "},{"emoji":{"emojiId":"\u{1f44d}"}}]},
        "actionButtons":{"commentActionButtonsRenderer":{
            "likeButton":{"toggleButtonRenderer":{"defaultServiceEndpoint":{"performCommentActionEndpoint":{"action":"test-like-opaque"}}}},
            "dislikeButton":{"toggleButtonRenderer":{"defaultServiceEndpoint":{"performCommentActionEndpoint":{"action":"test-dislike-opaque"}}}}
        }}, "reply":{"createReplyParams":"test-reply-opaque"}
    }}], "createCommentParams":"test-create-opaque"});
    let batch = yute_core::backend::parse::parse_comments(&raw);
    assert_eq!(batch.comments.len(), 1);
    let comment = &batch.comments[0];
    assert_eq!(comment.author, "Reader");
    assert_eq!(comment.author_handle.as_deref(), Some("@reader"));
    assert_eq!(comment.text, "Great हिन्दी \u{1f44d}");
    assert_eq!(comment.like_action.as_deref(), Some("test-like-opaque"));
    assert_eq!(comment.reply_params.as_deref(), Some("test-reply-opaque"));
    assert_eq!(batch.create_params.as_deref(), Some("test-create-opaque"));
    let persisted = serde_json::to_string(&batch).unwrap();
    assert!(!persisted.contains("opaque"));
}

#[test]
fn http_only_cookie_rows_are_not_mistaken_for_comments() {
    let mut auth = yute_core::backend::auth::AuthContext::default();
    auth.parse_netscape_cookies("# Netscape cookie file\n#HttpOnly_.youtube.com\tTRUE\t/\tTRUE\t0\tSID\ttest-session\n.youtube.com\tTRUE\t/\tTRUE\t0\tSAPISID\ttest-sapisid\n");
    assert_eq!(
        auth.cookies.get("SID").map(|secret| secret.expose()),
        Some("test-session")
    );
    assert_eq!(auth.sapisid.as_deref(), Some("test-sapisid"));
}

#[test]
fn playlist_destinations_only_use_actual_unsaved_server_options() {
    use serde_json::json;
    let raw = json!({"contents":[{"addToPlaylistRenderer":{"playlists":[
        {"playlistAddToOptionRenderer":{"playlistId":"PLavailable","title":{"simpleText":"Learning"},"containsSelectedVideos":"NONE"}},
        {"playlistAddToOptionRenderer":{"playlistId":"PLsaved","title":{"simpleText":"Saved already"},"containsSelectedVideos":"ALL"}},
        {"playlistAddToOptionRenderer":{"playlistId":"PLreadonly","title":{"simpleText":"Read only"},"isEditable":false}}
    ]}}]});
    let page = yute_core::backend::parse::parse_playlist_destinations(&raw);
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id(), "PLavailable");
    assert!(matches!(page.items[0], Card::Playlist(_)));
    assert!(
        yute_core::backend::parse::parse_playlist_destinations(&json!({}))
            .items
            .is_empty()
    );
}

#[test]
fn channel_avatar_renderer_urls_are_normalized() {
    for renderer in [
        "channelRenderer",
        "gridChannelRenderer",
        "pivotChannelRenderer",
        "compactChannelRenderer",
    ] {
        let value = serde_json::json!({renderer:{"channelId":"UCchannel","title":{"simpleText":"Channel"},"thumbnail":{"thumbnails":[{"url":"//yt3.ggpht.com/avatar=s88-c-k-c0x00ffffff-no-rj","width":88,"height":88}]}}});
        let card = yute_core::backend::parse::parse_card(&value).unwrap();
        let Card::Channel(channel) = card else {
            panic!("channel expected")
        };
        assert_eq!(
            channel.avatar_url.as_deref(),
            Some("https://yt3.ggpht.com/avatar=s88-c-k-c0x00ffffff-no-rj")
        );
    }
}

#[test]
fn avatar_sizes_preserve_google_flags_queries_and_unknown_hosts() {
    use yute_core::media::thumbs::avatar_thumbnail_url;
    assert_eq!(
        avatar_thumbnail_url("//yt3.ggpht.com/avatar=s88-c-k-c0x00ffffff-no-rj?x=1", 256),
        "https://yt3.ggpht.com/avatar=s256-c-k-c0x00ffffff-no-rj?x=1"
    );
    assert_eq!(
        avatar_thumbnail_url(
            "https://yt3.googleusercontent.com/avatar=w88-h88-p-k-no-rj",
            200
        ),
        "https://yt3.googleusercontent.com/avatar=w200-h200-p-k-no-rj"
    );
    assert_eq!(
        avatar_thumbnail_url("https://example.com/id=s88", 200),
        "https://example.com/id=s88"
    );
    assert_eq!(
        avatar_thumbnail_url("https://yt3.ggpht.com/id=opaque-token", 200),
        "https://yt3.ggpht.com/id=opaque-token"
    );
}

#[test]
fn legacy_cookie_material_is_never_sent_as_authentication() {
    let mut auth = yute_core::backend::auth::AuthContext::default();
    auth.parse_netscape_cookies(".youtube.com\tTRUE\t/\tTRUE\t0\tSAPISID\tlegacy-value\n");
    let headers = auth.build_headers();
    assert!(!headers.contains_key(reqwest::header::COOKIE));
    assert!(!headers.contains_key(reqwest::header::AUTHORIZATION));
}

#[test]
fn tv_channel_header_avatar_is_extracted_for_home_enrichment() {
    let value = serde_json::json!({"header":{"channelHeaderRenderer":{"title":{"simpleText":"Channel"},"avatar":{"thumbnails":[{"url":"//yt3.ggpht.com/avatar=s72-c-k-no-rj","width":72,"height":72}]}}}});
    let page = yute_core::backend::parse::parse_channel_page(&value, "UCchannel");
    assert_eq!(
        page.channel.avatar_url.as_deref(),
        Some("https://yt3.ggpht.com/avatar=s72-c-k-no-rj")
    );
}

#[tokio::test]
#[ignore = "live anonymous YouTube/CDN check; run explicitly"]
async fn live_public_channel_avatar_download() {
    use yute_core::backend::{auth::AuthContext, innertube::InnerTubeBackend, Backend};
    let backend = InnerTubeBackend::new(AuthContext::default());
    let page = backend
        .channel_page("UCX6OQ3DkcsbYNE6H8uQQuVA", None)
        .await
        .unwrap();
    let avatar = page.channel.avatar_url.expect("TV channel header avatar");
    let dir = tempfile::tempdir().unwrap();
    let image = yute_core::media::thumbs::fetch_thumbnail(
        &avatar,
        256,
        256,
        &dir.path().join("avatar.jpg"),
    )
    .await
    .expect("decoded CDN avatar");
    assert!(image.width() > 0 && image.height() > 0);
}

#[test]
fn oauth_uses_tv_identity_and_never_legacy_cookie_headers() {
    let auth = yute_core::backend::auth::AuthContext {
        client_name: "WEB".into(),
        client_version: "legacy-web".into(),
        refresh_token: Some("fixture".into()),
        cookie_header: Some("legacy".into()),
        ..Default::default()
    };
    assert_eq!(
        auth.build_client_context()["client"]["clientName"],
        "TVHTML5"
    );
    assert_eq!(auth.build_headers()["X-YouTube-Client-Name"], "7");
    assert!(!auth.build_headers().contains_key(reqwest::header::COOKIE));
}
#[test]
fn configuration_parse_errors_never_echo_values() {
    let error = yute_core::config::Config::parse_checked("[api]\nkey = secret-fixture")
        .unwrap_err()
        .to_string();
    assert!(!error.contains("secret-fixture"));
    assert!(error.contains("Invalid config.toml"));
}
#[test]
fn nested_avatar_views_and_unknown_sizing_urls_are_safe() {
    use yute_core::{backend::parse::extract_best_thumbnail, media::thumbs::avatar_thumbnail_url};
    let value = serde_json::json!({"avatar":{"avatarViewModel":{"image":{"sources":[{"url":"//yt3.ggpht.com/id=s88-c-k-c0x00ffffff-no-rj","width":88}]}}}});
    assert!(extract_best_thumbnail(&value).starts_with("https://yt3.ggpht.com/"));
    let url = "https://yt3.ggpht.com/opaque=unknown?key=preserved";
    assert_eq!(avatar_thumbnail_url(url, 200), url);
    let sized = avatar_thumbnail_url("//yt3.ggpht.com/opaque=w88-h88-c-k-no-rj?x=1", 200);
    assert_eq!(
        sized,
        "https://yt3.ggpht.com/opaque=w200-h200-c-k-no-rj?x=1"
    );
}

#[test]
fn channel_avatar_view_model_ignores_empty_sources() {
    let card = yute_core::backend::parse::parse_card(&serde_json::json!({
        "channelRenderer": {"channelId":"UCfixture", "title":{"simpleText":"Channel"},
            "avatarViewModel":{"image":{"sources":[{"url":"//yt3.ggpht.com/valid=s88-c-k-no-rj"},{"url":""}]}}}
    })).unwrap();
    let yute_core::backend::models::Card::Channel(channel) = card else {
        panic!("typed channel expected");
    };
    assert_eq!(
        channel.avatar_url.as_deref(),
        Some("https://yt3.ggpht.com/valid=s88-c-k-no-rj")
    );
}
