use crate::app::{App, Focus, Screen};
use ratatui::{layout::Rect, style::Style, widgets::Paragraph, Frame};
use unicode_width::UnicodeWidthStr;

pub fn render_statusbar(f: &mut Frame, app: &App, area: Rect) {
    if area.is_empty() {
        return;
    }
    if matches!(
        app.focus,
        Focus::CommandLine | Focus::SearchInput | Focus::CommentInput
    ) {
        super::cmdline::render_input(f, app, area);
        return;
    }
    let route = match app.screen {
        Screen::Home => "Home",
        Screen::Search => "Search",
        Screen::Watch => "Watch",
        Screen::ChannelView => "Channel",
        Screen::PlaylistView | Screen::Playlists => "Playlists",
        Screen::Subscriptions => "Subscriptions",
        Screen::History => "History",
        Screen::WatchLater => "Watch Later",
        Screen::Liked => "Liked",
        Screen::LocalVideos => "Local",
        Screen::Queue => "Queue",
        Screen::Settings => "Settings",
        Screen::Accounts => "Account",
        Screen::Help => "Help",
    };
    let count = app.filtered_items().len();
    let pos = if count == 0 {
        0
    } else {
        (app.selected_index + 1).min(count)
    };
    let left = if matches!(
        app.screen,
        Screen::Watch | Screen::Settings | Screen::Accounts | Screen::Help
    ) {
        format!(" {route}")
    } else if app.filter_kind != crate::app::FilterKind::All {
        format!(" {route} · {} · {pos}/{count}", app.filter_kind.label())
    } else {
        format!(" {route} · {pos}/{count}")
    };
    let left = match app.focus {
        Focus::Sidebar => format!(" Sidebar ▸{left}"),
        Focus::Chips => format!(" Filters ▸{left}"),
        _ => left,
    };
    let state = if app.is_loading_more {
        "Loading more…"
    } else if app.is_loading && !app.items.is_empty() {
        "Refreshing…"
    } else if app.is_loading {
        "Loading…"
    } else if app.status_text.starts_with("Loaded ") || app.status_text == "Ready" {
        ""
    } else {
        &app.status_text
    };
    let right = if !state.is_empty() { state } else { "? help" };
    let right = super::text::truncate_str(right, area.width.saturating_sub(2) as usize);
    let right_width = right.width().min(area.width.saturating_sub(2) as usize) as u16;
    let left_width = if right == "? help" {
        area.width.saturating_sub(right_width + 2)
    } else {
        (left.width() as u16).min(area.width / 2)
    };
    f.render_widget(
        Paragraph::new(left).style(Style::default().fg(app.theme.text_secondary)),
        Rect::new(area.x, area.y, left_width, 1),
    );
    let right_area = Rect::new(
        area.x + left_width + 1,
        area.y,
        area.width.saturating_sub(left_width + 2),
        1,
    );
    f.render_widget(
        Paragraph::new(super::text::truncate_str(&right, right_area.width as usize))
            .alignment(ratatui::layout::Alignment::Right)
            .style(Style::default().fg(app.theme.text)),
        right_area,
    );
}
