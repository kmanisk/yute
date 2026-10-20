use crate::app::{App, Focus};
use crate::backend::models::{Card, LiveState};
use crate::ui::text::{format_meta_line, sanitize_title, wrap_and_truncate_title};
use ratatui::layout::{Alignment, Rect};
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
            "No videos found. Press 'r' to refresh, 'f' to change filter, or '/' to search."
        };
        let p = Paragraph::new(msg)
            .alignment(Alignment::Center)
            .style(Style::default().fg(app.theme.text_secondary));
        f.render_widget(p, area);
        return;
    }

    let cell_w = app.cell_w_px;
    let cell_h = app.cell_h_px;

    // Number of columns: respect config.grid.columns if set (> 0), clamped to max fit (min 24 cols per card)
    let num_cols = if app.config.grid.columns > 0 {
        let max_fit = (area.width / 24).max(1);
        (app.config.grid.columns).min(max_fit).clamp(1, 4) as usize
    } else {
        (area.width / 36).clamp(1, 4) as usize
    };

    // 1-cell gutter between columns, equal 1-cell left and right margin
    let margin_x = 1u16;
    let gutter_x = 1u16;
    let net_avail_w = area.width.saturating_sub(margin_x * 2 + (num_cols.saturating_sub(1) as u16) * gutter_x);
    let base_col_w = net_avail_w / (num_cols as u16);
    let rem_w = net_avail_w % (num_cols as u16);

    let mut col_widths = Vec::with_capacity(num_cols);
    for c in 0..num_cols {
        let w = base_col_w + if (c as u16) < rem_w { 1 } else { 0 };
        col_widths.push(w);
    }

    // Fixed card height calculation:
    // Thumbnail width is card inner width (card_w - 2 for reserved 1-cell padding ring)
    let min_col_w = *col_widths.iter().min().unwrap_or(&30);
    let thumb_cols = min_col_w.saturating_sub(2);
    let thumb_rows = ((thumb_cols as f32) * cell_w * 9.0 / (16.0 * cell_h)).round() as u16;

    // Dynamic vertical gutter: in tight viewports (< 24 cells), omit gutter between rows to maximize visible content
    let gutter_y = if area.height < 24 { 0u16 } else { 1u16 };

    // Determine target rows to maximize screen usage without leaving large blank gaps
    let target_rows = if area.height >= 21 {
        3u16
    } else if area.height >= 14 {
        2u16
    } else {
        1u16
    };

    let max_card_h = (area.height + gutter_y) / target_rows - gutter_y;
    let card_h = max_card_h.clamp(7, 12);
    let inner_h = card_h.saturating_sub(2);
    let title_h = if inner_h >= 6 { 2u16 } else { 1u16 };
    let thumb_h = inner_h.saturating_sub(title_h + 1).min(thumb_rows).clamp(3, 7);

    // visible_rows: whole rows that fit cleanly
    let visible_rows = ((area.height + gutter_y) / (card_h + gutter_y)).max(1) as usize;
    let total_rows = (total_items + num_cols - 1) / num_cols;

    let selected = app.selected_index.min(total_items.saturating_sub(1));
    let selected_row = selected / num_cols;

    // Clean scroll-off navigation without spurious row offsets
    if visible_rows >= 3 {
        if selected_row < app.scroll_row + 1 && app.scroll_row > 0 {
            app.scroll_row = selected_row.saturating_sub(1);
        } else if selected_row + 1 >= app.scroll_row + visible_rows {
            app.scroll_row = (selected_row + 2).saturating_sub(visible_rows);
        }
    } else {
        if selected_row < app.scroll_row {
            app.scroll_row = selected_row;
        } else if selected_row >= app.scroll_row + visible_rows {
            app.scroll_row = selected_row + 1 - visible_rows;
        }
    }
    app.scroll_row = app.scroll_row.min(total_rows.saturating_sub(visible_rows));

    let start_row = app.scroll_row;
    let start_idx = start_row * num_cols;
    let visible_count = visible_rows * num_cols;

    let cards_to_render: Vec<Card> = app
        .filtered_items()
        .into_iter()
        .skip(start_idx)
        .take(visible_count)
        .cloned()
        .collect();

    // Render cards
    for r in 0..visible_rows {
        let card_y = area.y + (r as u16) * (card_h + gutter_y);
        if card_y + card_h > area.y + area.height {
            break;
        }

        let mut card_x = area.x + margin_x;
        for c in 0..num_cols {
            let i = r * num_cols + c;
            let card_w = col_widths[c];

            let card_rect = Rect {
                x: card_x,
                y: card_y,
                width: card_w,
                height: card_h,
            };

            card_x += card_w + gutter_x;

            if i < cards_to_render.len() {
                let item_idx = start_idx + i;
                let is_selected = item_idx == selected && app.focus == Focus::Content;
                render_grid_card(f, app, &cards_to_render[i], card_rect, is_selected, thumb_h);
            }
        }
    }

    // Scrollbar on the right edge if there are more rows than visible
    if total_rows > visible_rows && area.width > 2 {
        let sb_x = area.x + area.width - 1;
        let sb_height = area.height.saturating_sub(2);
        if sb_height > 2 {
            let thumb_size = ((visible_rows as f32 / total_rows as f32) * (sb_height as f32)).round().max(1.0) as u16;
            let thumb_pos = if start_row == 0 || total_rows <= visible_rows {
                0
            } else {
                ((start_row as f32 / (total_rows.saturating_sub(visible_rows).max(1) as f32))
                    * ((sb_height.saturating_sub(thumb_size)) as f32)).round() as u16
            };

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

fn render_grid_card(
    f: &mut Frame,
    app: &mut App,
    card: &Card,
    area: Rect,
    is_selected: bool,
    thumb_h: u16,
) {
    if area.width < 10 || area.height < 4 {
        return;
    }

    // Awesome-TUI rounded card container
    let card_block = if is_selected {
        Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(app.theme.surface_hover))
    } else {
        Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Rounded)
            .border_style(Style::default().fg(app.theme.border))
            .style(Style::default().bg(app.theme.bg))
    };
    f.render_widget(card_block, area);

    // Content area inside 1-cell padding ring
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    if inner.width < 8 || inner.height < 3 {
        return;
    }

    let title_h = if inner.height >= 6 { 2u16 } else { 1u16 };
    let actual_thumb_h = inner.height.saturating_sub(title_h + 1).clamp(2, thumb_h);

    let thumb_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: actual_thumb_h,
    };

    let title_area = Rect {
        x: inner.x,
        y: inner.y + actual_thumb_h,
        width: inner.width,
        height: title_h,
    };

    let meta_area = Rect {
        x: inner.x,
        y: inner.y + actual_thumb_h + title_h,
        width: inner.width,
        height: 1,
    };

    let is_shorts = match card {
        Card::Video(v) => v.badges.iter().any(|b| b == "SHORTS"),
        _ => false,
    };

    // Modal overlap suppression
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

    let thumb_url = card.thumb_url().to_string();
    if overlaps_modal {
        let p = Paragraph::new("").style(Style::default().bg(app.theme.surface));
        f.render_widget(p, thumb_area);
    } else if !thumb_url.is_empty() && app.config.general.graphics != "none" {
        if let Some(proto) = app.thumbs.get_protocol(&thumb_url) {
            if is_shorts {
                // Center 9:16 vertical Short thumbnail in the 16:9 slot
                let short_w = ((thumb_area.height as f32) * app.cell_h_px * 9.0 / (16.0 * app.cell_w_px)).round() as u16;
                let short_w = short_w.clamp(6, thumb_area.width);
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
            // Skeleton of exact thumbnail rect (dim ▒ block)
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
    } else if !overlaps_modal {
        // Graphics disabled or thumb_url pending: render exact skeleton rect (dim ▒ block)
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

    // Render corner badges
    let now_playing = app.player.is_alive() && app.player.status.file_loaded && app.player.current_video_id() == Some(card.id());
    if now_playing {
        let badge_text = " ▶ Now playing ";
        let badge_w = (badge_text.len() as u16).min(thumb_area.width);
        let badge_rect = Rect {
            x: thumb_area.x + thumb_area.width.saturating_sub(badge_w),
            y: thumb_area.y,
            width: badge_w,
            height: 1,
        };
        let p = Paragraph::new(badge_text)
            .style(Style::default().bg(app.theme.accent).fg(Color::White).add_modifier(Modifier::BOLD));
        f.render_widget(p, badge_rect);
    } else {
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
    }

    // Secondary text style with guaranteed readable contrast
    let sec_style = if is_selected {
        Style::default().fg(app.theme.text)
    } else {
        Style::default().fg(app.theme.text_secondary)
    };

    // Render Title (max 2 lines, word-boundary wrapped with ellipsis)
    let raw_title = card.title();
    let clean = sanitize_title(raw_title, app.config.strip_emoji());
    let title_lines = wrap_and_truncate_title(&clean, title_area.width as usize, title_h as usize);

    let mut line_spans = Vec::new();
    for (l_idx, tl) in title_lines.into_iter().enumerate() {
        if l_idx == 0 {
            if let Card::Video(v) = card {
                if let Some(pct) = v.progress_pct {
                    let filled = ((pct as usize) * 5) / 100;
                    let track = format!("[{}{}] ", "=".repeat(filled), " ".repeat(5 - filled));
                    line_spans.push(Line::from(vec![
                        Span::styled(track, Style::default().fg(app.theme.red)),
                        Span::styled(tl, Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
                    ]));
                    continue;
                }
            }
        }
        line_spans.push(Line::from(Span::styled(
            tl,
            Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD),
        )));
    }
    f.render_widget(Paragraph::new(line_spans), title_area);

    // Render Meta Line
    match card {
        Card::Video(v) => {
            let meta_spans = format_meta_line(
                &v.channel,
                v.verified,
                v.views.as_deref(),
                v.published.as_deref(),
                meta_area.width as usize,
                sec_style,
            );
            f.render_widget(Paragraph::new(Line::from(meta_spans)), meta_area);
        }
        Card::Playlist(p) => {
            let meta_text = if p.is_mix {
                "YouTube Mix".to_string()
            } else {
                let count = p.video_count.as_deref().unwrap_or("Playlist");
                format!("{} · {}", p.channel, count)
            };
            f.render_widget(Paragraph::new(meta_text).style(sec_style), meta_area);
        }
        Card::Channel(c) => {
            let subs = c.subscribers.as_deref().unwrap_or("Channel");
            f.render_widget(Paragraph::new(subs).style(sec_style), meta_area);
        }
    }
}
