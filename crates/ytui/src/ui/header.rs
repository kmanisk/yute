use crate::app::{App, Focus, Screen};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn render_header(f: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let is_nerd = app.config.icons() == "nerd";

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
            Constraint::Length(left_width),   // Left: App title / section
            Constraint::Min(20),              // Center: search box
            Constraint::Length(right_width),  // Right: YouTube badge + Account
        ])
        .split(area);

    // Left: hamburger icon / app branding & section title
    let hamburger = if is_nerd {
        if app.sidebar_expanded { "󰍜 " } else { " " }
    } else if app.sidebar_expanded {
        "[=] "
    } else {
        "[-] "
    };

    let section_name = match app.screen {
        Screen::Home => "Home",
        Screen::Subscriptions => "Subscriptions",
        Screen::History => "History",
        Screen::WatchLater => "Watch Later",
        Screen::Playlists | Screen::PlaylistView => "Playlists",
        Screen::Liked => "Liked",
        Screen::LocalVideos => "Local",
        Screen::Queue => "Queue",
        Screen::Settings => "Settings",
        Screen::Accounts => "Accounts",
        Screen::Watch => "Watch",
        Screen::Search => "Search",
        Screen::Help => "Help",
    };

    let logo_spans = vec![
        Span::styled(hamburger, Style::default().fg(app.theme.text_secondary)),
        Span::styled("ytui ", Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD)),
        Span::styled(format!("· {section_name}"), Style::default().fg(app.theme.text)),
    ];
    let logo_lines = if area.height >= 3 {
        vec![Line::from(""), Line::from(logo_spans)]
    } else {
        vec![Line::from(logo_spans)]
    };
    f.render_widget(Paragraph::new(logo_lines), chunks[0]);

    // Center: search box with accent border and visible cursor when focused
    let is_search_focused = app.focus == Focus::SearchInput;
    let search_border_color = if is_search_focused {
        app.theme.accent
    } else {
        app.theme.border
    };

    let search_icon = if is_nerd { "  " } else { " [/] " };

    let search_spans = if is_search_focused {
        if app.search_query.is_empty() {
            vec![
                Span::styled(search_icon, Style::default().fg(app.theme.accent)),
                Span::styled("█", Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD)),
                Span::styled(" Type to search... (Enter to submit, Esc to cancel)", Style::default().fg(app.theme.text_secondary)),
            ]
        } else {
            vec![
                Span::styled(search_icon, Style::default().fg(app.theme.accent)),
                Span::styled(format!(" {}", app.search_query), Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
                Span::styled("█", Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD)),
            ]
        }
    } else if app.screen == Screen::Search && !app.search_query.is_empty() {
        let total = app.filtered_items().len();
        vec![
            Span::styled(search_icon, Style::default().fg(app.theme.accent)),
            Span::styled(format!(" Results for '{}' · {}", app.search_query, total), Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
        ]
    } else if app.search_query.is_empty() {
        vec![
            Span::styled(search_icon, Style::default().fg(app.theme.text_secondary)),
            Span::styled(" Search (/ to search)...", Style::default().fg(app.theme.text_secondary)),
        ]
    } else {
        vec![
            Span::styled(search_icon, Style::default().fg(app.theme.text_secondary)),
            Span::styled(format!(" {}", app.search_query), Style::default().fg(app.theme.text)),
        ]
    };

    let search_pill = Paragraph::new(Line::from(search_spans))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_set(ratatui::symbols::border::ROUNDED)
                .border_style(Style::default().fg(search_border_color))
                .style(Style::default().bg(app.theme.bg)),
        );
    f.render_widget(search_pill, chunks[1]);

    // Right: Loading indicator + YouTube badge with unclipped ▶ + account
    let spinners = ["-", "\\", "|", "/"];
    let spinner_str = if app.is_loading {
        spinners[app.spinner_frame % spinners.len()]
    } else {
        ""
    };

    let mut right_spans = Vec::new();
    if app.is_loading && !spinner_str.is_empty() {
        right_spans.push(Span::styled(format!("{spinner_str} Loading... "), Style::default().fg(app.theme.accent)));
    }

    // Wide glyphs measured with unicode-width and given trailing space so ▶ is never clipped
    if right_width >= 32 {
        right_spans.push(Span::styled(
            " ▕▶ ▏ YouTube ",
            Style::default()
                .bg(app.theme.accent)
                .fg(ratatui::style::Color::Black)
                .add_modifier(Modifier::BOLD),
        ));
        right_spans.push(Span::raw(" "));
    } else if right_width >= 24 {
        right_spans.push(Span::styled(
            " ▕▶ ▏",
            Style::default()
                .bg(app.theme.accent)
                .fg(ratatui::style::Color::Black)
                .add_modifier(Modifier::BOLD),
        ));
        right_spans.push(Span::raw(" "));
    }

    let account_initial = if is_nerd {
        ""
    } else if app.account_initial.is_empty() {
        "M"
    } else {
        &app.account_initial
    };

    let initial_display = if is_nerd {
        format!(" {account_initial} ")
    } else {
        format!(" [{account_initial}] ")
    };

    right_spans.push(Span::styled(
        initial_display,
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
    }

    let right_lines = if area.height >= 3 {
        vec![Line::from(""), Line::from(right_spans)]
    } else {
        vec![Line::from(right_spans)]
    };
    let header_right = Paragraph::new(right_lines).alignment(ratatui::layout::Alignment::Right);
    f.render_widget(header_right, chunks[2]);
}
