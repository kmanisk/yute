use crate::app::{App, DetailRoute};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn render_channel(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(DetailRoute::Channel(route)) = &app.detail_route else {
        return;
    };
    let info = [
        route.channel.handle.as_deref(),
        route.channel.subscribers.as_deref(),
        route
            .subscribed
            .map(|s| if s { "Subscribed" } else { "Not subscribed" }),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");
    let header_height = if route.tabs.is_empty() { 1 } else { 2 };
    let tabs = route
        .tabs
        .iter()
        .enumerate()
        .map(|(i, t)| {
            Span::styled(
                format!(" {} ", t.title),
                if i == route.active {
                    Style::default()
                        .fg(app.theme.chip_active_fg)
                        .bg(app.theme.chip_active_bg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    app.theme.style("text_dim")
                },
            )
        })
        .collect::<Vec<_>>();
    f.render_widget(
        Paragraph::new(vec![Line::from(info), Line::from(tabs)]),
        Rect::new(area.x, area.y, area.width, area.height.min(header_height)),
    );
    let grid = Rect::new(
        area.x,
        area.y.saturating_add(header_height),
        area.width,
        area.height.saturating_sub(header_height),
    );
    super::grid::render_grid(f, app, grid);
}
