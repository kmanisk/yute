use ratatui::{backend::TestBackend, style::Color, Terminal};
use yute::{
    app::{App, Screen},
    config::Config,
    terminal_input::Decoder,
    theme::{Capabilities, Theme, ROLES},
    ui,
};
fn caps() -> Capabilities {
    Capabilities {
        no_color: false,
        truecolor: true,
        colors256: true,
    }
}
fn app() -> App {
    let mut c = Config::default();
    c.general.graphics = "none".into();
    App::new(c, ratatui_image::picker::Picker::halfblocks())
}
#[test]
fn semantic_themes_parse_and_terminal_avoids_rgb() {
    let t = Theme::terminal();
    for role in ROLES {
        assert!(t.styles.contains_key(*role));
        let s = t.style(role);
        assert!(!matches!(s.fg, Some(Color::Rgb(..))));
        assert!(!matches!(s.bg, Some(Color::Rgb(..))));
    }
    for text in [
        include_str!("../themes/gruvbox-dark.toml"),
        include_str!("../themes/gruvbox-light.toml"),
    ] {
        let theme = Theme::parse(text, caps()).unwrap();
        for role in ROLES {
            assert!(theme.styles.contains_key(*role));
        }
    }
    assert!(Theme::parse("[roles.text]\nfg='unknown'", caps()).is_err());
    assert!(Theme::parse("[roles.typo]\nfg='red'", caps()).is_err());
}
#[test]
fn no_color_downgrade_and_contrast_do_not_invent_reset_rgb() {
    let text = "[roles.text]\nfg='#ffffff'\nbg='#ffffff'";
    let theme = Theme::parse(text, caps()).unwrap();
    assert!(theme
        .contrast_warnings()
        .iter()
        .any(|s| s.starts_with("text:")));
    assert!(Theme::terminal().contrast_warnings().is_empty());
    assert!(Theme::parse("[roles.text]\nfg='black'\nbg='black'", caps())
        .unwrap()
        .contrast_warnings()
        .is_empty());
    let no = Theme::parse(
        text,
        Capabilities {
            no_color: true,
            ..caps()
        },
    )
    .unwrap();
    for s in no.styles.values() {
        assert_eq!(s.fg, Some(Color::Reset));
        assert_eq!(s.bg, Some(Color::Reset));
    }
    let low = Theme::parse(
        text,
        Capabilities {
            truecolor: false,
            colors256: true,
            ..caps()
        },
    )
    .unwrap();
    assert!(matches!(low.text, Color::Indexed(_)));
    let low = Theme::parse(
        text,
        Capabilities {
            truecolor: false,
            colors256: false,
            ..caps()
        },
    )
    .unwrap();
    assert!(!matches!(low.text, Color::Rgb(..) | Color::Indexed(_)));
    let ansi = Capabilities {
        truecolor: false,
        colors256: false,
        ..caps()
    };
    for index in 0..=255 {
        assert!(!matches!(
            yute::theme::downgrade(Color::Indexed(index), ansi),
            Color::Rgb(..) | Color::Indexed(_)
        ));
    }
    assert_eq!(
        yute::theme::downgrade(Color::Indexed(200), caps()),
        Color::Indexed(200)
    );
}
#[test]
fn reports_are_separate_from_keys_and_parse_fragmented_backgrounds() {
    use crossterm::event::{Event, KeyCode, KeyModifiers};
    let mut d = Decoder::default();
    assert!(d.feed(b"\x1b]11;rgb:ffff/ffff/").is_empty());
    assert!(d.feed(b"ffff\x1b\\").is_empty());
    assert_eq!(d.report.light, Some(true));
    assert_eq!(d.report.background, Some((255, 255, 255)));
    assert!(d
        .feed(b"\x1b[?997;1n\x1b_Gi=31;OK\x1b\\\x1b[?62;4c\x1b[6;20;10t")
        .is_empty());
    assert_eq!(d.report.light, Some(false));
    assert!(d.report.kitty && d.report.sixel);
    assert_eq!(d.report.cells, Some((10, 20)));
    let events = d.feed(b"\x1b[13;2u\x1b[59:58;2u\x03\x1b[A");
    assert!(
        matches!(events[0],Event::Key(k)if k.code==KeyCode::Enter&&k.modifiers.contains(KeyModifiers::SHIFT))
    );
    assert!(matches!(events[1],Event::Key(k)if k.code==KeyCode::Char(':')));
    assert_eq!(events.len(), 4);
    assert!(d
        .feed(b"\x1b[200~q:quit\x1b[201~")
        .iter()
        .all(|e| matches!(e, Event::Paste(_))));
}
#[test]
fn malformed_reports_do_not_overflow_or_leak_as_keys() {
    let mut d = Decoder::default();
    let _ = d.feed(b"\x1b]11;rgb:ffffffff/0/0\x07");
    assert_eq!(d.report.background, None);
    assert!(d.feed(b"\x1b[6;0;0t\x1b[6;99999;10t").is_empty());
    assert_eq!(d.report.cells, None);
    assert!(d.feed(b"\x1b]1337;ReportCellSize=17.5;8;2\x07").is_empty());
    assert_eq!(d.report.cells, Some((16, 35)));
}
#[test]
fn graphics_auto_needs_a_reply_and_mux_is_safe() {
    use ratatui_image::picker::ProtocolType;
    use yute::terminal::graphics_kind;
    let mut r = Default::default();
    assert_eq!(graphics_kind("auto", &r, false), ProtocolType::Halfblocks);
    r.kitty = true;
    assert_eq!(graphics_kind("auto", &r, false), ProtocolType::Kitty);
    assert_eq!(graphics_kind("kitty", &r, true), ProtocolType::Halfblocks);
    assert_eq!(
        graphics_kind("iterm2", &Default::default(), false),
        ProtocolType::Iterm2
    );
}
#[test]
fn image_visibility_honours_off_alias_and_exact_overlay_intersections() {
    use ratatui::layout::Rect;
    let mut app = app();
    let image = Rect::new(10, 10, 8, 4);
    for mode in ["off", "none"] {
        app.config.general.graphics = mode.into();
        assert!(!ui::thumbnail::visible(&app, image));
    }
    app.config.general.graphics = "halfblocks".into();
    assert!(ui::thumbnail::visible(&app, image));
    app.overlay_area = Some(Rect::new(17, 13, 4, 3));
    assert!(!ui::thumbnail::visible(&app, image));
    app.overlay_area = Some(Rect::new(18, 14, 4, 3));
    assert!(ui::thumbnail::visible(&app, image));
    app.overlay_area = None;
    assert!(ui::thumbnail::visible(&app, image));
}
#[test]
fn text_keeps_long_word_remainder_and_complex_scripts_inside_bounds() {
    use ui::text::{truncate_str, wrap_and_truncate_title};
    use unicode_width::UnicodeWidthStr;
    let lines = wrap_and_truncate_title("The Ketchup Song (Aserejé)", 10, 5);
    assert_eq!(lines.join(" "), "The Ketchup Song (Aserejé)");
    let lines = wrap_and_truncate_title("abcdefghijklmnop", 5, 4);
    assert_eq!(lines.concat(), "abcdefghijklmnop");
    for text in [
        "और यह कैसे काम करता है?",
        "क्या बहस करना सही है?",
        "लिनक्स कैसे काम करता है",
        "日本語のタイトル",
        "naïve café",
        "\u{1f1ee}\u{1f1f3}",
        "\u{1f469}\u{200d}\u{1f4bb}",
        "a\u{301}",
    ] {
        for width in 0..45 {
            let s = truncate_str(text, width);
            assert!(s.width() <= width);
            for line in wrap_and_truncate_title(text, width, 2) {
                assert!(line.width() <= width);
            }
        }
    }
    let c = Config::default();
    assert_eq!(
        ui::text::title_for("Hindi हिन्दी · \u{1f469}\u{200d}\u{1f4bb}", &c),
        "Hindi हिन्दी · *"
    );
    let mut c = c;
    c.text.emoji = "strip".into();
    assert_eq!(
        ui::text::title_for("café 日本語 \u{1f600}", &c),
        "café 日本語"
    );
}
#[test]
fn renderer_matrix_for_terminal_dark_light_and_sidebar() {
    for (w, h) in [(50, 15), (80, 24), (120, 40), (200, 60), (320, 90)] {
        for sidebar in [false, true] {
            for name in ["terminal", "gruvbox-dark", "gruvbox-light"] {
                for screen in [
                    Screen::Home,
                    Screen::Search,
                    Screen::Subscriptions,
                    Screen::Playlists,
                    Screen::Settings,
                ] {
                    let mut app = app();
                    app.sidebar_expanded = sidebar;
                    app.screen = screen;
                    app.theme = if name == "terminal" {
                        Theme::terminal()
                    } else {
                        Theme::parse(
                            if name.ends_with("light") {
                                include_str!("../themes/gruvbox-light.toml")
                            } else {
                                include_str!("../themes/gruvbox-dark.toml")
                            },
                            caps(),
                        )
                        .unwrap()
                    };
                    let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
                    term.draw(|f| ui::draw(f, &mut app)).unwrap();
                    if name.ends_with("light") {
                        for cell in &term.backend().buffer().content {
                            if cell.symbol().trim().is_empty() {
                                continue;
                            }
                            if !matches!(cell.fg, Color::Reset) && !matches!(cell.bg, Color::Reset)
                            {
                                assert_ne!(cell.fg, cell.bg);
                            }
                        }
                    }
                }
            }
        }
    }
}
#[test]
fn arbitrary_palette_literals_are_confined_to_theme_module() {
    for p in std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ui"))
        .unwrap()
        .flatten()
    {
        let text = std::fs::read_to_string(p.path()).unwrap();
        for line in text.lines() {
            assert!(
                !line.contains("Color::") || line.contains("qrcode::Color::Dark"),
                "palette literal outside theme: {}",
                p.path().display()
            );
        }
    }
}
