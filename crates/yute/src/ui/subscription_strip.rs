use crate::{
    app::{App, Focus, Screen},
    backend::models::Card,
};
use ratatui::{layout::Rect, widgets::Paragraph, Frame};
pub fn height(app: &App, area: Rect) -> u16 {
    if app.screen == Screen::Subscriptions
        && !app.subscribed_channels.is_empty()
        && area.height >= 13
    {
        3
    } else {
        0
    }
}
pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let columns = (area.width / 16).max(1) as usize;
    let start = app.subscriptions_strip_index.saturating_sub(columns - 1);
    let channels = app.subscribed_channels.clone();
    for (slot, (index, card)) in channels
        .iter()
        .enumerate()
        .skip(start)
        .take(columns)
        .enumerate()
    {
        let Card::Channel(channel) = card else {
            continue;
        };
        let x = area.x + slot as u16 * 16;
        let width = 16.min(area.right() - x);
        let a = Rect::new(x, area.y, 2.min(width), 1);
        super::thumbnail::avatar(f, app, channel.avatar_url.as_deref(), &channel.title, a);
        let label = super::text::truncate_str(&channel.title, width.saturating_sub(1) as usize);
        f.render_widget(
            Paragraph::new(label).style(
                if app.focus == Focus::SubscriptionsStrip && index == app.subscriptions_strip_index
                {
                    app.theme.style("selection")
                } else {
                    app.theme.style("text_dim")
                },
            ),
            Rect::new(x, area.y + 1, width, 1),
        );
    }
}
