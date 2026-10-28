use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub fn picker_area(app: &App, area: Rect) -> Option<Rect> {
    if !app.account_picker {
        return None;
    }
    let width = area.width.saturating_sub(4).min(68);
    let height = (app.accounts.len() as u16 + 5).min(area.height.saturating_sub(2));
    Some(Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    ))
}
pub fn render_picker(f: &mut Frame, app: &App) {
    let Some(area) = picker_area(app, f.area()) else {
        return;
    };
    f.render_widget(Clear, area);
    let block = Block::default()
        .title(" Switch account ")
        .borders(Borders::ALL)
        .border_style(app.theme.style("accent"))
        .style(Style::default().bg(app.theme.bg));
    let inner = block.inner(area);
    f.render_widget(block, area);
    render_accounts(f, app, inner);
}
pub fn render_accounts(f: &mut Frame, app: &App, area: Rect) {
    let rows = area.height.saturating_sub(3).max(1) as usize;
    let start = app.accounts_cursor.saturating_sub(rows - 1);
    let mut lines = Vec::new();
    for (index, account) in app.accounts.iter().enumerate().skip(start).take(rows) {
        let selected = index == app.accounts_cursor;
        let style = if selected {
            app.theme
                .style("accent")
                .patch(app.theme.style("selection"))
                .add_modifier(Modifier::BOLD)
        } else {
            app.theme.style("text")
        };
        let marker = if account.status == yute_core::accounts::AccountStatus::NeedsLogin {
            "needs re-login via TV code"
        } else if index == app.active_account_index {
            "active"
        } else {
            ""
        };
        let text = format!(
            "{} {}. {} {}  {}",
            if selected { "▸" } else { " " },
            index + 1,
            account.name,
            account.handle,
            marker
        );
        lines.push(Line::from(Span::styled(
            super::text::truncate_str(&text, area.width as usize),
            style,
        )));
    }
    lines.push(Line::default());
    if app.credentials_pending {
        lines.push(Line::from("Waiting for the system keyring to unlock."));
    }
    lines.push(Line::from(Span::styled(
        "j/k select · Enter/1-9 switch · a add · d remove",
        app.theme.style("text_dim"),
    )));
    lines.push(Line::from(Span::styled(
        "Esc close · :account whoami",
        app.theme.style("text_dim"),
    )));
    f.render_widget(Paragraph::new(lines), area);
}
