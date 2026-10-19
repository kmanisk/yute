use ratatui::backend::TestBackend;
use ratatui::Terminal;
use ytui::app::App;
use ytui::backend::models::{Card, Chip, LiveState, VideoCard};
use ytui::config::Config;
use ytui::ui;

fn create_mock_app() -> App {
    let picker = ratatui_image::picker::Picker::halfblocks();
    let mut app = App::new(Config::default(), picker);

    app.chips = vec![
        Chip { title: "All".to_string(), token: None, active: true },
        Chip { title: "Gaming".to_string(), token: None, active: false },
        Chip { title: "Linux".to_string(), token: None, active: false },
        Chip { title: "Music".to_string(), token: None, active: false },
    ];

    for i in 1..=12 {
        app.items.push(Card::Video(VideoCard {
            id: format!("test_vid_{i}"),
            title: format!("Test Video Title Number {i} with Extended Description"),
            channel: format!("Channel {i}"),
            channel_id: None,
            verified: i % 2 == 0,
            duration: Some(format!("{}:30", 10 + i)),
            views: Some(format!("{}K views", i * 15)),
            published: Some("3 days ago".to_string()),
            thumb_url: String::new(),
            progress_pct: if i == 1 { Some(65) } else { None },
            live: if i == 2 { LiveState::Live } else { LiveState::None },
            badges: Vec::new(),
        }));
    }

    app
}

#[test]
fn test_render_at_80x24() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = create_mock_app();

    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.area.width, 80);
    assert_eq!(buffer.area.height, 24);
}

#[test]
fn test_render_at_120x40() {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = create_mock_app();

    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.area.width, 120);
    assert_eq!(buffer.area.height, 40);
}

#[test]
fn test_render_at_200x60() {
    let backend = TestBackend::new(200, 60);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = create_mock_app();

    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.area.width, 200);
    assert_eq!(buffer.area.height, 60);
}

#[test]
fn test_collapsed_sidebar_shows_queue_at_80x24() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = create_mock_app();
    app.sidebar_expanded = false;

    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buffer = terminal.backend().buffer();

    let mut found_queue = false;
    for y in 0..24 {
        let line: String = (0..12).map(|x| buffer[(x, y)].symbol()).collect();
        if line.contains("[Q]") {
            found_queue = true;
            break;
        }
    }
    assert!(found_queue, "Queue [Q] must be visible in collapsed sidebar on 80x24 terminal");
}

#[test]
fn test_capital_letter_navigation_keymaps() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ytui::keymap::{Action, KeymapState};

    let mut keymap = KeymapState::new();

    let check_key = |keymap: &mut KeymapState, c: char, expected: Action| {
        let key = KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT);
        let res = keymap.handle_key(key, false);
        assert_eq!(res, Some((expected, 1)), "Key '{c}' should map to {expected:?}");
    };

    check_key(&mut keymap, 'H', Action::NavHome);
    check_key(&mut keymap, 'S', Action::NavSubs);
    check_key(&mut keymap, 'Y', Action::NavHistory);
    check_key(&mut keymap, 'P', Action::NavPlaylists);
    check_key(&mut keymap, 'W', Action::NavWatchLater);
    check_key(&mut keymap, 'L', Action::NavLiked);
    check_key(&mut keymap, 'V', Action::NavLocal);
    check_key(&mut keymap, 'Q', Action::NavQueue);
}

#[test]
fn test_thumbnail_cache_key_canonicalization() {
    use ytui::media::thumbs::cache_key_for_url;

    let url1 = "https://i.ytimg.com/vi/xnP7qKxwzjg/hqdefault.jpg?sqp=-oaymwEbCKgBEF5IVfKriqkDDggBFQAAiEIYAXABwAEG&rs=AOn4CLB7K3yljM3gHdl2Dv3MFIzcVIS0_Q";
    let url2 = "https://i.ytimg.com/vi/xnP7qKxwzjg/hqdefault.jpg?sqp=different_token&rs=different_salt";
    let key1 = cache_key_for_url(url1);
    let key2 = cache_key_for_url(url2);

    assert_eq!(key1, "yt_xnP7qKxwzjg");
    assert_eq!(key2, "yt_xnP7qKxwzjg");
    assert_eq!(key1, key2, "Canonical cache keys must match regardless of transient query parameters");
}
