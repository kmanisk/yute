use crate::app::{App, WatchPane};
use crate::backend::models::Card;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use unicode_width::UnicodeWidthStr;

fn section(f: &mut Frame, app: &App, area: Rect, title: &str, active: bool) -> Rect {
    if area.is_empty() {
        return area;
    }
    let block = Block::default()
        .borders(Borders::TOP)
        .title(format!(" {}{title} ", if active { "▸ " } else { "" }))
        .border_style(Style::default().fg(if active {
            app.theme.accent
        } else {
            app.theme.border
        }))
        .title_style(
            Style::default()
                .fg(if active {
                    app.theme.accent
                } else {
                    app.theme.text_secondary
                })
                .add_modifier(Modifier::BOLD),
        );
    let inner = block.inner(area);
    f.render_widget(block, area);
    inner
}

pub fn render_watch(f: &mut Frame, app: &mut App, area: Rect) {
    if area.width < 10 || area.height < 5 {
        return;
    }
    let area = Rect::new(
        area.x + 1,
        area.y,
        area.width.saturating_sub(2),
        area.height,
    );
    let right_w = if area.width >= 85 {
        (area.width / 3).clamp(30, 42)
    } else {
        0
    };
    if right_w == 0 && app.watch_pane == WatchPane::Suggestions {
        render_suggestions(f, app, area);
        return;
    }
    let left = Rect::new(
        area.x,
        area.y,
        area.width
            .saturating_sub(right_w + u16::from(right_w > 0) * 2),
        area.height,
    );
    let heading_h = left.height.min(6);
    render_heading(f, app, Rect::new(left.x, left.y, left.width, heading_h));
    let rest = Rect::new(
        left.x,
        left.y + heading_h,
        left.width,
        left.height.saturating_sub(heading_h),
    );
    let desc_h = if app.watch_pane == WatchPane::Description {
        (rest.height / 2).clamp(4, 9).min(rest.height)
    } else {
        (rest.height / 3).min(6)
    };
    render_description(f, app, Rect::new(rest.x, rest.y, rest.width, desc_h));
    render_comments(
        f,
        app,
        Rect::new(
            rest.x,
            rest.y + desc_h,
            rest.width,
            rest.height.saturating_sub(desc_h),
        ),
    );
    if right_w > 0 {
        render_suggestions(
            f,
            app,
            Rect::new(area.right() - right_w, area.y, right_w, area.height),
        );
    }
}

fn render_heading(f: &mut Frame, app: &App, area: Rect) {
    let video = app
        .player
        .current_video
        .as_ref()
        .or_else(|| app.watch_data.as_ref().map(|w| &w.video));
    let title = video.map_or("Video", |v| v.title.as_str());
    f.render_widget(
        Paragraph::new(title).wrap(Wrap { trim: true }).style(
            Style::default()
                .fg(app.theme.text)
                .add_modifier(Modifier::BOLD),
        ),
        Rect::new(area.x, area.y, area.width, 2.min(area.height)),
    );
    let meta = video
        .map(|v| {
            [
                Some(v.channel.clone()),
                v.views.as_deref().map(super::text::compact_views),
                v.published.clone(),
            ]
            .into_iter()
            .flatten()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
        })
        .unwrap_or_default();
    if area.height > 2 {
        f.render_widget(
            Paragraph::new(meta).style(Style::default().fg(app.theme.text_secondary)),
            Rect::new(area.x, area.y + 2, area.width, 1),
        );
    }
    if area.height > 3 {
        let status = &app.player.status;
        let time = format!(
            " {} / {}",
            format_time(status.time_pos),
            format_time(status.duration)
        );
        let width = area.width.saturating_sub(time.width() as u16).max(1);
        let fraction = if status.duration > 0.0 {
            (status.time_pos / status.duration).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let filled = (fraction * f64::from(width.saturating_sub(1))).round() as usize;
        let line = Line::from(vec![
            Span::styled("━".repeat(filled), Style::default().fg(app.theme.accent)),
            Span::styled("●", Style::default().fg(app.theme.accent)),
            Span::styled(
                "─".repeat(width.saturating_sub(filled as u16 + 1) as usize),
                Style::default().fg(app.theme.border),
            ),
            Span::styled(time, Style::default().fg(app.theme.text_secondary)),
        ]);
        f.render_widget(
            Paragraph::new(line),
            Rect::new(area.x, area.y + 3, area.width, 1),
        );
    }
    if area.height > 4 {
        let watch = app.watch_data.as_ref();
        let like = if watch.and_then(|w| w.is_liked) == Some(true) {
            "Liked"
        } else {
            "Like"
        };
        let dislike = if watch.and_then(|w| w.is_disliked) == Some(true) {
            "Disliked"
        } else {
            "Dislike"
        };
        f.render_widget(
            Paragraph::new(format!("+ {like}   − {dislike}"))
                .style(Style::default().fg(app.theme.text_secondary)),
            Rect::new(area.x, area.y + 4, area.width, 1),
        );
    }
}

pub fn description_links(app: &App) -> Vec<(String, String)> {
    let Some(watch) = &app.watch_data else {
        return Vec::new();
    };
    let mut links = watch.description_links.clone();
    for word in watch.description.split_whitespace() {
        let url = word.trim_matches(|c: char| {
            matches!(c, '<' | '>' | '(' | ')' | '[' | ']' | '"' | '\'' | ',')
        });
        if (url.starts_with("https://") || url.starts_with("http://"))
            && !links.iter().any(|(_, u)| u == url)
        {
            links.push((url.to_string(), url.to_string()));
        }
    }
    links
}
fn render_description(f: &mut Frame, app: &App, area: Rect) {
    let inner = section(
        f,
        app,
        area,
        "Description",
        app.watch_pane == WatchPane::Description,
    );
    let Some(watch) = &app.watch_data else {
        f.render_widget(
            Paragraph::new("Loading video details…")
                .style(Style::default().fg(app.theme.text_secondary)),
            inner,
        );
        return;
    };
    let mut lines = Vec::new();
    for text in watch.description.lines() {
        if text.starts_with("# ") || text.starts_with("## ") {
            lines.push(Line::from(Span::styled(
                text.trim_start_matches('#').trim(),
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            )));
        } else {
            let spans = text
                .split_inclusive(char::is_whitespace)
                .map(|word| {
                    Span::styled(
                        word,
                        if word.starts_with("https://") || word.starts_with("http://") {
                            Style::default()
                                .fg(app.theme.accent)
                                .add_modifier(Modifier::UNDERLINED)
                        } else {
                            Style::default().fg(app.theme.text)
                        },
                    )
                })
                .collect::<Vec<_>>();
            lines.push(Line::from(spans));
        }
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No description provided.",
            Style::default().fg(app.theme.text_secondary),
        )));
    }
    for (time, title) in &watch.chapters {
        lines.push(Line::from(Span::styled(
            format!("{}  {title}", format_time(f64::from(*time))),
            Style::default().fg(app.theme.accent),
        )));
    }
    for (i, (label, url)) in description_links(app).iter().enumerate() {
        lines.push(Line::from(Span::styled(
            format!(
                "{} {}",
                if i == app.watch_link_index { ">" } else { " " },
                if label == url {
                    url.clone()
                } else {
                    format!("{label}  {url}")
                }
            ),
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::UNDERLINED),
        )));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((app.description_scroll, 0)),
        inner,
    );
}

fn render_comments(f: &mut Frame, app: &App, area: Rect) {
    let count = app.watch_data.as_ref().map_or(0, |w| w.comments.len());
    let inner = section(
        f,
        app,
        area,
        &format!("Comments · {count}"),
        app.watch_pane == WatchPane::Comments,
    );
    let Some(watch) = &app.watch_data else {
        return;
    };
    if watch.comments.is_empty() {
        let text = if app.comments_loading {
            "Loading comments…"
        } else if watch.comments_token.is_some() {
            "Press c to load comments"
        } else {
            "Comments are unavailable for this video or session"
        };
        f.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: true })
                .style(Style::default().fg(app.theme.text_secondary)),
            inner,
        );
        return;
    }
    let mut y = inner.y;
    for (i, comment) in watch.comments.iter().enumerate().skip(app.comments_cursor) {
        if inner.bottom().saturating_sub(y) < 3 {
            break;
        }
        let active = i == app.comments_cursor && app.watch_pane == WatchPane::Comments;
        f.render_widget(
            Paragraph::new(format!(
                "{}{}",
                if active { "> " } else { "" },
                comment.author
            ))
            .style(
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Rect::new(inner.x, y, inner.width, 1),
        );
        y += 1;
        // Keep Unicode/emoji intact; Ratatui wraps in terminal display columns.
        let body_rows = comment
            .text
            .lines()
            .map(|line| line.width().max(1).div_ceil(inner.width.max(1) as usize))
            .sum::<usize>()
            .clamp(1, 4) as u16;
        let body_rows = body_rows.min(inner.bottom().saturating_sub(y + 1));
        f.render_widget(
            Paragraph::new(comment.text.as_str())
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(app.theme.text)),
            Rect::new(inner.x, y, inner.width, body_rows),
        );
        y += body_rows;
        let meta = [
            comment.published.clone(),
            comment.likes.as_ref().map(|v| format!("{v} likes")),
            comment.reply_count.map(|n| format!("{n} replies")),
        ]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
        f.render_widget(
            Paragraph::new(meta).style(Style::default().fg(app.theme.text_secondary)),
            Rect::new(inner.x, y, inner.width, 1),
        );
        y = y.saturating_add(2);
    }
}

fn render_suggestions(f: &mut Frame, app: &mut App, area: Rect) {
    let inner = section(
        f,
        app,
        area,
        "Suggestions",
        app.watch_pane == WatchPane::Suggestions,
    );
    let thumb_w = 14.min(inner.width / 2);
    let thumb_h = (f32::from(thumb_w) * app.cell_w_px * 9.0 / (16.0 * app.cell_h_px.max(1.0)))
        .round()
        .max(1.0) as u16;
    let stride = thumb_h.max(4) + 1;
    let visible = (inner.height / stride).max(1) as usize;
    let first = app.selected_index.saturating_sub(visible.saturating_sub(1));
    let items = app
        .watch_suggestions()
        .into_iter()
        .enumerate()
        .skip(first)
        .take(visible)
        .map(|(i, c)| (i, c.clone()))
        .collect::<Vec<_>>();
    if items.is_empty() {
        f.render_widget(
            Paragraph::new("No suggestions available")
                .style(Style::default().fg(app.theme.text_secondary)),
            inner,
        );
        return;
    }
    for (offset, (index, item)) in items.iter().enumerate() {
        let y = inner.y + offset as u16 * stride;
        if y + stride > inner.bottom() {
            break;
        }
        let selected = *index == app.selected_index && app.watch_pane == WatchPane::Suggestions;
        let rect = Rect::new(inner.x, y, inner.width, stride - 1);
        if selected {
            f.render_widget(
                Block::default().style(Style::default().bg(app.theme.surface_hover)),
                rect,
            );
        }
        let image_area = Rect::new(rect.x, rect.y, thumb_w, thumb_h);
        if !item.thumb_url().is_empty()
            && app.config.general.graphics != "none"
            && !app.overlay_area.is_some_and(|a| a.intersects(image_area))
        {
            if let Some(protocol) = app
                .thumbs
                .get_cover_protocol(item.thumb_url(), image_area.as_size())
            {
                f.render_widget(ratatui_image::Image::new(protocol), image_area);
            }
        }
        let x = image_area.right() + 1;
        let width = rect.right().saturating_sub(x);
        let title =
            super::text::wrap_and_truncate_title(item.title(), width as usize, 2).join("\n");
        f.render_widget(
            Paragraph::new(title).style(
                Style::default()
                    .fg(if selected {
                        app.theme.accent
                    } else {
                        app.theme.text
                    })
                    .add_modifier(Modifier::BOLD),
            ),
            Rect::new(x, y, width, 2),
        );
        if let Card::Video(video) = item {
            f.render_widget(
                Paragraph::new(super::text::truncate_str(&video.channel, width as usize))
                    .style(Style::default().fg(app.theme.text_secondary)),
                Rect::new(x, y + 2, width, 1),
            );
            let meta = super::text::format_meta_line(
                "",
                false,
                video.views.as_deref(),
                video.published.as_deref(),
                width as usize,
                Style::default().fg(app.theme.text_secondary),
            );
            f.render_widget(
                Paragraph::new(Line::from(meta)),
                Rect::new(x, y + 3, width, 1),
            );
            if let Some(duration) = &video.duration {
                let w = (duration.width() as u16 + 1).min(image_area.width);
                super::thumbnail::badge(
                    f,
                    Paragraph::new(format!(" {duration}"))
                        .style(Style::default().fg(app.theme.text).bg(app.theme.badge_bg)),
                    Rect::new(image_area.right() - w, image_area.bottom() - 1, w, 1),
                );
            }
        }
    }
}

pub fn format_time(time: f64) -> String {
    let seconds = if time.is_finite() {
        time.max(0.0) as u64
    } else {
        0
    };
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            (seconds / 60) % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}
