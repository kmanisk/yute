use crate::app::{App, Focus, Screen};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub const SIDEBAR_ITEMS: &[(&str, Screen, &str)] = &[
    ("Home", Screen::Home, "[H]"),
    ("Subscriptions", Screen::Subscriptions, "[S]"),
    ("History", Screen::History, "[Y]"),
    ("Playlists", Screen::Playlists, "[P]"),
    ("Watch Later", Screen::WatchLater, "[W]"),
    ("Liked Videos", Screen::Liked, "[L]"),
    ("Local Videos", Screen::LocalVideos, "[V]"),
    ("Queue", Screen::Queue, "[Q]"),
];

pub fn sidebar_width(expanded: bool) -> u16 {
    if expanded {
        24
    } else {
        10
    }
}

pub fn render_sidebar(f: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let is_focused = app.focus == Focus::Sidebar;
    let mut lines = Vec::new();
    lines.push(Line::from(""));

    for (idx, (label, screen, icon)) in SIDEBAR_ITEMS.iter().enumerate() {
        if app.sidebar_expanded && idx == 2 {
            lines.push(Line::from(Span::styled(
                "  --- You ---",
                Style::default().fg(app.theme.text_secondary),
            )));
        }

        let is_cursor = is_focused && (app.sidebar_index == idx);
        let is_current = app.screen == *screen;

        let style = if is_cursor {
            Style::default()
                .bg(app.theme.surface_hover)
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default()
                .bg(app.theme.surface)
                .fg(app.theme.text)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.text_secondary)
        };

        let pointer = if is_cursor { ">" } else { " " };

        if app.sidebar_expanded {
            let marker = if is_current { "*" } else { " " };
            lines.push(Line::from(vec![
                Span::styled(format!("{pointer} "), if is_cursor { Style::default().fg(app.theme.accent) } else { Style::default().fg(app.theme.text_secondary) }),
                Span::styled(format!("{icon} {label:<14} {marker}"), style),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(format!("{pointer}{icon}"), style),
            ]));
            let short_label = match *screen {
                Screen::Subscriptions => "Subs",
                Screen::WatchLater => "Later",
                Screen::Liked => "Liked",
                Screen::LocalVideos => "Local",
                _ => label,
            };
            lines.push(Line::from(vec![
                Span::styled(format!(" {short_label}"), style),
            ]));
            lines.push(Line::from(""));
        }
    }

    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(if is_focused { app.theme.accent } else { app.theme.border }))
        .style(Style::default().bg(app.theme.bg));

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}
