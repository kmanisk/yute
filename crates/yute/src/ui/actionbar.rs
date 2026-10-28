use crate::{app::App, backend::models::Card};
use ratatui::{layout::Rect, widgets::Paragraph, Frame};
use unicode_width::UnicodeWidthStr;
pub fn render(f: &mut Frame, app: &App, area: Rect) {
    if area.is_empty() {
        return;
    }
    if area.width < 60 {
        f.render_widget(
            Paragraph::new("? help").style(app.theme.style("action_hint")),
            area,
        );
        return;
    }
    let actions = match app.selected_card() {
        Some(Card::Video(_)) if app.is_authenticated() => vec![
            "Enter play",
            "a queue",
            "w watch later",
            "+ like",
            "S subscribe",
            "d download",
            "Shift-Enter more",
        ],
        Some(Card::Video(_)) => vec!["Enter play", "a queue", "d download", "Shift-Enter more"],
        Some(Card::Channel(_)) if app.is_authenticated() => {
            vec!["Enter open", "S subscribe", "Shift-Enter more"]
        }
        Some(Card::Playlist(_)) => vec![
            "Enter open",
            "p play all",
            "a queue",
            "d download",
            "Shift-Enter more",
        ],
        _ => vec!["? help"],
    };
    let mut text = String::new();
    for action in actions {
        let next = if text.is_empty() {
            action.into()
        } else {
            format!("{text} · {action}")
        };
        if next.width() > area.width as usize {
            break;
        }
        text = next;
    }
    f.render_widget(
        Paragraph::new(text).style(app.theme.style("action_hint")),
        area,
    );
}
