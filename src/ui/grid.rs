use crate::app::{App, Focus};
use crate::backend::models::{Card, LiveState};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn render_grid(f: &mut Frame, app: &mut App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let total_items = app.filtered_items().len();
    if total_items == 0 {
        let msg = if app.is_loading {
            "Loading feed..."
        } else {
            "No videos found. Press 'r' to refresh or '/' to search."
        };
        let p = Paragraph::new(msg)
            .alignment(Alignment::Center)
            .style(Style::default().fg(app.theme.text_secondary));
        f.render_widget(p, area);
        return;
    }

    let min_w = app.config.grid.min_card_width.max(28);
    let num_cols = (area.width / min_w).clamp(1, 6) as usize;
    let card_w = area.width / (num_cols as u16);

    let thumb_rows = app.thumbs.compute_thumbnail_rows(card_w);
    let card_h = thumb_rows + 7; // border(2) + thumb + badge(1) + title(2) + channel(1) + stats(1)

    if card_h == 0 {
        return;
    }

    let num_rows = (area.height / card_h).max(1) as usize;

    // Scroll calculation to keep selected_index in view
    let selected = app.selected_index.min(total_items.saturating_sub(1));
    let start_row = (selected / num_cols).saturating_sub(num_rows / 2);
    let start_idx = start_row * num_cols;
    let visible_count = num_rows * num_cols;

    let cards_to_render: Vec<Card> = app
        .filtered_items()
        .into_iter()
        .skip(start_idx)
        .take(visible_count)
        .cloned()
        .collect();

    for (i, card) in cards_to_render.iter().enumerate() {
        let item_idx = start_idx + i;
        let r = i / num_cols;
        let c = i % num_cols;

        let card_rect = Rect {
            x: area.x + (c as u16) * card_w,
            y: area.y + (r as u16) * card_h,
            width: card_w,
            height: card_h,
        };

        let is_selected = item_idx == selected && app.focus == Focus::Content;
        render_card(f, app, card, card_rect, thumb_rows, is_selected);
    }
}

fn render_card(
    f: &mut Frame,
    app: &mut App,
    card: &Card,
    area: Rect,
    thumb_rows: u16,
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

    if inner.height < 5 || inner.width < 10 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(thumb_rows.min(inner.height.saturating_sub(4))),
            Constraint::Length(1), // Badge row
            Constraint::Length(2), // Title (up to 2 lines)
            Constraint::Length(1), // Channel line
            Constraint::Length(1), // Stats line
        ])
        .split(inner);

    let thumb_area = chunks[0];
    let badge_area = chunks[1];
    let title_area = chunks[2];
    let channel_area = chunks[3];
    let stats_area = chunks[4];

    // Thumbnail image rendering
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
            // Badge row (duration / LIVE / progress)
            let mut badge_spans = Vec::new();
            if let Some(pct) = v.progress_pct {
                let filled = (pct as usize * 10) / 100;
                let track = format!("[{}{}]", "=".repeat(filled), " ".repeat(10 - filled));
                badge_spans.push(Span::styled(track, Style::default().fg(app.theme.red)));
            }

            badge_spans.push(Span::raw(" "));
            match v.live {
                LiveState::Live => {
                    badge_spans.push(Span::styled(
                        " LIVE ",
                        Style::default()
                            .bg(app.theme.live_badge_bg)
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
                LiveState::Upcoming => {
                    badge_spans.push(Span::styled(
                        " UPCOMING ",
                        Style::default()
                            .bg(app.theme.badge_bg)
                            .fg(app.theme.text)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
                LiveState::None => {
                    if let Some(ref dur) = v.duration {
                        badge_spans.push(Span::styled(
                            format!(" {dur} "),
                            Style::default()
                                .bg(app.theme.badge_bg)
                                .fg(app.theme.text),
                        ));
                    }
                }
            }

            let badge_p = Paragraph::new(Line::from(badge_spans)).alignment(Alignment::Right);
            f.render_widget(badge_p, badge_area);

            // Title
            let title_p = Paragraph::new(v.title.as_str())
                .style(Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD));
            f.render_widget(title_p, title_area);

            // Channel line with verified mark
            let mut ch_spans = vec![
                Span::styled(&v.channel, Style::default().fg(app.theme.text_secondary)),
            ];
            if v.verified {
                ch_spans.push(Span::styled(" [v]", Style::default().fg(app.theme.text_secondary)));
            }
            f.render_widget(Paragraph::new(Line::from(ch_spans)), channel_area);

            // Stats line
            let mut stat_parts = Vec::new();
            if let Some(ref vw) = v.views {
                stat_parts.push(vw.as_str());
            }
            if let Some(ref pub_time) = v.published {
                stat_parts.push(pub_time.as_str());
            }
            let stat_str = stat_parts.join(" · ");
            let stat_p = Paragraph::new(stat_str)
                .style(Style::default().fg(app.theme.text_secondary));
            f.render_widget(stat_p, stats_area);
        }
        Card::Playlist(p) => {
            let count_str = p.video_count.as_deref().unwrap_or("Playlist");
            let badge_str = if p.is_mix { "MIX" } else { "PLAYLIST" };
            let badge_p = Paragraph::new(Line::from(vec![
                Span::styled(format!(" {badge_str} · {count_str} "), Style::default().bg(app.theme.badge_bg).fg(app.theme.text)),
            ])).alignment(Alignment::Right);
            f.render_widget(badge_p, badge_area);

            let title_p = Paragraph::new(p.title.as_str())
                .style(Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD));
            f.render_widget(title_p, title_area);

            let ch_p = Paragraph::new(p.channel.as_str())
                .style(Style::default().fg(app.theme.text_secondary));
            f.render_widget(ch_p, channel_area);
        }
        Card::Channel(c) => {
            let title_p = Paragraph::new(c.title.as_str())
                .style(Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD));
            f.render_widget(title_p, title_area);

            let subs = c.subscribers.as_deref().unwrap_or("Channel");
            let subs_p = Paragraph::new(subs)
                .style(Style::default().fg(app.theme.text_secondary));
            f.render_widget(subs_p, channel_area);
        }
    }
}
