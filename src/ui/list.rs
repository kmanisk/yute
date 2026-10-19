use crate::app::{App, Focus};
use crate::backend::models::{Card, LiveState};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn render_list(f: &mut Frame, app: &mut App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let total_items = app.filtered_items().len();
    if total_items == 0 {
        let msg = if app.is_loading {
            "Searching..."
        } else {
            "No results. Press '/' to search with new query."
        };
        let p = Paragraph::new(msg)
            .alignment(Alignment::Center)
            .style(Style::default().fg(app.theme.text_secondary));
        f.render_widget(p, area);
        return;
    }

    let row_h = 7u16;
    let visible_rows = (area.height / row_h).max(1) as usize;

    let selected = app.selected_index.min(total_items.saturating_sub(1));
    let start_idx = selected.saturating_sub(visible_rows / 2);

    let cards_to_render: Vec<Card> = app
        .filtered_items()
        .into_iter()
        .skip(start_idx)
        .take(visible_rows)
        .cloned()
        .collect();

    for (r, card) in cards_to_render.iter().enumerate() {
        let idx = start_idx + r;

        let row_rect = Rect {
            x: area.x,
            y: area.y + (r as u16) * row_h,
            width: area.width,
            height: row_h,
        };

        let is_selected = idx == selected && app.focus == Focus::Content;
        render_list_row(f, app, card, row_rect, is_selected);
    }
}

fn render_list_row(
    f: &mut Frame,
    app: &mut App,
    card: &Card,
    area: Rect,
    is_selected: bool,
) {
    let (border_col, bg_col) = if is_selected {
        (app.theme.accent, app.theme.surface_hover)
    } else {
        (app.theme.border, app.theme.surface)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_col))
        .style(Style::default().bg(bg_col));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.width < 35 || inner.height < 4 {
        return;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(28), // Thumbnail left
            Constraint::Min(20),   // Video details right
        ])
        .split(inner);

    let thumb_area = cols[0];
    let details_area = cols[1];

    let thumb_url = card.thumb_url().to_string();
    if !thumb_url.is_empty() && app.config.general.graphics != "none" {
        if let Some(proto) = app.thumbs.get_protocol(&thumb_url) {
            let img_widget = ratatui_image::StatefulImage::default();
            f.render_stateful_widget(img_widget, thumb_area, proto);
        } else {
            let p = Paragraph::new("[Thumb loading...]")
                .alignment(Alignment::Center)
                .style(Style::default().fg(app.theme.text_secondary));
            f.render_widget(p, thumb_area);
        }
    }

    match card {
        Card::Video(v) => {
            let mut lines = Vec::new();

            // Title
            lines.push(Line::from(vec![
                Span::styled(&v.title, Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
            ]));

            // Channel and stats
            let stat_line = format!(
                "{} · {} · {}",
                v.channel,
                v.views.as_deref().unwrap_or(""),
                v.published.as_deref().unwrap_or("")
            );
            lines.push(Line::from(Span::styled(stat_line, Style::default().fg(app.theme.text_secondary))));

            // Badges
            let mut badge_spans = Vec::new();
            if v.live == LiveState::Live {
                badge_spans.push(Span::styled(" LIVE ", Style::default().bg(app.theme.live_badge_bg).fg(Color::White).add_modifier(Modifier::BOLD)));
                badge_spans.push(Span::raw(" "));
            }
            if let Some(ref dur) = v.duration {
                badge_spans.push(Span::styled(format!(" {dur} "), Style::default().bg(app.theme.badge_bg).fg(app.theme.text)));
                badge_spans.push(Span::raw(" "));
            }
            for b in &v.badges {
                badge_spans.push(Span::styled(format!(" {b} "), Style::default().bg(app.theme.border).fg(app.theme.text)));
                badge_spans.push(Span::raw(" "));
            }
            if !badge_spans.is_empty() {
                lines.push(Line::from(badge_spans));
            }

            let p = Paragraph::new(lines);
            f.render_widget(p, details_area);
        }
        Card::Playlist(p) => {
            let lines = vec![
                Line::from(Span::styled(&p.title, Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(&p.channel, Style::default().fg(app.theme.text_secondary))),
                Line::from(Span::styled(p.video_count.as_deref().unwrap_or("Playlist"), Style::default().fg(app.theme.text_secondary))),
            ];
            f.render_widget(Paragraph::new(lines), details_area);
        }
        Card::Channel(c) => {
            let lines = vec![
                Line::from(Span::styled(&c.title, Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD))),
                Line::from(Span::styled(c.subscribers.as_deref().unwrap_or("Channel"), Style::default().fg(app.theme.text_secondary))),
            ];
            f.render_widget(Paragraph::new(lines), details_area);
        }
    }
}
