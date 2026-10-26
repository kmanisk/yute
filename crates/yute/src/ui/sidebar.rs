use crate::app::{App, Focus, Screen};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub const SIDEBAR_ITEMS: &[(&str, Screen, &str)] = &[
    ("Home", Screen::Home, "gh"),
    ("Subscriptions", Screen::Subscriptions, "gs"),
    ("History", Screen::History, "gy"),
    ("Watch Later", Screen::WatchLater, "gw"),
    ("Liked", Screen::Liked, "gl"),
    ("Playlists", Screen::Playlists, "gp"),
    ("Queue", Screen::Queue, "gq"),
    ("Local", Screen::LocalVideos, "gv"),
    ("Account", Screen::Accounts, "ga"),
    ("Settings", Screen::Settings, "g,"),
];
pub fn index_of(screen: Screen) -> Option<usize> {
    let screen = if screen == Screen::PlaylistView {
        Screen::Playlists
    } else {
        screen
    };
    SIDEBAR_ITEMS.iter().position(|(_, s, _)| *s == screen)
}
pub fn sidebar_width(expanded: bool) -> u16 {
    if expanded {
        20
    } else {
        0
    }
}
pub fn render_sidebar(f: &mut Frame, app: &App, area: Rect) {
    if area.is_empty() {
        return;
    }
    let focused = app.focus == Focus::Sidebar;
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Navigation ")
        .border_style(Style::default().fg(if focused {
            app.theme.accent
        } else {
            app.theme.border
        }));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut lines = Vec::new();
    let mut cursor_line = 0usize;
    for (i, (name, _, hint)) in SIDEBAR_ITEMS.iter().enumerate() {
        if i == 3 || i == 8 {
            lines.push(Line::from(""));
        }
        let current = index_of(app.screen) == Some(i);
        let cursor = focused && app.sidebar_index == i;
        if cursor {
            cursor_line = lines.len();
        }
        let marker = if current {
            "| "
        } else if cursor {
            "> "
        } else {
            "  "
        };
        let style = Style::default().fg(if current {
            app.theme.accent
        } else {
            app.theme.text
        });
        let style = if current || cursor {
            style.add_modifier(Modifier::BOLD)
        } else {
            style
        };
        let name = super::text::truncate_str(name, inner.width.saturating_sub(5) as usize);
        let pad = inner
            .width
            .saturating_sub(2 + name.len() as u16 + hint.len() as u16);
        let line = Line::from(vec![
            Span::styled(marker, style),
            Span::styled(name, style),
            Span::raw(" ".repeat(pad as usize)),
            Span::styled(
                *hint,
                Style::default()
                    .fg(app.theme.text_secondary)
                    .add_modifier(Modifier::DIM),
            ),
        ]);
        lines.push(if cursor {
            line.style(Style::default().bg(app.theme.surface_hover))
        } else {
            line
        });
    }
    let scroll = cursor_line
        .saturating_add(1)
        .saturating_sub(inner.height as usize) as u16;
    f.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner);
}
