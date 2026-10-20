use crate::app::{App, WatchRightMode};
use crate::backend::models::{Card, LiveState};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

pub fn render_watch(f: &mut Frame, app: &mut App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(62), // Left: Video info + player stats + controls + description
            Constraint::Percentage(38), // Right: Up Next recommendations list
        ])
        .split(area);

    render_left_pane(f, app, cols[0]);
    render_right_pane(f, app, cols[1]);
}

fn render_left_pane(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border))
        .title(" Player ")
        .style(Style::default().bg(app.theme.bg));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 6 || inner.width < 10 {
        return;
    }

    let video = app.player.current_video.as_ref();
    let title = video.map(|v| v.title.as_str()).unwrap_or("No Video Playing");
    let channel = video.map(|v| v.channel.as_str()).unwrap_or("");
    let verified = video.map(|v| v.verified).unwrap_or(false);

    // Pull richer metadata from watch_data if available
    let (views_str, published_str) = if let Some(ref wd) = app.watch_data {
        let v_str = wd.video.views.clone().or_else(|| video.and_then(|v| v.views.clone()));
        let p_str = wd.video.published.clone().or_else(|| video.and_then(|v| v.published.clone()));
        (v_str, p_str)
    } else {
        (
            video.and_then(|v| v.views.clone()),
            video.and_then(|v| v.published.clone()),
        )
    };

    let status = &app.player.status;
    let time_curr = format_time(status.time_pos);
    let time_total = format_time(status.duration);
    let pct = if status.duration > 0.0 {
        ((status.time_pos / status.duration) * 100.0).clamp(0.0, 100.0) as usize
    } else {
        0
    };

    let (state_str, state_style) = if status.stall_error {
        ("[STALLED]", Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD))
    } else if !status.file_loaded {
        ("[LOADING]", Style::default().bg(app.theme.badge_bg).fg(app.theme.accent).add_modifier(Modifier::BOLD))
    } else if status.paused_for_cache {
        ("[BUFFERING]", Style::default().bg(Color::Rgb(200, 140, 20)).fg(Color::Black).add_modifier(Modifier::BOLD))
    } else if status.paused {
        ("[PAUSED]", Style::default().bg(app.theme.badge_bg).fg(app.theme.text_secondary).add_modifier(Modifier::BOLD))
    } else {
        ("[PLAYING]", Style::default().bg(app.theme.accent).fg(Color::Black).add_modifier(Modifier::BOLD))
    };

    // Calculate layout: Top info (6), Controls bar (1), Description box (remainder)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Header, meta, actions, audio/cache, progress bar
            Constraint::Length(1), // Hotkey guide bar
            Constraint::Min(4),    // Description box
        ])
        .split(inner);

    // Section 1: Title, Meta, and Progress Bar
    let mut header_lines = Vec::new();

    // Line 1: State Badge + Title
    header_lines.push(Line::from(vec![
        Span::styled(format!(" {state_str} "), state_style),
        Span::raw(" "),
        Span::styled(title, Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
    ]));

    // Line 2: Channel [v] · views · published
    let mut meta_spans = vec![
        Span::raw("  "),
        Span::styled(if channel.is_empty() { "YouTube" } else { channel }, Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD)),
    ];
    if verified {
        meta_spans.push(Span::styled(" [v]", Style::default().fg(app.theme.accent)));
    }
    if let Some(v_txt) = views_str {
        if !v_txt.is_empty() {
            meta_spans.push(Span::styled("  ·  ", Style::default().fg(app.theme.text_secondary)));
            meta_spans.push(Span::styled(v_txt, Style::default().fg(app.theme.text)));
        }
    }
    if let Some(p_txt) = published_str {
        if !p_txt.is_empty() {
            meta_spans.push(Span::styled("  ·  ", Style::default().fg(app.theme.text_secondary)));
            meta_spans.push(Span::styled(p_txt, Style::default().fg(app.theme.text_secondary)));
        }
    }
    header_lines.push(Line::from(meta_spans));

    // Line 3: Interactive Action Chips (Likes, Dislikes, Rating, Subscribe)
    let is_liked = app.watch_data.as_ref().and_then(|w| w.is_liked).unwrap_or(false);
    let is_disliked = app.watch_data.as_ref().and_then(|w| w.is_disliked).unwrap_or(false);
    let is_sub = app.watch_data.as_ref().and_then(|w| w.is_subscribed).unwrap_or(false);
    let likes_txt = app.watch_data.as_ref().and_then(|w| w.likes.clone()).unwrap_or_else(|| "Like".to_string());
    let dislikes_txt = app.watch_data.as_ref().and_then(|w| w.dislikes.clone()).unwrap_or_else(|| "Dislike".to_string());

    let mut action_spans = vec![Span::raw("  ")];
    if is_liked {
        action_spans.push(Span::styled(format!("[+ Liked {likes_txt}]"), Style::default().fg(Color::Black).bg(app.theme.accent).add_modifier(Modifier::BOLD)));
    } else {
        action_spans.push(Span::styled(format!("[+ Like {likes_txt}]"), Style::default().fg(app.theme.accent)));
    }

    action_spans.push(Span::styled("   ", Style::default().fg(app.theme.border)));

    if is_disliked {
        action_spans.push(Span::styled(format!("[- Disliked {dislikes_txt}]"), Style::default().fg(Color::White).bg(app.theme.red).add_modifier(Modifier::BOLD)));
    } else {
        action_spans.push(Span::styled(format!("[- Dislike {dislikes_txt}]"), Style::default().fg(app.theme.text_secondary)));
    }

    if let Some(r) = app.watch_data.as_ref().and_then(|w| w.rating) {
        if r > 0.0 {
            action_spans.push(Span::styled("   ", Style::default().fg(app.theme.border)));
            action_spans.push(Span::styled(format!("[Rating: {r:.1}/5]"), Style::default().fg(app.theme.text_secondary)));
        }
    }

    action_spans.push(Span::styled("   ", Style::default().fg(app.theme.border)));

    if is_sub {
        action_spans.push(Span::styled("[Subscribed]", Style::default().fg(app.theme.text_secondary).add_modifier(Modifier::BOLD)));
    } else {
        action_spans.push(Span::styled("[s Subscribe]", Style::default().fg(Color::Black).bg(app.theme.red).add_modifier(Modifier::BOLD)));
    }
    header_lines.push(Line::from(action_spans));

    // Line 4: Audio/Video status & cache duration
    let vol_str = format!("Vol: {:.0}%", status.volume);
    let speed_str = format!("Speed: {:.2}x", status.speed);
    let buffer_str = if status.cache_duration > 0.0 {
        format!("Buffer: {:.1}s", status.cache_duration)
    } else {
        "Buffer: ready".to_string()
    };
    header_lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(vol_str, Style::default().fg(app.theme.text_secondary)),
        Span::styled("   ·   ", Style::default().fg(app.theme.border)),
        Span::styled(speed_str, Style::default().fg(app.theme.text_secondary)),
        Span::styled("   ·   ", Style::default().fg(app.theme.border)),
        Span::styled(buffer_str, Style::default().fg(app.theme.accent)),
    ]));

    // Line 5: Progress Bar
    let track_w = (chunks[0].width as usize).saturating_sub(28).max(10);
    let filled = (pct * track_w) / 100;
    let progress_bar = format!(
        "[{}{}] {} / {} ({:.0}%)",
        "=".repeat(filled),
        " ".repeat(track_w.saturating_sub(filled)),
        time_curr,
        time_total,
        pct
    );
    header_lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(progress_bar, Style::default().fg(app.theme.red).add_modifier(Modifier::BOLD)),
    ]));

    let p_header = Paragraph::new(header_lines);
    f.render_widget(p_header, chunks[0]);

    // Section 2: Hotkey controls cheatsheet bar
    let controls_line = Line::from(vec![
        Span::styled(" Keys: ", Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled("Space/k", Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
        Span::styled(" pause · ", Style::default().fg(app.theme.text_secondary)),
        Span::styled("h/l", Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
        Span::styled(" seek · ", Style::default().fg(app.theme.text_secondary)),
        Span::styled("[/'", Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
        Span::styled(" vol · ", Style::default().fg(app.theme.text_secondary)),
        Span::styled("+/-", Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
        Span::styled(" like/dis · ", Style::default().fg(app.theme.text_secondary)),
        Span::styled("s", Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
        Span::styled(" sub · ", Style::default().fg(app.theme.text_secondary)),
        Span::styled("c", Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
        Span::styled(" comments · ", Style::default().fg(app.theme.text_secondary)),
        Span::styled("i", Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
        Span::styled(" stats · ", Style::default().fg(app.theme.text_secondary)),
        Span::styled("q", Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
        Span::styled(" back", Style::default().fg(app.theme.text_secondary)),
    ]);
    f.render_widget(Paragraph::new(controls_line), chunks[1]);

    // Section 3: Description & Chapters Box
    let desc_title = if app.show_description {
        " Description (press 'd' to collapse) "
    } else {
        " Description (press 'd' to expand) "
    };

    let desc_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border))
        .title(desc_title)
        .style(Style::default().bg(app.theme.surface));

    let desc_inner = desc_block.inner(chunks[2]);
    f.render_widget(desc_block, chunks[2]);

    if desc_inner.height > 0 && desc_inner.width > 0 {
        let mut desc_lines = Vec::new();
        if let Some(ref wd) = app.watch_data {
            if !wd.chapters.is_empty() {
                desc_lines.push(Line::from(Span::styled("Chapters:", Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD))));
                for (offset_sec, ch_title) in &wd.chapters {
                    let t_str = format_time(*offset_sec as f64);
                    desc_lines.push(Line::from(vec![
                        Span::styled(format!("  [{t_str}] "), Style::default().fg(app.theme.accent)),
                        Span::styled(ch_title, Style::default().fg(app.theme.text)),
                    ]));
                }
                desc_lines.push(Line::from(""));
            }

            if wd.description.is_empty() {
                desc_lines.push(Line::from(Span::styled("No description provided.", Style::default().fg(app.theme.text_secondary))));
            } else if app.show_description {
                for line in wd.description.lines().take(desc_inner.height as usize * 2) {
                    desc_lines.push(Line::from(Span::styled(line, Style::default().fg(app.theme.text))));
                }
            } else {
                let max_preview = (desc_inner.height as usize).saturating_sub(2).max(1);
                for line in wd.description.lines().take(max_preview) {
                    desc_lines.push(Line::from(Span::styled(line, Style::default().fg(app.theme.text))));
                }
                desc_lines.push(Line::from(Span::styled("... (press 'd' to read full description)", Style::default().fg(app.theme.text_secondary))));
            }
        } else {
            desc_lines.push(Line::from(Span::styled("Loading video details and chapters...", Style::default().fg(app.theme.text_secondary))));
        }

        let p_desc = Paragraph::new(desc_lines).wrap(Wrap { trim: true });
        f.render_widget(p_desc, desc_inner);
    }
}

fn render_right_pane(f: &mut Frame, app: &mut App, area: Rect) {
    match app.watch_right_mode {
        WatchRightMode::Comments => render_comments_pane(f, app, area),
        WatchRightMode::UpNext => render_up_next_pane(f, app, area),
    }
}

fn render_comments_pane(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border))
        .title(" Comments (press 'c' for recommendations) ")
        .style(Style::default().bg(app.theme.surface));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let comments = app.watch_data.as_ref().map(|w| &w.comments);
    let Some(items) = comments else {
        let p = Paragraph::new("Loading comments...")
            .alignment(Alignment::Center)
            .style(Style::default().fg(app.theme.text_secondary));
        f.render_widget(p, inner);
        return;
    };

    if items.is_empty() {
        let p = Paragraph::new("No comments available")
            .alignment(Alignment::Center)
            .style(Style::default().fg(app.theme.text_secondary));
        f.render_widget(p, inner);
        return;
    }

    let mut lines = Vec::new();
    let skip_count = app.comments_cursor;
    for comment in items.iter().skip(skip_count) {
        let mut header_spans = vec![
            Span::styled(&comment.author, Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD)),
        ];
        if let Some(ref pub_time) = comment.published {
            header_spans.push(Span::styled(format!("  ·  {pub_time}"), Style::default().fg(app.theme.text_secondary)));
        }
        if let Some(ref likes) = comment.likes {
            header_spans.push(Span::styled(format!("  ·  {likes} likes"), Style::default().fg(app.theme.text)));
        }
        lines.push(Line::from(header_spans));

        for line in comment.text.lines() {
            lines.push(Line::from(Span::styled(format!("  {line}"), Style::default().fg(app.theme.text))));
        }
        lines.push(Line::from(Span::styled("  ─────────────────────────────────────", Style::default().fg(app.theme.border))));
        lines.push(Line::from(""));
    }

    let p = Paragraph::new(lines).wrap(Wrap { trim: true });
    f.render_widget(p, inner);
}

fn render_up_next_pane(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border))
        .title(" Up Next (press 'c' for comments) ")
        .style(Style::default().bg(app.theme.surface));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let up_next = app.watch_data.as_ref().map(|w| &w.up_next);
    let Some(items) = up_next else {
        let p = Paragraph::new("Loading recommendations...")
            .alignment(Alignment::Center)
            .style(Style::default().fg(app.theme.text_secondary));
        f.render_widget(p, inner);
        return;
    };

    if items.is_empty() {
        let p = Paragraph::new("No recommendations available")
            .alignment(Alignment::Center)
            .style(Style::default().fg(app.theme.text_secondary));
        f.render_widget(p, inner);
        return;
    }

    let mut lines = Vec::new();
    let max_items = (inner.height as usize / 3).max(1);

    for (i, item) in items.iter().take(max_items).enumerate() {
        let num_prefix = format!("{}. ", i + 1);
        match item {
            Card::Video(v) => {
                // Line 1: Number + Title
                lines.push(Line::from(vec![
                    Span::styled(num_prefix, Style::default().fg(app.theme.text_secondary)),
                    Span::styled(&v.title, Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
                ]));

                // Line 2: Dynamically construct meta string (never print bare dots)
                let mut meta_parts = Vec::new();
                if !v.channel.is_empty() {
                    let mut ch = v.channel.clone();
                    if v.verified {
                        ch.push_str(" [v]");
                    }
                    meta_parts.push(ch);
                }
                if v.badges.iter().any(|b| b == "SHORTS") {
                    meta_parts.push("[SHORTS]".to_string());
                }
                if v.live == LiveState::Live {
                    meta_parts.push("[LIVE]".to_string());
                }
                if let Some(ref dur) = v.duration {
                    if !dur.is_empty() {
                        meta_parts.push(dur.clone());
                    }
                }
                if let Some(ref views) = v.views {
                    if !views.is_empty() {
                        meta_parts.push(views.clone());
                    }
                }
                if let Some(ref pub_time) = v.published {
                    if !pub_time.is_empty() {
                        meta_parts.push(pub_time.clone());
                    }
                }

                let meta_str = if meta_parts.is_empty() {
                    "YouTube Video".to_string()
                } else {
                    meta_parts.join(" · ")
                };

                lines.push(Line::from(vec![
                    Span::raw("   "),
                    Span::styled(meta_str, Style::default().fg(app.theme.text_secondary)),
                ]));
                lines.push(Line::from(""));
            }
            Card::Playlist(p) => {
                lines.push(Line::from(vec![
                    Span::styled(num_prefix, Style::default().fg(app.theme.text_secondary)),
                    Span::styled(&p.title, Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
                ]));

                let type_badge = if p.is_mix {
                    "Mix · YouTube"
                } else {
                    "Playlist"
                };
                let meta_str = if p.channel.is_empty() {
                    type_badge.to_string()
                } else {
                    format!("{} · {}", p.channel, type_badge)
                };

                lines.push(Line::from(vec![
                    Span::raw("   "),
                    Span::styled(meta_str, Style::default().fg(app.theme.accent)),
                ]));
                lines.push(Line::from(""));
            }
            Card::Channel(c) => {
                lines.push(Line::from(vec![
                    Span::styled(num_prefix, Style::default().fg(app.theme.text_secondary)),
                    Span::styled(&c.title, Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
                ]));
                let sub_str = c.subscribers.as_deref().unwrap_or("Channel");
                lines.push(Line::from(vec![
                    Span::raw("   "),
                    Span::styled(sub_str, Style::default().fg(app.theme.text_secondary)),
                ]));
                lines.push(Line::from(""));
            }
        }
    }

    let p = Paragraph::new(lines).wrap(Wrap { trim: true });
    f.render_widget(p, inner);
}

fn format_time(secs: f64) -> String {
    if secs <= 0.0 || secs.is_nan() {
        return "0:00".to_string();
    }
    let s = secs as u64;
    let hours = s / 3600;
    let mins = (s % 3600) / 60;
    let sec = s % 60;
    if hours > 0 {
        format!("{hours}:{mins:02}:{sec:02}")
    } else {
        format!("{mins}:{sec:02}")
    }
}
