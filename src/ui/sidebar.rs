use crate::app::{App, Focus, Screen};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

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

    let items = if app.sidebar_expanded {
        vec![
            ("Home", Screen::Home, "[H]"),
            ("Subscriptions", Screen::Subscriptions, "[S]"),
            ("--- You ---", Screen::Home, "---"),
            ("History", Screen::History, "[Y]"),
            ("Playlists", Screen::Playlists, "[P]"),
            ("Watch Later", Screen::WatchLater, "[W]"),
            ("Liked Videos", Screen::Liked, "[L]"),
            ("Local Videos", Screen::LocalVideos, "[V]"),
            ("Queue", Screen::Queue, "[Q]"),
        ]
    } else {
        vec![
            ("Home", Screen::Home, "[H]"),
            ("Subs", Screen::Subscriptions, "[S]"),
            ("History", Screen::History, "[Y]"),
            ("Later", Screen::WatchLater, "[W]"),
            ("Local", Screen::LocalVideos, "[V]"),
            ("Queue", Screen::Queue, "[Q]"),
        ]
    };

    lines.push(Line::from(""));

    for (label, screen, icon) in items {
        if label.starts_with("---") {
            lines.push(Line::from(Span::styled(
                format!("  {label}"),
                Style::default().fg(app.theme.text_secondary),
            )));
            continue;
        }

        let is_current = app.screen == screen;
        let is_selected = is_focused && is_current;

        let style = if is_selected {
            Style::default()
                .bg(app.theme.surface_hover)
                .fg(app.theme.text)
                .add_modifier(Modifier::BOLD)
        } else if is_current {
            Style::default()
                .bg(app.theme.surface)
                .fg(app.theme.text)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.text_secondary)
        };

        if app.sidebar_expanded {
            lines.push(Line::from(vec![
                Span::raw(" "),
                Span::styled(format!(" {icon}  {label:<16}"), style),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(format!("  {icon}"), style),
            ]));
            lines.push(Line::from(vec![
                Span::styled(format!("  {label}"), style),
            ]));
            lines.push(Line::from(""));
        }
    }

    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(app.theme.border))
        .style(Style::default().bg(app.theme.bg));

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}
