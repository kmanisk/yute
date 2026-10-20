pub mod accounts;
pub mod chat;
pub mod chips;
pub mod cmdline;
pub mod grid;
pub mod header;
pub mod help;
pub mod list;
pub mod settings;
pub mod sidebar;
pub mod statusbar;
pub mod text;
pub mod watch;

use crate::app::{App, Focus, Screen, ViewMode};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();
    if size.width < 30 || size.height < 10 {
        return;
    }

    let show_chips = !matches!(app.screen, Screen::Watch | Screen::Settings | Screen::Accounts | Screen::Help);
    let header_h = 3u16;
    let chips_h = if show_chips { 1u16 } else { 0u16 };
    let status_h = 2u16;

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_h),
            Constraint::Length(chips_h),
            Constraint::Min(5),
            Constraint::Length(status_h),
        ])
        .split(size);

    let header_area = main_chunks[0];
    let chips_area = main_chunks[1];
    let body_area = main_chunks[2];
    let status_area = main_chunks[3];

    // Render top header
    header::render_header(f, app, header_area);

    // Render filter chips row
    if show_chips {
        chips::render_chips(f, app, chips_area);
    }

    // Render middle body: sidebar + content
    let sb_width = sidebar::sidebar_width(app.sidebar_expanded);
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(sb_width),
            Constraint::Min(20),
        ])
        .split(body_area);

    let sidebar_area = body_chunks[0];
    let content_area = body_chunks[1];

    sidebar::render_sidebar(f, app, sidebar_area);

    // Concord-style Content Pane with rounded borders and focused highlight
    let is_content_focused = app.focus == Focus::Content;
    let content_border_col = if is_content_focused {
        app.theme.accent
    } else {
        app.theme.border
    };

    let title_prefix = match app.screen {
        Screen::Home => "Home".to_string(),
        Screen::Subscriptions => "Subscriptions".to_string(),
        Screen::History => "History".to_string(),
        Screen::WatchLater => "Watch Later".to_string(),
        Screen::Playlists | Screen::PlaylistView => "Playlists".to_string(),
        Screen::Liked => "Liked Videos".to_string(),
        Screen::LocalVideos => "Local Videos".to_string(),
        Screen::Queue => "Queue".to_string(),
        Screen::Search => {
            if app.search_query.is_empty() {
                "Search".to_string()
            } else {
                format!("Search: '{}'", app.search_query)
            }
        }
        Screen::Settings => "Settings".to_string(),
        Screen::Accounts => "Accounts".to_string(),
        Screen::Watch => "Now Playing".to_string(),
        Screen::Help => "Help".to_string(),
    };

    let total = app.filtered_items().len();
    let view_tag = if app.current_view_mode() == ViewMode::List { " [List] " } else { " [Grid] " };
    let pane_title = if matches!(app.screen, Screen::Settings | Screen::Accounts | Screen::Watch | Screen::Help) {
        format!(" {title_prefix} ")
    } else {
        format!(" {title_prefix} ({total}){view_tag}")
    };

    let content_block = Block::default()
        .borders(Borders::ALL)
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(content_border_col))
        .title(pane_title)
        .style(Style::default().bg(app.theme.bg));

    let inner_content = content_block.inner(content_area);
    f.render_widget(content_block, content_area);

    // Render specific screen content inside Concord pane
    match app.screen {
        Screen::Home
        | Screen::Subscriptions
        | Screen::History
        | Screen::WatchLater
        | Screen::Playlists
        | Screen::Liked
        | Screen::LocalVideos
        | Screen::Queue
        | Screen::Search
        | Screen::PlaylistView => {
            if app.current_view_mode() == ViewMode::List {
                list::render_list(f, app, inner_content);
            } else {
                grid::render_grid(f, app, inner_content);
            }
        }
        Screen::Settings => {
            settings::render_settings(f, app, inner_content);
        }
        Screen::Accounts => {
            accounts::render_accounts(f, app, inner_content);
        }
        Screen::Watch => {
            watch::render_watch(f, app, inner_content);
        }
        Screen::Help => {
            grid::render_grid(f, app, inner_content);
            help::render_help(f, app, size);
        }
    }

    // Render statusbar
    statusbar::render_statusbar(f, app, status_area);

    // Render command line popup if focused
    if app.focus == Focus::CommandLine {
        cmdline::render_cmdline(f, app, size);
    }

    // Render quit confirmation modal
    if app.confirming_quit {
        render_quit_confirmation(f, app, size);
    }
}

fn render_quit_confirmation(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    use ratatui::layout::{Alignment, Constraint, Direction, Layout};
    use ratatui::style::{Modifier, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Paragraph};

    let dialog_w = 44.min(area.width.saturating_sub(4));
    let dialog_h = 7.min(area.height.saturating_sub(2));

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height.saturating_sub(dialog_h)) / 2),
            Constraint::Length(dialog_h),
            Constraint::Min(0),
        ])
        .split(area);

    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length((area.width.saturating_sub(dialog_w)) / 2),
            Constraint::Length(dialog_w),
            Constraint::Min(0),
        ])
        .split(vert[1]);

    let card_area = horiz[1];
    f.render_widget(ratatui::widgets::Clear, card_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_set(ratatui::symbols::border::ROUNDED)
        .border_style(Style::default().fg(app.theme.accent))
        .title(" Quit ytui ")
        .style(Style::default().bg(app.theme.surface));

    let inner = block.inner(card_area);
    f.render_widget(block, card_area);

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("Are you sure you want to quit?", Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("[y] Yes", Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD)),
            Span::raw("        "),
            Span::styled("[n] No", Style::default().fg(app.theme.text_secondary)),
        ]),
    ];

    let p = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .style(Style::default().bg(app.theme.surface));
    f.render_widget(p, inner);
}
