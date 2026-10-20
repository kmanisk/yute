use crate::app::{App, FilterKind, Focus};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn render_chips(f: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let is_focused = app.focus == Focus::Chips;
    let mut spans = Vec::new();

    // Use FilterKind chips (All | Videos | Shorts | Live | Playlists | Channels)
    for filter in FilterKind::all() {
        let is_active = *filter == app.filter_kind;

        let style = if is_focused && is_active {
            Style::default()
                .bg(app.theme.accent)
                .fg(ratatui::style::Color::Black)
                .add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default()
                .bg(app.theme.chip_active_bg)
                .fg(app.theme.chip_active_fg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .bg(app.theme.chip_inactive_bg)
                .fg(app.theme.chip_inactive_fg)
        };

        spans.push(Span::raw(" "));
        spans.push(Span::styled(format!(" {} ", filter.label()), style));
    }

    spans.push(Span::styled(
        "  [f] Cycle",
        Style::default().fg(app.theme.text_secondary),
    ));

    let p = Paragraph::new(Line::from(spans));
    f.render_widget(p, area);
}
