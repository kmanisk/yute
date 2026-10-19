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

    let right_width = if area.width >= 120 {
        44u16
    } else if area.width >= 90 {
        36u16
    } else {
        24u16
    };

    let left_width = if area.width >= 90 {
        22u16
    } else {
        16u16
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(left_width), // Left: hamburger + current section
            Constraint::Min(20),            // Center: search pill
            Constraint::Length(right_width), // Right: ASCII YouTube badge + Account block
        ])
        .split(area);

    // Left: hamburger toggle + current section title
    let hamburger = if app.sidebar_expanded { "[=]" } else { "[-]" };
    let section_name = match app.screen {
        crate::app::Screen::Home => "Home",
        crate::app::Screen::Subscriptions => "Subscriptions",
        crate::app::Screen::History => "History",
        crate::app::Screen::WatchLater => "Watch Later",
        crate::app::Screen::Playlists | crate::app::Screen::PlaylistView => "Playlists",
        crate::app::Screen::Liked => "Liked",
        crate::app::Screen::LocalVideos => "Local",
        crate::app::Screen::Queue => "Queue",
        crate::app::Screen::Settings => "Settings",
        crate::app::Screen::Accounts => "Accounts",
        crate::app::Screen::Watch => "Watch",
        crate::app::Screen::Search => "Search",
        crate::app::Screen::Help => "Help",
    };

    let logo_spans = vec![
        Span::styled(format!("{hamburger} "), Style::default().fg(app.theme.text_secondary)),
        Span::styled(section_name, Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
    ];
    let logo_lines = if area.height >= 3 {
        vec![Line::from(""), Line::from(logo_spans)]
    } else {
        vec![Line::from(logo_spans)]
    };
    f.render_widget(Paragraph::new(logo_lines), chunks[0]);

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

    // Right: spinner indicator + ASCII YouTube badge + account avatar
    let spinners = ["-", "\\", "|", "/"];
    let spinner_str = if app.is_loading {
        spinners[app.spinner_frame % spinners.len()]
    } else {
        ""
    };

    let account_initial = if app.account_initial.is_empty() {
        "M"
    } else {
        &app.account_initial
    };

    let mut right_spans = Vec::new();
    if app.is_loading && !spinner_str.is_empty() {
        right_spans.push(Span::styled(format!("{spinner_str} "), Style::default().fg(app.theme.accent)));
    }

    if right_width >= 32 {
        right_spans.push(Span::styled(
            " ▕▶▏YouTube ",
            Style::default()
                .bg(app.theme.accent)
                .fg(ratatui::style::Color::White)
                .add_modifier(Modifier::BOLD),
        ));
        right_spans.push(Span::raw(" "));
    } else if right_width >= 24 {
        right_spans.push(Span::styled(
            " ▕▶▏",
            Style::default()
                .bg(app.theme.accent)
                .fg(ratatui::style::Color::White)
                .add_modifier(Modifier::BOLD),
        ));
        right_spans.push(Span::raw(" "));
    }

    // Account pill with turquoise avatar circle
    right_spans.push(Span::styled(
        format!(" [{account_initial}] "),
        Style::default()
            .bg(ratatui::style::Color::Rgb(0, 150, 136))
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::BOLD),
    ));

    if right_width >= 36 && !app.account_name.is_empty() {
        right_spans.push(Span::styled(
            format!(" {} ", app.account_name),
            Style::default()
                .bg(app.theme.surface_hover)
                .fg(app.theme.text)
                .add_modifier(Modifier::BOLD),
        ));
    } else if right_width >= 28 && !app.account_name.is_empty() {
        let first_word = app.account_name.split_whitespace().next().unwrap_or(&app.account_name);
        right_spans.push(Span::styled(
            format!(" {first_word} "),
            Style::default()
                .bg(app.theme.surface_hover)
                .fg(app.theme.text)
                .add_modifier(Modifier::BOLD),
        ));
    }
    right_spans.push(Span::raw(" "));

    let right_lines = if area.height >= 3 {
        vec![Line::from(""), Line::from(right_spans)]
    } else {
        vec![Line::from(right_spans)]
    };
    let right_p = Paragraph::new(right_lines).alignment(Alignment::Right);
    f.render_widget(right_p, chunks[2]);
}
