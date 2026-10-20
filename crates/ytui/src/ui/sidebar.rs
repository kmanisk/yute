use crate::app::{App, Focus, Screen};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub const SIDEBAR_ITEMS: &[(&str, Screen, &str)] = &[
    ("Home", Screen::Home, "gh"),
    ("Subscriptions", Screen::Subscriptions, "gs"),
    ("History", Screen::History, "gy"),
    ("Playlists", Screen::Playlists, "gp"),
    ("Watch Later", Screen::WatchLater, "gw"),
    ("Liked Videos", Screen::Liked, "gl"),
    ("Local Videos", Screen::LocalVideos, "gv"),
    ("Queue", Screen::Queue, "gq"),
    ("Settings", Screen::Settings, "go"),
    ("Accounts", Screen::Accounts, "ga"),
];

pub fn sidebar_width(expanded: bool) -> u16 {
    if expanded {
        24
    } else {
        20
    }
}

pub fn render_sidebar(f: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let is_focused = app.focus == Focus::Sidebar;
    let border_color = if is_focused {
        app.theme.accent
    } else {
        app.theme.border
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(border_color))
        .title(" Navigation ")
        .style(Style::default().bg(app.theme.bg));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height == 0 || inner.width == 0 {
        return;
    }

    let mut lines = Vec::new();

    for (idx, (label, screen, shortcut)) in SIDEBAR_ITEMS.iter().enumerate() {
        let is_cursor = is_focused && (app.sidebar_index == idx);
        let is_current = app.screen == *screen;

        // Active item = accent color + bold + "▌" marker
        let marker = if is_current {
            Span::styled("▌ ", Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD))
        } else if is_cursor {
            Span::styled("> ", Style::default().fg(app.theme.accent))
        } else {
            Span::styled("  ", Style::default().fg(app.theme.border))
        };

        let label_style = if is_current {
            Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD)
        } else if is_cursor {
            Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.text)
        };

        // Render key hint dimmer than label
        let hint_style = Style::default().fg(app.theme.text_secondary);

        // Calculate spacing so shortcut aligns nicely on right if space allows
        let max_label_w = inner.width.saturating_sub(7) as usize; // marker(2) + space(1) + shortcut(2) + margin(2)
        let display_label = if label.len() > max_label_w {
            &label[..max_label_w]
        } else {
            label
        };

        let pad_len = inner.width.saturating_sub(2 + display_label.len() as u16 + 1 + shortcut.len() as u16 + 1) as usize;
        let padding = " ".repeat(pad_len.max(1));

        let item_spans = vec![
            marker,
            Span::styled(display_label, label_style),
            Span::raw(padding),
            Span::styled(*shortcut, hint_style),
        ];

        let line_style = if is_cursor {
            Style::default().bg(app.theme.surface_hover)
        } else {
            Style::default()
        };

        lines.push(Line::from(item_spans).style(line_style));
    }

    f.render_widget(Paragraph::new(lines), inner);
}
