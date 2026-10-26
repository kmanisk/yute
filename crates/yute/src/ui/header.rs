use crate::app::{App, Screen};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::Paragraph,
    Frame,
};

pub fn route_name(app: &App) -> String {
    match app.screen {
        Screen::Home => "Home".into(),
        Screen::Search => format!("Search: {}", app.search_query),
        Screen::Subscriptions => "Subscriptions".into(),
        Screen::History => "History".into(),
        Screen::WatchLater => "Watch Later".into(),
        Screen::Liked => "Liked".into(),
        Screen::Playlists => "Playlists".into(),
        Screen::Queue => "Queue".into(),
        Screen::LocalVideos => "Local".into(),
        Screen::Settings => "Settings".into(),
        Screen::Accounts => "Account".into(),
        Screen::Watch => "Watch".into(),
        Screen::Help => "Help".into(),
        Screen::ChannelView => match &app.detail_route {
            Some(crate::app::DetailRoute::Channel(r)) => r.channel.title.clone(),
            _ => "Channel".into(),
        },
        Screen::PlaylistView => match &app.detail_route {
            Some(crate::app::DetailRoute::Playlist(r)) => r.title.clone(),
            _ => "Playlist".into(),
        },
    }
}
pub fn label(app: &App, width: u16) -> String {
    crate::ui::text::truncate_str(
        &format!(" yute · {}", route_name(app)),
        width.max(8) as usize,
    )
}
pub fn render_header(f: &mut Frame, app: &App, area: Rect) {
    f.render_widget(
        Paragraph::new(label(app, area.width)).style(
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        area,
    );
}
