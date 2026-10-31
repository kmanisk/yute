use crate::app::{App, WatchPane};
use crate::backend::models::{Card, CommentMediaKind};
use crate::ui::text::DisplayWidth;
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

fn section(f: &mut Frame, app: &App, area: Rect, title: &str, active: bool) -> Rect {
    super::pane::Pane::new(title, active).render(f, area, &app.theme)
}
#[derive(Clone, Copy, Debug)]
pub struct WatchLayout {
    pub header: Rect,
    pub video: Rect,
    pub playback: Rect,
    pub description: Rect,
    pub comments: Rect,
    pub compose: Rect,
    pub right: Rect,
}
impl WatchLayout {
    pub fn new(area: Rect, wide: bool, expanded: bool) -> Self {
        let header = Rect::new(area.x, area.y, area.width, 1.min(area.height));
        let body = Rect::new(
            area.x,
            header.bottom(),
            area.width,
            area.height.saturating_sub(1),
        );
        let rw = if wide {
            body.width.saturating_sub(1) * 38 / 100
        } else {
            0
        };
        let left = Rect::new(
            body.x,
            body.y,
            body.width.saturating_sub(rw + u16::from(wide)),
            body.height,
        );
        let right = Rect::new(body.right().saturating_sub(rw), body.y, rw, body.height);
        let compose_h = if left.height >= 12 { 3 } else { 0 };
        let compose = Rect::new(
            left.x,
            left.bottom().saturating_sub(compose_h),
            left.width,
            compose_h,
        );
        let video_h = 7.min(left.height.saturating_sub(compose_h));
        let video = Rect::new(left.x, left.y, left.width, video_h);
        let play_y = (video.bottom() + 1).min(compose.y);
        let playback = Rect::new(
            left.x,
            play_y,
            left.width,
            4.min(compose.y.saturating_sub(play_y)),
        );
        let remaining_y = (playback.bottom() + 1).min(compose.y);
        let remaining = compose.y.saturating_sub(remaining_y);
        let desc_h = if expanded {
            remaining
        } else {
            (remaining / 3).max(4).min(remaining)
        };
        let description = Rect::new(left.x, remaining_y, left.width, desc_h);
        let comment_y = (description.bottom() + u16::from(desc_h > 0)).min(compose.y);
        let comments = Rect::new(
            left.x,
            comment_y,
            left.width,
            compose
                .y
                .saturating_sub(comment_y + u16::from(compose_h > 0)),
        );
        Self {
            header,
            video,
            playback,
            description,
            comments,
            compose,
            right,
        }
    }
}
pub fn render_watch(f: &mut Frame, app: &mut App, area: Rect) {
    if area.width < 10 || area.height < 5 {
        return;
    }
    let wide = area.width >= 116;
    let layout = WatchLayout::new(area, wide, app.description_expanded);
    let state = &app.player.status;
    let mode = if state.playback_error.is_some() {
        "Playback error"
    } else if state.paused_for_cache || state.seeking {
        "Buffering"
    } else if state.paused {
        "Paused"
    } else if app.player.is_alive() {
        "Playing"
    } else {
        "Stopped"
    };
    let mode = match state.recovery {
        yute_core::player::network::Phase::Refreshing => "Refreshing media",
        yute_core::player::network::Phase::Proxy if !state.file_loaded => {
            "Switching to range transport"
        }
        yute_core::player::network::Phase::Failed => "Network stall · retry playback",
        _ => mode,
    };
    let format = match (state.video_height, state.video_codec.as_deref()) {
        (Some(h), Some(c)) => format!(" · {h}p {c}"),
        (_, Some(c)) => format!(" · {c}"),
        _ => String::new(),
    };
    let info = format!(
        "{mode}{format} · {:.1}× · cache {:.0} s · engine {}",
        if state.speed > 0. { state.speed } else { 1. },
        state.cache_duration,
        if state.proxy_active { "proxy" } else { "mpv" }
    );
    f.render_widget(
        Paragraph::new(super::text::truncate_str(
            &info,
            layout.header.width as usize,
        ))
        .style(app.theme.style("status")),
        layout.header,
    );
    if wide {
        render_heading(f, app, layout.video);
        render_playback(f, app, layout.playback);
        render_description(f, app, layout.description);
        if !app.description_expanded {
            render_comments(f, app, layout.comments);
        }
        render_compose(f, app, layout.compose);
        render_suggestions(f, app, layout.right);
    } else {
        let tabs = Rect::new(area.x, layout.header.bottom(), area.width, 1);
        let names = [
            ("Video", WatchPane::Video),
            ("Description", WatchPane::Description),
            ("Comments", WatchPane::Comments),
            ("Up next", WatchPane::Suggestions),
        ];
        let line = Line::from(
            names
                .into_iter()
                .map(|(s, p)| {
                    Span::styled(
                        format!(" {s} "),
                        if p == app.watch_pane {
                            app.theme.style("chip_active")
                        } else {
                            app.theme.style("chip_idle")
                        },
                    )
                })
                .collect::<Vec<_>>(),
        );
        f.render_widget(Paragraph::new(line), tabs);
        let body = Rect::new(
            area.x,
            tabs.bottom(),
            area.width,
            area.bottom().saturating_sub(tabs.bottom()),
        );
        match app.watch_pane {
            WatchPane::Video => {
                let h = 7.min(body.height);
                render_heading(f, app, Rect::new(body.x, body.y, body.width, h));
                let y = (body.y + h + 1).min(body.bottom());
                render_playback(f, app, Rect::new(body.x, y, body.width, body.bottom() - y));
            }
            WatchPane::Description => render_description(f, app, body),
            WatchPane::Comments => {
                let h = 3.min(body.height);
                render_comments(
                    f,
                    app,
                    Rect::new(body.x, body.y, body.width, body.height - h),
                );
                render_compose(f, app, Rect::new(body.x, body.bottom() - h, body.width, h));
            }
            WatchPane::Suggestions => render_suggestions(f, app, body),
        }
    }
}
fn render_heading(f: &mut Frame, app: &mut App, area: Rect) {
    if area.is_empty() {
        return;
    }
    let inner = section(f, app, area, "Video", app.watch_pane == WatchPane::Video);
    let video = app
        .player
        .current_video
        .as_ref()
        .or_else(|| app.watch_data.as_ref().map(|w| &w.video))
        .cloned();
    let title = video.as_ref().map_or(
        app.watch_error
            .as_deref()
            .unwrap_or("Loading video details…"),
        |v| v.title.as_str(),
    );
    super::text::title(
        f,
        super::text::wrap_and_truncate_title(
            &super::text::display(title, "native"),
            inner.width as usize,
            2,
        ),
        Rect::new(inner.x, inner.y, inner.width, inner.height.min(2)),
        app.theme.style("text").add_modifier(Modifier::BOLD),
    );
    if inner.height > 2 {
        let channel = app.watch_channel.clone();
        let name = video.as_ref().map_or("Channel", |v| v.channel.as_str());
        let a = Rect::new(
            inner.x,
            inner.y + 2,
            4.min(inner.width),
            2.min(inner.height - 2),
        );
        super::thumbnail::avatar(
            f,
            app,
            channel.as_ref().and_then(|c| c.avatar_url.as_deref()),
            name,
            a,
        );
        let x = (a.right() + 1).min(inner.right());
        let w = inner.right() - x;
        let subscribed = app.watch_data.as_ref().and_then(|w| w.is_subscribed) == Some(true);
        let channel_line = format!(
            "{name}{} · {} · {}",
            if video.as_ref().is_some_and(|v| v.verified) {
                " [v]"
            } else {
                ""
            },
            channel
                .as_ref()
                .and_then(|c| c.subscribers.as_deref())
                .unwrap_or(""),
            if subscribed {
                "Subscribed"
            } else {
                "Subscribe"
            }
        );
        f.render_widget(
            Paragraph::new(super::text::truncate_str(&channel_line, w as usize)).style(
                if app.watch_control == 0 && app.watch_pane == WatchPane::Video {
                    app.theme.style("selection")
                } else {
                    app.theme.style("text")
                },
            ),
            Rect::new(x, a.y, w, 1),
        );
        if inner.height > 3 {
            let watch = app.watch_data.as_ref();
            let like = watch.and_then(|w| w.is_liked) == Some(true);
            let dislike = watch.and_then(|w| w.is_disliked) == Some(true);
            let controls = [
                format!(
                    "{} {}",
                    super::icons::Icon::Like.glyph(app.config.icons()),
                    if like { "Unlike" } else { "Like" }
                ),
                format!(
                    "{} {}",
                    super::icons::Icon::Dislike.glyph(app.config.icons()),
                    if dislike { "Remove dislike" } else { "Dislike" }
                ),
            ];
            let mut spans = vec![];
            for (i, c) in controls.into_iter().enumerate() {
                spans.push(Span::styled(
                    format!(" {c} "),
                    if app.watch_pane == WatchPane::Video && app.watch_control == i + 1 {
                        app.theme.style("selection")
                    } else {
                        app.theme.style("text")
                    },
                ));
            }
            if let Some(v) = video.as_ref() {
                spans.push(Span::styled(
                    format!(
                        " · {} · {}",
                        v.views.as_deref().unwrap_or(""),
                        v.published.as_deref().unwrap_or("")
                    ),
                    app.theme.style("text_dim"),
                ));
            }
            f.render_widget(
                Paragraph::new(Line::from(spans)),
                Rect::new(x, a.y + 1, w, 1),
            );
        }
    }
}
fn render_playback(f: &mut Frame, app: &App, area: Rect) {
    if area.height < 3 {
        return;
    }
    let inner = section(f, app, area, "Playback", false);
    if inner.is_empty() {
        return;
    }
    let s = &app.player.status;
    let width = inner.width as usize;
    let mut bar = vec!['─'; width];
    if s.duration > 0. {
        let played = (s.time_pos / s.duration * width as f64).clamp(0., width as f64) as usize;
        let buffered = ((s.time_pos + s.cache_duration) / s.duration * width as f64)
            .clamp(0., width as f64) as usize;
        for c in bar.iter_mut().take(buffered) {
            *c = '┄';
        }
        for c in bar.iter_mut().take(played) {
            *c = '━';
        }
        if let Some(w) = &app.watch_data {
            for (t, _) in &w.chapters {
                let i = (*t as f64 / s.duration * width as f64) as usize;
                if let Some(c) = bar.get_mut(i) {
                    *c = '│';
                }
            }
        }
    }
    f.render_widget(
        Paragraph::new(bar.into_iter().collect::<String>()).style(app.theme.style("progress")),
        Rect::new(inner.x, inner.y, inner.width, 1),
    );
    if inner.height > 1 {
        let status = format!(
            "{} / {} · {:.1}× · volume {:.0}%",
            format_time(s.time_pos),
            format_time(s.duration),
            if s.speed > 0. { s.speed } else { 1. },
            s.volume
        );
        f.render_widget(
            Paragraph::new(super::text::truncate_str(&status, width))
                .style(app.theme.style("text_dim")),
            Rect::new(inner.x, inner.y + 1, inner.width, 1),
        );
    }
}
fn render_compose(f: &mut Frame, app: &App, area: Rect) {
    if area.height < 3 {
        return;
    }
    let count = app.comment_input.chars().count();
    let label = if app.comment_draft.as_ref().is_some_and(|d| d.reply) {
        "Reply"
    } else {
        "Add a comment"
    };
    let inner = section(
        f,
        app,
        area,
        &format!("{label} · {count}/10000"),
        app.focus == crate::app::Focus::CommentInput,
    );
    let value = if app.focus == crate::app::Focus::CommentInput {
        app.comment_input.as_str()
    } else {
        "n new comment · r reply"
    };
    f.render_widget(
        Paragraph::new(super::text::truncate_str(value, inner.width as usize))
            .style(app.theme.style("text")),
        inner,
    );
    if app.focus == crate::app::Focus::CommentInput && !inner.is_empty() {
        f.set_cursor_position((
            inner.x
                + (super::text::DisplayWidth::width(value)
                    .min(inner.width.saturating_sub(1) as usize) as u16),
            inner.y,
        ));
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
            Paragraph::new(
                app.watch_error
                    .as_deref()
                    .unwrap_or("Loading video details…"),
            )
            .style(app.theme.style("text_dim")),
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
        for (index, (time, title)) in watch.chapters.iter().enumerate() {
            lines.push(Line::from(Span::styled(
                format!(
                    "{} {}  {title}",
                    if index == app.watch_link_index {
                        ">"
                    } else {
                        " "
                    },
                    format_time(f64::from(*time))
                ),
                app.theme.style("accent"),
            )));
        }
    }
    for (i, (label, url)) in description_links(app).iter().enumerate() {
        lines.push(Line::from(Span::styled(
            format!(
                "{} {}",
                if i + watch.chapters.len() == app.watch_link_index {
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
        let avatar_w = 2.min(row_w.saturating_sub(8));
        let avatar_h = 1u16;
        let text_x = row_x + avatar_w + 1;
        let text_w = row_w.saturating_sub(avatar_w + 1);
        if avatar_w > 0 {
            let avatar_area = Rect::new(row_x, y, avatar_w, avatar_h);
            super::thumbnail::avatar(
                f,
                app,
                comment.author_thumb.as_deref(),
                &comment.author,
                avatar_area,
            );
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
        if app.watch_right_queue {
            "Queue · [ ] Up next"
        } else {
            "Up next · [ ] Queue"
        },
        app.watch_pane == WatchPane::Suggestions,
    );
    let thumb_w = (inner.width / 3).clamp(14, 24).min(inner.width / 2);
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
