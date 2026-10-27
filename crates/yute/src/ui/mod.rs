pub mod accounts;
pub mod channel;
pub mod chat;
pub mod chips;
pub mod cmdline;
pub mod grid;
pub mod header;
pub mod help;
pub mod list;
pub mod settings;
pub mod sidebar;
pub mod signin;
pub mod statusbar;
pub mod text;
pub mod thumbnail;
pub mod watch;

use crate::app::{App, Focus, Screen, ViewMode};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

pub mod miniplayer;

#[derive(Clone, Copy, Debug)]
pub struct ShellLayout {
    pub header: Rect,
    pub pane: Rect,
    pub sidebar: Rect,
    pub tabs: Rect,
    pub title: Rect,
    pub content: Rect,
    pub player: Rect,
    pub footer: Rect,
}
impl ShellLayout {
    pub fn new(area: Rect, app: &App) -> Self {
        Self::with_playback(area, app, app.player.is_alive())
    }
    /// Pure geometry entrypoint, also usable by offline playback-state previews.
    pub fn with_playback(area: Rect, app: &App, active: bool) -> Self {
        let footer = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
        let player_h = u16::from(active && app.screen != Screen::Watch);
        let player = Rect::new(
            area.x,
            footer.y.saturating_sub(player_h),
            area.width,
            player_h,
        );
        let body = Rect::new(
            area.x,
            area.y + 1,
            area.width,
            area.height.saturating_sub(2 + player_h),
        );
        let sb = sidebar::sidebar_width(app.sidebar_expanded).min(area.width.saturating_sub(20));
        let sidebar = Rect::new(body.x, body.y, sb, body.height);
        let content_x = body.x + sb;
        let width = body.width.saturating_sub(sb);
        let has_tabs = !matches!(
            app.screen,
            Screen::Watch
                | Screen::Settings
                | Screen::Accounts
                | Screen::Help
                | Screen::ChannelView
        );
        let label_width = header::label(app, area.width / 3).width() as u16;
        let tabs_x = area.x + sb.max(label_width + 2).min(area.width);
        let tabs = Rect::new(
            tabs_x,
            area.y,
            area.right().saturating_sub(tabs_x),
            u16::from(has_tabs),
        );
        let pane = Rect::new(content_x, body.y, width, body.height);
        let content = pane.inner(ratatui::layout::Margin::new(1, 1));
        let title = Rect::new(content.x, content.y, content.width, 0);
        Self {
            header: Rect::new(
                area.x,
                area.y,
                if has_tabs {
                    tabs_x - area.x
                } else {
                    area.width
                },
                1,
            ),
            pane,
            sidebar,
            tabs,
            title,
            content,
            player,
            footer,
        }
    }
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();
    app.thumbs.begin_frame();
    app.overlay_area = signin::modal_area(app, size)
        .or_else(|| accounts::picker_area(app, size))
        .or_else(|| super::actions::menu_rect(app, size))
        .or_else(|| cmdline::popup_area(app, size));
    if size.width < 30 || size.height < 10 {
        return;
    }

    let shell = ShellLayout::new(size, app);
    header::render_header(f, app, shell.header);
    sidebar::render_sidebar(f, app, shell.sidebar);
    let focused = app.focus == Focus::Content && app.action_menu.is_none();
    let frame = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(Style::default().fg(if focused {
            app.theme.accent
        } else {
            app.theme.border
        }))
        .title(if matches!(app.screen, Screen::Watch) {
            " Video details "
        } else {
            " Content "
        })
        .title_style(Style::default().fg(if focused {
            app.theme.accent
        } else {
            app.theme.text_secondary
        }));
    f.render_widget(frame, shell.pane);
    if !shell.tabs.is_empty() {
        chips::render_chips(f, app, shell.tabs);
    }
    let content_area = shell.content;
    let status_area = shell.footer;
    let inner_content = content_area;
    app.rendered_columns = if app.current_view_mode() == ViewMode::List || content_area.width < 72 {
        1
    } else {
        grid::column_count(inner_content.width, &app.config.grid)
    };
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
            if app.current_view_mode() == ViewMode::List || inner_content.width < 72 {
                list::render_list(f, app, inner_content);
            } else {
                grid::render_grid(f, app, inner_content);
            }
        }
        Screen::ChannelView => channel::render_channel(f, app, inner_content),
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

    if !shell.player.is_empty() {
        miniplayer::render(f, app, shell.player);
    }

    // Render statusbar
    statusbar::render_statusbar(f, app, status_area);

    // Render command line popup if focused
    if app.which_key || app.focus == Focus::CommandLine || app.command_help.is_some() {
        cmdline::render_cmdline(f, app);
    }

    crate::actions::render_menu(f, app);
    accounts::render_picker(f, app);
    signin::render(f, app);

    let prefetch = app.config.thumbnails.prefetch_pages.min(3);
    let columns = app.rendered_columns.max(1);
    let rows = app.rendered_rows.max(1);
    let start = (app.scroll_row + rows.saturating_sub(1)) * columns;
    let preloads = if app.config.general.graphics != "none" {
        app.filtered_items()
            .into_iter()
            .skip(start)
            .take(rows * columns * prefetch)
            .map(|c| yute_core::media::thumbs::card_thumbnail_url(c).into_owned())
            .collect()
    } else {
        Vec::new()
    };
    let geometry = grid::GridGeometry::new(
        shell.content,
        &app.config.grid,
        app.cell_w_px,
        app.cell_h_px,
    );
    app.thumbs.finish_frame(
        preloads,
        geometry.thumbnail_width as u32 * app.cell_w_px as u32,
        geometry.thumbnail_height as u32 * app.cell_h_px as u32,
    );

    // Render quit confirmation modal
    if app.confirming_quit || app.confirming_logout {
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
        .border_type(ratatui::widgets::BorderType::Plain)
        .border_style(Style::default().fg(app.theme.accent))
        .title(if app.confirming_logout {
            " Sign out "
        } else {
            " Quit yute "
        })
        .style(Style::default().bg(app.theme.surface));

    let inner = block.inner(card_area);
    f.render_widget(block, card_area);

    let lines = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "Are you sure you want to quit?",
            Style::default()
                .fg(app.theme.text)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "[y] Yes",
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("        "),
            Span::styled("[n] No", Style::default().fg(app.theme.text_secondary)),
        ]),
    ];

    let p = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .style(Style::default().bg(app.theme.surface));
    f.render_widget(p, inner);
}
