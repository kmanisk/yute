use yute_core::backend::{models::Card, parse::parse_card};

#[test]
fn metadata_fixtures_preserve_dates_and_ignore_separator_only_fields() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_slice(include_bytes!("fixtures/metadata.json")).unwrap();
    for case in cases {
        let card = parse_card(&case["card"]).unwrap();
        let expected = case["date"].as_str();
        match card {
            Card::Video(video) => {
                assert_eq!(video.published.as_deref(), expected, "{}", case["name"]);
                assert_eq!(
                    video.views.as_deref(),
                    case["views"].as_str(),
                    "{}",
                    case["name"]
                );
            }
            Card::Playlist(playlist) => assert_eq!(playlist.updated.as_deref(), expected),
            _ => panic!("Unexpected fixture entity"),
        }
    }
}
#[test]
fn tv_owned_playlist_browse_identity_is_preserved_and_missing_identity_is_rejected() {
    let mut tile = serde_json::json!({"tileRenderer":{
        "contentType":"TILE_CONTENT_TYPE_PLAYLIST",
        "metadata":{"tileMetadataRenderer":{"title":{"simpleText":"Fixture playlist"}}},
        "onSelectCommand":{"browseEndpoint":{"browseId":"VLPLfixture"}}
    }});
    assert!(matches!(parse_card(&tile), Some(Card::Playlist(p)) if p.id == "VLPLfixture"));
    tile["tileRenderer"]["onSelectCommand"] = serde_json::json!({});
    assert!(parse_card(&tile).is_none());
    tile["tileRenderer"]["onSelectCommand"] =
        serde_json::json!({"browseEndpoint":{"browseId":"UCfixture"}});
    assert!(parse_card(&tile).is_none());
}
