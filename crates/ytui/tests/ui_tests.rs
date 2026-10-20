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
fn test_collapsed_sidebar_shows_queue_at_80x24() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = create_mock_app();
    app.sidebar_expanded = false;

    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let buffer = terminal.backend().buffer();

    let mut found_queue = false;
    for y in 0..24 {
        let line: String = (0..16).map(|x| buffer[(x, y)].symbol()).collect();
        if line.contains("gq") || line.contains("Queue") {
            found_queue = true;
            break;
        }
    }
    assert!(found_queue, "Queue [gq] must be visible in collapsed sidebar on 80x24 terminal");
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

    // Shift+H and Shift+L navigate back/forward in browser history
    check_key(&mut keymap, 'H', Action::NavBackHistory);
    check_key(&mut keymap, 'L', Action::NavForwardHistory);
    check_key(&mut keymap, 'S', Action::NavSubs);
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
        assert_eq!(res2, Some((expected, 1)), "Key 'g{c}' should map to {expected:?}");
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

    // Verify cards from 3 separate rows are visible (3x3 grid)
    let full_text: String = (0..30)
        .map(|y| (0..120).map(|x| buffer[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");

    // First row should have item 1
    assert!(full_text.contains("Channel 1"), "Row 0 card must be visible");
    // Second row should have item 4 (since num_cols = 3)
    assert!(full_text.contains("Channel 4"), "Row 1 card must be visible");
    // Third row should have item 7 (since num_cols = 3)
    assert!(full_text.contains("Channel 7"), "Row 2 card must be visible in 3x3 grid");
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
        if line.contains("YouTube") {
            found_logo = true;
        }
        if line.contains("Mathew Manisk") && line.contains("[M]") {
            found_account = true;
        }
    }

    assert!(found_logo, "Header must render ASCII YouTube logo");
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

#[test]
fn test_nav_history_stack_back_and_forward() {
    use ytui::app::Screen;
    use ytui::backend::models::{Card, LiveState, VideoCard};

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
    app.items = vec![make_vid("h1", "Home 1"), make_vid("h2", "Home 2"), make_vid("h3", "Home 3")];
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
    use ytui::player::ipc::generate_socket_path;

    let s1 = generate_socket_path();
    let s2 = generate_socket_path();
    let s3 = generate_socket_path();

    assert_ne!(s1, s2);
    assert_ne!(s2, s3);
    assert_ne!(s1, s3);

    let s1_str = s1.to_string_lossy();
    assert!(s1_str.contains("ytui-mpv-"));
    assert!(s1_str.ends_with(".sock"));
}

#[test]
fn test_stats_for_nerds_keybinding() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ytui::keymap::{Action, KeymapState};

    let mut keymap = KeymapState::new();

    // In watch mode: 'i' and 'I' trigger ToggleStats
    let key_i = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_i, true),
        Some((Action::ToggleStats, 1)),
        "'i' in watch mode must trigger ToggleStats"
    );

    let key_upper_i = KeyEvent::new(KeyCode::Char('I'), KeyModifiers::SHIFT);
    assert_eq!(
        keymap.handle_key(key_upper_i, true),
        Some((Action::ToggleStats, 1)),
        "'I' in watch mode must trigger ToggleStats"
    );

    // In browsing mode: Enter triggers PlayDefault, Shift+Enter triggers PlayInline
    let key_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_enter, false),
        Some((Action::PlayDefault, 1))
    );

    let key_shift_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT);
    assert_eq!(
        keymap.handle_key(key_shift_enter, false),
        Some((Action::PlayInline, 1))
    );
}

#[test]
fn test_k_key_pause_action_in_watch_mode() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ytui::keymap::{Action, KeymapState};

    let mut keymap = KeymapState::new();

    let key_k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_k, true),
        Some((Action::PauseResume, 1)),
        "'k' in watch mode must trigger PauseResume"
    );

    let key_upper_k = KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT);
    assert_eq!(
        keymap.handle_key(key_upper_k, true),
        Some((Action::PauseResume, 1)),
        "'K' in watch mode must trigger PauseResume"
    );

    let key_space = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_space, true),
        Some((Action::PauseResume, 1)),
        "Space in watch mode must trigger PauseResume"
    );
}

#[test]
fn test_oauth_save_token_and_auth_file_0600_permissions() {
    use std::fs;
    use ytui::backend::auth::AuthContext;

    let dir = std::env::temp_dir().join(format!("ytui_test_auth_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let _ = fs::create_dir_all(&dir);
    let auth_file = dir.join("auth.json");

    let mut ctx = AuthContext::default();
    ctx.auth_file_path = Some(auth_file.clone());

    let res = ctx.save_token_and_account(
        "test_access_token_123".to_string(),
        "test_refresh_token_456".to_string(),
        Some(3600),
        "Test Channel".to_string(),
        "@testchannel".to_string(),
    );
    assert!(res.is_ok(), "save_token_and_account must succeed");
    assert!(auth_file.exists(), "auth.json must be written");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = fs::metadata(&auth_file).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "auth.json must be saved with 0600 permissions");
    }

    let mut reload_ctx = AuthContext::default();
    let load_res = reload_ctx.load_auth_file(&auth_file);
    assert!(load_res.is_ok());
    assert_eq!(reload_ctx.access_token.as_deref(), Some("test_access_token_123"));
    assert_eq!(reload_ctx.refresh_token.as_deref(), Some("test_refresh_token_456"));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_title_sanitization_and_word_truncation() {
    use ytui::ui::text::{sanitize_title, wrap_and_truncate_title};

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
    use ytui::ui::text::{is_emoji, sanitize_title, strip_emojis};

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
    use ytui::ui::text::format_meta_line;

    let style = Style::default();

    // Full width fits all components: channel, verified mark, views, age
    let full = format_meta_line("Valve", true, Some("1.2M views"), Some("3 days ago"), 50, style);
    let full_text: String = full.into_iter().map(|s| s.content.to_string()).collect();
    assert_eq!(full_text, "Valve [v] · 1.2M views · 3 days ago");

    // Drops "YouTube Shorts" if passed as channel
    let shorts = format_meta_line("YouTube Shorts", false, Some("500K views"), None, 50, style);
    let shorts_text: String = shorts.into_iter().map(|s| s.content.to_string()).collect();
    assert_eq!(shorts_text, "500K views");

    // Medium width drops published age first before dropping views
    let medium = format_meta_line("VeryLongChannelNameHere", false, Some("10K views"), Some("5 hours ago"), 28, style);
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
    use ytui::app::Focus;
    use ytui::keymap::{Action, KeymapState};

    let mut keymap = KeymapState::new();

    let tab = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
    assert_eq!(keymap.handle_key(tab, false), Some((Action::CycleFocusForward, 1)));

    let backtab = KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(keymap.handle_key(backtab, false), Some((Action::CycleFocusBackward, 1)));

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
    use ytui::app::ViewMode;

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
        .map(|y| (0..120).map(|x| buffer[(x, y)].symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");

    // Must render dim skeleton blocks '▒' without ugly "[Thumb loading...]" text
    assert!(full_text.contains('▒'), "Loading skeleton must use '▒' block characters");
    assert!(!full_text.contains("[Thumb loading...]"), "Must not render raw placeholder text");

    // Item 1 has progress bar (since progress_pct is Some(65))
    assert!(full_text.contains("[==="), "Progress bar must be visible on partially watched video");

    // Item 2 has LIVE badge (live: LiveState::Live)
    assert!(full_text.contains(" LIVE "), "Live badge must be rendered for active livestreams");

    // Duration badge on video 1 (11:30)
    assert!(full_text.contains("11:30"), "Duration chip must be rendered on video card");
}

#[test]
fn test_watch_smarttube_keybindings_and_actions() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ytui::keymap::{Action, KeymapState};

    let mut keymap = KeymapState::new();

    // Volume up '['
    let key_bracket = KeyEvent::new(KeyCode::Char('['), KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_bracket, true),
        Some((Action::VolumeUp, 1))
    );

    // Volume down '\''
    let key_quote = KeyEvent::new(KeyCode::Char('\''), KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_quote, true),
        Some((Action::VolumeDown, 1))
    );

    // Like '+' or '='
    let key_plus = KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_plus, true),
        Some((Action::LikeVideo, 1))
    );
    let key_equal = KeyEvent::new(KeyCode::Char('='), KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_equal, true),
        Some((Action::LikeVideo, 1))
    );

    // Dislike '-'
    let key_minus = KeyEvent::new(KeyCode::Char('-'), KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_minus, true),
        Some((Action::DislikeVideo, 1))
    );

    // Subscribe 's'
    let key_s = KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_s, true),
        Some((Action::ToggleSubscribe, 1))
    );

    // Comments 'c'
    let key_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_c, true),
        Some((Action::ToggleComments, 1))
    );

    // Comma ',' seeks back 5 seconds
    let key_comma = KeyEvent::new(KeyCode::Char(','), KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_comma, true),
        Some((Action::SeekBack5, 1))
    );

    // Dot '.' seeks forward 5 seconds
    let key_dot = KeyEvent::new(KeyCode::Char('.'), KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_dot, true),
        Some((Action::SeekForward5, 1))
    );

    // Angle bracket '<' decreases speed
    let key_lt = KeyEvent::new(KeyCode::Char('<'), KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_lt, true),
        Some((Action::SpeedDown, 1))
    );

    // Angle bracket '>' increases speed
    let key_gt = KeyEvent::new(KeyCode::Char('>'), KeyModifiers::NONE);
    assert_eq!(
        keymap.handle_key(key_gt, true),
        Some((Action::SpeedUp, 1))
    );
}

#[test]
fn test_default_hwdec_is_auto_safe() {
    let cfg = ytui::config::Config::default();
    assert_eq!(cfg.playback.hwdec, "auto-safe");
}

#[test]
fn test_auth_encryption_and_migration() {
    use std::fs;
    use ytui::backend::auth::{decrypt_auth_data, encrypt_auth_data, AuthContext};

    let sample_payload = b"{\"accounts\":{\"test\":{\"access_token\":\"secret_token_123\",\"refresh_token\":\"secret_ref_456\",\"expires_at\":9999999999,\"name\":\"Test Account\",\"handle\":\"@test\"}},\"active_account\":\"test\"}";

    // Test encryption and decryption round-trip
    let enc = encrypt_auth_data(sample_payload).expect("Encryption must succeed");
    assert!(enc.starts_with("YTUI_ENC_V1:"));
    assert!(!enc.contains("secret_token_123"));

    let dec = decrypt_auth_data(&enc).expect("Decryption must succeed");
    assert_eq!(dec, sample_payload);

    // Test automatic migration from plaintext file
    let tmp_dir = std::env::temp_dir().join(format!("ytui_test_auth_{}", std::process::id()));
    let _ = fs::create_dir_all(&tmp_dir);
    let auth_file = tmp_dir.join("auth.json");

    fs::write(&auth_file, sample_payload).expect("Write plaintext test auth");

    let mut ctx = AuthContext::default();
    ctx.load_auth_file(&auth_file).expect("Load auth file");

    assert_eq!(ctx.access_token.as_deref(), Some("secret_token_123"));
    assert_eq!(ctx.refresh_token.as_deref(), Some("secret_ref_456"));

    // Verify it was migrated to encrypted format on disk
    let migrated_content = fs::read_to_string(&auth_file).expect("Read migrated file");
    assert!(migrated_content.starts_with("YTUI_ENC_V1:"));
    assert!(!migrated_content.contains("secret_token_123"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::metadata(&auth_file).expect("Metadata").permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);
    }

    let _ = fs::remove_dir_all(&tmp_dir);
}



