use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let status = &app.player.status;
    let title = app
        .player
        .current_video
        .as_ref()
        .map(|v| v.title.as_str())
        .unwrap_or(&status.title);
    let time = |t: f64| {
        let s = t.max(0.0) as u64;
        format!("{}:{:02}", s / 60, s % 60)
    };
    let suffix = format!("  {} / {}", time(status.time_pos), time(status.duration));
    let title = super::text::truncate_str(
        title,
        area.width.saturating_sub(suffix.len() as u16 + 4) as usize,
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                if status.paused { " Ⅱ " } else { " ▶ " },
                Style::default().fg(app.theme.accent),
            ),
            Span::styled(
                title,
                Style::default()
                    .fg(app.theme.text)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(suffix, Style::default().fg(app.theme.text_secondary)),
        ])),
        area,
    );
}
