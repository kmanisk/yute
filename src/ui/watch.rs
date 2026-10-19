use crate::app::App;
use crate::backend::models::Card;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn render_watch(f: &mut Frame, app: &mut App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(65), // Left: Now playing + description + chapters
            Constraint::Percentage(35), // Right: Up Next list
        ])
        .split(area);

    render_left_pane(f, app, cols[0]);
    render_right_pane(f, app, cols[1]);
}

fn render_left_pane(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border))
        .style(Style::default().bg(app.theme.bg));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 6 || inner.width < 10 {
        return;
    }

    let video = app.player.current_video.as_ref();
    let title = video.map(|v| v.title.as_str()).unwrap_or("No Video Playing");
    let channel = video.map(|v| v.channel.as_str()).unwrap_or("");

    // Calculate progress and time
    let status = &app.player.status;
    let time_curr = format_time(status.time_pos);
    let time_total = format_time(status.duration);
    let pct = if status.duration > 0.0 {
        ((status.time_pos / status.duration) * 100.0).clamp(0.0, 100.0) as usize
    } else {
        0
    };

    let track_w = (inner.width as usize).saturating_sub(25).max(10);
    let filled = (pct * track_w) / 100;
    let progress_bar = format!(
        "[{}{}] {} / {} ({:.0}%)",
        "=".repeat(filled),
        " ".repeat(track_w.saturating_sub(filled)),
        time_curr,
        time_total,
        pct
    );

    let (state_str, state_style) = if status.stall_error {
        ("[STALLED]", Style::default().fg(ratatui::style::Color::Red).add_modifier(Modifier::BOLD))
    } else if !status.file_loaded {
        ("[LOADING]", Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD))
    } else if status.paused_for_cache {
        ("[BUFFERING]", Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD))
    } else if status.paused {
        ("[PAUSED]", Style::default().fg(app.theme.text_secondary).add_modifier(Modifier::BOLD))
    } else {
        ("[PLAYING]", Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD))
    };
    let vol_str = format!("Vol: {:.0}%", status.volume);
    let speed_str = format!("Speed: {:.2}x", status.speed);

    let mut lines = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(format!(" {state_str} "), state_style),
        Span::styled(title, Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
    ]));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(channel, Style::default().fg(app.theme.text_secondary)),
        Span::raw("    "),
        Span::styled(vol_str, Style::default().fg(app.theme.text_secondary)),
        Span::raw("    "),
        Span::styled(speed_str, Style::default().fg(app.theme.text_secondary)),
    ]));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(progress_bar, Style::default().fg(app.theme.red)),
    ]));
    lines.push(Line::from(""));

    // Description / chapters if watch_data is present
    if let Some(ref wd) = app.watch_data {
        if app.show_description && !wd.description.is_empty() {
            lines.push(Line::from(Span::styled("--- Description (press 'd' to hide) ---", Style::default().fg(app.theme.text_secondary))));
            for line in wd.description.lines().take(15) {
                lines.push(Line::from(Span::styled(format!("  {line}"), Style::default().fg(app.theme.text))));
            }
        }
    }

    let p = Paragraph::new(lines);
    f.render_widget(p, inner);
}

fn render_right_pane(f: &mut Frame, app: &mut App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border))
        .title(" Up Next ")
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

    let mut lines = Vec::new();
    for (i, item) in items.iter().take(inner.height as usize / 3).enumerate() {
        let title = item.title();
        match item {
            Card::Video(v) => {
                lines.push(Line::from(vec![
                    Span::styled(format!("{}. ", i + 1), Style::default().fg(app.theme.text_secondary)),
                    Span::styled(title, Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
                ]));
                lines.push(Line::from(vec![
                    Span::raw("   "),
                    Span::styled(&v.channel, Style::default().fg(app.theme.text_secondary)),
                    Span::raw(" · "),
                    Span::styled(v.duration.as_deref().unwrap_or(""), Style::default().fg(app.theme.text_secondary)),
                ]));
                lines.push(Line::from(""));
            }
            _ => {}
        }
    }

    let p = Paragraph::new(lines);
    f.render_widget(p, inner);
}

fn format_time(secs: f64) -> String {
    if secs <= 0.0 || secs.is_nan() {
        return "0:00".to_string();
    }
    let s = secs as u64;
    let mins = s / 60;
    let sec = s % 60;
    format!("{mins}:{sec:02}")
}
