use crate::app::{App, Screen};
use ratatui::{layout::Rect, style::Modifier, widgets::Paragraph, Frame};
use unicode_width::UnicodeWidthStr;

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
    crate::ui::text::truncate_str(&format!(" {}", route_name(app)), width.max(8) as usize)
}
fn account_label(app: &App) -> String {
    if app.auth.lock().unwrap().needs_login {
        "! Sign in".into()
    } else if app.is_authenticated() {
        app.account_name.clone()
    } else {
        "Anonymous".into()
    }
}
pub fn account_area(app: &App, size: Rect) -> Rect {
    let width = (account_label(app)
        .width()
        .saturating_add(7)
        .min(u16::MAX as usize) as u16)
        .min(size.width / 3)
        .max(1)
        .min(size.width.saturating_sub(2));
    Rect::new(size.right().saturating_sub(width + 1), size.y, width, 1)
}
pub fn render_header(f: &mut Frame, app: &mut App, area: Rect) {
    let size = f.area();
    let account = account_label(app);
    let avatar_url = app
        .accounts
        .get(app.active_account_index)
        .and_then(|profile| profile.avatar_url.clone());
    // Two terminal cells by one row is approximately square with common 1:2
    // cell geometry and keeps the account avatar compact like YouTube's header.
    let avatar_w =
        if avatar_url.is_some() && crate::terminal::images_enabled(&app.config.general.graphics) {
            2
        } else {
            0
        };
    let account_area = account_area(app, size).inner(ratatui::layout::Margin::new(1, 0));
    let route_area = Rect::new(
        area.x,
        area.y,
        area.width.min(account_area.x.saturating_sub(area.x + 1)),
        area.height,
    );
    f.render_widget(
        Paragraph::new(label(app, route_area.width))
            .style(app.theme.style("accent").add_modifier(Modifier::BOLD)),
        route_area,
    );
    let mut text_area = account_area;
    if let Some(url) = avatar_url {
        if avatar_w > 0 {
            let avatar_area = Rect::new(
                account_area.right().saturating_sub(avatar_w),
                account_area.y,
                avatar_w,
                1,
            );
            if super::thumbnail::visible(app, avatar_area) {
                if let Some(proto) = app.thumbs.get_cover_protocol(&url, avatar_area.as_size()) {
                    f.render_widget(proto, avatar_area);
                }
            }
            text_area.width = text_area.width.saturating_sub(avatar_w + 1);
        }
    }
    f.render_widget(
        Paragraph::new(crate::ui::text::truncate_str(
            &account,
            text_area.width as usize,
        ))
        .alignment(ratatui::layout::Alignment::Right)
        .style(app.theme.style("status")),
        text_area,
    );
}
