use crate::app::{App, Focus};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn render_chips(f: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    if app.chips.is_empty() {
        return;
    }

    let mut spans = Vec::new();
    let is_focused = app.focus == Focus::Chips;

    for (idx, chip) in app.chips.iter().enumerate() {
        let is_active = idx == app.active_chip_index;
        let is_selected_cursor = is_focused && is_active;

        let style = if is_selected_cursor {
            Style::default()
                .bg(app.theme.chip_active_bg)
                .fg(app.theme.chip_active_fg)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
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
        spans.push(Span::styled(format!(" {} ", chip.title), style));
    }

    spans.push(Span::styled(" >", Style::default().fg(app.theme.text_secondary)));

    let p = Paragraph::new(Line::from(spans));
    f.render_widget(p, area);
}
