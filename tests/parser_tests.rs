use serde_json::Value;
use ytui::backend::models::{Card, LiveState};
use ytui::backend::parse::{parse_feed_page, parse_watch_data};

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

    assert_eq!(page.continuation.as_deref(), Some("continuation_home_page_2"));
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
    assert_eq!(watch.video.title, "Building a High-Performance YouTube TUI in Rust");
    assert_eq!(watch.video.channel, "Antigravity Engineering");
    assert_eq!(watch.video.duration.as_deref(), Some("20:45"));
    assert!(watch.description.contains("Deep dive into building an interactive YouTube TUI"));
}
