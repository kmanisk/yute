use crate::app::{App, Focus};
use crate::backend::models::{Card, LiveState};
use crate::ui::text::{format_meta_line, truncate_str, wrap_and_truncate_title};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Content width includes the outer margins and one-cell gutters.
pub fn column_count(width: u16, config: &crate::config::GridConfig) -> usize {
    if config.columns > 0 {
        return config
            .columns
            .min(width.saturating_sub(1).max(1) / 2)
            .max(1) as usize;
    }
    let min_width = config.min_card_width.clamp(28, 40);
    (width.saturating_add(2) / min_width.saturating_add(2)).clamp(1, 8) as usize
}

/// Shared geometry for complete selection rows and a clipped trailing row.
#[derive(Clone, Copy, Debug)]
pub struct GridGeometry {
    pub columns: usize,
    pub card_width: u16,
    pub column_stride: u16,
    pub first_column_offset: u16,
    pub thumbnail_width: u16,
    pub thumbnail_height: u16,
    pub title_rows: u16,
    pub card_height: u16,
    pub visible_rows: usize,
    pub complete_rows: usize,
    pub row_stride: u16,
}
impl GridGeometry {
    pub fn new(area: Rect, config: &crate::config::GridConfig, cell_w: f32, cell_h: f32) -> Self {
        let aspect_height_for = |width: u16| {
            (width.saturating_sub(2) as f32 * cell_w / cell_h.max(1.0) * 9.0 / 16.0).round() as u16
        };

        // Auto layout may add a column when that is what lets two complete rows
        // fit. Unlike the old width-shrinking path, every chosen column still
        // consumes the whole content pane so wide terminals never grow dead
        // bands at the left and right edges.
        let mut columns = column_count(area.width, config);
        if config.columns == 0 && area.height >= 20 {
            loop {
                let gutters = 2 * columns.saturating_sub(1) as u16;
                let width = area.width.saturating_sub(gutters) / columns as u16;
                let height = aspect_height_for(width).max(1).saturating_add(6);
                if area.height >= height.saturating_mul(2).saturating_add(1) || columns >= 8 {
                    break;
                }
                let next = columns + 1;
                let next_gutters = 2 * next.saturating_sub(1) as u16;
                let next_width = area.width.saturating_sub(next_gutters) / next as u16;
                if next_width < 24 {
                    break;
                }
                columns = next;
            }
        }

        let gutters = 2 * columns.saturating_sub(1) as u16;
        let usable = area.width.saturating_sub(gutters);
        let card_width = (usable / columns as u16).max(1);
        let column_stride = card_width.saturating_add(2);
        let first_column_offset = 0;
        let thumbnail_width = card_width.saturating_sub(2);
        let thumbnail_height = aspect_height_for(card_width).max(1);
        let title_rows = 2;
        let card_height = thumbnail_height.saturating_add(6);
        let row_stride = if area.height >= card_height.saturating_mul(2).saturating_add(1) {
            card_height.saturating_add(1)
        } else {
            card_height.max(1)
        };
        let complete_rows = if area.height >= card_height {
            1 + (area.height - card_height) / row_stride
        } else {
            1
        };
        Self {
            columns,
            card_width,
            column_stride,
            first_column_offset,
            thumbnail_width,
            thumbnail_height,
            title_rows,
            card_height,
            visible_rows: complete_rows as usize,
            complete_rows: complete_rows as usize,
            row_stride,
        }
    }
    pub fn visible_range(&self, scroll_row: usize, count: usize) -> std::ops::Range<usize> {
        let start = (scroll_row * self.columns).min(count);
        start..(start + self.visible_rows * self.columns).min(count)
    }
}

fn distributed_column_widths(width: u16, columns: usize) -> Vec<u16> {
    let gutters = 2 * columns.saturating_sub(1) as u16;
    let usable = width.saturating_sub(gutters);
    let base = usable / columns.max(1) as u16;
    let extra = usable % columns.max(1) as u16;
    let extra_start = columns.saturating_sub(extra as usize);
    (0..columns)
        .map(|index| base + u16::from(index >= extra_start))
        .collect()
}

pub fn render_grid(f: &mut Frame, app: &mut App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let total_items = app.filtered_items().len();
    if total_items == 0 {
        let msg = app.empty_feed_message();
        let p = Paragraph::new(msg)
            .alignment(Alignment::Center)
            .style(app.theme.style("text_dim"));
        f.render_widget(p, area);
        if app.is_loading && area.height >= 10 {
            let geometry = GridGeometry::new(area, &app.config.grid, app.cell_w_px, app.cell_h_px);
            let widths = distributed_column_widths(area.width, geometry.columns);
            let mut x = area.x;
            for width in widths {
                let rect = Rect::new(
                    x + 1,
                    area.y + 3,
                    width.saturating_sub(2),
                    geometry.thumbnail_height.min(area.height.saturating_sub(3)),
                );
                f.render_widget(
                    Block::default().style(Style::default().bg(app.theme.surface)),
                    rect,
                );
                x = x.saturating_add(width + 2);
            }
        }
        return;
    }
    if area.width < 72 {
        super::list::render_list(f, app, area);
        return;
    }

    let geometry = GridGeometry::new(area, &app.config.grid, app.cell_w_px, app.cell_h_px);
    if geometry.card_height > area.height {
        super::list::render_list(f, app, area);
        return;
    }
    let num_cols = geometry.columns;
    app.rendered_columns = num_cols;
    let col_widths = distributed_column_widths(area.width, num_cols);
    let card_h = geometry.card_height;
    let thumb_h = geometry.thumbnail_height;
    let row_stride = geometry.row_stride;
    let visible_rows = geometry.complete_rows;
    app.rendered_rows = geometry.visible_rows;
    let total_rows = total_items.div_ceil(num_cols);

    let selected = app.selected_index.min(total_items.saturating_sub(1));
    let selected_row = selected / num_cols;

    // Vim-style scrolloff: preserve context above/below the selected row when
    // the viewport is tall enough. `zz` can still center explicitly.
    if visible_rows > 1 {
        let margin = usize::from(app.config.ui.scrolloff).min(visible_rows.saturating_sub(1) / 2);
        if selected_row < app.scroll_row.saturating_add(margin) {
            app.scroll_row = selected_row.saturating_sub(margin);
        } else if selected_row.saturating_add(margin) >= app.scroll_row + visible_rows {
            app.scroll_row = selected_row
                .saturating_add(margin + 1)
                .saturating_sub(visible_rows);
        }
    } else {
        app.scroll_row = selected_row;
    }
    app.scroll_row = app.scroll_row.min(total_rows.saturating_sub(visible_rows));

    let start_row = app.scroll_row;
    let start_idx = start_row * num_cols;
    let visible_count = geometry.visible_rows * num_cols;

    let cards_to_render: Vec<Card> = app
        .filtered_items()
        .into_iter()
        .skip(start_idx)
        .take(visible_count)
        .cloned()
        .collect();

    let mut selected_card = None;
    // Render inactive cards first so shared frame rows cannot overwrite selection.
    for r in 0..geometry.visible_rows {
        let card_y = area.y + (r as u16) * row_stride;
        let remaining = area.bottom().saturating_sub(card_y);
        if remaining < (card_h / 2).max(1) {
            break;
        }

        let mut card_x = area.x;
        for (c, &card_w) in col_widths.iter().enumerate() {
            let i = r * num_cols + c;

            let card_rect = Rect {
                x: card_x,
                y: card_y,
                width: card_w,
                height: card_h.min(remaining),
            };

            card_x = card_x.saturating_add(card_w + 2);

            if i < cards_to_render.len() {
                if remaining < card_h {
                    render_partial_card(f, app, &cards_to_render[i], card_rect, thumb_h);
                    continue;
                }
                let item_idx = start_idx + i;
                let is_selected = item_idx == selected
                    && matches!(app.focus, Focus::Content | Focus::CommandLine);
                if is_selected {
                    selected_card = Some((i, card_rect));
                } else {
                    render_grid_card(
                        f,
                        app,
                        &cards_to_render[i],
                        card_rect,
                        false,
                        thumb_h,
                        geometry.title_rows,
                    );
                }
            }
        }
    }

    if let Some((index, rect)) = selected_card {
        render_grid_card(
            f,
            app,
            &cards_to_render[index],
            rect,
            true,
            thumb_h,
            geometry.title_rows,
        );
    }

    // Scrollbar on the right edge if there are more rows than visible
    if total_rows > visible_rows && area.width > 2 {
        let sb_x = area.x + area.width - 1;
        let sb_height = area.height.saturating_sub(2);
        if sb_height > 2 {
            let thumb_size = ((visible_rows as f32 / total_rows as f32) * (sb_height as f32))
                .round()
                .max(1.0) as u16;
            let thumb_pos = if start_row == 0 || total_rows <= visible_rows {
                0
            } else {
                ((start_row as f32 / (total_rows.saturating_sub(visible_rows).max(1) as f32))
                    * ((sb_height.saturating_sub(thumb_size)) as f32))
                    .round() as u16
            };

            for y_off in 0..sb_height {
                let y = area.y + 1 + y_off;
                let (ch, style) = if y_off >= thumb_pos && y_off < thumb_pos + thumb_size {
                    ("█", app.theme.style("accent"))
                } else {
                    ("│", app.theme.style("border"))
                };
                f.render_widget(
                    Paragraph::new(ch).style(style),
                    Rect {
                        x: sb_x,
                        y,
                        width: 1,
                        height: 1,
                    },
                );
            }
        }
    }
}

/// Clip cell-based image placements; other protocols show text and a skeleton
/// rather than emitting an image payload outside the viewport.
fn render_partial_card(f: &mut Frame, app: &mut App, card: &Card, area: Rect, thumb_h: u16) {
    let block = Block::default()
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
        .border_style(app.theme.style("border"));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.is_empty() {
        return;
    }
    let mut artwork = Rect::new(inner.x, inner.y, inner.width, inner.height.min(thumb_h));
    if matches!(card, Card::Channel(_)) {
        let width = ((thumb_h as f32 * app.cell_h_px / app.cell_w_px.max(1.)).round() as u16)
            .min(inner.width)
            .max(1);
        artwork.x += (inner.width - width) / 2;
        artwork.width = width;
    }
    let url = yute_core::media::thumbs::card_thumbnail_url(card).into_owned();
    let can_draw = app.thumbs.supports_partial()
        && app.config.general.graphics != "none"
        && app.overlay_area.is_none()
        && !app.confirming_quit
        && !app.confirming_logout;
    f.render_widget(
        Paragraph::new(vec![
            Line::from("▒".repeat(artwork.width as usize));
            artwork.height as usize
        ])
        .style(app.theme.style("border")),
        artwork,
    );
    if can_draw && !url.is_empty() {
        if let Some(cover) = app
            .thumbs
            .get_cover_protocol(&url, ratatui::layout::Size::new(artwork.width, thumb_h))
        {
            f.render_widget(cover, artwork);
        }
    }
    // Keep the same artwork/title order and dimensions as complete rows.
    let title_h = inner.height.saturating_sub(thumb_h).min(2);
    if title_h > 0 {
        let title = crate::ui::text::title_for(card.title(), &app.config);
        let lines = wrap_and_truncate_title(&title, inner.width as usize, 2);
        f.render_widget(
            Paragraph::new(lines.into_iter().map(Line::from).collect::<Vec<_>>())
                .style(app.theme.style("text_dim")),
            Rect::new(inner.x, inner.y + thumb_h, inner.width, title_h),
        );
    }
}

fn render_grid_card(
    f: &mut Frame,
    app: &mut App,
    card: &Card,
    area: Rect,
    is_selected: bool,
    thumb_h: u16,
    title_h: u16,
) {
    if area.width < 10 || area.height < 4 {
        return;
    }

    // Full frames establish card boundaries; selection gets the stronger accent.
    let card_block = if is_selected {
        Block::default()
            .borders(Borders::ALL)
            .border_type(ratatui::widgets::BorderType::Plain)
            .border_style(app.theme.style("accent").add_modifier(Modifier::BOLD))
            .style(Style::default().bg(app.theme.bg))
    } else {
        Block::default()
            .borders(Borders::ALL)
            .border_style(app.theme.style("border"))
            .style(Style::default().bg(app.theme.bg))
    };
    f.render_widget(card_block, area);

    // Artwork and text remain strictly inside the frame.
    let inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };

    if inner.width < 8 || inner.height < 4 {
        return;
    }

    let is_shorts = matches!(card, Card::Video(v) if v.badges.iter().any(|b| b == "SHORTS"));
    let is_channel = matches!(card, Card::Channel(_));
    let effective_title_h = if is_channel { 1 } else { title_h };
    let metadata_rows = if is_channel { 1 } else { 2 };
    let available_art_h = inner
        .height
        .saturating_sub(effective_title_h + metadata_rows)
        .max(1);
    let actual_thumb_h = if is_channel {
        // Make the avatar the largest physical square that fits the card. Terminal
        // cells are usually taller than they are wide, so square-in-cells would
        // visibly distort a channel logo.
        let square_h =
            ((inner.width as f32 * app.cell_w_px / app.cell_h_px.max(1.0)).round() as u16).max(1);
        available_art_h.min(square_h)
    } else {
        available_art_h.min(thumb_h)
    };
    let mut thumb_area = Rect::new(inner.x, inner.y, inner.width, actual_thumb_h);
    if is_channel {
        let width = ((actual_thumb_h as f32 * app.cell_h_px / app.cell_w_px.max(1.0)).round()
            as u16)
            .min(inner.width)
            .max(1);
        thumb_area.x += (inner.width - width) / 2;
        thumb_area.width = width;
    }
    let text_x = inner.x;
    let text_y = inner.y + actual_thumb_h;
    let text_w = inner.width;
    let title_area = Rect::new(text_x, text_y, text_w, effective_title_h);
    let channel_area = Rect::new(
        text_x,
        text_y + effective_title_h,
        text_w,
        u16::from(!is_channel),
    );
    let meta_area = Rect::new(
        text_x,
        text_y + effective_title_h + u16::from(!is_channel),
        text_w,
        1,
    );

    // Modal overlap suppression
    let screen_size = f.area();
    let dialog_w = 44.min(screen_size.width.saturating_sub(4));
    let dialog_h = 7.min(screen_size.height.saturating_sub(2));
    let modal_x = (screen_size.width.saturating_sub(dialog_w)) / 2;
    let modal_y = (screen_size.height.saturating_sub(dialog_h)) / 2;
    let overlaps_modal = app
        .overlay_area
        .is_some_and(|area| area.intersects(thumb_area))
        || app.screen == crate::app::Screen::Help
        || (app.confirming_quit || app.confirming_logout)
            && thumb_area.x < modal_x + dialog_w
            && thumb_area.x + thumb_area.width > modal_x
            && thumb_area.y < modal_y + dialog_h
            && thumb_area.y + thumb_area.height > modal_y;

    let thumb_url = yute_core::media::thumbs::card_thumbnail_url(card).into_owned();
    if overlaps_modal {
        let p = Paragraph::new("").style(Style::default().bg(app.theme.surface));
        f.render_widget(p, thumb_area);
    } else if matches!(card, Card::Channel(_)) && thumb_url.is_empty() {
        render_monogram(f, app, card, thumb_area);
    } else if !thumb_url.is_empty() && app.config.general.graphics != "none" {
        if let Some(proto) = app
            .thumbs
            .get_cover_protocol(&thumb_url, thumb_area.as_size())
        {
            f.render_widget(proto, thumb_area);
        } else if matches!(card, Card::Channel(_)) {
            render_monogram(f, app, card, thumb_area);
        } else {
            // Skeleton of exact thumbnail rect (dim ▒ block)
            let skeleton_line = "▒".repeat(thumb_area.width as usize);
            let mut skel_lines = Vec::new();
            for _ in 0..thumb_area.height {
                skel_lines.push(Line::from(Span::styled(
                    skeleton_line.clone(),
                    app.theme.style("border"),
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
                app.theme.style("border"),
            )));
        }
        f.render_widget(Paragraph::new(skel_lines), thumb_area);
    }

    if let Card::Video(video) = card {
        if let Some(percent) = video.progress_pct.filter(|p| *p > 0) {
            let filled = usize::from(thumb_area.width) * usize::from(percent.min(100)) / 100;
            let line = Line::from(vec![
                Span::styled("█".repeat(filled), app.theme.style("progress")),
                Span::styled(
                    "░".repeat(thumb_area.width as usize - filled),
                    app.theme.style("skeleton"),
                ),
            ]);
            super::thumbnail::badge(
                f,
                Paragraph::new(line),
                Rect::new(thumb_area.x, thumb_area.bottom() - 1, thumb_area.width, 1),
            );
        }
    }
    // Render corner badges
    let now_playing = app.player.is_alive()
        && app.player.status.file_loaded
        && app.player.current_video_id() == Some(card.id());
    if now_playing {
        let badge_text = " ▶ Now playing ";
        let badge_w = (badge_text.len() as u16).min(thumb_area.width);
        let badge_rect = Rect {
            x: thumb_area.x + thumb_area.width.saturating_sub(badge_w),
            y: thumb_area.y,
            width: badge_w,
            height: 1,
        };
        let p = Paragraph::new(badge_text).style(
            Style::default()
                .bg(app.theme.accent)
                .fg(app.theme.badge_fg)
                .add_modifier(Modifier::BOLD),
        );
        super::thumbnail::badge(f, p, badge_rect);
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
                    let p = Paragraph::new(badge_text).style(
                        Style::default()
                            .bg(app.theme.live_badge_bg)
                            .fg(app.theme.badge_fg)
                            .add_modifier(Modifier::BOLD),
                    );
                    super::thumbnail::badge(f, p, badge_rect);
                } else if v.live == LiveState::Upcoming {
                    let rect = Rect::new(thumb_area.x, thumb_area.y, 10.min(thumb_area.width), 1);
                    super::thumbnail::badge(
                        f,
                        Paragraph::new(" UPCOMING ").style(
                            Style::default()
                                .fg(app.theme.badge_fg)
                                .bg(app.theme.badge_bg),
                        ),
                        rect,
                    );
                } else if is_shorts {
                    let badge_text = " SHORTS ";
                    let badge_w = 8u16.min(thumb_area.width);
                    let badge_rect = Rect {
                        x: thumb_area.x,
                        y: thumb_area.y,
                        width: badge_w,
                        height: 1,
                    };
                    let p = Paragraph::new(badge_text).style(
                        Style::default()
                            .bg(app.theme.live_badge_bg)
                            .fg(app.theme.badge_fg)
                            .add_modifier(Modifier::BOLD),
                    );
                    super::thumbnail::badge(f, p, badge_rect);
                } else if let Some(ref dur) = v.duration {
                    let badge_text = format!(" {dur} ");
                    let badge_w = (badge_text.len() as u16).min(thumb_area.width);
                    let badge_rect = Rect {
                        x: thumb_area.x + thumb_area.width.saturating_sub(badge_w),
                        y: thumb_area.y + thumb_area.height.saturating_sub(1),
                        width: badge_w,
                        height: 1,
                    };
                    let p = Paragraph::new(badge_text).style(
                        Style::default()
                            .bg(app.theme.badge_bg)
                            .fg(app.theme.badge_fg)
                            .add_modifier(Modifier::BOLD),
                    );
                    super::thumbnail::badge(f, p, badge_rect);
                }
            }
            Card::Playlist(p) => {
                let badge_text = if p.is_mix {
                    " MIX ".into()
                } else {
                    format!(
                        " {} ",
                        p.video_count
                            .as_deref()
                            .unwrap_or("PLAYLIST")
                            .to_uppercase()
                    )
                };
                let badge_w = (badge_text.len() as u16).min(thumb_area.width);
                let badge_rect = Rect {
                    x: thumb_area.x + thumb_area.width.saturating_sub(badge_w),
                    y: thumb_area.y + thumb_area.height.saturating_sub(1),
                    width: badge_w,
                    height: 1,
                };
                let p_w = Paragraph::new(badge_text).style(
                    Style::default()
                        .bg(app.theme.badge_bg)
                        .fg(app.theme.accent)
                        .add_modifier(Modifier::BOLD),
                );
                super::thumbnail::badge(f, p_w, badge_rect);
            }
            Card::Channel(_) => {}
        }
    }

    if let Card::Video(video) = card {
        if is_shorts && video.live == LiveState::None {
            if let Some(duration) = &video.duration {
                let text = format!(" {duration} ");
                let width = (text.len() as u16).min(thumb_area.width);
                super::thumbnail::badge(
                    f,
                    Paragraph::new(text).style(
                        Style::default()
                            .fg(app.theme.badge_fg)
                            .bg(app.theme.badge_bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Rect::new(
                        thumb_area.right().saturating_sub(width),
                        thumb_area.bottom().saturating_sub(1),
                        width,
                        1,
                    ),
                );
            }
        }
    }

    let sec_style = app.theme.style("text_dim");

    // Render Title (max 2 lines, word-boundary wrapped with ellipsis)
    let raw_title = card.title();
    let clean = crate::ui::text::title_for(raw_title, &app.config);
    let title_lines = wrap_and_truncate_title(&clean, title_area.width as usize, title_h as usize);

    let mut line_spans = Vec::new();
    for tl in title_lines {
        line_spans.push(Line::from(Span::styled(
            tl,
            app.theme.style("text").add_modifier(Modifier::BOLD),
        )));
    }
    f.render_widget(
        Paragraph::new(line_spans).style(app.theme.style("text")),
        title_area,
    );

    // Render Meta Line
    match card {
        Card::Video(v) => {
            let channel = if v.verified {
                format!("{} [v]", v.channel)
            } else {
                v.channel.clone()
            };
            f.render_widget(
                Paragraph::new(truncate_str(&channel, channel_area.width as usize))
                    .style(app.theme.style("text")),
                channel_area,
            );
            let meta = format_meta_line(
                "",
                false,
                v.views.as_deref(),
                v.published.as_deref(),
                meta_area.width as usize,
                sec_style,
            );
            f.render_widget(Paragraph::new(Line::from(meta)), meta_area);
        }
        Card::Playlist(p) => {
            let owner = if p.channel.is_empty() {
                p.updated.as_deref().unwrap_or("")
            } else {
                &p.channel
            };
            f.render_widget(
                Paragraph::new(truncate_str(owner, channel_area.width as usize))
                    .style(app.theme.style("text_dim")),
                channel_area,
            );
            if !p.channel.is_empty() {
                f.render_widget(
                    Paragraph::new(truncate_str(
                        p.updated.as_deref().unwrap_or(""),
                        meta_area.width as usize,
                    ))
                    .style(app.theme.style("text_dim")),
                    meta_area,
                );
            }
        }
        Card::Channel(c) => {
            let meta = match (c.handle.as_deref(), c.subscribers.as_deref()) {
                (Some(handle), Some(subs)) if !handle.is_empty() && !subs.is_empty() => {
                    format!("{handle} · {subs}")
                }
                (Some(handle), _) => handle.to_string(),
                (_, Some(subs)) => subs.to_string(),
                _ => String::new(),
            };
            f.render_widget(
                Paragraph::new(truncate_str(&meta, meta_area.width as usize)).style(sec_style),
                meta_area,
            );
        }
    }
}

fn render_monogram(f: &mut Frame, app: &App, card: &Card, area: Rect) {
    let initial = card
        .title()
        .chars()
        .find(|c| !c.is_whitespace())
        .unwrap_or('?')
        .to_uppercase()
        .to_string();
    f.render_widget(
        Paragraph::new(initial)
            .alignment(Alignment::Center)
            .style(app.theme.style("accent").add_modifier(Modifier::BOLD)),
        Rect::new(area.x, area.y + area.height / 2, area.width, 1),
    );
}
