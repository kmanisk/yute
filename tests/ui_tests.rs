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

#[test]
fn test_search_input_visible_and_typable() {
    use ytui::app::Focus;

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = create_mock_app();

    app.focus = Focus::SearchInput;
    app.search_query = "rust ratatui".to_string();

    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buffer = terminal.backend().buffer();

    // Line 1 should contain search text inside the 3-height header
    let line1: String = (0..80).map(|x| buffer[(x, 1)].symbol()).collect();
    assert!(
        line1.contains("rust ratatui"),
        "Search query text must be rendered and visible in row 1 of header, got: {line1}"
    );
}

#[test]
fn test_settings_screen_rendering_and_cycling() {
    use ytui::app::Screen;

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = create_mock_app();

    app.set_screen(Screen::Settings);
    app.settings_index = 0; // Max Resolution

    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buffer = terminal.backend().buffer();

    let full_text: String = (0..24)
        .map(|y| {
            let line: String = (0..80).map(|x| buffer[(x, y)].symbol()).collect();
            line
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(full_text.contains("Settings & Playback Tuning"));
    assert!(full_text.contains("Max Resolution"));

    // Cycle max resolution forward
    assert_eq!(app.config.playback.max_resolution, "1080p");
    ui::settings::cycle_setting(&mut app, true);
    assert_eq!(app.config.playback.max_resolution, "720p");
    assert_eq!(
        app.config.playback.resolved_ytdl_format(),
        "bestvideo[height<=?720]+bestaudio/best[height<=?720]/best"
    );

    // Reset back
    ui::settings::cycle_setting(&mut app, false);
    assert_eq!(app.config.playback.max_resolution, "1080p");
}

#[test]
fn test_nav_settings_keymap() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ytui::keymap::{Action, KeymapState};

    let mut keymap = KeymapState::new();

    let key_shift_o = KeyEvent::new(KeyCode::Char('O'), KeyModifiers::SHIFT);
    assert_eq!(keymap.handle_key(key_shift_o, false), Some((Action::NavSettings, 1)));

    let key_g = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
    assert_eq!(keymap.handle_key(key_g, false), None);

    let key_o = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE);
    assert_eq!(keymap.handle_key(key_o, false), Some((Action::NavSettings, 1)));
}

#[test]
fn test_3x3_grid_dimensions() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = create_mock_app();

    assert_eq!(app.config.grid.columns, 3);
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.area.width, 120);
    assert_eq!(buffer.area.height, 30);
}

#[test]
fn test_shift_enter_inline_playback_keymap() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ytui::keymap::{Action, KeymapState};

    let mut keymap = KeymapState::new();

    let key_shift_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT);
    assert_eq!(
        keymap.handle_key(key_shift_enter, false),
        Some((Action::PlayInline, 1)),
        "Shift+Enter must trigger PlayInline action"
    );

    let key_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_enter, false),
        Some((Action::PlayDefault, 1)),
        "Enter must trigger PlayDefault action"
    );
}

#[test]
fn test_header_shows_account_and_ascii_logo() {
    let backend = TestBackend::new(140, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = create_mock_app();
    app.account_name = "Mathew Manisk".to_string();
    app.account_initial = "M".to_string();

    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buffer = terminal.backend().buffer();

    // Check header rows (0..3) for logo and account details
    let mut found_logo = false;
    let mut found_account = false;
    for y in 0..3 {
        let line: String = (0..140).map(|x| buffer[(x, y)].symbol()).collect();
        if line.contains("|> YouTube") {
            found_logo = true;
        }
        if line.contains("Mathew Manisk") && line.contains("[M]") {
            found_account = true;
        }
    }

    assert!(found_logo, "Header must render ASCII '|> YouTube' logo");
    assert!(found_account, "Header must render user account pill with initial and name at top-right");
}

#[test]
fn test_accounts_screen_switching() {
    let mut app = create_mock_app();
    assert_eq!(app.account_name, "I am Hunter");
    assert_eq!(app.account_handle, "@iamhunter2570");
    assert_eq!(app.account_initial, "I");

    // Switch to Mathew Manisk (index 1)
    let switched = app.switch_account(1);
    assert!(switched);
    assert_eq!(app.active_account_index, 1);
    assert_eq!(app.account_name, "Mathew Manisk");
    assert_eq!(app.account_handle, "@mathewmanisk5707");
    assert_eq!(app.account_initial, "M");

    // Switch back to I am Hunter (index 0)
    let switched_back = app.switch_account(0);
    assert!(switched_back);
    assert_eq!(app.active_account_index, 0);
    assert_eq!(app.account_name, "I am Hunter");
    assert_eq!(app.account_handle, "@iamhunter2570");
}

#[test]
fn test_accounts_keymap() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ytui::keymap::{Action, KeymapState};

    let mut keymap = KeymapState::new();

    let key_shift_a = KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT);
    assert_eq!(
        keymap.handle_key(key_shift_a, false),
        Some((Action::NavAccounts, 1)),
        "Shift+A must map to NavAccounts"
    );

    let key_g = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
    assert_eq!(keymap.handle_key(key_g, false), None);

    let key_a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_a, false),
        Some((Action::NavAccounts, 1)),
        "g a must map to NavAccounts"
    );
}

#[test]
fn test_theme_system_integration() {
    use ytui::theme::Theme;

    let sys = Theme::system_theme();
    assert!(sys.is_some(), "System theme should be successfully loaded from ~/.local/state/theme/colors.sh");
    let theme = sys.unwrap();
    assert_eq!(theme.bg, ratatui::style::Color::Reset, "System theme bg must be Color::Reset to merge with terminal");
}

#[test]
fn test_confirming_quit_modal() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = create_mock_app();
    assert_eq!(app.screen, ytui::app::Screen::Home);
    assert!(!app.confirming_quit);

    // Pressing nav_back at root home screen must set confirming_quit to true instead of immediate quitting
    app.nav_back();
    assert!(app.confirming_quit, "nav_back on root home screen must trigger confirming_quit");
    assert!(!app.should_quit, "nav_back on root home screen must NOT immediately quit");

    // Rendering draw with confirming_quit = true must succeed
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| ytui::ui::draw(f, &mut app)).unwrap();
}



