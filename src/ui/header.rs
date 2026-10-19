use crate::app::{App, Focus};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn render_header(f: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(22), // Logo + hamburger
            Constraint::Min(20),   // Centered search pill
            Constraint::Length(16), // Spinner + Account block
        ])
        .split(area);

    // Left: hamburger toggle + YouTube wordmark
    let hamburger = if app.sidebar_expanded { "[=]" } else { "[-]" };
    let logo_spans = vec![
        Span::styled(format!("{hamburger} "), Style::default().fg(app.theme.text_secondary)),
        Span::styled(" |> ", Style::default().fg(app.theme.red).add_modifier(Modifier::BOLD)),
        Span::styled("YouTube", Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
    ];
    f.render_widget(Paragraph::new(Line::from(logo_spans)), chunks[0]);

    // Center: search pill with rounded or square borders
    let is_search_focused = app.focus == Focus::SearchInput;
    let search_border_color = if is_search_focused {
        app.theme.accent
    } else {
        app.theme.border
    };

    let search_spans = if is_search_focused {
        if app.search_query.is_empty() {
            vec![
                Span::styled(" [?] ", Style::default().fg(app.theme.accent)),
                Span::styled("█ ", Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled("Type to search... (Enter to submit, Esc to cancel)", Style::default().fg(app.theme.text_secondary)),
            ]
        } else {
            vec![
                Span::styled(" [?] ", Style::default().fg(app.theme.accent)),
                Span::styled(format!(" {}", app.search_query), Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
                Span::styled("█", Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD)),
            ]
        }
    } else if app.search_query.is_empty() {
        vec![
            Span::styled(" [?] ", Style::default().fg(app.theme.text_secondary)),
            Span::styled(" Search (/ to search)...", Style::default().fg(app.theme.text_secondary)),
        ]
    } else {
        vec![
            Span::styled(" [?] ", Style::default().fg(app.theme.text_secondary)),
            Span::styled(format!(" {}", app.search_query), Style::default().fg(app.theme.text)),
        ]
    };

    let search_pill = Paragraph::new(Line::from(search_spans))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(search_border_color))
            .style(Style::default().bg(app.theme.surface)),
    );
    f.render_widget(search_pill, chunks[1]);

    // Right: spinner indicator + account initial block
    let spinners = ["-", "\\", "|", "/"];
    let spinner_str = if app.is_loading {
        spinners[app.spinner_frame % spinners.len()]
    } else {
        " "
    };

    let account_initial = if app.account_initial.is_empty() {
        "U"
    } else {
        &app.account_initial
    };

    let right_spans = vec![
        Span::styled(format!(" {spinner_str}  "), Style::default().fg(app.theme.red)),
        Span::styled(
            format!(" [{account_initial}] "),
            Style::default()
                .bg(app.theme.surface_hover)
                .fg(app.theme.text)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ];
    let right_p = Paragraph::new(Line::from(right_spans)).alignment(Alignment::Right);
    f.render_widget(right_p, chunks[2]);
}
