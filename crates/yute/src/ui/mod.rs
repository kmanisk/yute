pub mod accounts;
pub mod actionbar;
pub mod channel;
pub mod chat;
pub mod chips;
pub mod cmdline;
pub mod grid;
pub mod header;
pub mod help;
pub mod icons;
pub mod list;
pub mod pane;
pub mod search_filters;
pub mod settings;
pub mod sidebar;
pub mod signin;
pub mod statusbar;
pub mod subscription_strip;
pub mod text;
pub mod thumbnail;
pub mod watch;

use crate::app::{App, Focus, Screen, ViewMode};
use crate::ui::text::DisplayWidth;
use ratatui::layout::Rect;
use ratatui::Frame;

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
    pub actions: Rect,
    pub filterbar: Rect,
}
impl ShellLayout {
    pub fn new(area: Rect, app: &App) -> Self {
        Self::with_playback(area, app, app.player.is_alive())
    }
    /// Pure geometry entrypoint, also usable by offline playback-state previews.
    pub fn with_playback(area: Rect, app: &App, active: bool) -> Self {
        let footer = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
        // Keep the footer to at most two rows. During background playback the
        // miniplayer is the first row and status is the second; otherwise the
        // contextual action row can use that first row.
        let action_h = u16::from(
            area.height >= 30
                && app.config.ui.action_bar != "off"
                && !(active && app.screen != Screen::Watch),
        );
        let actions = Rect::new(
            area.x,
            footer.y.saturating_sub(action_h),
            area.width,
            action_h,
        );
        let player_h = u16::from(active && app.screen != Screen::Watch);
        let player = Rect::new(
            area.x,
            actions.y.saturating_sub(player_h),
            area.width,
            player_h,
        );
        let filter_h =
            u16::from(app.screen == Screen::Search && !search_filters::active(app).is_empty());
        let filterbar = Rect::new(area.x, area.y + 1, area.width, filter_h);
        let body = Rect::new(
            area.x,
            area.y + 1 + filter_h,
            area.width,
            area.height
                .saturating_sub(2 + player_h + action_h + filter_h),
        );
        let sb = sidebar::width_for(app).min(area.width.saturating_sub(20));
        let sidebar = Rect::new(body.x, body.y, sb, body.height);
        let gutter = u16::from(sb > 0);
        let content_x = body.x + sb + gutter;
        let width = body.width.saturating_sub(sb + gutter);
        let has_tabs = !matches!(
            app.screen,
            Screen::Watch
                | Screen::Settings
                | Screen::Accounts
                | Screen::Help
                | Screen::ChannelView
        );
        let account_x = header::account_area(app, area).x.saturating_sub(1);
        let label_width = header::label(app, area.width / 3).width() as u16;
        let tabs_x = area.x + (label_width + 2).min(area.width);
        let tabs = Rect::new(
            tabs_x,
            area.y,
            account_x.saturating_sub(tabs_x),
            u16::from(has_tabs),
        );
        let pane = Rect::new(content_x, body.y, width, body.height);
        // Keep card borders inside the content frame; the grid reserves its own
        // scrollbar column so neither can overwrite a card edge.
        let content = pane::Pane::new("", false).inner(pane);
        let title = Rect::new(content.x, content.y, content.width, 0);
        Self {
            filterbar,
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
            actions,
        }
    }
}

/// Set the visible window before deciding which graphics placements to erase.
pub fn prepare_viewport(app: &mut App, size: Rect) {
    if matches!(
        app.screen,
        Screen::Watch | Screen::Settings | Screen::Accounts | Screen::Help
    ) {
        return;
    }
    let mut area = ShellLayout::new(size, app).content;
    let strip = subscription_strip::height(app, area);
    area.y += strip;
    area.height = area.height.saturating_sub(strip);
    let header = match app.screen {
        Screen::PlaylistView => 1,
        Screen::ChannelView => match &app.detail_route {
            Some(crate::app::DetailRoute::Channel(route)) => {
                if route.tabs.is_empty() {
                    1
                } else {
                    2
                }
            }
            _ => 0,
        },
        _ => 0,
    };
    area.y += header.min(area.height);
    area.height = area.height.saturating_sub(header);
    let geometry = grid::GridGeometry::new(area, &app.config.grid, app.cell_w_px, app.cell_h_px);
    if app.current_view_mode() == ViewMode::List
        || area.width < 72
        || geometry.card_height > area.height
    {
        list::prepare_viewport(app, area);
    } else {
        grid::prepare_viewport(app, area);
    }
}

pub fn draw(f: &mut Frame, app: &mut App) {
    text::begin_frame();
    if app.screen == Screen::Watch && app.watch_right_queue {
        app.watch_queue_cards = app
            .player
            .queue
            .items()
            .iter()
            .cloned()
            .map(crate::backend::models::Card::Video)
            .collect();
    }
    let size = f.area();
    f.render_widget(
        ratatui::widgets::Block::default().style(app.theme.style("text")),
        size,
    );
    app.terminal_height = size.height;
    prepare_viewport(app, size);
    app.thumbs.begin_frame();
    app.overlay_area = overlay_rect(app, size);
    if size.width < 30 || size.height < 10 {
        return;
    }

    let shell = ShellLayout::new(size, app);
    header::render_header(f, app, shell.header);
    sidebar::render_sidebar(f, app, shell.sidebar);
    let focused = app.focus == Focus::Content && app.action_menu.is_none();
    pane::Pane::new(
        if app.screen == Screen::Watch {
            "Watch"
        } else {
            ""
        },
        focused,
    )
    .render(f, shell.pane, &app.theme);
    if !shell.tabs.is_empty() {
        chips::render_chips(f, app, shell.tabs);
    }
    if !shell.filterbar.is_empty() {
        f.render_widget(
            ratatui::widgets::Paragraph::new(text::truncate_str(
                &search_filters::active(app),
                shell.filterbar.width as usize,
            ))
            .style(app.theme.style("text_dim")),
            shell.filterbar,
        );
    }
    let content_area = shell.content;
    let status_area = shell.footer;
    let mut inner_content = content_area;
    let strip = subscription_strip::height(app, inner_content);
    if strip > 0 {
        subscription_strip::render(
            f,
            app,
            Rect::new(inner_content.x, inner_content.y, inner_content.width, strip),
        );
        inner_content.y += strip;
        inner_content.height = inner_content.height.saturating_sub(strip);
    }
    if app.screen == Screen::PlaylistView {
        if let Some(crate::app::DetailRoute::Playlist(p)) = &app.detail_route {
            let text = format!(
                "{} · {} · {} · p play all",
                p.title,
                p.channel,
                p.video_count.as_deref().unwrap_or("Playlist")
            );
            f.render_widget(
                ratatui::widgets::Paragraph::new(text).style(app.theme.style("text_dim")),
                Rect::new(inner_content.x, inner_content.y, inner_content.width, 1),
            );
            inner_content.y += 1;
            inner_content.height = inner_content.height.saturating_sub(1);
        }
    }
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

    actionbar::render(f, app, shell.actions);
    // Render statusbar
    statusbar::render_statusbar(f, app, status_area);

    // Render command line popup if focused
    if app.which_key || app.focus == Focus::CommandLine || app.command_help.is_some() {
        cmdline::render_cmdline(f, app);
    }

    search_filters::render(f, app);
    crate::actions::render_menu(f, app);
    accounts::render_picker(f, app);
    signin::render(f, app);
    settings::render_editor(f, app);

    let prefetch = app.config.thumbnails.prefetch_pages.min(3);
    let columns = app.rendered_columns.max(1);
    let rows = app.rendered_rows.max(1);
    let start = (app.scroll_row + rows) * columns;
    let preloads = if crate::terminal::images_enabled(&app.config.general.graphics) {
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
        columns,
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
        .border_style(app.theme.style("accent"))
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
            app.theme.style("text").add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "[y] Yes",
                app.theme.style("accent").add_modifier(Modifier::BOLD),
            ),
            Span::raw("        "),
            Span::styled("[n] No", app.theme.style("text_dim")),
        ]),
    ];

    let p = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .style(Style::default().bg(app.theme.surface));
    f.render_widget(p, inner);
}

pub fn overlay_rect(app: &App, size: Rect) -> Option<Rect> {
    if app.search_modal.is_some() {
        return Some(search_filters::rect(size));
    }
    if app.confirming_quit || app.confirming_logout {
        return Some(Rect::new(
            size.x + (size.width - size.width.saturating_sub(4).min(44)) / 2,
            size.y + (size.height - size.height.saturating_sub(2).min(7)) / 2,
            size.width.saturating_sub(4).min(44),
            size.height.saturating_sub(2).min(7),
        ));
    }
    if app.screen == Screen::Help {
        return Some(size);
    }
    settings::editor_area(app, size)
        .or_else(|| signin::modal_area(app, size))
        .or_else(|| accounts::picker_area(app, size))
        .or_else(|| crate::actions::menu_rect(app, size))
        .or_else(|| cmdline::popup_area(app, size))
}
