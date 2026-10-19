use crate::app::App;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

pub fn render_cmdline(f: &mut Frame, app: &App, area: Rect) {
    let popup_w = 60u16.min(area.width.saturating_sub(4));
    let popup_h = 3u16;

    let popup_x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_h)) / 2;

    let popup_rect = Rect::new(popup_x, popup_y, popup_w, popup_h);
    f.render_widget(Clear, popup_rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.accent))
        .title(" Command ")
        .style(Style::default().bg(app.theme.surface));

    let p = Paragraph::new(Line::from(vec![
        Span::styled(":", Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled(&app.cmd_input, Style::default().fg(app.theme.text)),
        Span::styled("█", Style::default().fg(app.theme.accent)),
    ]))
    .block(block);

    f.render_widget(p, popup_rect);
}
