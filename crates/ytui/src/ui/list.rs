use crate::app::{App, Focus};
use crate::backend::models::{Card, LiveState};
use crate::ui::text::{format_meta_line, sanitize_title, wrap_and_truncate_title};
use ratatui::layout::Rect;
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
            .style(Style::default().fg(app.theme.text_secondary));
        f.render_widget(p, area);
        return;
    }

    let cell_w = app.cell_w_px;
    let cell_h = app.cell_h_px;

    // Fixed 24 columns for thumbnail on left
    let thumb_w = 24u16;
    let thumb_rows = ((thumb_w as f32) * cell_w * 9.0 / (16.0 * cell_h)).round() as u16;
    let thumb_h = thumb_rows.clamp(3, 6);

    // Row height = thumbnail height + 1 (reserved gutter)
    let row_h = thumb_h + 2;
    let gutter_y = 1u16;

    let visible_rows = ((area.height + gutter_y) / (row_h + gutter_y)).max(1) as usize;
    let selected = app.selected_index.min(total_items.saturating_sub(1));

    if selected < app.scroll_row + 1 {
        app.scroll_row = selected.saturating_sub(1);
    }
    if selected + 1 >= app.scroll_row + visible_rows {
        app.scroll_row = (selected + 2).saturating_sub(visible_rows);
    }
    app.scroll_row = app.scroll_row.min(total_items.saturating_sub(visible_rows));

    let start_idx = app.scroll_row;
    let cards_to_render: Vec<Card> = app
        .filtered_items()
        .into_iter()
        .skip(start_idx)
        .take(visible_rows)
        .cloned()
        .collect();

    for (r, card) in cards_to_render.iter().enumerate() {
        let idx = start_idx + r;
        let card_y = area.y + (r as u16) * (row_h + gutter_y);
        if card_y + row_h > area.y + area.height {
            break;
        }

        let row_rect = Rect {
            x: area.x + 1,
            y: card_y,
            width: area.width.saturating_sub(2),
            height: row_h,
        };

        let is_selected = idx == selected && app.focus == Focus::Content;
        render_list_row(f, app, card, row_rect, is_selected, thumb_w, thumb_h);
    }

    // Scrollbar on right edge
    if total_items > visible_rows && area.width > 2 {
        let sb_x = area.x + area.width - 1;
        let sb_height = area.height.saturating_sub(2);
        if sb_height > 2 {
            let thumb_size = ((visible_rows as f32 / total_items as f32) * (sb_height as f32)).round().max(1.0) as u16;
            let thumb_pos = ((start_idx as f32 / (total_items.saturating_sub(visible_rows).max(1) as f32))
                * ((sb_height.saturating_sub(thumb_size)) as f32)).round() as u16;

            for y_off in 0..sb_height {
                let y = area.y + 1 + y_off;
                let (ch, style) = if y_off >= thumb_pos && y_off < thumb_pos + thumb_size {
                    ("█", Style::default().fg(app.theme.accent))
                } else {
                    ("│", Style::default().fg(app.theme.border))
                };
                f.render_widget(Paragraph::new(ch).style(style), Rect { x: sb_x, y, width: 1, height: 1 });
            }
        }
    }
}

fn render_list_row(
    f: &mut Frame,
    app: &mut App,
    card: &Card,
    area: Rect,
    is_selected: bool,
    thumb_w: u16,
    thumb_h: u16,
) {
    if area.width < 35 || area.height < 4 {
        return;
    }

    if is_selected {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.theme.accent))
            .style(Style::default().bg(app.theme.surface_hover));
        f.render_widget(block, area);
    }

    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    let actual_thumb_w = thumb_w.min(inner.width.saturating_sub(20));
    let actual_thumb_h = thumb_h.min(inner.height);

    let thumb_area = Rect {
        x: inner.x,
        y: inner.y,
        width: actual_thumb_w,
        height: actual_thumb_h,
    };

    let details_x = inner.x + actual_thumb_w + 2;
    let details_w = inner.width.saturating_sub(actual_thumb_w + 2);

    let is_shorts = match card {
        Card::Video(v) => v.badges.iter().any(|b| b == "SHORTS"),
        _ => false,
    };

    // Render thumbnail or skeleton
    let thumb_url = card.thumb_url().to_string();
    if !thumb_url.is_empty() && app.config.general.graphics != "none" {
        if let Some(proto) = app.thumbs.get_protocol(&thumb_url) {
            if is_shorts {
                let short_w = ((thumb_area.height as f32) * app.cell_h_px * 9.0 / (16.0 * app.cell_w_px)).round() as u16;
                let short_w = short_w.clamp(4, thumb_area.width);
                let x_off = (thumb_area.width.saturating_sub(short_w)) / 2;
                let bg_p = Paragraph::new("").style(Style::default().bg(Color::Rgb(15, 15, 15)));
                f.render_widget(bg_p, thumb_area);

                let short_rect = Rect {
                    x: thumb_area.x + x_off,
                    y: thumb_area.y,
                    width: short_w,
                    height: thumb_area.height,
                };
                let img_widget = ratatui_image::StatefulImage::default();
                f.render_stateful_widget(img_widget, short_rect, proto);
            } else {
                let img_widget = ratatui_image::StatefulImage::default();
                f.render_stateful_widget(img_widget, thumb_area, proto);
            }
        } else {
            let skeleton_line = "▒".repeat(thumb_area.width as usize);
            let mut skel_lines = Vec::new();
            for _ in 0..thumb_area.height {
                skel_lines.push(Line::from(Span::styled(
                    skeleton_line.clone(),
                    Style::default().fg(app.theme.border),
                )));
            }
            f.render_widget(Paragraph::new(skel_lines), thumb_area);
        }
    } else {
        let skeleton_line = "▒".repeat(thumb_area.width as usize);
        let mut skel_lines = Vec::new();
        for _ in 0..thumb_area.height {
            skel_lines.push(Line::from(Span::styled(
                skeleton_line.clone(),
                Style::default().fg(app.theme.border),
            )));
        }
        f.render_widget(Paragraph::new(skel_lines), thumb_area);
    }

    // Corner badges on thumbnail
    match card {
        Card::Video(v) => {
            if v.live == LiveState::Live {
                let badge_text = " LIVE ";
                let badge_w = 6u16.min(thumb_area.width);
                let badge_rect = Rect {
                    x: thumb_area.x + thumb_area.width.saturating_sub(badge_w),
                    y: thumb_area.y + thumb_area.height.saturating_sub(1),
                    width: badge_w,
                    height: 1,
                };
                let p = Paragraph::new(badge_text)
                    .style(Style::default().bg(app.theme.live_badge_bg).fg(Color::White).add_modifier(Modifier::BOLD));
                f.render_widget(p, badge_rect);
            } else if is_shorts {
                let badge_text = " SHORTS ";
                let badge_w = 8u16.min(thumb_area.width);
                let badge_rect = Rect {
                    x: thumb_area.x,
                    y: thumb_area.y,
                    width: badge_w,
                    height: 1,
                };
                let p = Paragraph::new(badge_text)
                    .style(Style::default().bg(Color::Rgb(200, 20, 20)).fg(Color::White).add_modifier(Modifier::BOLD));
                f.render_widget(p, badge_rect);
            } else if let Some(ref dur) = v.duration {
                let badge_text = format!(" {dur} ");
                let badge_w = (badge_text.len() as u16).min(thumb_area.width);
                let badge_rect = Rect {
                    x: thumb_area.x + thumb_area.width.saturating_sub(badge_w),
                    y: thumb_area.y + thumb_area.height.saturating_sub(1),
                    width: badge_w,
                    height: 1,
                };
                let p = Paragraph::new(badge_text)
                    .style(Style::default().bg(Color::Rgb(15, 15, 15)).fg(Color::White).add_modifier(Modifier::BOLD));
                f.render_widget(p, badge_rect);
            }
        }
        Card::Playlist(p) => {
            let badge_text = if p.is_mix { " MIX " } else { " PLAYLIST " };
            let badge_w = (badge_text.len() as u16).min(thumb_area.width);
            let badge_rect = Rect {
                x: thumb_area.x + thumb_area.width.saturating_sub(badge_w),
                y: thumb_area.y + thumb_area.height.saturating_sub(1),
                width: badge_w,
                height: 1,
            };
            let p_w = Paragraph::new(badge_text)
                .style(Style::default().bg(app.theme.badge_bg).fg(app.theme.accent).add_modifier(Modifier::BOLD));
            f.render_widget(p_w, badge_rect);
        }
        _ => {}
    }

    let sec_style = if is_selected {
        Style::default().fg(app.theme.text)
    } else {
        Style::default().fg(app.theme.text_secondary)
    };

    // Render details right
    let raw_title = card.title();
    let clean = sanitize_title(raw_title, app.config.strip_emoji());
    let title_lines = wrap_and_truncate_title(&clean, details_w as usize, 2);

    let mut details_lines = Vec::new();
    for tl in title_lines {
        details_lines.push(Line::from(Span::styled(
            tl,
            Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD),
        )));
    }

    match card {
        Card::Video(v) => {
            let meta_spans = format_meta_line(
                &v.channel,
                v.verified,
                v.views.as_deref(),
                v.published.as_deref(),
                details_w as usize,
                sec_style,
            );
            details_lines.push(Line::from(meta_spans));
        }
        Card::Playlist(p) => {
            let meta_text = if p.is_mix {
                "YouTube Mix".to_string()
            } else {
                let count = p.video_count.as_deref().unwrap_or("Playlist");
                format!("{} · {}", p.channel, count)
            };
            details_lines.push(Line::from(Span::styled(meta_text, sec_style)));
        }
        Card::Channel(c) => {
            let subs = c.subscribers.as_deref().unwrap_or("Channel");
            details_lines.push(Line::from(Span::styled(subs, sec_style)));
        }
    }

    let details_area = Rect {
        x: details_x,
        y: inner.y,
        width: details_w,
        height: inner.height,
    };
    f.render_widget(Paragraph::new(details_lines), details_area);
}
