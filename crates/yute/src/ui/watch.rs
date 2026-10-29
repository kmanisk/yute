use crate::app::{App, WatchPane};
use crate::backend::models::{Card, CommentMediaKind};
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
        .title(format!(" {title} "))
        .border_style(if active {
            app.theme.style("border_focused")
        } else {
            app.theme.style("border")
        })
        .title_style(if active {
            app.theme.style("accent").add_modifier(Modifier::BOLD)
        } else {
            app.theme.style("text_dim").add_modifier(Modifier::BOLD)
        });
    let inner = block.inner(area);
    f.render_widget(block, area);
    inner
}

pub fn render_watch(f: &mut Frame, app: &mut App, area: Rect) {
    if area.width < 10 || area.height < 5 {
        return;
    }
    let right_w = if area.width >= 86 {
        (area.width / 3).clamp(30, 44)
    } else {
        0
    };
    if right_w == 0 && app.watch_pane == WatchPane::Suggestions {
        render_suggestions(f, app, area);
        return;
    }
    let gap = u16::from(right_w > 0);
    let left = Rect::new(
        area.x,
        area.y,
        area.width.saturating_sub(right_w + gap),
        area.height,
    );
    let heading_h = left.height.min(6);
    render_heading(f, app, Rect::new(left.x, left.y, left.width, heading_h));
    let body = Rect::new(
        left.x,
        left.y + heading_h,
        left.width,
        left.height.saturating_sub(heading_h),
    );
    match app.watch_pane {
        WatchPane::Comments => render_comments(f, app, body),
        WatchPane::Description | WatchPane::Suggestions => render_description(f, app, body),
    }
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
        Paragraph::new(title)
            .wrap(Wrap { trim: true })
            .style(app.theme.style("text").add_modifier(Modifier::BOLD)),
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
            Paragraph::new(meta).style(app.theme.style("text_dim")),
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
            Span::styled("━".repeat(filled), app.theme.style("accent")),
            Span::styled("●", app.theme.style("accent")),
            Span::styled(
                "─".repeat(width.saturating_sub(filled as u16 + 1) as usize),
                app.theme.style("border"),
            ),
            Span::styled(time, app.theme.style("text_dim")),
        ]);
        f.render_widget(
            Paragraph::new(line),
            Rect::new(area.x, area.y + 3, area.width, 1),
        );
    }
    if area.height > 4 {
        let watch = app.watch_data.as_ref();
        let liked = watch.and_then(|w| w.is_liked) == Some(true);
        let disliked = watch.and_then(|w| w.is_disliked) == Some(true);
        let likes = watch
            .and_then(|w| w.likes.as_deref())
            .filter(|value| !value.is_empty())
            .unwrap_or("");
        let mut spans = Vec::new();
        spans.push(Span::styled(
            super::icons::Icon::Like.label(
                app.config.icons(),
                if likes.is_empty() {
                    if liked {
                        "Liked"
                    } else {
                        "Like"
                    }
                } else {
                    likes
                },
            ),
            if liked {
                app.theme.style("accent").add_modifier(Modifier::BOLD)
            } else {
                app.theme.style("text_dim")
            },
        ));
        spans.push(Span::raw("   "));
        spans.push(Span::styled(
            super::icons::Icon::Dislike.label(
                app.config.icons(),
                if disliked { "Disliked" } else { "Dislike" },
            ),
            if disliked {
                app.theme.style("accent").add_modifier(Modifier::BOLD)
            } else {
                app.theme.style("text_dim")
            },
        ));
        spans.push(Span::styled(
            "    d description · c comments · w suggestions",
            app.theme.style("text_dim").add_modifier(Modifier::DIM),
        ));
        f.render_widget(
            Paragraph::new(Line::from(spans)),
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
            Paragraph::new("Loading video details…").style(app.theme.style("text_dim")),
            inner,
        );
        return;
    };
    let mut lines = Vec::new();
    for text in watch.description.lines() {
        let spans = text
            .split_inclusive(char::is_whitespace)
            .map(|word| {
                Span::styled(
                    word,
                    if word.starts_with("https://") || word.starts_with("http://") {
                        app.theme.style("accent").add_modifier(Modifier::UNDERLINED)
                    } else {
                        app.theme.style("text")
                    },
                )
            })
            .collect::<Vec<_>>();
        lines.push(Line::from(spans));
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "No description provided.",
            app.theme.style("text_dim"),
        )));
    }
    if !watch.chapters.is_empty() {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            "Chapters",
            app.theme.style("text").add_modifier(Modifier::BOLD),
        )));
        for (time, title) in &watch.chapters {
            lines.push(Line::from(Span::styled(
                format!("{}  {title}", format_time(f64::from(*time))),
                app.theme.style("accent"),
            )));
        }
    }
    for (i, (label, url)) in description_links(app).iter().enumerate() {
        lines.push(Line::from(Span::styled(
            format!(
                "{} {}",
                if i == app.watch_link_index {
                    "▎"
                } else {
                    " "
                },
                if label == url {
                    url.clone()
                } else {
                    format!("{label}  {url}")
                }
            ),
            app.theme.style("accent").add_modifier(Modifier::UNDERLINED),
        )));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((app.description_scroll, 0)),
        inner,
    );
}

fn render_comments(f: &mut Frame, app: &mut App, area: Rect) {
    let comments = app.visible_comments().to_vec();
    let thread_label = app
        .comment_thread
        .as_ref()
        .map(|thread| format!("Replies · {}", thread.parent_author));
    let title = thread_label.unwrap_or_else(|| format!("Comments · {}", comments.len()));
    let inner = section(f, app, area, &title, app.watch_pane == WatchPane::Comments);
    if comments.is_empty() {
        let text = if app.comments_loading {
            "Loading comments…"
        } else if app
            .watch_data
            .as_ref()
            .and_then(|watch| watch.comments_token.as_ref())
            .is_some()
        {
            "Press c to load comments"
        } else {
            "Comments are unavailable for this video or session"
        };
        f.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: true })
                .style(app.theme.style("text_dim")),
            inner,
        );
        return;
    }

    let first = app.comments_cursor.saturating_sub(1);
    let mut y = inner.y;
    for (i, comment) in comments.iter().enumerate().skip(first) {
        if inner.bottom().saturating_sub(y) < 4 {
            break;
        }
        let active = i == app.comments_cursor;
        if active {
            f.render_widget(
                Paragraph::new("▎").style(app.theme.style("accent")),
                Rect::new(inner.x, y, 1, inner.bottom().saturating_sub(y).min(5)),
            );
        }
        let row_x = inner.x + 2;
        let row_w = inner.width.saturating_sub(2);
        let avatar_w = 5.min(row_w.saturating_sub(8));
        let avatar_h = 2u16;
        let text_x = row_x + avatar_w + 1;
        let text_w = row_w.saturating_sub(avatar_w + 1);
        if avatar_w > 0 {
            let avatar_area = Rect::new(row_x, y, avatar_w, avatar_h);
            if let Some(url) = &comment.author_thumb {
                if super::thumbnail::visible(app, avatar_area) {
                    if let Some(proto) = app.thumbs.get_cover_protocol(url, avatar_area.as_size()) {
                        f.render_widget(proto, avatar_area);
                    }
                }
            }
        }
        let author = if comment.author.starts_with('@') {
            comment.author.clone()
        } else {
            format!("@{}", comment.author)
        };
        let published = comment.published.as_deref().unwrap_or("");
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(author, app.theme.style("text").add_modifier(Modifier::BOLD)),
                Span::styled(
                    if published.is_empty() {
                        String::new()
                    } else {
                        format!("  {published}")
                    },
                    app.theme.style("text_dim"),
                ),
            ])),
            Rect::new(text_x, y, text_w, 1),
        );
        let body_width = text_w.max(1) as usize;
        let body_rows = comment
            .text
            .lines()
            .map(|line| line.width().max(1).div_ceil(body_width))
            .sum::<usize>()
            .clamp(1, 4) as u16;
        let body_rows = body_rows.min(inner.bottom().saturating_sub(y + 2));
        f.render_widget(
            Paragraph::new(comment.text.as_str())
                .wrap(Wrap { trim: false })
                .style(app.theme.style("text")),
            Rect::new(text_x, y + 1, text_w, body_rows),
        );
        let mut next_y = y + 1 + body_rows;

        if let Some(media) = comment.media.first() {
            let media_h = inner.bottom().saturating_sub(next_y + 1).min(6);
            if media_h >= 2
                && text_w >= 12
                && super::thumbnail::visible(
                    app,
                    Rect::new(text_x, next_y, text_w.min(24), media_h),
                )
            {
                let media_w = text_w.min(24);
                let media_area = Rect::new(text_x, next_y, media_w, media_h);
                // Animated GIFs use the same bounded image pipeline; when animation
                // support is available the manager advances only while this media is visible.
                let mut rendered = false;
                match media.kind {
                    CommentMediaKind::Gif => {
                        if let Some(protocol) = app
                            .thumbs
                            .get_animated_cover_protocol(&media.url, media_area.as_size())
                        {
                            f.render_widget(protocol, media_area);
                            rendered = true;
                        }
                    }
                    CommentMediaKind::Image => {
                        if let Some(protocol) = app
                            .thumbs
                            .get_cover_protocol(&media.url, media_area.as_size())
                        {
                            f.render_widget(protocol, media_area);
                            rendered = true;
                        }
                    }
                }
                if rendered {
                    next_y += media_h;
                }
            }
        }

        if next_y < inner.bottom() {
            let like_style = if comment.is_liked {
                app.theme.style("accent").add_modifier(Modifier::BOLD)
            } else {
                app.theme.style("text_dim")
            };
            let dislike_style = if comment.is_disliked {
                app.theme.style("accent").add_modifier(Modifier::BOLD)
            } else {
                app.theme.style("text_dim")
            };
            let likes = comment.likes.as_deref().unwrap_or("");
            let replies = comment.reply_count.unwrap_or(0);
            let mut spans = vec![
                Span::styled(
                    super::icons::Icon::Like.label(
                        app.config.icons(),
                        if likes.is_empty() { "Like" } else { likes },
                    ),
                    like_style,
                ),
                Span::raw("  "),
                Span::styled(
                    super::icons::Icon::Dislike.label(app.config.icons(), "Dislike"),
                    dislike_style,
                ),
            ];
            if replies > 0 {
                spans.push(Span::styled(
                    format!("   {replies} replies · Enter thread"),
                    app.theme.style("text_dim"),
                ));
            }
            if active {
                let mut hint = "   + like · - dislike · r reply · n new".to_string();
                if app.comment_is_own(comment) {
                    hint.push_str(" · x delete");
                }
                spans.push(Span::styled(
                    hint,
                    app.theme.style("text_dim").add_modifier(Modifier::DIM),
                ));
            }
            f.render_widget(
                Paragraph::new(Line::from(spans)),
                Rect::new(text_x, next_y, text_w, 1),
            );
            next_y += 1;
        }
        y = next_y.saturating_add(1);
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
            Paragraph::new("No suggestions available").style(app.theme.style("text_dim")),
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
                Block::default()
                    .borders(Borders::LEFT)
                    .border_style(app.theme.style("border_focused")),
                rect,
            );
        }
        let image_area = Rect::new(rect.x + u16::from(selected), rect.y, thumb_w, thumb_h);
        let thumbnail_url = yute_core::media::thumbs::card_thumbnail_url(item);
        if !thumbnail_url.is_empty()
            && crate::terminal::images_enabled(&app.config.general.graphics)
            && !app.overlay_area.is_some_and(|a| a.intersects(image_area))
        {
            if let Some(protocol) = app
                .thumbs
                .get_cover_protocol(&thumbnail_url, image_area.as_size())
            {
                f.render_widget(protocol, image_area);
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
                    .style(app.theme.style("text_dim")),
                Rect::new(x, y + 2, width, 1),
            );
            let meta = super::text::format_meta_line(
                "",
                false,
                video.views.as_deref(),
                video.published.as_deref(),
                width as usize,
                app.theme.style("text_dim"),
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
                        .style(app.theme.style("text").bg(app.theme.badge_bg)),
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
