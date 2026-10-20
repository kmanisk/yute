use crate::app::{App, Screen, ViewMode};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn render_statusbar(f: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let rows = if area.height >= 2 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Length(1)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1)])
            .split(area)
    };

    // Calculate grid / row information
    let total_items = app.filtered_items().len();
    let selected_pos = if total_items > 0 {
        (app.selected_index + 1).min(total_items)
    } else {
        0
    };

    let content_w = area.width.saturating_sub(26); // roughly subtract sidebar
    let num_cols = if app.current_view_mode() == ViewMode::List {
        1
    } else if app.config.grid.columns > 0 {
        let max_fit = (content_w / 24).max(1);
        (app.config.grid.columns).min(max_fit).clamp(1, 4) as usize
    } else {
        (content_w / 36).clamp(1, 4) as usize
    };

    let current_row = if total_items > 0 {
        (app.selected_index / num_cols) + 1
    } else {
        0
    };

    let total_rows = if total_items > 0 {
        (total_items + num_cols - 1) / num_cols
    } else {
        0
    };

    let mut status_spans = Vec::new();
    status_spans.push(Span::styled(
        " YTUI ",
        Style::default()
            .bg(app.theme.accent)
            .fg(ratatui::style::Color::Black)
            .add_modifier(Modifier::BOLD),
    ));
    status_spans.push(Span::raw(" "));
    status_spans.push(Span::styled(
        format!("innertube · {total_items} items · {selected_pos}/{total_items} · row {current_row}/{total_rows}"),
        Style::default().fg(app.theme.text_secondary),
    ));

    if app.player.is_alive() && app.player.status.file_loaded && !app.player.status.title.is_empty() {
        status_spans.push(Span::styled(" · ", Style::default().fg(app.theme.border)));
        status_spans.push(Span::styled(
            format!("▶ {}", app.player.status.title),
            Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD),
        ));
    } else if !app.status_text.is_empty() {
        status_spans.push(Span::styled(" · ", Style::default().fg(app.theme.border)));
        status_spans.push(Span::styled(
            &app.status_text,
            Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD),
        ));
    }

    f.render_widget(Paragraph::new(Line::from(status_spans)), rows[0]);

    if rows.len() > 1 {
        // Context-sensitive key hints that drop from right as width shrinks, but always keep "? help"
        let all_hints = match app.screen {
            Screen::Watch => vec![
                "Space: pause",
                ",/.: seek",
                "[ ]: vol",
                "</>: speed",
                "i: stats",
                "c: chat",
                "d: desc",
                "f: full",
                "?: help",
                "q: back",
            ],
            _ => vec![
                "Tab: Pane",
                "Enter: Play",
                "Shift+Enter: Inline",
                "v: View",
                "/: Search",
                "f: Filter",
                "r: Refresh",
                "b: Sidebar",
                "gh: Home",
                "gs: Subs",
                "?: Help",
                "q: Quit",
            ],
        };

        let mut available_w = area.width.saturating_sub(2) as usize;
        let mut active_hints = Vec::new();

        // Always prioritize the last hint ("? help")
        let help_hint = *all_hints.last().unwrap_or(&"? Help");
        available_w = available_w.saturating_sub(help_hint.len() + 3);

        for hint in &all_hints[..all_hints.len().saturating_sub(1)] {
            let needed = hint.len() + 3; // " · "
            if available_w >= needed {
                active_hints.push(*hint);
                available_w -= needed;
            } else {
                break;
            }
        }
        active_hints.push(help_hint);

        let hints_str = active_hints.join(" · ");
        let hints_line = Line::from(vec![
            Span::styled(format!(" {hints_str}"), Style::default().fg(app.theme.text_secondary)),
        ]);
        f.render_widget(Paragraph::new(hints_line), rows[1]);
    }
}
