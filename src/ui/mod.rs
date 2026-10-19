pub mod chat;
pub mod chips;
pub mod cmdline;
pub mod grid;
pub mod header;
pub mod help;
pub mod list;
pub mod sidebar;
pub mod statusbar;
pub mod watch;

use crate::app::{App, Focus, Screen};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::Frame;

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();
    if size.width < 30 || size.height < 10 {
        return;
    }

    let has_chips = app.screen == Screen::Home && !app.chips.is_empty();
    let header_h = 2u16;
    let chips_h = if has_chips { 1u16 } else { 0u16 };
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

    // Render chips if on Home
    if has_chips {
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

    // Render specific screen content
    match app.screen {
        Screen::Home | Screen::Subscriptions | Screen::History | Screen::WatchLater
        | Screen::Playlists | Screen::Liked | Screen::LocalVideos | Screen::Queue => {
            grid::render_grid(f, app, content_area);
        }
        Screen::Search => {
            list::render_list(f, app, content_area);
        }
        Screen::Watch => {
            watch::render_watch(f, app, content_area);
        }
        Screen::Help => {
            grid::render_grid(f, app, content_area);
            help::render_help(f, app, size);
        }
    }

    // Render statusbar
    statusbar::render_statusbar(f, app, status_area);

    // Render command line popup if focused
    if app.focus == Focus::CommandLine {
        cmdline::render_cmdline(f, app, size);
    }
}
