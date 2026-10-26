use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::Paragraph,
    Frame,
};

pub fn render_header(f: &mut Frame, app: &App, area: Rect) {
    f.render_widget(
        Paragraph::new(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(
                " yute",
                Style::default()
                    .fg(app.theme.text)
                    .add_modifier(Modifier::BOLD),
            ),
            ratatui::text::Span::styled(
                if app.screen == crate::app::Screen::Search {
                    format!("  · Search: {}", app.search_query)
                } else {
                    String::new()
                },
                Style::default().fg(app.theme.text_secondary),
            ),
        ]))
        .style(Style::default().fg(app.theme.text)),
        area,
    );
}
