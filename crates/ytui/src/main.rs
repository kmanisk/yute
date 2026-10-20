#![allow(dead_code)]

use clap::{Parser, Subcommand};
use crossterm::event::{
    Event, EventStream, KeyCode, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use futures_util::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::stdout;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{channel, Sender};
use ytui::app::{App, Focus, Screen};
use ytui::backend::models::{Card, ChannelTab, LiveState, SearchFilters, VideoCard};
use ytui::backend::Backend;
use ytui::config::Config;
use ytui::events::AppEvent;
use ytui::keymap::{Action, KeymapState};
use ytui::local::scan_local_videos;
use ytui::player::ipc::IpcCommand;
use ytui::player::mpvconf::MpvProfile;
use ytui::player::PlaybackMode;
use ytui::ui;

#[derive(Parser, Debug)]
#[command(
    name = "ytui",
    about = "Fast, keyboard-driven YouTube terminal client in Rust"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Dump raw JSON for a given screen to cache/debug/
    #[arg(long)]
    dump_raw: Option<String>,

    /// Enable verbose file logging
    #[arg(long)]
    log: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Interactive YouTube device-code login (enter code at google.com/device or youtube.com/activate)
    Login,
    /// Spike: Fetch and dump raw TVHTML5 JSON for browse/home, next, and search
    DumpTv {
        /// Search query to test
        #[arg(default_value = "gaming")]
        query: String,
    },
    /// Download recommended mpv plugins into dedicated profile
    SetupMpv,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if let Some(Commands::SetupMpv) = cli.command {
        let profile = MpvProfile::new();
        profile.install_recommended_scripts().await?;
        return Ok(());
    }

    if let Some(Commands::Login) = cli.command {
        return run_login().await;
    }

    if let Some(Commands::DumpTv { query }) = cli.command {
        return run_dump_tv(&query).await;
    }

    if let Some(ref screen_or_q) = cli.dump_raw {
        return run_dump_tv(screen_or_q).await;
    }

    let config = Config::load();

    // Query terminal capabilities for sixel/kitty/halfblocks
    let picker = ratatui_image::picker::Picker::from_query_stdio()
        .unwrap_or_else(|_| ratatui_image::picker::Picker::halfblocks());

    // Setup terminal with keyboard enhancement for Shift+Enter support
    enable_raw_mode()?;
    let mut out = stdout();
    let _ = execute!(
        out,
        EnterAlternateScreen,
        crossterm::cursor::Hide,
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
        )
    );
    let term_backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(term_backend)?;

    // Setup panic hook to cleanly restore terminal
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = execute!(
            stdout(),
            PopKeyboardEnhancementFlags,
            LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        let _ = disable_raw_mode();
        orig_hook(panic_info);
    }));

    let mut app = App::new(config, picker);
    let (tx_event, mut rx_event) = channel::<AppEvent>(64);

    // Initial load: load disk cache immediately for fast launch, then fetch fresh
    try_load_feed_cache(&mut app, "feed_home");
    if app.items.is_empty() {
        app.is_loading = true;
    }
    trigger_home_fetch(&app, tx_event.clone(), None);

    // Prune cache in background to prevent disk bloat
    let cache_clone = app.thumbs.cache().clone();
    tokio::task::spawn_blocking(move || {
        let _ = cache_clone.prune_thumbnails();
        let _ = cache_clone.prune_feeds(3);
    });

    // Fetch authenticated account profile info in background
    let backend_acc = Arc::clone(&app.backend);
    let tx_acc = tx_event.clone();
    tokio::spawn(async move {
        let res = backend_acc.account_info().await;
        let _ = tx_acc.send(AppEvent::AccountLoaded(res)).await;
    });

    let mut keymap = KeymapState::new();
    let mut reader = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(100));

    while !app.should_quit {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        tokio::select! {
            _ = ticker.tick() => {
                if app.tick() {
                    // Triggers redraw on next loop
                }
            }

            event_opt = reader.next() => {
                if let Some(Ok(event)) = event_opt {
                    match event {
                        Event::Key(key) => {
                            if key.kind != crossterm::event::KeyEventKind::Press {
                                continue;
                            }
                            handle_key_event(&mut app, &mut keymap, key, tx_event.clone()).await;
                        }
                        Event::Resize(w, h) => {
                            let _ = tx_event.send(AppEvent::Resize(w, h)).await;
                        }
                        _ => {}
                    }
                }
            }

            app_event_opt = rx_event.recv() => {
                if let Some(app_event) = app_event_opt {
                    handle_app_event(&mut app, app_event, tx_event.clone());
                }
            }
        }
    }

    // Clean exit
    if app.config.general.stop_mpv_on_quit {
        app.player.stop();
    }
    let _ = app.thumbs.cache().prune_thumbnails();
    let _ = app.thumbs.cache().prune_feeds(3);
    let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen, crossterm::cursor::Show)?;

    Ok(())
}

fn trigger_home_fetch(app: &App, tx: Sender<AppEvent>, chip_token: Option<String>) {
    let backend = Arc::clone(&app.backend);
    tokio::spawn(async move {
        let res = backend.home(chip_token.as_deref()).await;
        let _ = tx.send(AppEvent::FeedLoaded(res)).await;
    });
}

fn trigger_search_fetch(app: &App, tx: Sender<AppEvent>, query: String) {
    let backend = Arc::clone(&app.backend);
    tokio::spawn(async move {
        let res = backend
            .search(&query, &SearchFilters::default(), None)
            .await;
        let _ = tx.send(AppEvent::FeedLoaded(res)).await;
    });
}

fn trigger_watch_fetch(app: &App, tx: Sender<AppEvent>, video_id: String) {
    let backend = Arc::clone(&app.backend);
    tokio::spawn(async move {
        let res = backend.watch(&video_id).await;
        let _ = tx.send(AppEvent::WatchDataLoaded(res)).await;
    });
}

async fn handle_key_event(
    app: &mut App,
    keymap: &mut KeymapState,
    key: crossterm::event::KeyEvent,
    tx: Sender<AppEvent>,
) {
    // If confirmation modal is open
    if app.confirming_quit {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                app.player.stop();
                app.should_quit = true;
            }
            KeyCode::Char('n')
            | KeyCode::Char('N')
            | KeyCode::Char('q')
            | KeyCode::Char('Q')
            | KeyCode::Esc => {
                app.confirming_quit = false;
            }
            _ => {}
        }
        return;
    }

    // If focused on command line
    if app.focus == Focus::CommandLine {
        match key.code {
            KeyCode::Enter => {
                execute_command(app, tx.clone());
                app.focus = Focus::Content;
                app.cmd_input.clear();
            }
            KeyCode::Esc => {
                app.focus = Focus::Content;
                app.cmd_input.clear();
            }
            KeyCode::Backspace => {
                app.cmd_input.pop();
            }
            KeyCode::Char(c) => {
                app.cmd_input.push(c);
            }
            _ => {}
        }
        return;
    }

    // If focused on search input
    if app.focus == Focus::SearchInput {
        match key.code {
            KeyCode::Enter => {
                let q = app.search_query.trim().to_string();
                if !q.is_empty() {
                    app.filter_text.clear();
                    app.selected_index = 0;
                    app.items.clear();
                    app.set_screen(Screen::Search);
                    app.focus = Focus::Content;
                    app.is_loading = true;
                    app.status_text = format!("Searching for '{}'...", q);
                    trigger_search_fetch(app, tx.clone(), q);
                }
            }
            KeyCode::Esc => {
                app.focus = Focus::Content;
            }
            KeyCode::Backspace => {
                app.search_query.pop();
            }
            KeyCode::Char('u')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                app.search_query.clear();
            }
            KeyCode::Char('w')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                if let Some(idx) = app.search_query.trim_end().rfind(' ') {
                    app.search_query.truncate(idx + 1);
                } else {
                    app.search_query.clear();
                }
            }
            KeyCode::Char('c')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                app.focus = Focus::Content;
            }
            KeyCode::Char(c) => {
                if !key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(crossterm::event::KeyModifiers::ALT)
                {
                    app.search_query.push(c);
                }
            }
            _ => {}
        }
        return;
    }

    let in_watch = app.screen == Screen::Watch;
    let Some((action, count)) = keymap.handle_key(key, in_watch) else {
        return;
    };

    for _ in 0..count {
        if app.screen == Screen::Settings {
            match action {
                Action::Up => {
                    app.settings_index = app.settings_index.saturating_sub(1);
                }
                Action::Down => {
                    app.settings_index = (app.settings_index + 1)
                        .min(ui::settings::SETTINGS_ITEMS.len().saturating_sub(1));
                }
                Action::Left => {
                    ui::settings::cycle_setting(app, false);
                }
                Action::Right | Action::PlayDefault => {
                    ui::settings::cycle_setting(app, true);
                }
                Action::Back | Action::Quit => {
                    app.nav_back();
                    ensure_screen_loaded(app, tx.clone());
                }
                Action::ForceQuit => {
                    app.should_quit = true;
                }
                Action::NavHome => {
                    app.sidebar_index = 0;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::NavSubs => {
                    app.sidebar_index = 1;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::NavHistory => {
                    app.sidebar_index = 2;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::NavPlaylists => {
                    app.sidebar_index = 3;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::NavWatchLater => {
                    app.sidebar_index = 4;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::NavLiked => {
                    app.sidebar_index = 5;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::NavLocal => {
                    app.sidebar_index = 6;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::NavQueue => {
                    app.sidebar_index = 7;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::NavAccounts => {
                    app.sidebar_index = 9;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::FocusSearch => {
                    app.focus = Focus::SearchInput;
                }
                Action::ToggleSidebar => {
                    app.focus = Focus::Sidebar;
                }
                _ => {}
            }
            return;
        }

        if app.screen == Screen::Accounts {
            match action {
                Action::Up => {
                    app.accounts_cursor = app.accounts_cursor.saturating_sub(1);
                }
                Action::Down => {
                    app.accounts_cursor =
                        (app.accounts_cursor + 1).min(app.accounts.len().saturating_sub(1));
                }
                Action::PlayDefault | Action::Right => {
                    app.switch_account(app.accounts_cursor);
                    app.status_text = format!("Switched to account '{}'", app.account_name);
                    app.sidebar_index = 0;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::Back | Action::Quit => {
                    app.nav_back();
                    ensure_screen_loaded(app, tx.clone());
                }
                Action::ForceQuit => {
                    app.should_quit = true;
                }
                Action::NavHome => {
                    app.sidebar_index = 0;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::NavSubs => {
                    app.sidebar_index = 1;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::NavHistory => {
                    app.sidebar_index = 2;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::NavPlaylists => {
                    app.sidebar_index = 3;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::NavWatchLater => {
                    app.sidebar_index = 4;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::NavLiked => {
                    app.sidebar_index = 5;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::NavLocal => {
                    app.sidebar_index = 6;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::NavQueue => {
                    app.sidebar_index = 7;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::NavSettings => {
                    app.sidebar_index = 8;
                    activate_sidebar_item(app, tx.clone());
                }
                Action::FocusSearch => {
                    app.focus = Focus::SearchInput;
                }
                Action::ToggleSidebar => {
                    app.focus = Focus::Sidebar;
                }
                _ => {}
            }
            return;
        }

        let grid_cols = if app.config.grid.columns > 0 {
            app.config.grid.columns as usize
        } else {
            3
        };

        match action {
            Action::Quit => {
                app.confirming_quit = true;
            }
            Action::Back => {
                if app.screen == Screen::Watch {
                    app.player.stop_playback();
                    app.nav_back();
                    ensure_screen_loaded(app, tx.clone());
                } else if app.focus == Focus::Sidebar {
                    app.focus = Focus::Content;
                } else {
                    app.nav_back();
                    ensure_screen_loaded(app, tx.clone());
                }
            }
            Action::ForceQuit => {
                app.player.stop();
                app.should_quit = true;
            }
            Action::Up => {
                if app.screen == Screen::Watch {
                    if app.watch_right_mode == ytui::app::WatchRightMode::Comments {
                        app.comments_cursor = app.comments_cursor.saturating_sub(1);
                    }
                } else if app.focus == Focus::Sidebar {
                    app.sidebar_index = app.sidebar_index.saturating_sub(1);
                } else {
                    app.selected_index = app.selected_index.saturating_sub(grid_cols);
                }
            }
            Action::Down => {
                if app.screen == Screen::Watch {
                    if app.watch_right_mode == ytui::app::WatchRightMode::Comments {
                        let total_comments = app
                            .watch_data
                            .as_ref()
                            .map(|w| w.comments.len())
                            .unwrap_or(0);
                        if total_comments > 0 {
                            app.comments_cursor =
                                (app.comments_cursor + 1).min(total_comments.saturating_sub(1));
                        }
                    }
                } else if app.focus == Focus::Sidebar {
                    app.sidebar_index = (app.sidebar_index + 1)
                        .min(ui::sidebar::SIDEBAR_ITEMS.len().saturating_sub(1));
                } else {
                    let total = app.filtered_items().len();
                    if total > 0 {
                        app.selected_index = (app.selected_index + grid_cols).min(total - 1);
                    }
                }
            }
            Action::Left => {
                if app.focus == Focus::Sidebar {
                    // Already in sidebar
                } else {
                    if app.items.is_empty() || app.selected_index % grid_cols == 0 {
                        app.focus = Focus::Sidebar;
                    } else {
                        app.selected_index = app.selected_index.saturating_sub(1);
                    }
                }
            }
            Action::Right => {
                if app.focus == Focus::Sidebar {
                    activate_sidebar_item(app, tx.clone());
                } else {
                    let total = app.filtered_items().len();
                    if total > 0 {
                        app.selected_index = (app.selected_index + 1).min(total - 1);
                    }
                }
            }
            Action::First => {
                if app.focus == Focus::Sidebar {
                    app.sidebar_index = 0;
                } else {
                    app.selected_index = 0;
                }
            }
            Action::Last => {
                if app.focus == Focus::Sidebar {
                    app.sidebar_index = ui::sidebar::SIDEBAR_ITEMS.len().saturating_sub(1);
                } else {
                    let total = app.filtered_items().len();
                    if total > 0 {
                        app.selected_index = total - 1;
                    }
                }
            }
            Action::CycleFocusForward => {
                app.focus = match app.focus {
                    Focus::Sidebar => Focus::Chips,
                    Focus::Chips => Focus::Content,
                    Focus::Content => Focus::SearchInput,
                    Focus::SearchInput => Focus::Sidebar,
                    _ => Focus::Content,
                };
            }
            Action::CycleFocusBackward => {
                app.focus = match app.focus {
                    Focus::Sidebar => Focus::SearchInput,
                    Focus::SearchInput => Focus::Content,
                    Focus::Content => Focus::Chips,
                    Focus::Chips => Focus::Sidebar,
                    _ => Focus::Content,
                };
            }
            Action::ToggleSidebar => {
                if app.focus == Focus::Sidebar {
                    app.sidebar_expanded = false;
                    app.focus = Focus::Content;
                } else {
                    app.sidebar_expanded = true;
                    app.focus = Focus::Sidebar;
                }
            }
            Action::PrevChip => {
                if !app.chips.is_empty() {
                    app.active_chip_index = app.active_chip_index.saturating_sub(1);
                    switch_chip(app, tx.clone());
                }
            }
            Action::NextChip => {
                if !app.chips.is_empty() {
                    app.active_chip_index = (app.active_chip_index + 1).min(app.chips.len() - 1);
                    switch_chip(app, tx.clone());
                }
            }
            Action::FocusSearch => {
                app.focus = Focus::SearchInput;
            }
            Action::Filter => {
                app.cycle_filter_kind();
            }
            Action::ToggleViewMode => {
                app.toggle_view_mode();
                app.status_text = format!("View: {:?}", app.current_view_mode());
            }
            Action::CommandLine => {
                app.focus = Focus::CommandLine;
                app.cmd_input.clear();
            }
            Action::Help => {
                if app.screen == Screen::Help {
                    app.nav_back();
                } else {
                    app.set_screen(Screen::Help);
                }
            }
            Action::Refresh => {
                app.is_loading = true;
                app.status_text = "Refreshing feed...".to_string();
                let _ = app.thumbs.cache().clear();
                trigger_home_fetch(app, tx.clone(), None);
            }
            Action::NavHome => {
                app.set_screen(Screen::Home);
                app.sidebar_index = 0;
                try_load_feed_cache(app, "feed_home");
                app.is_loading = true;
                trigger_home_fetch(app, tx.clone(), None);
            }
            Action::NavSubs => {
                app.set_screen(Screen::Subscriptions);
                app.sidebar_index = 1;
                try_load_feed_cache(app, "feed_subs");
                app.is_loading = true;
                app.status_text = "Loading Subscriptions...".to_string();
                let backend = Arc::clone(&app.backend);
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = backend.subscriptions().await;
                    let _ = tx_c.send(AppEvent::FeedLoaded(res)).await;
                });
            }
            Action::NavHistory => {
                app.set_screen(Screen::History);
                app.sidebar_index = 2;
                try_load_feed_cache(app, "feed_history");
                app.is_loading = true;
                app.status_text = "Loading History...".to_string();
                let backend = Arc::clone(&app.backend);
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = backend.history().await;
                    let _ = tx_c.send(AppEvent::FeedLoaded(res)).await;
                });
            }
            Action::NavWatchLater => {
                app.set_screen(Screen::WatchLater);
                app.sidebar_index = 4;
                try_load_feed_cache(app, "feed_watchlater");
                app.is_loading = true;
                app.status_text = "Loading Watch Later...".to_string();
                let backend = Arc::clone(&app.backend);
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = backend.watch_later().await;
                    let _ = tx_c.send(AppEvent::FeedLoaded(res)).await;
                });
            }
            Action::NavPlaylists => {
                app.set_screen(Screen::Playlists);
                app.sidebar_index = 3;
                try_load_feed_cache(app, "feed_playlists");
                app.is_loading = true;
                app.status_text = "Loading Playlists...".to_string();
                let backend = Arc::clone(&app.backend);
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = backend.playlists().await;
                    let _ = tx_c.send(AppEvent::FeedLoaded(res)).await;
                });
            }
            Action::NavLocal => {
                app.set_screen(Screen::LocalVideos);
                app.sidebar_index = 6;
                let page = scan_local_videos(&app.config.local.dirs);
                app.items = page.items;
                app.status_text = format!("Loaded {} local videos", app.items.len());
            }
            Action::NavLiked => {
                app.set_screen(Screen::Liked);
                app.sidebar_index = 5;
                try_load_feed_cache(app, "feed_liked");
                app.is_loading = true;
                app.status_text = "Loading Liked Videos...".to_string();
                let backend = Arc::clone(&app.backend);
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = backend.playlist("LL").await;
                    let _ = tx_c.send(AppEvent::FeedLoaded(res)).await;
                });
            }
            Action::NavQueue => {
                app.set_screen(Screen::Queue);
                app.sidebar_index = 7;
                app.items = app
                    .player
                    .queue
                    .items()
                    .iter()
                    .cloned()
                    .map(Card::Video)
                    .collect();
                app.status_text = format!("Queue: {} items", app.items.len());
            }
            Action::NavSettings => {
                app.set_screen(Screen::Settings);
                app.sidebar_index = 8;
                app.status_text =
                    "Settings: [j/k] Navigate  [h/l/Enter] Change  [q/Esc] Back".to_string();
            }
            Action::NavAccounts => {
                app.set_screen(Screen::Accounts);
                app.sidebar_index = 9;
                app.accounts_cursor = app.active_account_index;
                app.status_text =
                    "Accounts: [j/k] Navigate  [Enter/Space] Switch Account  [q/Esc] Back"
                        .to_string();
            }
            Action::PlayDefault => {
                if app.focus == Focus::Sidebar {
                    activate_sidebar_item(app, tx.clone());
                } else {
                    play_selected_card(app, None, tx.clone());
                }
            }
            Action::PlayInline => {
                play_selected_card(app, Some(PlaybackMode::Inline), tx.clone());
            }
            Action::PlayWindow => {
                play_selected_card(app, Some(PlaybackMode::Window), tx.clone());
            }
            Action::PauseResume => {
                app.player.send_command(IpcCommand::TogglePause);
            }
            Action::SeekBack5 => {
                app.player.send_command(IpcCommand::Seek(-5.0, true));
            }
            Action::SeekForward5 => {
                app.player.send_command(IpcCommand::Seek(5.0, true));
            }
            Action::SeekBack30 => {
                app.player.send_command(IpcCommand::Seek(-30.0, true));
            }
            Action::SeekForward30 => {
                app.player.send_command(IpcCommand::Seek(30.0, true));
            }
            Action::VolumeUp => {
                app.player.send_command(IpcCommand::AdjustVolume(5.0));
            }
            Action::VolumeDown => {
                app.player.send_command(IpcCommand::AdjustVolume(-5.0));
            }
            Action::SpeedUp => {
                app.player.send_command(IpcCommand::AdjustSpeed(0.25));
            }
            Action::SpeedDown => {
                app.player.send_command(IpcCommand::AdjustSpeed(-0.25));
            }
            Action::ToggleDescription => {
                app.show_description = !app.show_description;
            }
            Action::ToggleFullscreen => {
                app.player.send_command(IpcCommand::ToggleFullscreen);
            }
            Action::NavBackHistory => {
                if app.screen == Screen::Watch {
                    app.player.stop_playback();
                }
                app.nav_back();
                ensure_screen_loaded(app, tx.clone());
            }
            Action::NavForwardHistory => {
                app.nav_forward();
                ensure_screen_loaded(app, tx.clone());
            }
            Action::ToggleStats => {
                app.player.toggle_stats();
                app.status_text = "Toggled Stats for Nerds overlay".to_string();
            }
            Action::StopPlayback => {
                app.player.stop_playback();
            }
            Action::LikeVideo => {
                if let Some(ref current) = app.player.current_video {
                    let vid = current.id.clone();
                    let backend = app.backend.clone();
                    let tx_cloned = tx.clone();
                    let is_liked = app
                        .watch_data
                        .as_ref()
                        .and_then(|w| w.is_liked)
                        .unwrap_or(false);
                    if let Some(ref mut wd) = app.watch_data {
                        if is_liked {
                            wd.is_liked = Some(false);
                            app.status_text = "Removing like...".to_string();
                            tokio::spawn(async move {
                                let _ = backend.remove_rating(&vid).await;
                                let _ = tx_cloned
                                    .send(AppEvent::StatusMessage("Removed rating".to_string()))
                                    .await;
                            });
                        } else {
                            wd.is_liked = Some(true);
                            wd.is_disliked = Some(false);
                            app.status_text = "Liking video...".to_string();
                            tokio::spawn(async move {
                                let _ = backend.like_video(&vid).await;
                                let _ = tx_cloned
                                    .send(AppEvent::StatusMessage("Liked video".to_string()))
                                    .await;
                            });
                        }
                    }
                }
            }
            Action::DislikeVideo => {
                if let Some(ref current) = app.player.current_video {
                    let vid = current.id.clone();
                    let backend = app.backend.clone();
                    let tx_cloned = tx.clone();
                    let is_disliked = app
                        .watch_data
                        .as_ref()
                        .and_then(|w| w.is_disliked)
                        .unwrap_or(false);
                    if let Some(ref mut wd) = app.watch_data {
                        if is_disliked {
                            wd.is_disliked = Some(false);
                            app.status_text = "Removing dislike...".to_string();
                            tokio::spawn(async move {
                                let _ = backend.remove_rating(&vid).await;
                                let _ = tx_cloned
                                    .send(AppEvent::StatusMessage("Removed rating".to_string()))
                                    .await;
                            });
                        } else {
                            wd.is_disliked = Some(true);
                            wd.is_liked = Some(false);
                            app.status_text = "Disliking video...".to_string();
                            tokio::spawn(async move {
                                let _ = backend.dislike_video(&vid).await;
                                let _ = tx_cloned
                                    .send(AppEvent::StatusMessage("Disliked video".to_string()))
                                    .await;
                            });
                        }
                    }
                }
            }
            Action::ToggleSubscribe => {
                let channel_id = app
                    .watch_data
                    .as_ref()
                    .and_then(|w| w.video.channel_id.clone())
                    .or_else(|| {
                        app.player
                            .current_video
                            .as_ref()
                            .and_then(|v| v.channel_id.clone())
                    });
                if let Some(cid) = channel_id {
                    let backend = app.backend.clone();
                    let tx_cloned = tx.clone();
                    let is_sub = app
                        .watch_data
                        .as_ref()
                        .and_then(|w| w.is_subscribed)
                        .unwrap_or(false);
                    if let Some(ref mut wd) = app.watch_data {
                        if is_sub {
                            wd.is_subscribed = Some(false);
                            app.status_text = "Unsubscribing from channel...".to_string();
                            tokio::spawn(async move {
                                let _ = backend.unsubscribe_channel(&cid).await;
                                let _ = tx_cloned
                                    .send(AppEvent::StatusMessage(
                                        "Unsubscribed from channel".to_string(),
                                    ))
                                    .await;
                            });
                        } else {
                            wd.is_subscribed = Some(true);
                            app.status_text = "Subscribing to channel...".to_string();
                            tokio::spawn(async move {
                                let _ = backend.subscribe_channel(&cid).await;
                                let _ = tx_cloned
                                    .send(AppEvent::StatusMessage(
                                        "Subscribed to channel".to_string(),
                                    ))
                                    .await;
                            });
                        }
                    }
                } else {
                    app.status_text = "Channel ID not found for subscription".to_string();
                }
            }
            Action::ToggleComments => {
                app.watch_right_mode = match app.watch_right_mode {
                    ytui::app::WatchRightMode::UpNext => {
                        if let Some(ref wd) = app.watch_data {
                            if wd.comments.is_empty() {
                                if let Some(ref c_tok) = wd.comments_token {
                                    let c_tok = c_tok.clone();
                                    let backend = app.backend.clone();
                                    let tx_cloned = tx.clone();
                                    app.status_text = "Loading comments...".to_string();
                                    tokio::spawn(async move {
                                        let res = backend.fetch_comments(&c_tok).await;
                                        let _ = tx_cloned.send(AppEvent::CommentsLoaded(res)).await;
                                    });
                                }
                            }
                        }
                        ytui::app::WatchRightMode::Comments
                    }
                    ytui::app::WatchRightMode::Comments => ytui::app::WatchRightMode::UpNext,
                };
            }
            Action::MarkWatched => {
                if let Some(card) = app.selected_card() {
                    let vid = card.id().to_string();
                    let title = card.title().to_string();
                    let backend = Arc::clone(&app.backend);
                    tokio::spawn(async move {
                        let _ = backend.mark_watched(&vid).await;
                    });
                    app.status_text = format!("Marked '{}' as watched in YouTube history", title);
                }
            }
            _ => {}
        }
    }
}

fn activate_sidebar_item(app: &mut App, tx: Sender<AppEvent>) {
    if app.screen == Screen::Watch {
        app.player.stop_playback();
    }
    app.focus = Focus::Content;
    match app.sidebar_index {
        0 => {
            app.set_screen(Screen::Home);
            try_load_feed_cache(app, "feed_home");
            app.is_loading = true;
            app.status_text = "Loading Home...".to_string();
            trigger_home_fetch(app, tx, None);
        }
        1 => {
            app.set_screen(Screen::Subscriptions);
            try_load_feed_cache(app, "feed_subs");
            app.is_loading = true;
            app.status_text = "Loading Subscriptions...".to_string();
            let backend = Arc::clone(&app.backend);
            tokio::spawn(async move {
                let res = backend.subscriptions().await;
                let _ = tx.send(AppEvent::FeedLoaded(res)).await;
            });
        }
        2 => {
            app.set_screen(Screen::History);
            try_load_feed_cache(app, "feed_history");
            app.is_loading = true;
            app.status_text = "Loading History...".to_string();
            let backend = Arc::clone(&app.backend);
            tokio::spawn(async move {
                let res = backend.history().await;
                let _ = tx.send(AppEvent::FeedLoaded(res)).await;
            });
        }
        3 => {
            app.set_screen(Screen::Playlists);
            try_load_feed_cache(app, "feed_playlists");
            app.is_loading = true;
            app.status_text = "Loading Playlists...".to_string();
            let backend = Arc::clone(&app.backend);
            tokio::spawn(async move {
                let res = backend.playlists().await;
                let _ = tx.send(AppEvent::FeedLoaded(res)).await;
            });
        }
        4 => {
            app.set_screen(Screen::WatchLater);
            try_load_feed_cache(app, "feed_watchlater");
            app.is_loading = true;
            app.status_text = "Loading Watch Later...".to_string();
            let backend = Arc::clone(&app.backend);
            tokio::spawn(async move {
                let res = backend.watch_later().await;
                let _ = tx.send(AppEvent::FeedLoaded(res)).await;
            });
        }
        5 => {
            app.set_screen(Screen::Liked);
            try_load_feed_cache(app, "feed_liked");
            app.is_loading = true;
            app.status_text = "Loading Liked Videos...".to_string();
            let backend = Arc::clone(&app.backend);
            tokio::spawn(async move {
                let res = backend.playlist("LL").await;
                let _ = tx.send(AppEvent::FeedLoaded(res)).await;
            });
        }
        6 => {
            app.set_screen(Screen::LocalVideos);
            let page = scan_local_videos(&app.config.local.dirs);
            app.items = page.items;
            app.status_text = format!("Loaded {} local videos", app.items.len());
        }
        7 => {
            app.set_screen(Screen::Queue);
            app.items = app
                .player
                .queue
                .items()
                .iter()
                .cloned()
                .map(Card::Video)
                .collect();
            app.status_text = format!("Queue: {} items", app.items.len());
        }
        8 => {
            app.set_screen(Screen::Settings);
            app.sidebar_index = 8;
            app.status_text =
                "Settings: [j/k] Navigate  [h/l/Enter] Change  [q/Esc] Back".to_string();
        }
        9 => {
            app.set_screen(Screen::Accounts);
            app.sidebar_index = 9;
            app.accounts_cursor = app.active_account_index;
            app.status_text =
                "Accounts: [j/k] Navigate  [Enter/Space] Switch Account  [q/Esc] Back".to_string();
        }
        _ => {}
    }
}

fn switch_chip(app: &mut App, tx: Sender<AppEvent>) {
    if let Some(chip) = app.chips.get(app.active_chip_index) {
        let token = chip.token.clone();
        app.is_loading = true;
        app.status_text = format!("Loading chip: {}...", chip.title);
        trigger_home_fetch(app, tx, token);
    }
}

fn play_selected_card(app: &mut App, mode: Option<PlaybackMode>, tx: Sender<AppEvent>) {
    let Some(card) = app.selected_card().cloned() else {
        return;
    };

    match card {
        Card::Video(v) => {
            let id = v.id.clone();
            app.status_text = format!("Launching '{}'...", v.title);
            if mode == Some(PlaybackMode::Inline) {
                let _ = crossterm::execute!(
                    std::io::stdout(),
                    crossterm::event::PopKeyboardEnhancementFlags,
                    crossterm::terminal::LeaveAlternateScreen,
                    crossterm::cursor::Show
                );
                let _ = crossterm::terminal::disable_raw_mode();
                let _ = app.player.play_video(v, mode, &app.config);
                let _ = crossterm::terminal::enable_raw_mode();
                let _ = crossterm::execute!(
                    std::io::stdout(),
                    crossterm::terminal::EnterAlternateScreen,
                    crossterm::cursor::Hide,
                    crossterm::event::PushKeyboardEnhancementFlags(
                        crossterm::event::KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                            | crossterm::event::KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                    )
                );
                app.status_text = "Inline playback finished".to_string();
            } else {
                let _ = app.player.play_video(v, mode, &app.config);
                app.watch_tracker = Some(ytui_core::player::tracking::WatchTracker::new(&id));
                app.set_screen(Screen::Watch);
                trigger_watch_fetch(app, tx, id);
            }
        }
        Card::Playlist(p) => {
            if p.is_mix {
                let seed_id = p.first_video_id.as_deref().or_else(|| {
                    if p.id.starts_with("RDMM") && p.id.len() >= 15 {
                        Some(&p.id[4..15])
                    } else if p.id.starts_with("RDAMVM") && p.id.len() >= 17 {
                        Some(&p.id[6..17])
                    } else if p.id.starts_with("RD") && p.id.len() == 13 {
                        Some(&p.id[2..13])
                    } else {
                        None
                    }
                });

                if let Some(vid) = seed_id {
                    let v = VideoCard {
                        id: vid.to_string(),
                        title: p.title.clone(),
                        channel: p.channel.clone(),
                        channel_id: None,
                        verified: false,
                        duration: None,
                        views: None,
                        published: None,
                        thumb_url: p.thumb_url.clone(),
                        progress_pct: None,
                        live: LiveState::None,
                        badges: Vec::new(),
                        resume_time_sec: None,
                    };
                    app.status_text = format!("Launching mix seed '{}'...", v.title);
                    let _ = app.player.play_video(v, mode, &app.config);
                    app.watch_tracker = Some(ytui_core::player::tracking::WatchTracker::new(vid));
                    if mode != Some(PlaybackMode::Inline) {
                        app.set_screen(Screen::Watch);
                        trigger_watch_fetch(app, tx, vid.to_string());
                    } else {
                        app.status_text = "Inline playback finished".to_string();
                    }
                    return;
                }
            }

            app.status_text = format!("Opening playlist '{}'...", p.title);
            app.set_screen(Screen::PlaylistView);
            app.selected_index = 0;
            app.items.clear();
            app.is_loading = true;
            let backend = Arc::clone(&app.backend);
            let p_id = p.id.clone();
            tokio::spawn(async move {
                let res = backend.playlist(&p_id).await;
                let _ = tx.send(AppEvent::FeedLoaded(res)).await;
            });
        }
        Card::Channel(c) => {
            app.status_text = format!("Opening channel '{}'...", c.title);
            let backend = Arc::clone(&app.backend);
            let c_id = c.id.clone();
            tokio::spawn(async move {
                let res = backend.channel(&c_id, ChannelTab::Videos).await;
                let _ = tx.send(AppEvent::FeedLoaded(res)).await;
            });
        }
    }
}

fn execute_command(app: &mut App, tx: Sender<AppEvent>) {
    let cmd = app.cmd_input.trim().to_string();
    if cmd == "q!" || cmd == "quit!" {
        app.player.stop();
        app.should_quit = true;
    } else if cmd == "q" || cmd == "quit" {
        app.confirming_quit = true;
    } else if cmd == "home" {
        app.set_screen(Screen::Home);
        trigger_home_fetch(app, tx, None);
    } else if cmd == "subs" {
        app.set_screen(Screen::Subscriptions);
        let backend = Arc::clone(&app.backend);
        tokio::spawn(async move {
            let res = backend.subscriptions().await;
            let _ = tx.send(AppEvent::FeedLoaded(res)).await;
        });
    } else if let Some(query) = cmd.strip_prefix("search ") {
        let q = query.to_string();
        app.search_query = q.clone();
        app.set_screen(Screen::Search);
        trigger_search_fetch(app, tx, q);
    } else if cmd == "help" {
        app.set_screen(Screen::Help);
    } else if cmd == "settings" {
        app.set_screen(Screen::Settings);
        app.sidebar_index = 8;
        app.status_text = "Settings: [j/k] Navigate  [h/l/Enter] Change  [q/Esc] Back".to_string();
    } else if cmd == "account" || cmd == "accounts" || cmd == "switch" {
        app.set_screen(Screen::Accounts);
        app.sidebar_index = 9;
        app.accounts_cursor = app.active_account_index;
        app.status_text =
            "Accounts: [j/k] Navigate  [Enter/Space] Switch Account  [q/Esc] Back".to_string();
    } else if cmd == "login" || cmd == "auth" {
        trigger_oauth_device_flow(app, tx);
    } else if cmd == "cache clear" {
        let _ = app.thumbs.cache().clear();
        app.status_text = "Cache cleared".to_string();
    }
}

fn handle_app_event(app: &mut App, event: AppEvent, _tx: Sender<AppEvent>) {
    match event {
        AppEvent::FeedLoaded(res) => {
            app.is_loading = false;
            match res {
                Ok(page) => {
                    if !page.chips.is_empty() {
                        app.chips = page.chips.clone();
                    }
                    app.items = page.items.clone();
                    app.continuation_token = page.continuation.clone();
                    app.status_text = format!("Loaded {} items", app.items.len());

                    let cache_key = match app.screen {
                        Screen::Home => "feed_home",
                        Screen::Subscriptions => "feed_subs",
                        Screen::History => "feed_history",
                        Screen::Playlists => "feed_playlists",
                        Screen::WatchLater => "feed_watchlater",
                        Screen::Liked => "feed_liked",
                        _ => "",
                    };
                    if !cache_key.is_empty() && !page.items.is_empty() {
                        let final_key = if !app.account_handle.is_empty() {
                            format!(
                                "{}_{}",
                                app.account_handle.trim_start_matches('@'),
                                cache_key
                            )
                        } else {
                            cache_key.to_string()
                        };
                        if let Ok(json) = serde_json::to_string(&page) {
                            let _ = app.thumbs.cache().save_feed_cache(&final_key, &json);
                        }
                    }
                }
                Err(err) => {
                    if app.items.is_empty() {
                        app.status_text = format!("Error loading feed: {err}");
                    } else {
                        app.status_text = format!("Could not refresh: {err}");
                    }
                }
            }
        }
        AppEvent::WatchDataLoaded(res) => {
            if let Ok(wd) = res {
                app.watch_data = Some(wd);
            }
        }
        AppEvent::CommentsLoaded(res) => match res {
            Ok(batch) => {
                if let Some(ref mut wd) = app.watch_data {
                    wd.comments = batch.comments;
                    wd.comments_token = batch.continuation;
                }
                app.status_text = "Loaded comments".to_string();
            }
            Err(err) => {
                app.status_text = format!("Failed to load comments: {err}");
            }
        },
        AppEvent::StatusMessage(msg) => {
            app.status_text = msg;
        }
        AppEvent::AccountLoaded(res) => {
            if let Ok((name, handle)) = res {
                if !name.is_empty() {
                    app.account_name = name;
                }
                if !handle.is_empty() {
                    app.account_handle = handle;
                }
                if let Some(ch) = app.account_name.chars().next() {
                    app.account_initial = ch.to_uppercase().to_string();
                }
            }
        }
        _ => {}
    }
}

fn try_load_feed_cache(app: &mut App, key: &str) {
    let final_key = if !app.account_handle.is_empty() {
        format!("{}_{}", app.account_handle.trim_start_matches('@'), key)
    } else {
        key.to_string()
    };
    let ttl_sec = (app.config.cache.home_ttl_min.max(15) as u64) * 60;
    let json = app
        .thumbs
        .cache()
        .load_feed_cache(&final_key, ttl_sec)
        .or_else(|| app.thumbs.cache().load_feed_cache(key, ttl_sec));
    if let Some(json) = json {
        if let Ok(page) = serde_json::from_str::<ytui::backend::models::FeedPage>(&json) {
            if !page.items.is_empty() {
                app.items = page.items;
                if !page.chips.is_empty() {
                    app.chips = page.chips;
                }
                app.status_text = format!("Loaded {} items (cached)", app.items.len());
            }
        }
    }
}

fn ensure_screen_loaded(app: &mut App, tx: Sender<AppEvent>) {
    if !app.items.is_empty() || app.is_loading {
        return;
    }
    match app.screen {
        Screen::Home => {
            try_load_feed_cache(app, "feed_home");
            if app.items.is_empty() {
                app.is_loading = true;
                app.status_text = "Loading Home...".to_string();
                trigger_home_fetch(app, tx, None);
            }
        }
        Screen::Subscriptions => {
            try_load_feed_cache(app, "feed_subs");
            if app.items.is_empty() {
                app.is_loading = true;
                app.status_text = "Loading Subscriptions...".to_string();
                let backend = Arc::clone(&app.backend);
                tokio::spawn(async move {
                    let res = backend.subscriptions().await;
                    let _ = tx.send(AppEvent::FeedLoaded(res)).await;
                });
            }
        }
        Screen::History => {
            try_load_feed_cache(app, "feed_history");
            if app.items.is_empty() {
                app.is_loading = true;
                app.status_text = "Loading History...".to_string();
                let backend = Arc::clone(&app.backend);
                tokio::spawn(async move {
                    let res = backend.history().await;
                    let _ = tx.send(AppEvent::FeedLoaded(res)).await;
                });
            }
        }
        Screen::Playlists => {
            try_load_feed_cache(app, "feed_playlists");
            if app.items.is_empty() {
                app.is_loading = true;
                app.status_text = "Loading Playlists...".to_string();
                let backend = Arc::clone(&app.backend);
                tokio::spawn(async move {
                    let res = backend.playlists().await;
                    let _ = tx.send(AppEvent::FeedLoaded(res)).await;
                });
            }
        }
        Screen::WatchLater => {
            try_load_feed_cache(app, "feed_watchlater");
            if app.items.is_empty() {
                app.is_loading = true;
                app.status_text = "Loading Watch Later...".to_string();
                let backend = Arc::clone(&app.backend);
                tokio::spawn(async move {
                    let res = backend.watch_later().await;
                    let _ = tx.send(AppEvent::FeedLoaded(res)).await;
                });
            }
        }
        Screen::Liked => {
            try_load_feed_cache(app, "feed_liked");
            if app.items.is_empty() {
                app.is_loading = true;
                app.status_text = "Loading Liked Videos...".to_string();
                let backend = Arc::clone(&app.backend);
                tokio::spawn(async move {
                    let res = backend.playlist("LL").await;
                    let _ = tx.send(AppEvent::FeedLoaded(res)).await;
                });
            }
        }
        Screen::LocalVideos => {
            let page = scan_local_videos(&app.config.local.dirs);
            app.items = page.items;
            app.status_text = format!("Loaded {} local videos", app.items.len());
        }
        Screen::Queue => {
            app.items = app
                .player
                .queue
                .items()
                .iter()
                .cloned()
                .map(Card::Video)
                .collect();
            app.status_text = format!("Queue: {} items", app.items.len());
        }
        _ => {}
    }
}

fn trigger_oauth_device_flow(app: &mut App, tx: Sender<AppEvent>) {
    app.status_text = "Requesting device code from Google...".to_string();
    let backend = Arc::clone(&app.backend);
    let auth = Arc::clone(&app.auth);
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        let code_res = match ytui::backend::auth::AuthContext::request_device_code(&client).await {
            Ok(c) => c,
            Err(e) => {
                let _ = tx
                    .send(AppEvent::StatusMessage(format!(
                        "Failed to request device code: {e}"
                    )))
                    .await;
                return;
            }
        };

        let msg = format!(
            "Login: Visit {} and enter code: {}",
            code_res.verification_url, code_res.user_code
        );
        let _ = tx.send(AppEvent::StatusMessage(msg)).await;

        let interval = code_res.interval.max(5);
        let max_wait = std::time::Duration::from_secs(code_res.expires_in.min(600));
        let start = std::time::Instant::now();

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
            if start.elapsed() > max_wait {
                let _ = tx
                    .send(AppEvent::StatusMessage(
                        "OAuth device code expired. Run :login again.".to_string(),
                    ))
                    .await;
                break;
            }

            match ytui::backend::auth::AuthContext::poll_device_token(
                &client,
                &code_res.device_code,
            )
            .await
            {
                Ok(Some(tok)) => {
                    let access_token = tok.access_token.unwrap_or_default();
                    let refresh_token = tok.refresh_token.unwrap_or_default();

                    {
                        if let Ok(mut a) = auth.lock() {
                            a.access_token = Some(access_token.clone());
                            a.refresh_token = Some(refresh_token.clone());
                        }
                    }

                    let (name, handle) = backend.account_info().await.unwrap_or_default();

                    {
                        if let Ok(mut a) = auth.lock() {
                            let _ = a.save_token_and_account(
                                access_token,
                                refresh_token,
                                tok.expires_in,
                                name.clone(),
                                handle.clone(),
                            );
                        }
                    }

                    let display_name = if !name.is_empty() {
                        name.clone()
                    } else {
                        "Logged in user".to_string()
                    };
                    let _ = tx
                        .send(AppEvent::StatusMessage(format!(
                            "Logged in successfully as {display_name}"
                        )))
                        .await;
                    let _ = tx.send(AppEvent::AccountLoaded(Ok((name, handle)))).await;

                    let feed_res = backend.home(None).await;
                    let _ = tx.send(AppEvent::FeedLoaded(feed_res)).await;
                    break;
                }
                Ok(None) => {}
                Err(e) => {
                    let _ = tx
                        .send(AppEvent::StatusMessage(format!("OAuth polling error: {e}")))
                        .await;
                    break;
                }
            }
        }
    });
}

async fn run_login() -> anyhow::Result<()> {
    use std::time::Duration;
    use tokio::time::sleep;
    use ytui::backend::auth::AuthContext;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()?;

    println!("==================================================");
    println!("          YouTube Device Sign-In");
    println!("==================================================");
    println!("Requesting device authorization code from YouTube...");

    let device_code = AuthContext::request_device_code(&client).await?;

    println!();
    println!("1. On your phone or computer, open your browser and go to:");
    println!("   {}", device_code.verification_url);
    println!();
    println!("2. Enter this code:");
    println!("   >>> {} <<<", device_code.user_code);
    println!();
    println!("Waiting for authorization in browser... (Press Ctrl+C to cancel)");

    let interval = device_code.interval.max(5);
    let start = std::time::Instant::now();
    let max_duration = Duration::from_secs(device_code.expires_in.max(300));

    while start.elapsed() < max_duration {
        sleep(Duration::from_secs(interval)).await;

        match AuthContext::poll_device_token(&client, &device_code.device_code).await {
            Ok(Some(token_res)) => {
                if let (Some(access), Some(refresh)) =
                    (token_res.access_token, token_res.refresh_token)
                {
                    let mut auth = AuthContext::default();
                    auth.save_token_and_account(
                        access,
                        refresh,
                        token_res.expires_in,
                        "Authenticated User".to_string(),
                        String::new(),
                    )?;
                    println!();
                    println!("==================================================");
                    println!("Authorization successful!");
                    println!("Credentials stored securely at ~/.config/ytui/auth.json (mode 0600)");
                    println!("==================================================");
                    return Ok(());
                }
            }
            Ok(None) => {
                use std::io::Write;
                print!(".");
                let _ = std::io::stdout().flush();
            }
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("slow_down") {
                    sleep(Duration::from_secs(5)).await;
                } else {
                    eprintln!("\nDevice login error: {e}");
                    return Err(e);
                }
            }
        }
    }

    anyhow::bail!("Authorization code expired. Please run `ytui login` again.");
}

async fn run_dump_tv(query: &str) -> anyhow::Result<()> {
    use serde_json::json;
    use std::fs;
    use std::io::Write;
    use ytui::backend::auth::AuthContext;
    use ytui::backend::innertube::InnerTubeBackend;
    use ytui::config::resolve_path;

    let debug_dir = resolve_path("~/.cache/ytui/debug");
    let _ = fs::create_dir_all(&debug_dir);

    let auth_path = resolve_path("~/.config/ytui/auth.json");
    let cookies_path = resolve_path("~/.config/ytui/cookies.txt");
    let mut auth = AuthContext::default();
    let _ = auth.load_auth(&auth_path, &cookies_path);

    let is_authed = auth.access_token.is_some();
    let backend = InnerTubeBackend::new(auth);

    println!("==================================================");
    println!("     TVHTML5 Milestone 0 Raw Dumps Spike");
    println!("==================================================");
    println!(
        "Auth mode: {}",
        if is_authed {
            "OAuth Bearer Token"
        } else {
            "Unauthenticated / Guest"
        }
    );
    println!("Output directory: {}\n", debug_dir.display());

    // 1. Home / Browse (FEwhat_to_watch)
    print!("Fetching TVHTML5 browse (FEwhat_to_watch)... ");
    let _ = std::io::stdout().flush();
    let home_body = backend.make_body(json!({ "browseId": "FEwhat_to_watch" }));
    let home_val = backend.post_endpoint("browse", home_body).await?;
    let home_path = debug_dir.join("tv_browse_home.json");
    fs::write(&home_path, serde_json::to_string_pretty(&home_val)?)?;
    println!(
        "OK ({} bytes written to {})",
        fs::metadata(&home_path)?.len(),
        home_path.file_name().unwrap().to_string_lossy()
    );

    // 2. Search
    print!("Fetching TVHTML5 search ('{}')... ", query);
    let _ = std::io::stdout().flush();
    let search_body = backend.make_body(json!({ "query": query }));
    let search_val = backend.post_endpoint("search", search_body).await?;
    let search_path = debug_dir.join("tv_search.json");
    fs::write(&search_path, serde_json::to_string_pretty(&search_val)?)?;
    println!(
        "OK ({} bytes written to {})",
        fs::metadata(&search_path)?.len(),
        search_path.file_name().unwrap().to_string_lossy()
    );

    // Extract first video id from home or search for next & player
    let mut sample_video_id = "dQw4w9WgXcQ".to_string();
    let page = ytui::backend::parse::parse_feed_page(&home_val);
    if let Some(card) = page.items.first() {
        sample_video_id = card.id().to_string();
    } else {
        let s_page = ytui::backend::parse::parse_feed_page(&search_val);
        if let Some(card) = s_page.items.first() {
            sample_video_id = card.id().to_string();
        }
    }

    // 3. Next endpoint
    print!("Fetching TVHTML5 next (videoId: '{}')... ", sample_video_id);
    let _ = std::io::stdout().flush();
    let next_body = backend.make_body(json!({ "videoId": sample_video_id }));
    let next_val = backend.post_endpoint("next", next_body).await?;
    let next_path = debug_dir.join("tv_next.json");
    fs::write(&next_path, serde_json::to_string_pretty(&next_val)?)?;
    println!(
        "OK ({} bytes written to {})",
        fs::metadata(&next_path)?.len(),
        next_path.file_name().unwrap().to_string_lossy()
    );

    // 4. Player endpoint (for playback tracking inspection)
    print!(
        "Fetching TVHTML5 player (videoId: '{}')... ",
        sample_video_id
    );
    let _ = std::io::stdout().flush();
    let player_body = backend.make_body(json!({ "videoId": sample_video_id }));
    let player_val = backend.post_endpoint("player", player_body).await?;
    let player_path = debug_dir.join("tv_player.json");
    fs::write(&player_path, serde_json::to_string_pretty(&player_val)?)?;
    println!(
        "OK ({} bytes written to {})",
        fs::metadata(&player_path)?.len(),
        player_path.file_name().unwrap().to_string_lossy()
    );

    println!("\n==================================================");
    println!("                  Dump Analysis");
    println!("==================================================");

    // Analyze visitorData
    let vis = home_val
        .pointer("/responseContext/visitorData")
        .and_then(|v| v.as_str())
        .unwrap_or("(none)");
    println!("visitorData: {}", vis);

    // Analyze Home Renderers
    let home_str = serde_json::to_string(&home_val)?;
    let mut detected_renderers = Vec::new();
    for r in &[
        "tileRenderer",
        "lockupViewModel",
        "gridVideoRenderer",
        "videoRenderer",
        "compactVideoRenderer",
        "reelItemRenderer",
        "shelfRenderer",
        "pivotBarRenderer",
    ] {
        if home_str.contains(r) {
            detected_renderers.push(*r);
        }
    }
    println!("Home items parsed: {}", page.items.len());
    println!("Home renderers detected: {:?}", detected_renderers);

    // Analyze Search Renderers
    let search_str = serde_json::to_string(&search_val)?;
    let mut search_renderers = Vec::new();
    for r in &[
        "tileRenderer",
        "lockupViewModel",
        "gridVideoRenderer",
        "videoRenderer",
        "compactVideoRenderer",
        "reelItemRenderer",
        "shelfRenderer",
    ] {
        if search_str.contains(r) {
            search_renderers.push(*r);
        }
    }
    let s_page = ytui::backend::parse::parse_feed_page(&search_val);
    println!("Search items parsed: {}", s_page.items.len());
    println!("Search renderers detected: {:?}", search_renderers);

    // Analyze Playback Tracking
    let pb_url = player_val
        .pointer("/playbackTracking/videostatsPlaybackUrl/baseUrl")
        .and_then(|v| v.as_str());
    let wt_url = player_val
        .pointer("/playbackTracking/videostatsWatchtimeUrl/baseUrl")
        .and_then(|v| v.as_str());
    println!("Playback tracking URL: {}", pb_url.unwrap_or("(none)"));
    println!("Watchtime tracking URL: {}", wt_url.unwrap_or("(none)"));

    println!(
        "\nAll raw dumps successfully saved to: {}",
        debug_dir.display()
    );
    Ok(())
}
