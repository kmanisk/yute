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
