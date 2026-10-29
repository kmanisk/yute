use ratatui::{backend::TestBackend, Terminal};
use yute::{
    actions::{ActionMenu, MenuStage},
    app::App,
    config::Config,
};
fn menu(stage: MenuStage) -> ActionMenu {
    ActionMenu {
        request: 1,
        download: false, ffmpeg: false,
        card: yute::backend::parse::parse_card(&serde_json::json!({"videoRenderer":{"videoId":"fixture","title":{"simpleText":"Fixture video"}}})).unwrap(),
        cursor: 0, stage,
        playlists: vec![], message: String::new(),
        formats: Some(yute_core::media::formats::AvailableFormats::from_json(include_bytes!("../../yute-core/tests/fixtures/formats/available.json")).unwrap()),
        cancelled: yute_core::oauth::CancellationToken::new(),
    }
}
#[test]
fn available_media_menus_render_at_all_requested_sizes_and_icon_modes() {
    for (width, height) in [(50, 15), (80, 24), (120, 40), (200, 60), (320, 90)] {
        for icons in ["nerd", "unicode", "ascii", "none"] {
            for stage in [
                MenuStage::PlayAs,
                MenuStage::LoadingFormats,
                MenuStage::VideoFormats,
                MenuStage::AudioFormats,
            ] {
                let mut config = Config::default();
                config.general.icons = icons.into();
                let mut app = App::new(config, ratatui_image::picker::Picker::halfblocks());
                app.action_menu = Some(menu(stage));
                let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                terminal
                    .draw(|frame| yute::actions::render_menu(frame, &app))
                    .unwrap();
                let text = terminal
                    .backend()
                    .buffer()
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>();
                assert!(text.contains("Fixture video"));
            }
        }
    }
}
#[test]
fn menu_contains_actual_formats_and_pairs_audio_without_conversion() {
    let video = menu(MenuStage::VideoFormats);
    let rows = video.media_rows();
    assert!(rows
        .iter()
        .any(|(label, selector)| label.contains("1080p60")
            && label.contains("HDR")
            && selector == "399+251"));
    assert!(!rows.iter().any(|(label, _)| label.contains("2160p")));
    let audio = menu(MenuStage::AudioFormats);
    let rows = audio.media_rows();
    assert!(rows.iter().any(|(label, selector)| label.contains("hi")
        && label.contains("AAC")
        && selector == "140"));
    assert!(!rows
        .iter()
        .any(|(_, selector)| selector.contains("convert")));
}
#[test]
fn closing_menu_cancels_its_probe() {
    let menu = menu(MenuStage::LoadingFormats);
    let token = menu.cancelled.clone();
    drop(menu);
    assert!(token.is_cancelled());
}
#[test]
fn contextual_menu_keeps_selected_actions_visible_and_never_overwrites_its_border() {
    use ratatui::{layout::Rect, widgets::Paragraph};
    use yute::backend::models::{Card, ChannelCard, PlaylistCard};
    let mut video = menu(MenuStage::Actions).card.clone();
    if let Card::Video(v) = &mut video {
        v.title = "The Ketchup Song (Aserejé) और यह कैसे काम करता है?".into();
        v.channel_id = Some("UCfixture".into());
    }
    let cards = [
        video,
        Card::Channel(ChannelCard {
            id: "UCfixture".into(),
            title: "लिनक्स कैसे काम करता है".into(),
            handle: None,
            subscribers: None,
            avatar_url: None,
            verified: false,
        }),
        Card::Playlist(PlaylistCard {
            id: "PLfixture".into(),
            title: "日本語のタイトル long playlist title".into(),
            channel: "Fixture".into(),
            video_count: Some("24".into()),
            thumb_url: String::new(),
            updated: None,
            is_mix: false,
            first_video_id: None,
        }),
    ];
    for height in [15, 24, 40] {
        for card in &cards {
            for active_download in [false, true] {
                let mut app = App::new(
                    Config::default(),
                    ratatui_image::picker::Picker::halfblocks(),
                );
                app.auth.lock().unwrap().access_token = Some("synthetic-menu-session".into());
                app.download_active = active_download;
                app.config.general.icons = "ascii".into();
                let mut state = menu(MenuStage::Actions);
                state.card = card.clone();
                app.action_menu = Some(state);
                let entries =
                    yute::actions::visible_entries(&app, &app.action_menu.as_ref().unwrap().card);
                for cursor in 0..entries.len() {
                    app.action_menu.as_mut().unwrap().cursor = cursor;
                    let rect =
                        yute::actions::menu_rect(&app, Rect::new(0, 0, 120, height)).unwrap();
                    let mut terminal = Terminal::new(TestBackend::new(120, height)).unwrap();
                    terminal
                        .draw(|f| {
                            for y in 0..height {
                                f.render_widget(
                                    Paragraph::new(".".repeat(120)),
                                    Rect::new(0, y, 120, 1),
                                );
                            }
                            yute::actions::render_menu(f, &app);
                        })
                        .unwrap();
                    let buffer = terminal.backend().buffer();
                    for y in 0..height {
                        for x in 0..120 {
                            if !rect.contains((x, y).into()) {
                                assert_eq!(buffer[(x, y)].symbol(), ".");
                            }
                        }
                    }
                    for y in rect.y + 1..rect.bottom() - 1 {
                        assert_eq!(buffer[(rect.x, y)].symbol(), "│");
                        assert_eq!(buffer[(rect.right() - 1, y)].symbol(), "│");
                    }
                    let text = buffer
                        .content
                        .iter()
                        .map(|c| c.symbol())
                        .collect::<String>();
                    assert!(
                        text.contains(entries[cursor].label),
                        "selected row clipped: {} at {height}",
                        entries[cursor].label
                    );
                    if height >= 24 {
                        for entry in &entries {
                            assert!(
                                text.contains(entry.label),
                                "action clipped at {height}: {}",
                                entry.label
                            );
                        }
                        assert!(!text.contains('▲') && !text.contains('▼'));
                    } else if entries.len() > 8 {
                        assert!(text.contains('▲') || text.contains('▼'));
                    }
                }
            }
        }
    }
}
