use ratatui::{backend::TestBackend, layout::Rect, Terminal};
use yute::{
    app::{App, Screen, WatchPane},
    config::Config,
    ui::{self, pane::Pane, watch::WatchLayout},
};
fn app() -> App {
    let mut app = App::new(
        Config::default(),
        ratatui_image::picker::Picker::halfblocks(),
    );
    app.config.general.graphics = "off".into();
    app.screen = Screen::Watch;
    let video = serde_json::json!({"id":"fixture0001","title":"रजनीकांत को पता चल गया इंडिया में 20 लाख करोड़ का काला धन है","channel":"Fixture channel","verified":true,"duration":"12:34","thumb_url":"","live":"None","badges":[]});
    app.watch_data=Some(serde_json::from_value(serde_json::json!({"video":video,"description":"A description with a link https://example.com/","chapters":[[0,"Introduction"],[20,"Details"]],"up_next":[],"comments":[],"description_links":[]})).unwrap());
    app
}
#[test]
fn panes_and_watch_dashboard_stay_inside_bounds() {
    for (w, h) in [(50, 15), (80, 24), (120, 40), (200, 60), (320, 90)] {
        for sidebar in [false, true] {
            let mut app = app();
            app.sidebar_expanded = sidebar;
            for pane in [
                WatchPane::Video,
                WatchPane::Description,
                WatchPane::Comments,
                WatchPane::Suggestions,
            ] {
                app.watch_pane = pane;
                for state in 0..4 {
                    let old = app.watch_data.clone();
                    match state {
                        0 => app.watch_data = None,
                        1 => {}
                        2 => {
                            app.watch_data = None;
                            app.watch_error = Some("Network error · r retry".into());
                        }
                        _ => {
                            app.watch_data.as_mut().unwrap().comments=vec![serde_json::from_value(serde_json::json!({"id":"comment-fixture","author":"Fixture author","text":"日本語のタイトル 안녕하세요 हिन्दी"})).unwrap()];
                        }
                    }
                    let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
                    t.draw(|f| ui::draw(f, &mut app)).unwrap();
                    let shell = ui::ShellLayout::new(Rect::new(0, 0, w, h), &app);
                    assert_eq!(
                        t.backend().buffer()[(shell.pane.x, shell.pane.y)].symbol(),
                        "┌"
                    );
                    assert_eq!(
                        t.backend().buffer()[(shell.pane.right() - 1, shell.pane.bottom() - 1)]
                            .symbol(),
                        "┘"
                    );
                    app.watch_data = old;
                }
            }
            let outer = Rect::new(1, 1, w - 2, h - 2);
            let layout = WatchLayout::new(outer, w >= 120, false);
            for r in [
                layout.header,
                layout.video,
                layout.playback,
                layout.description,
                layout.comments,
                layout.compose,
                layout.right,
            ] {
                if !r.is_empty() {
                    assert!(
                        r.x >= outer.x
                            && r.y >= outer.y
                            && r.right() <= outer.right()
                            && r.bottom() <= outer.bottom(),
                        "{w}x{h}: {r:?}"
                    );
                }
            }
        }
    }
}
#[test]
fn pane_padding_and_border_geometry_are_shared() {
    for (w, h) in [(50, 15), (80, 24), (120, 40), (200, 60), (320, 90)] {
        let area = Rect::new(0, 0, w, h);
        let p = Pane::new("Long हिन्दी title", true);
        let inner = p.inner(area);
        assert_eq!(inner.x, 2);
        assert!(inner.right() < area.right());
        let mut a = app();
        a.screen = Screen::Settings;
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| ui::draw(f, &mut a)).unwrap();
        let text = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(!text.contains("0/0"));
    }
}
