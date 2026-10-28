use ratatui::{backend::TestBackend, layout::Rect, Terminal};
use yute::{
    app::{App, ChannelState, DetailRoute, FilterKind, Focus, Screen},
    backend::models::{Card, ChannelCard, LiveState, PlaylistCard, VideoCard},
    config::Config,
    ui::{self, grid::GridGeometry, ShellLayout},
};
fn app() -> App {
    let mut app = App::new(
        Config::default(),
        ratatui_image::picker::Picker::halfblocks(),
    );
    app.config.general.graphics = "none".into();
    app.sidebar_expanded = true;
    app.items = (0..30)
        .map(|i| {
            Card::Video(VideoCard {
                id: format!("v{i}"),
                title: format!("Rust हिन्दी 日本語 é {i}"),
                channel: "Readable Channel".into(),
                channel_id: None,
                verified: false,
                duration: Some("12:43".into()),
                views: Some("428K".into()),
                published: Some("2d".into()),
                thumb_url: String::new(),
                progress_pct: None,
                live: LiveState::None,
                badges: vec![],
                resume_time_sec: None,
            })
        })
        .collect();
    app
}
fn channel() -> ChannelCard {
    ChannelCard {
        id: "UCfixture".into(),
        title: "BeastBoyShub".into(),
        handle: Some("@example".into()),
        subscribers: Some("7.2M subscribers".into()),
        avatar_url: None,
        verified: false,
    }
}
fn playlist() -> PlaylistCard {
    PlaylistCard {
        id: "PLfixture".into(),
        title: "Playlist Name".into(),
        channel: "Example".into(),
        video_count: Some("24 videos".into()),
        thumb_url: String::new(),
        is_mix: false,
        first_video_id: None,
    }
}
#[test]
fn shell_geometry_and_rendering_matrix() {
    for (w, h) in [(80, 24), (120, 30), (135, 31), (160, 40), (200, 50)] {
        let mut app = app();
        for screen in [
            Screen::Home,
            Screen::Subscriptions,
            Screen::Search,
            Screen::History,
            Screen::ChannelView,
            Screen::PlaylistView,
            Screen::Settings,
            Screen::Help,
        ] {
            app.screen = screen;
            app.detail_route = match screen {
                Screen::ChannelView => Some(DetailRoute::Channel(Box::new(ChannelState {
                    channel: channel(),
                    tabs: vec![],
                    active: 0,
                    subscribed: None,
                    pages: Default::default(),
                }))),
                Screen::PlaylistView => Some(DetailRoute::Playlist(playlist())),
                _ => None,
            };
            if screen == Screen::Search {
                app.items[0] = Card::Channel(channel());
                app.items[1] = Card::Playlist(playlist());
            }
            let area = Rect::new(0, 0, w, h);
            let shell = ShellLayout::new(area, &app);
            assert_eq!(shell.sidebar.y, 1, "{screen:?} {w}x{h}");
            assert_eq!(shell.tabs.y, shell.header.y);
            assert_eq!(shell.pane.y, 1);
            assert_eq!(shell.content.x, shell.pane.x + 1);
            assert_eq!(shell.title.x, shell.content.x);
            assert!(shell.tabs.x >= shell.sidebar.right());
            assert!(shell.player.is_empty());
            let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
            terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
            assert_eq!(terminal.backend().buffer().area, area);
            if screen == Screen::Home {
                assert_eq!(shell.title.height, 0);
            }
            // Complete row count follows physical image geometry, without forcing
            // a shallow thumbnail to satisfy a fixed row count.
            if app.rendered_columns > 1 && screen != Screen::ChannelView {
                let geometry = GridGeometry::new(
                    shell.content,
                    &app.config.grid,
                    app.cell_w_px,
                    app.cell_h_px,
                );
                if !matches!(screen, Screen::Settings | Screen::Help) {
                    assert_eq!(app.rendered_rows, geometry.visible_rows);
                }
            }
        }
        app.screen = Screen::Home;
        app.focus = Focus::CommandLine;
        app.cmd_input = "sub".into();
        let shell = ShellLayout::new(Rect::new(0, 0, w, h), &app);
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
        assert_eq!(shell.sidebar.y, 1);
        let footer: String = (0..w)
            .map(|x| terminal.backend().buffer()[(x, h - 1)].symbol())
            .collect();
        assert!(footer.contains("COMMAND"));
    }
}
#[test]
fn selected_card_tint_never_reaches_gutters_or_border() {
    let mut app = app();
    let shell = ShellLayout::new(Rect::new(0, 0, 135, 31), &app);
    let g = GridGeometry::new(
        shell.content,
        &app.config.grid,
        app.cell_w_px,
        app.cell_h_px,
    );
    let mut terminal = Terminal::new(TestBackend::new(135, 31)).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let inner = Rect::new(
        shell.content.x + g.first_column_offset + 1,
        shell.content.y + 1,
        g.card_width - 2,
        g.card_height - 2,
    );
    for y in shell.content.y..shell.content.bottom() {
        for x in shell.content.x..shell.content.right() {
            if terminal.backend().buffer()[(x, y)].bg == app.theme.surface_hover {
                assert!(
                    inner.contains((x, y).into()),
                    "tint at {x},{y} outside {inner:?}"
                );
            }
        }
    }
}
#[test]
fn resize_preserves_selected_entity_and_complete_rows() {
    let mut app = app();
    app.selected_index = 19;
    let selected = app.selected_card().unwrap().id().to_string();
    for (w, h) in [(135, 31), (80, 24), (200, 50), (120, 30)] {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
        assert_eq!(app.selected_card().unwrap().id(), selected);
        let g = GridGeometry::new(
            ShellLayout::new(Rect::new(0, 0, w, h), &app).content,
            &app.config.grid,
            app.cell_w_px,
            app.cell_h_px,
        );
        if app.rendered_columns == 1 {
            assert!((app.scroll_row..app.scroll_row + app.rendered_rows).contains(&19));
        } else {
            assert!(g.visible_range(app.scroll_row, 30).contains(&19));
        }
    }
}
#[test]
fn channel_fallback_and_playlist_are_not_video_duration() {
    let mut app = app();
    app.screen = Screen::Search;
    app.items = vec![Card::Channel(channel()), Card::Playlist(playlist())];
    let mut terminal = Terminal::new(TestBackend::new(135, 31)).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect::<String>();
    assert!(text.contains("CHANNEL"));
    assert!(text.contains("PLAYLIST"));
    assert!(!text.contains("12:43"));
    assert!(matches!(
        yute::actions::activation(&app.items[1]),
        yute::actions::OpenAction::OpenPlaylist(_)
    ));
}
#[test]
fn shorts_policy_preserves_saved_content_and_explicit_filter() {
    let mut app = app();
    if let Card::Video(v) = &mut app.items[0] {
        v.badges.push("SHORTS".into());
    }
    for screen in [Screen::Home, Screen::Subscriptions, Screen::Search] {
        app.screen = screen;
        assert_eq!(app.filtered_items().len(), 29);
    }
    for screen in [
        Screen::History,
        Screen::WatchLater,
        Screen::Liked,
        Screen::PlaylistView,
    ] {
        app.screen = screen;
        assert_eq!(app.filtered_items().len(), 30);
    }
    app.screen = Screen::Home;
    app.filter_kind = FilterKind::Shorts;
    assert_eq!(app.filtered_items().len(), 1);
}
#[test]
fn command_argument_completion_and_normalization() {
    assert_eq!(yute::commands::completions("filter ").len(), 6);
    assert_eq!(yute::commands::completions("set "), vec!["set graphics"]);
    assert!(yute::commands::parse(":sidebar toggle").is_ok());
    assert!(yute::commands::parse(":keys").is_ok());
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    for (input, expected) in [
        ('h', 'H'),
        ('l', 'L'),
        ('j', 'J'),
        ('k', 'K'),
        (';', ':'),
        ('/', '?'),
    ] {
        assert_eq!(
            yute::keymap::normalize_key(KeyEvent::new(KeyCode::Char(input), KeyModifiers::SHIFT))
                .code,
            KeyCode::Char(expected)
        );
    }
}
#[test]
fn active_playback_reserves_exactly_one_row_above_footer() {
    let mut app = app();
    for (w, h) in [(80, 24), (120, 30), (135, 31), (160, 40), (200, 50)] {
        let area = Rect::new(0, 0, w, h);
        let idle = ShellLayout::with_playback(area, &app, false);
        let active = ShellLayout::with_playback(area, &app, true);
        assert!(idle.player.is_empty());
        assert_eq!(active.player.height, 1);
        assert_eq!(active.player.bottom(), active.footer.y);
        assert_eq!(idle.content.height, active.content.height + 1);
        app.player.status.title = "Playback fixture".into();
        app.player.status.time_pos = 751.0;
        app.player.status.duration = 2892.0;
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal
            .draw(|f| ui::miniplayer::render(f, &app, active.player))
            .unwrap();
        let line = (0..w)
            .map(|x| terminal.backend().buffer()[(x, h - 2)].symbol())
            .collect::<String>();
        assert!(line.contains("12:31 / 48:12"));
    }
}

#[test]
fn thumbnails_keep_physical_sixteen_by_nine_dimensions() {
    let config = Config::default();
    for width in [100, 135, 160, 200] {
        for (cell_w, cell_h) in [(10.0, 20.0), (15.0, 38.0), (10.0, 25.0)] {
            let geometry =
                GridGeometry::new(Rect::new(0, 0, width, 40), &config.grid, cell_w, cell_h);
            let ideal = geometry.thumbnail_width as f32 * cell_w * 9.0 / (16.0 * cell_h);
            assert!((geometry.thumbnail_height as f32 - ideal).abs() <= 0.5);
        }
    }
}

#[test]
fn home_and_search_share_framed_two_row_layout_at_screenshot_geometry() {
    let mut app = app();
    app.sidebar_expanded = false;
    app.cell_w_px = 15.0;
    app.cell_h_px = 38.0;
    for screen in [Screen::Home, Screen::Search] {
        app.screen = screen;
        app.search_query = "bbs".into();
        let shell = ShellLayout::new(Rect::new(0, 0, 113, 28), &app);
        assert!(shell.title.is_empty());
        let g = GridGeometry::new(shell.content, &app.config.grid, 15.0, 38.0);
        assert!(g.visible_rows >= 2);
        let mut terminal = Terminal::new(TestBackend::new(113, 28)).unwrap();
        terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
        let buffer = terminal.backend().buffer();
        let x = shell.content.x + g.first_column_offset;
        let y = shell.content.y;
        assert_eq!(buffer[(x, y)].symbol(), "┌");
        assert_eq!(buffer[(x + g.card_width - 1, y)].symbol(), "┐");
        assert_eq!(buffer[(x, y + g.card_height - 1)].symbol(), "└");
        assert_eq!(
            buffer[(x + g.card_width - 1, y + g.card_height - 1)].symbol(),
            "┘"
        );
        if screen == Screen::Search {
            let header = (0..113)
                .map(|x| buffer[(x, 0)].symbol())
                .collect::<String>();
            assert!(header.contains("Search: bbs"));
        }
    }
}

#[test]
fn strongest_shorts_hiding_still_allows_explicit_shorts_tabs() {
    let mut app = app();
    if let Card::Video(v) = &mut app.items[0] {
        v.badges.push("SHORTS".into());
    }
    app.config.content.shorts = yute::config::ShortsVisibility::Never;
    for screen in [
        Screen::Home,
        Screen::Search,
        Screen::History,
        Screen::WatchLater,
        Screen::PlaylistView,
        Screen::Queue,
    ] {
        app.screen = screen;
        app.filter_kind = FilterKind::All;
        assert_eq!(app.filtered_items().len(), 29);
        app.filter_kind = FilterKind::Shorts;
        assert_eq!(app.filtered_items().len(), 1);
    }
    app.screen = Screen::ChannelView;
    app.filter_kind = FilterKind::All;
    app.detail_route = Some(DetailRoute::Channel(Box::new(ChannelState {
        channel: channel(),
        tabs: vec![yute::backend::models::ChannelLink {
            title: "Shorts".into(),
            params: None,
            selected: true,
        }],
        active: 0,
        subscribed: None,
        pages: Default::default(),
    })));
    assert_eq!(app.filtered_items().len(), 30);
    assert_eq!(app.items.len(), 30, "policy must not destroy cached items");
}

#[test]
fn compact_grid_and_settings_remain_bounded_at_requested_sizes() {
    let mut app = app();
    for (w, h) in [(80, 24), (120, 30), (135, 31), (160, 40), (200, 50)] {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        for screen in [Screen::Home, Screen::Settings, Screen::Watch] {
            app.screen = screen;
            for selected in [0, 3, 7] {
                app.settings_index = selected;
                terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
                let text = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|c| c.symbol())
                    .collect::<String>();
                assert!(!text.contains("NORMAL"));
                assert!(!text.contains("Show sidebar"));
            }
        }
        let g = GridGeometry::new(Rect::new(0, 2, w, h - 3), &app.config.grid, 15., 38.);
        assert_eq!(g.title_rows, 2);
        assert!(g.card_width <= 44);
    }
    app.settings_index = 3;
    yute::ui::settings::cycle_setting(&mut app, true);
    assert_eq!(app.config.playback.stream_cache_seconds, 40);
    assert_eq!(app.config.playback.stream_cache_mb(), 192);
    for _ in 0..20 {
        yute::ui::settings::cycle_setting(&mut app, true);
    }
    assert_eq!(app.config.playback.stream_cache_seconds, 60);
}

#[test]
fn focus_frames_and_single_row_header_stay_clear_with_hidden_sidebar() {
    for (w, h) in [
        (80, 24),
        (113, 28),
        (120, 30),
        (135, 31),
        (160, 40),
        (200, 50),
    ] {
        let mut app = app();
        app.sidebar_expanded = false;
        app.focus = Focus::Content;
        let shell = ShellLayout::new(Rect::new(0, 0, w, h), &app);
        assert_eq!(shell.tabs.y, 0);
        assert_eq!(shell.header.y, 0);
        assert_eq!(shell.pane, Rect::new(0, 1, w, h - 2));
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
        assert_eq!(
            terminal.backend().buffer()[(shell.pane.x, shell.pane.y)].fg,
            app.theme.accent
        );
        let g = GridGeometry::new(
            shell.content,
            &app.config.grid,
            app.cell_w_px,
            app.cell_h_px,
        );
        assert_eq!(g.column_stride - g.card_width, 2);
        if w == 135 {
            assert!(g.visible_rows >= 2);
        }
        app.sidebar_expanded = true;
        app.focus = Focus::Sidebar;
        let shell = ShellLayout::new(Rect::new(0, 0, w, h), &app);
        terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
        assert_eq!(
            terminal.backend().buffer()[(shell.sidebar.x, shell.sidebar.y)].fg,
            app.theme.accent
        );
        assert_eq!(
            terminal.backend().buffer()[(shell.pane.x, shell.pane.y)].fg,
            app.theme.border
        );
    }
}

#[test]
fn grid_bounds_complete_and_partial_rows_across_terminal_sizes() {
    let mut app = app();
    for width in (80..=300).step_by(5) {
        for height in 24..=80 {
            for sidebar in [true, false] {
                app.sidebar_expanded = sidebar;
                let shell = ShellLayout::new(Rect::new(0, 0, width, height), &app);
                let g = GridGeometry::new(shell.content, &app.config.grid, 15., 38.);
                assert!(g.visible_rows >= g.complete_rows);
                assert!(
                    g.card_width * g.columns as u16 + 2 * (g.columns - 1) as u16
                        <= shell.content.width
                );
                let used = g.row_stride * (g.visible_rows - 1) as u16;
                assert!(used < shell.content.height);
                assert!(shell.content.height - used <= g.row_stride);
                assert_eq!(
                    g.complete_rows,
                    if shell.content.height >= g.card_height {
                        (1 + (shell.content.height - g.card_height) / g.row_stride) as usize
                    } else {
                        1
                    }
                );
            }
        }
    }
}

#[test]
fn truncation_respects_graphemes_and_ellipsis_width() {
    use unicode_width::UnicodeWidthStr;
    for value in [
        "Vietnamese extended title",
        "\u{4e2d}\u{6587} title",
        "\u{1f469}\u{200d}\u{1f4bb} developer",
        "a\u{301} long title",
    ] {
        for width in 0..20 {
            let result = yute::ui::text::truncate_str(value, width);
            assert!(result.width() <= width, "{result:?} {width}");
            assert!(!result.ends_with("\u{200d}…"));
        }
    }
}

#[test]
fn completion_popup_is_framed_and_keeps_footer_clear() {
    let mut app = app();
    app.focus = Focus::CommandLine;
    app.cmd_input = "acc".into();
    for (w, h) in [(80, 24), (113, 28), (200, 60)] {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
        let rect = app.overlay_area.unwrap();
        assert!(rect.bottom() < h);
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(rect.x, rect.y)].symbol(), "┌");
        assert_eq!(buffer[(rect.right() - 1, rect.bottom() - 1)].symbol(), "┘");
        assert_eq!(buffer[(rect.right() - 1, rect.y + 1)].symbol(), "│");
    }
    assert_eq!(
        yute::commands::completions_with_accounts("account Al", &["Alice".into()]),
        vec!["account Alice"]
    );
}

#[test]
fn anonymous_library_empty_states_offer_sign_in_in_grid_and_list() {
    let mut app = app();
    app.items.clear();
    assert!(app.switch_account(0));
    for screen in [
        Screen::Subscriptions,
        Screen::History,
        Screen::Liked,
        Screen::WatchLater,
        Screen::Playlists,
    ] {
        app.screen = screen;
        for width in [60, 120] {
            let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
            terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>();
            assert!(text.contains(":account add"));
            assert!(!text.contains("could not"));
        }
    }
}
