use ratatui::backend::TestBackend;
use ratatui::Terminal;
use yute::app::App;
use yute::backend::models::{Card, Chip, LiveState, VideoCard};
use yute::config::Config;
use yute::ui;

fn create_mock_app() -> App {
    let picker = ratatui_image::picker::Picker::halfblocks();
    let mut app = App::new(Config::default(), picker);

    app.chips = vec![
        Chip {
            title: "All".to_string(),
            token: None,
            active: true,
        },
        Chip {
            title: "Gaming".to_string(),
            token: None,
            active: false,
        },
        Chip {
            title: "Linux".to_string(),
            token: None,
            active: false,
        },
        Chip {
            title: "Music".to_string(),
            token: None,
            active: false,
        },
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
            live: if i == 2 {
                LiveState::Live
            } else {
                LiveState::None
            },
            badges: Vec::new(),
            resume_time_sec: None,
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
fn test_expanded_sidebar_shows_queue_at_80x24() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = create_mock_app();
    app.sidebar_expanded = true;

    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buffer = terminal.backend().buffer();

    let mut found_queue = false;
    for y in 0..24 {
        let line: String = (0..24).map(|x| buffer[(x, y)].symbol()).collect();
        if line.contains("gq") || line.contains("Queue") {
            found_queue = true;
            break;
        }
    }
    assert!(
        found_queue,
        "Queue [gq] must be visible in expanded sidebar on 80x24 terminal"
    );
}

#[test]
fn test_capital_letter_navigation_keymaps() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use yute::keymap::{Action, KeymapState};

    let mut keymap = KeymapState::new();

    let check_key = |keymap: &mut KeymapState, c: char, expected: Action| {
        let key = KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT);
        let res = keymap.handle_key(key, false);
        assert_eq!(
            res,
            Some((expected, 1)),
            "Key '{c}' should map to {expected:?}"
        );
    };

    // Shift+H and Shift+L navigate back/forward in browser history
    check_key(&mut keymap, 'H', Action::NavBackHistory);
    check_key(&mut keymap, 'L', Action::NavForwardHistory);
    check_key(&mut keymap, 'S', Action::ToggleSubscribe);
    check_key(&mut keymap, 'Y', Action::NavHistory);
    check_key(&mut keymap, 'P', Action::NavPlaylists);
    check_key(&mut keymap, 'W', Action::NavWatchLater);
    check_key(&mut keymap, 'V', Action::NavLocal);
    check_key(&mut keymap, 'Q', Action::NavQueue);

    // Test 'g' prefix shortcuts
    let mut check_g_prefix = |c: char, expected: Action| {
        let key1 = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        let res1 = keymap.handle_key(key1, false);
        assert_eq!(res1, None, "Prefix 'g' should wait for second key");

        let key2 = KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        let res2 = keymap.handle_key(key2, false);
        assert_eq!(
            res2,
            Some((expected, 1)),
            "Key 'g{c}' should map to {expected:?}"
        );
    };

    check_g_prefix('h', Action::NavHome);
    check_g_prefix('s', Action::NavSubs);
    check_g_prefix('y', Action::NavHistory);
    check_g_prefix('p', Action::NavPlaylists);
    check_g_prefix('w', Action::NavWatchLater);
    check_g_prefix('l', Action::NavLiked);
    check_g_prefix('v', Action::NavLocal);
    check_g_prefix('q', Action::NavQueue);
    check_g_prefix('o', Action::NavSettings);
    check_g_prefix('a', Action::NavAccounts);
}

#[test]
fn test_thumbnail_cache_key_canonicalization() {
    use yute::media::thumbs::cache_key_for_url;

    let url1 = "https://i.ytimg.com/vi/xnP7qKxwzjg/hqdefault.jpg?sqp=-oaymwEbCKgBEF5IVfKriqkDDggBFQAAiEIYAXABwAEG&rs=AOn4CLB7K3yljM3gHdl2Dv3MFIzcVIS0_Q";
    let url2 =
        "https://i.ytimg.com/vi/xnP7qKxwzjg/hqdefault.jpg?sqp=different_token&rs=different_salt";
    let key1 = cache_key_for_url(url1);
    let key2 = cache_key_for_url(url2);

    assert_eq!(key1, "yt_xnP7qKxwzjg");
    assert_eq!(key2, "yt_xnP7qKxwzjg");
    assert_eq!(
        key1, key2,
        "Canonical cache keys must match regardless of transient query parameters"
    );
}

#[test]
fn test_search_input_visible_and_typable() {
    use yute::app::Focus;

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = create_mock_app();

    app.focus = Focus::SearchInput;
    app.search_input = "rust ratatui".to_string();

    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buffer = terminal.backend().buffer();

    // Search temporarily occupies the bottom status row.
    let line1: String = (0..80).map(|x| buffer[(x, 23)].symbol()).collect();
    assert!(
        line1.contains("rust ratatui"),
        "Search query text must be rendered and visible in the SEARCH status row, got: {line1}"
    );
}

#[test]
fn test_settings_screen_rendering_and_cycling() {
    use yute::app::Screen;

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

    assert!(full_text.contains("Preferences"));
    assert!(full_text.contains("Video quality"));

    assert_eq!(app.config.playback.max_resolution, "1080p");
    ui::settings::cycle_setting(&mut app, true);
    assert_eq!(app.config.playback.max_resolution, "1440p");
    assert!(app.config.playback.resolved_ytdl_format().contains("1440"));
    ui::settings::cycle_setting(&mut app, false);
    assert_eq!(app.config.playback.max_resolution, "1080p");
}

#[test]
fn test_nav_settings_keymap() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use yute::keymap::{Action, KeymapState};

    let mut keymap = KeymapState::new();

    let key_shift_o = KeyEvent::new(KeyCode::Char('O'), KeyModifiers::SHIFT);
    assert_eq!(
        keymap.handle_key(key_shift_o, false),
        Some((Action::NavSettings, 1))
    );

    let key_g = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
    assert_eq!(keymap.handle_key(key_g, false), None);

    let key_o = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_o, false),
        Some((Action::NavSettings, 1))
    );
}

#[test]
fn test_fixed_three_columns_preserve_selection_when_scrolling() {
    let mut app = create_mock_app();
    app.config.grid.columns = 3;
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    assert_eq!(app.rendered_columns, 3);
    app.selected_index = 9;
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let text: String = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(text.contains("Channel 10"));
    assert_eq!(app.selected_card().unwrap().id(), "test_vid_10");
}

#[test]
fn test_shift_enter_opens_item_actions_keymap() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use yute::keymap::{Action, KeymapState};

    let mut keymap = KeymapState::new();

    let key_shift_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT);
    assert_eq!(
        keymap.handle_key(key_shift_enter, false),
        Some((Action::ItemActions, 1)),
        "Shift+Enter must open the item menu"
    );

    let key_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_enter, false),
        Some((Action::PlayDefault, 1)),
        "Enter must trigger PlayDefault action"
    );
}

#[test]
fn test_header_shows_route_filters_and_account_once() {
    let mut app = create_mock_app();
    app.account_name = "Private account name".into();
    {
        let mut auth = app.auth.lock().unwrap();
        auth.access_token = Some("test-access".into());
        auth.token_expires_at = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                + 3600,
        );
        auth.needs_login = false;
    }
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let header: String = (0..120)
        .map(|x| terminal.backend().buffer()[(x, 0)].symbol())
        .collect();
    assert!(header.contains("Home") && header.contains("All"));
    assert!(header.contains("Private account"));
    assert!(!header.contains("OAuth"));
    assert!(!header.contains("yute ·"));
}

#[test]
fn test_account_switching_rejects_unknown_identity() {
    let mut app = create_mock_app();
    let name = app.account_name.clone();
    assert!(!app.switch_account(usize::MAX));
    assert_eq!(app.account_name, name);
    assert!(app.status_text.contains("unavailable"));
}

#[test]
fn test_accounts_keymap() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use yute::keymap::{Action, KeymapState};

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
fn test_terminal_theme_follows_terminal_defaults() {
    use yute::theme::Theme;

    let theme = Theme::terminal();
    assert_eq!(theme.text, ratatui::style::Color::Reset);
    assert_eq!(theme.bg, ratatui::style::Color::Reset);
}

#[test]
fn test_confirming_quit_modal() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let mut app = create_mock_app();
    assert_eq!(app.screen, yute::app::Screen::Home);
    assert!(!app.confirming_quit);

    // Pressing nav_back at root home screen must set confirming_quit to true instead of immediate quitting
    app.nav_back();
    assert!(
        app.confirming_quit,
        "nav_back on root home screen must trigger confirming_quit"
    );
    assert!(
        !app.should_quit,
        "nav_back on root home screen must NOT immediately quit"
    );

    // Rendering draw with confirming_quit = true must succeed
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| yute::ui::draw(f, &mut app)).unwrap();
}

#[test]
fn test_nav_history_stack_back_and_forward() {
    use yute::app::Screen;
    use yute::backend::models::{Card, LiveState, VideoCard};

    let mut app = create_mock_app();
    assert_eq!(app.screen, Screen::Home);

    let make_vid = |id: &str, title: &str| {
        Card::Video(VideoCard {
            id: id.to_string(),
            title: title.to_string(),
            channel: "Test Channel".to_string(),
            channel_id: None,
            verified: false,
            duration: Some("10:00".to_string()),
            views: Some("1K views".to_string()),
            published: None,
            thumb_url: String::new(),
            progress_pct: None,
            live: LiveState::None,
            badges: Vec::new(),
            resume_time_sec: None,
        })
    };

    // 1. Initial Home screen state
    app.items = vec![
        make_vid("h1", "Home 1"),
        make_vid("h2", "Home 2"),
        make_vid("h3", "Home 3"),
    ];
    app.selected_index = 2;

    // 2. Navigate to Search screen
    app.set_screen(Screen::Search);
    app.search_query = "rust ratatui".to_string();
    app.items = vec![make_vid("s1", "Search 1"), make_vid("s2", "Search 2")];
    app.selected_index = 1;

    // 3. Navigate to Channel/Playlist view
    app.set_screen(Screen::PlaylistView);
    app.items = vec![
        make_vid("p1", "Playlist 1"),
        make_vid("p2", "Playlist 2"),
        make_vid("p3", "Playlist 3"),
        make_vid("p4", "Playlist 4"),
    ];
    app.selected_index = 3;

    // 4. Navigate back: should restore Search screen exactly
    app.nav_back();
    assert_eq!(app.screen, Screen::Search);
    assert_eq!(app.selected_index, 1);
    assert_eq!(app.search_query, "rust ratatui");
    assert_eq!(app.items.len(), 2);
    assert_eq!(app.items[0].title(), "Search 1");

    // 5. Navigate back again: should restore Home screen exactly
    app.nav_back();
    assert_eq!(app.screen, Screen::Home);
    assert_eq!(app.selected_index, 2);
    assert_eq!(app.items.len(), 3);
    assert_eq!(app.items[2].title(), "Home 3");

    // 6. Navigate forward: should restore Search screen exactly
    app.nav_forward();
    assert_eq!(app.screen, Screen::Search);
    assert_eq!(app.selected_index, 1);
    assert_eq!(app.search_query, "rust ratatui");
    assert_eq!(app.items.len(), 2);

    // 7. Navigate forward again: should restore PlaylistView screen exactly
    app.nav_forward();
    assert_eq!(app.screen, Screen::PlaylistView);
    assert_eq!(app.selected_index, 3);
    assert_eq!(app.items.len(), 4);
    assert_eq!(app.items[3].title(), "Playlist 4");
}

#[test]
fn test_unique_socket_generation() {
    use yute::player::ipc::generate_socket_path;

    let s1 = generate_socket_path();
    let s2 = generate_socket_path();
    let s3 = generate_socket_path();

    assert_ne!(s1, s2);
    assert_ne!(s2, s3);
    assert_ne!(s1, s3);

    let s1_str = s1.to_string_lossy();
    assert!(s1_str.contains("yute-mpv-"));
    assert!(s1_str.ends_with(".sock"));
}

#[test]
fn watch_keys_navigate_content_without_player_controls() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use yute::keymap::{Action, KeymapState};
    for (key, action) in [
        ('k', Action::Up),
        ('j', Action::Down),
        ('h', Action::Left),
        ('l', Action::Right),
        ('H', Action::NavBackHistory),
        ('L', Action::NavForwardHistory),
    ] {
        let mut map = KeymapState::new();
        assert_eq!(
            map.handle_key(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE), true),
            Some((action, 1))
        );
    }
    for key in [' ', ',', '.', '<', '>'] {
        let mut map = KeymapState::new();
        assert_eq!(
            map.handle_key(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE), true),
            None
        );
    }
}

#[test]
fn credential_debug_never_exposes_auth_values() {
    let mut auth = yute::backend::auth::AuthContext::default();
    auth.access_token = Some("access-fixture".into());
    auth.refresh_token = Some("refresh-fixture".into());
    auth.cookie_header = Some("cookie-fixture".into());
    auth.visitor_data = Some("visitor-fixture".into());
    let debug = format!("{auth:?}");
    for secret in [
        "access-fixture",
        "refresh-fixture",
        "cookie-fixture",
        "visitor-fixture",
    ] {
        assert!(!debug.contains(secret));
    }
}

#[test]
fn test_title_sanitization_and_word_truncation() {
    use yute::ui::text::{sanitize_title, wrap_and_truncate_title};

    // Trailing symbols stripping and whitespace collapsing
    let raw = "  Amazing Video Title | - — \\ :   ";
    let clean = sanitize_title(raw, false);
    assert_eq!(clean, "Amazing Video Title");

    // Trailing run of 2+ hashtags removed
    let with_hashtags = "Counter-Strike 2 Highlights #shorts #cs2 #gaming";
    let stripped = sanitize_title(with_hashtags, false);
    assert_eq!(stripped, "Counter-Strike 2 Highlights");

    // Single hashtag preserved
    let single_hashtag = "Counter-Strike 2 Highlights #gaming";
    let preserved = sanitize_title(single_hashtag, false);
    assert_eq!(preserved, "Counter-Strike 2 Highlights #gaming");

    // Word boundary truncation with ellipsis, never splitting words mid-way
    let long_title = "First Second Third Fourth Fifth";
    let wrapped = wrap_and_truncate_title(long_title, 14, 1);
    assert_eq!(wrapped.len(), 1);
    assert_eq!(wrapped[0], "First Second…");

    // Two lines word boundary truncation
    let wrapped_2 = wrap_and_truncate_title(long_title, 14, 2);
    assert_eq!(wrapped_2.len(), 2);
    assert_eq!(wrapped_2[0], "First Second");
    assert_eq!(wrapped_2[1], "Third Fourth…");
}

#[test]
fn test_emoji_handling_and_stripping() {
    use yute::ui::text::{is_emoji, sanitize_title, strip_emojis};

    assert!(is_emoji('\u{1F525}'));
    assert!(is_emoji('\u{1F3AE}'));
    assert!(!is_emoji('A'));
    assert!(!is_emoji('7'));

    let title_with_emoji = "Insane Play \u{1F525} Must Watch \u{1F3AE}";
    assert_eq!(strip_emojis(title_with_emoji), "Insane Play  Must Watch ");

    let sanitized_stripped = sanitize_title(title_with_emoji, true);
    assert_eq!(sanitized_stripped, "Insane Play Must Watch");

    let sanitized_kept = sanitize_title(title_with_emoji, false);
    assert_eq!(sanitized_kept, "Insane Play \u{1F525} Must Watch \u{1F3AE}");
}

#[test]
fn test_meta_line_formatting_priority() {
    use ratatui::style::Style;
    use yute::ui::text::format_meta_line;

    let style = Style::default();

    // Full width fits all components: channel, verified mark, views, age
    let full = format_meta_line(
        "Valve",
        true,
        Some("1.2M views"),
        Some("3 days ago"),
        50,
        style,
    );
    let full_text: String = full.into_iter().map(|s| s.content.to_string()).collect();
    assert_eq!(full_text, "Valve [v] · 1.2M views · 3 days ago");

    // Drops "YouTube Shorts" if passed as channel
    let shorts = format_meta_line("YouTube Shorts", false, Some("500K views"), None, 50, style);
    let shorts_text: String = shorts.into_iter().map(|s| s.content.to_string()).collect();
    assert_eq!(shorts_text, "500K views");

    // Medium width drops published age first before dropping views
    let medium = format_meta_line(
        "VeryLongChannelNameHere",
        false,
        Some("10K views"),
        Some("5 hours ago"),
        28,
        style,
    );
    let med_text: String = medium.into_iter().map(|s| s.content.to_string()).collect();
    assert!(med_text.contains("10K views"));
    assert!(!med_text.contains("5 hours ago"));

    // Narrow width truncates channel without cutting numbers/words mid-way
    let narrow = format_meta_line("SuperLongChannelName", false, None, None, 10, style);
    let narrow_text: String = narrow.into_iter().map(|s| s.content.to_string()).collect();
    assert_eq!(narrow_text, "SuperLong…");
}

#[test]
fn test_concord_pane_focus_cycling() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use yute::app::Focus;
    use yute::keymap::{Action, KeymapState};

    let mut keymap = KeymapState::new();

    let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(tab, false),
        Some((Action::CycleFocusForward, 1))
    );

    let backtab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(
        keymap.handle_key(backtab, false),
        Some((Action::CycleFocusBackward, 1))
    );

    let mut app = create_mock_app();
    app.focus = Focus::Sidebar;

    // Cycle forward: Sidebar -> Chips -> Content -> SearchInput -> Sidebar
    app.focus = match app.focus {
        Focus::Sidebar => Focus::Chips,
        Focus::Chips => Focus::Content,
        Focus::Content => Focus::SearchInput,
        Focus::SearchInput => Focus::Sidebar,
        _ => Focus::Content,
    };
    assert_eq!(app.focus, Focus::Chips);

    app.focus = match app.focus {
        Focus::Sidebar => Focus::Chips,
        Focus::Chips => Focus::Content,
        Focus::Content => Focus::SearchInput,
        Focus::SearchInput => Focus::Sidebar,
        _ => Focus::Content,
    };
    assert_eq!(app.focus, Focus::Content);

    // Cycle backward: Content -> Chips -> Sidebar
    app.focus = match app.focus {
        Focus::Sidebar => Focus::SearchInput,
        Focus::SearchInput => Focus::Content,
        Focus::Content => Focus::Chips,
        Focus::Chips => Focus::Sidebar,
        _ => Focus::Content,
    };
    assert_eq!(app.focus, Focus::Chips);
}

#[test]
fn test_view_mode_toggle_grid_and_list() {
    use yute::app::ViewMode;

    let mut app = create_mock_app();
    assert_eq!(app.current_view_mode(), ViewMode::Grid);

    app.toggle_view_mode();
    assert_eq!(app.current_view_mode(), ViewMode::List);

    app.toggle_view_mode();
    assert_eq!(app.current_view_mode(), ViewMode::Grid);
}

#[test]
fn test_loading_skeleton_and_badges_render() {
    let backend = TestBackend::new(120, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = create_mock_app();

    // Disable graphics so skeleton is rendered instead of graphical protocols
    app.config.general.graphics = "none".to_string();

    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buffer = terminal.backend().buffer();

    let full_text: String = (0..30)
        .map(|y| {
            (0..120)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Must render dim skeleton blocks '▒' without ugly "[Thumb loading...]" text
    assert!(
        full_text.contains('▒'),
        "Loading skeleton must use '▒' block characters"
    );
    assert!(
        !full_text.contains("[Thumb loading...]"),
        "Must not render raw placeholder text"
    );

    // Item 1 has a thin progress row at the bottom edge of its thumbnail.
    assert!(
        full_text.contains('█'),
        "Progress bar must be visible on partially watched video"
    );

    // Item 2 has LIVE badge (live: LiveState::Live)
    assert!(
        full_text.contains(" LIVE "),
        "Live badge must be rendered for active livestreams"
    );

    // Duration badge on video 1 (11:30)
    assert!(
        full_text.contains("11:30"),
        "Duration chip must be rendered on video card"
    );
}

#[test]
fn watch_reactions_and_comment_bindings() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use yute::keymap::{Action, KeymapState};
    for (key, action) in [
        ('+', Action::LikeVideo),
        ('=', Action::LikeVideo),
        ('-', Action::DislikeVideo),
        ('c', Action::ToggleComments),
        ('n', Action::ComposeComment),
        ('r', Action::ReplyComment),
        ('w', Action::FocusSuggestions),
        ('[', Action::PrevChip),
        (']', Action::NextChip),
    ] {
        let mut map = KeymapState::new();
        assert_eq!(
            map.handle_key(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE), true),
            Some((action, 1))
        );
    }
}

#[test]
fn test_default_hwdec_is_auto_safe() {
    let cfg = yute::config::Config::default();
    assert_eq!(cfg.playback.hwdec, "auto-safe");
}

#[test]
fn resize_and_sidebar_preserve_selected_identity() {
    let mut app = create_mock_app();
    app.config.grid.columns = 0;
    app.config.grid.min_card_width = 30;
    app.selected_index = 8;
    app.sidebar_expanded = true;
    let mut narrow = Terminal::new(TestBackend::new(90, 30)).unwrap();
    narrow.draw(|f| ui::draw(f, &mut app)).unwrap();
    let narrow_columns = app.rendered_columns;
    assert_eq!(app.selected_card().unwrap().id(), "test_vid_9");
    app.sidebar_expanded = false;
    let mut wide = Terminal::new(TestBackend::new(150, 40)).unwrap();
    wide.draw(|f| ui::draw(f, &mut app)).unwrap();
    assert!(app.rendered_columns > narrow_columns);
    assert_eq!(app.selected_card().unwrap().id(), "test_vid_9");
    assert_eq!(app.items.len(), 12);
    assert!(!app.is_loading);
}

#[test]
fn watch_round_trip_preserves_feed_position_and_continuation() {
    let mut app = create_mock_app();
    app.selected_index = 8;
    app.scroll_row = 2;
    app.continuation_token = Some("next-page".into());
    app.set_screen(yute::app::Screen::Watch);
    app.nav_back();
    assert_eq!(app.screen, yute::app::Screen::Home);
    assert_eq!(app.selected_index, 8);
    assert_eq!(app.scroll_row, 2);
    assert_eq!(app.continuation_token.as_deref(), Some("next-page"));
    assert_eq!(app.items.len(), 12);
}

#[test]
fn compact_footer_and_card_hierarchy() {
    let mut app = create_mock_app();
    app.theme = yute::theme::Theme::youtube_dark();
    app.focus = yute::app::Focus::Content;
    let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buffer = terminal.backend().buffer();
    let footer: String = (0..120).map(|x| buffer[(x, 39)].symbol()).collect();
    assert!(!footer.contains("NORMAL"));
    assert!(!footer.contains("Home"));
    assert!(footer.contains("1/12"));
    assert!(footer.contains("? help"));
    assert!(!footer.contains("Shift"));
    let rows: Vec<String> = (0..39)
        .map(|y| (0..120).map(|x| buffer[(x, y)].symbol()).collect())
        .collect();
    let channel_row = rows
        .iter()
        .position(|line| line.contains("Channel 1"))
        .unwrap();
    assert!(rows[channel_row + 1].contains("15K views"));
    assert!(rows[channel_row + 1].contains("3 days ago"));
    let accent_cells = buffer
        .content
        .iter()
        .filter(|cell| cell.fg == app.theme.accent)
        .count();
    assert!(
        accent_cells > 10,
        "Selected card needs a visible accent border"
    );
}

#[test]
fn hindi_truncation_preserves_graphemes_and_display_width() {
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;
    use yute::ui::text::{truncate_str, wrap_and_truncate_title};
    let title = "हिन्दी क्षत्रिय विज्ञान की दुनिया में आपका स्वागत है";
    for width in 0..45 {
        let truncated = truncate_str(title, width);
        assert!(truncated.width() <= width);
        let prefix = truncated.strip_suffix('…').unwrap_or(&truncated);
        assert!(title.starts_with(prefix));
        assert!(
            prefix.is_empty()
                || title
                    .grapheme_indices(true)
                    .any(|(offset, g)| offset + g.len() == prefix.len())
        );
        let wrapped = wrap_and_truncate_title(title, width, 2);
        assert!(wrapped.len() <= 2);
        for line in wrapped {
            assert!(line.width() <= width, "{line:?} width {width}");
        }
    }
}

#[test]
fn account_feed_errors_and_legacy_state_remain_actionable() {
    use yute::app::Screen;
    let mut app = create_mock_app();
    app.items.clear();
    app.screen = Screen::Subscriptions;
    app.auth.lock().unwrap().needs_login = true;
    app.account_name = "Legacy account".into();
    assert_eq!(app.session_label(), "Needs re-login · Legacy account");
    assert!(app.empty_feed_message().contains(":account add"));
    {
        let mut auth = app.auth.lock().unwrap();
        auth.needs_login = false;
        auth.refresh_token = Some("fixture".into());
    }
    app.feed_error = Some("YouTube rejected this session — retry".into());
    app.status_text.clear();
    assert!(app.empty_feed_message().contains("rejected"));
    assert!(app.session_label().starts_with("OAuth ·"));
}

#[test]
fn wide_grid_fills_content_and_uses_more_than_three_columns() {
    use ratatui::layout::Rect;
    use yute::ui::grid::GridGeometry;
    let config = Config::default();
    for width in [180, 210, 240, 280] {
        let geometry = GridGeometry::new(Rect::new(0, 0, width, 50), &config.grid, 9., 18.);
        assert!(geometry.columns > 3 && geometry.columns <= 8);
        assert!((30..=44).contains(&geometry.card_width));
        assert!(geometry.first_column_offset <= 4);
        let physical_aspect =
            geometry.thumbnail_width as f32 * 9. / (geometry.thumbnail_height as f32 * 18.);
        assert!((physical_aspect - 16. / 9.).abs() < 0.3);
    }
}

#[test]
fn semantic_icons_keep_all_modes_usable() {
    use unicode_width::UnicodeWidthStr;
    use yute::ui::icons::Icon;
    for icon in [
        Icon::Home,
        Icon::Subscriptions,
        Icon::History,
        Icon::WatchLater,
        Icon::Like,
        Icon::Dislike,
        Icon::Playlists,
        Icon::Queue,
        Icon::Local,
        Icon::Account,
        Icon::Settings,
        Icon::Play,
        Icon::Audio,
        Icon::Download,
    ] {
        for mode in ["auto", "nerd", "unicode", "ascii", "none"] {
            let label = icon.label(mode, "Action");
            assert!(label.ends_with("Action"));
            assert!(label.width() >= 6);
            if mode == "ascii" {
                assert!(label.is_ascii());
            }
            if mode == "none" {
                assert_eq!(label, "Action");
            }
        }
    }
}

#[test]
fn screenshot_sized_terminal_uses_four_columns_without_sidebar() {
    let mut app = create_mock_app();
    app.sidebar_expanded = false;
    app.cell_w_px = 15.;
    app.cell_h_px = 38.;
    let mut terminal = Terminal::new(TestBackend::new(128, 31)).unwrap();
    terminal.draw(|frame| ui::draw(frame, &mut app)).unwrap();
    assert_eq!(app.rendered_columns, 4);
}

#[test]
fn feed_states_are_consistent_in_grid_and_list_and_survive_status_expiry() {
    use yute::app::{Screen, ViewMode};
    let mut app = create_mock_app();
    app.items.clear();
    app.auth.lock().unwrap().access_token = Some("fixture".into());
    for (screen, noun) in [
        (Screen::Subscriptions, "subscriptions"),
        (Screen::History, "history"),
        (Screen::Liked, "liked videos"),
        (Screen::WatchLater, "Watch Later videos"),
        (Screen::Playlists, "playlists"),
    ] {
        app.screen = screen;
        app.feed_error = None;
        app.is_loading = false;
        assert_eq!(app.empty_feed_message(), format!("No {noun} found."));
        app.feed_error = Some("HTTP request failed".into());
        app.status_text = "Transient message".into();
        app.tick();
        assert!(app
            .empty_feed_message()
            .contains(&format!("Could not load {noun}")));
        for mode in [ViewMode::Grid, ViewMode::List] {
            app.view_mode = mode;
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
            terminal.draw(|frame| ui::draw(frame, &mut app)).unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(text.contains("Could not load"));
        }
    }
}
#[test]
fn cursor_rendering_tolerates_legacy_mid_character_offsets() {
    let mut app = create_mock_app();
    app.focus = yute::app::Focus::CommandLine;
    app.cmd_input = "खोज: भारत की यात्रा".into();
    for cursor in 0..=app.cmd_input.len() {
        app.input_cursor = Some(cursor);
        let mut terminal = Terminal::new(TestBackend::new(50, 15)).unwrap();
        terminal
            .draw(|frame| yute::ui::cmdline::render_input(frame, &app, frame.area()))
            .unwrap();
    }
}
#[test]
fn navigation_cancels_avatar_metadata_and_retains_selection_on_resize() {
    let mut app = create_mock_app();
    app.selected_index = 3;
    let id = app.selected_card().unwrap().id().to_string();
    for width in [80, 120, 200, 320] {
        let mut terminal = Terminal::new(TestBackend::new(width, 40)).unwrap();
        terminal.draw(|frame| ui::draw(frame, &mut app)).unwrap();
        assert_eq!(app.selected_card().unwrap().id(), id);
    }
    let token = app.metadata_cancelled.clone();
    app.transition(yute::app::Screen::Search, false);
    assert!(token.is_cancelled());
}
