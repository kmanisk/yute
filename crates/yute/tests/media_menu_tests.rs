use ratatui::{backend::TestBackend, Terminal};
use yute::{
    actions::{ActionMenu, MenuStage},
    app::App,
    config::Config,
};
fn menu(stage: MenuStage) -> ActionMenu {
    ActionMenu {
        request: 1,
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
