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

    let num_cols = if app.config.grid.columns > 0 {
        let max_fit = (area.width / 24).max(1);
        (app.config.grid.columns).min(max_fit).max(1) as usize
    } else {
        let min_w = app.config.grid.min_card_width.max(26);
        (area.width / min_w).clamp(1, 6) as usize
    };
    let card_w = area.width / (num_cols as u16);

    // Compact card height to fit 3 rows of windows (3x3 grid)
    let card_h = (area.height / 3).clamp(7, 10);

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
        render_card(f, app, card, card_rect, is_selected);
    }
}

fn render_card(
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

    if inner.height < 3 || inner.width < 10 {
        return;
    }

    let title_lines = if inner.height >= 7 { 2 } else { 1 };
    let thumb_h = inner.height.saturating_sub(title_lines + 1);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(thumb_h),
            Constraint::Length(title_lines),
            Constraint::Length(1), // Meta line
        ])
        .split(inner);

    let thumb_area = chunks[0];
    let title_area = chunks[1];
    let meta_area = chunks[2];

    // Thumbnail image rendering
    let thumb_url = card.thumb_url().to_string();
    if !thumb_url.is_empty() && app.config.general.graphics != "none" {
        let screen_size = f.area();
        let dialog_w = 44.min(screen_size.width.saturating_sub(4));
        let dialog_h = 7.min(screen_size.height.saturating_sub(2));
        let modal_x = (screen_size.width.saturating_sub(dialog_w)) / 2;
        let modal_y = (screen_size.height.saturating_sub(dialog_h)) / 2;
        let overlaps_modal = app.confirming_quit
            && thumb_area.x < modal_x + dialog_w
            && thumb_area.x + thumb_area.width > modal_x
            && thumb_area.y < modal_y + dialog_h
            && thumb_area.y + thumb_area.height > modal_y;

        if overlaps_modal {
            // Suppress Kitty/Sixel graphics when dialog is displayed on top
            let p = Paragraph::new("")
                .style(Style::default().bg(app.theme.surface));
            f.render_widget(p, thumb_area);
        } else if let Some(proto) = app.thumbs.get_protocol(&thumb_url) {
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
            // Title line
            let mut title_spans = Vec::new();
            if let Some(pct) = v.progress_pct {
                let filled = ((pct as usize) * 5) / 100;
                let track = format!("[{}{}] ", "=".repeat(filled), " ".repeat(5 - filled));
                title_spans.push(Span::styled(track, Style::default().fg(app.theme.red)));
            }
            title_spans.push(Span::styled(
                v.title.as_str(),
                Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD),
            ));
            let title_p = Paragraph::new(Line::from(title_spans));
            f.render_widget(title_p, title_area);

            // Meta line: Channel [v] · duration/LIVE · views
            let mut meta_spans = Vec::new();
            meta_spans.push(Span::styled(&v.channel, Style::default().fg(app.theme.text_secondary)));
            if v.verified {
                meta_spans.push(Span::styled(" [v]", Style::default().fg(app.theme.text_secondary)));
            }

            match v.live {
                LiveState::Live => {
                    meta_spans.push(Span::styled(" · ", Style::default().fg(app.theme.text_secondary)));
                    meta_spans.push(Span::styled(
                        " LIVE ",
                        Style::default()
                            .bg(app.theme.live_badge_bg)
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
                LiveState::Upcoming => {
                    meta_spans.push(Span::styled(" · ", Style::default().fg(app.theme.text_secondary)));
                    meta_spans.push(Span::styled(
                        " UPCOMING ",
                        Style::default()
                            .bg(app.theme.badge_bg)
                            .fg(app.theme.text)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
                LiveState::None => {
                    if let Some(ref dur) = v.duration {
                        meta_spans.push(Span::styled(" · ", Style::default().fg(app.theme.text_secondary)));
                        meta_spans.push(Span::styled(
                            dur.as_str(),
                            Style::default().fg(app.theme.text_secondary),
                        ));
                    }
                }
            }

            if let Some(ref vw) = v.views {
                meta_spans.push(Span::styled(" · ", Style::default().fg(app.theme.text_secondary)));
                meta_spans.push(Span::styled(vw.as_str(), Style::default().fg(app.theme.text_secondary)));
            }

            let meta_p = Paragraph::new(Line::from(meta_spans));
            f.render_widget(meta_p, meta_area);
        }
        Card::Playlist(p) => {
            let title_p = Paragraph::new(p.title.as_str())
                .style(Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD));
            f.render_widget(title_p, title_area);

            let meta_spans = if p.is_mix {
                vec![
                    Span::styled(&p.channel, Style::default().fg(app.theme.text_secondary)),
                    Span::styled(" · ", Style::default().fg(app.theme.text_secondary)),
                    Span::styled(
                        "Mix · YouTube",
                        Style::default().bg(app.theme.badge_bg).fg(app.theme.accent).add_modifier(Modifier::BOLD),
                    ),
                ]
            } else {
                let count_str = p.video_count.as_deref().unwrap_or("Playlist");
                vec![
                    Span::styled(&p.channel, Style::default().fg(app.theme.text_secondary)),
                    Span::styled(" · ", Style::default().fg(app.theme.text_secondary)),
                    Span::styled(
                        format!("PLAYLIST ({count_str})"),
                        Style::default().bg(app.theme.badge_bg).fg(app.theme.text),
                    ),
                ]
            };
            let meta_p = Paragraph::new(Line::from(meta_spans));
            f.render_widget(meta_p, meta_area);
        }
        Card::Channel(c) => {
            let title_p = Paragraph::new(c.title.as_str())
                .style(Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD));
            f.render_widget(title_p, title_area);

            let subs = c.subscribers.as_deref().unwrap_or("Channel");
            let meta_p = Paragraph::new(subs)
                .style(Style::default().fg(app.theme.text_secondary));
            f.render_widget(meta_p, meta_area);
        }
    }
}
