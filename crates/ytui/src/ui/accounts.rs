use crate::app::App;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn render_accounts(f: &mut Frame, app: &App, area: Rect) {
    if area.height < 6 || area.width < 20 {
        return;
    }

    let dialog_w = (area.width.saturating_sub(10)).clamp(40, 70);
    let dialog_h = ((app.accounts.len() as u16) * 3 + 8).min(area.height.saturating_sub(4));

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height.saturating_sub(dialog_h)) / 2),
            Constraint::Length(dialog_h),
            Constraint::Min(0),
        ])
        .split(area);

    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((area.width.saturating_sub(dialog_w)) / 2),
            Constraint::Length(dialog_w),
            Constraint::Min(0),
        ])
        .split(vert[1]);

    let card_area = horiz[1];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.accent))
        .title(" Switch Active Account ")
        .style(Style::default().bg(app.theme.surface));

    let inner = block.inner(card_area);
    f.render_widget(block, card_area);

    let mut lines = Vec::new();
    lines.push(Line::from(""));

    for (idx, acc) in app.accounts.iter().enumerate() {
        let is_cursor = idx == app.accounts_cursor;
        let is_active = idx == app.active_account_index;

        let radio = if is_active { "(*)" } else { "( )" };
        let pointer = if is_cursor { ">" } else { " " };

        let style = if is_cursor {
            Style::default()
                .bg(app.theme.surface_hover)
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default()
                .fg(app.theme.text)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.text_secondary)
        };

        let badge = if is_active { " [ACTIVE]" } else { "" };

        lines.push(Line::from(vec![
            Span::styled(
                format!("{pointer} {radio} "),
                if is_cursor {
                    Style::default().fg(app.theme.accent)
                } else {
                    Style::default().fg(app.theme.text_secondary)
                },
            ),
            Span::styled(
                format!(
                    "[{}] {:<18} {:<20}{}",
                    acc.initial, acc.name, acc.handle, badge
                ),
                style,
            ),
        ]));
        lines.push(Line::from(""));
    }

    lines.push(Line::from(vec![Span::styled(
        "  [j/k] Navigate   [Enter/Space] Switch Account   [q/Esc] Back",
        Style::default().fg(app.theme.text_secondary),
    )]));

    let p = Paragraph::new(lines).alignment(Alignment::Left);
    f.render_widget(p, inner);
}
