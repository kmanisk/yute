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
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

#[derive(Parser, Debug)]
#[command(name = "ytui", about = "Fast, keyboard-driven YouTube terminal client in Rust")]
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
        let _ = execute!(stdout(), PopKeyboardEnhancementFlags, LeaveAlternateScreen, crossterm::cursor::Show);
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
        let res = backend.search(&query, &SearchFilters::default(), None).await;
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
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
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
            KeyCode::Char('u') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                app.search_query.clear();
            }
            KeyCode::Char('w') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                if let Some(idx) = app.search_query.trim_end().rfind(' ') {
                    app.search_query.truncate(idx + 1);
                } else {
                    app.search_query.clear();
                }
            }
            KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                app.focus = Focus::Content;
            }
            KeyCode::Char(c) => {
                if !key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(crossterm::event::KeyModifiers::ALT) {
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
                    app.settings_index = (app.settings_index + 1).min(ui::settings::SETTINGS_ITEMS.len().saturating_sub(1));
                }
                Action::Left => {
                    ui::settings::cycle_setting(app, false);
                }
                Action::Right | Action::PlayDefault => {
                    ui::settings::cycle_setting(app, true);
                }
                Action::Back | Action::Quit => {
                    app.nav_back();
                }
                Action::ForceQuit => {
                    app.should_quit = true;
                }
                Action::NavHome => {
                    app.set_screen(Screen::Home);
                    app.sidebar_index = 0;
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
                    app.accounts_cursor = (app.accounts_cursor + 1).min(app.accounts.len().saturating_sub(1));
                }
                Action::PlayDefault | Action::Right => {
                    app.switch_account(app.accounts_cursor);
                    app.status_text = format!("Switched to account '{}'", app.account_name);
                    app.items.clear();
                    app.is_loading = true;
                    trigger_home_fetch(app, tx.clone(), None);
                    app.set_screen(Screen::Home);
                    app.sidebar_index = 0;
                }
                Action::Back | Action::Quit => {
                    app.nav_back();
                }
                Action::ForceQuit => {
                    app.should_quit = true;
                }
                Action::NavHome => {
                    app.set_screen(Screen::Home);
                    app.sidebar_index = 0;
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
                    app.player.stop();
                    app.nav_back();
                } else if app.focus == Focus::Sidebar {
                    app.focus = Focus::Content;
                } else {
                    app.nav_back();
                }
            }
            Action::ForceQuit => {
                app.player.stop();
                app.should_quit = true;
            }
            Action::Up => {
                if app.focus == Focus::Sidebar {
                    app.sidebar_index = app.sidebar_index.saturating_sub(1);
                } else {
                    app.selected_index = app.selected_index.saturating_sub(grid_cols);
                }
            }
            Action::Down => {
                if app.focus == Focus::Sidebar {
                    app.sidebar_index = (app.sidebar_index + 1).min(ui::sidebar::SIDEBAR_ITEMS.len().saturating_sub(1));
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
                app.items = app.player.queue.items().iter().cloned().map(Card::Video).collect();
                app.status_text = format!("Queue: {} items", app.items.len());
            }
            Action::NavSettings => {
                app.set_screen(Screen::Settings);
                app.sidebar_index = 8;
                app.status_text = "Settings: [j/k] Navigate  [h/l/Enter] Change  [q/Esc] Back".to_string();
            }
            Action::NavAccounts => {
                app.set_screen(Screen::Accounts);
                app.sidebar_index = 9;
                app.accounts_cursor = app.active_account_index;
                app.status_text = "Accounts: [j/k] Navigate  [Enter/Space] Switch Account  [q/Esc] Back".to_string();
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
                    app.player.stop();
                }
                app.nav_back();
            }
            Action::NavForwardHistory => {
                app.nav_forward();
            }
            Action::ToggleStats => {
                app.player.toggle_stats();
                app.status_text = "Toggled Stats for Nerds overlay".to_string();
            }
            Action::StopPlayback => {
                app.player.stop();
            }
            _ => {}
        }
    }
}

fn activate_sidebar_item(app: &mut App, tx: Sender<AppEvent>) {
    if app.screen == Screen::Watch {
        app.player.stop();
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
            app.items = app.player.queue.items().iter().cloned().map(Card::Video).collect();
            app.status_text = format!("Queue: {} items", app.items.len());
        }
        8 => {
            app.set_screen(Screen::Settings);
            app.sidebar_index = 8;
            app.status_text = "Settings: [j/k] Navigate  [h/l/Enter] Change  [q/Esc] Back".to_string();
        }
        9 => {
            app.set_screen(Screen::Accounts);
            app.sidebar_index = 9;
            app.accounts_cursor = app.active_account_index;
            app.status_text = "Accounts: [j/k] Navigate  [Enter/Space] Switch Account  [q/Esc] Back".to_string();
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
            let _ = app.player.play_video(v, mode, &app.config);
            if mode != Some(PlaybackMode::Inline) {
                app.set_screen(Screen::Watch);
                trigger_watch_fetch(app, tx, id);
            } else {
                app.status_text = "Inline playback finished".to_string();
            }
        }
        Card::Playlist(p) => {
            if p.is_mix {
                let seed_id = if p.id.starts_with("RDMM") && p.id.len() >= 15 {
                    Some(&p.id[4..15])
                } else if p.id.starts_with("RDAMVM") && p.id.len() >= 17 {
                    Some(&p.id[6..17])
                } else if p.id.starts_with("RD") && p.id.len() >= 13 {
                    Some(&p.id[2..13])
                } else {
                    None
                };

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
                    };
                    app.status_text = format!("Launching mix seed '{}'...", v.title);
                    let _ = app.player.play_video(v, mode, &app.config);
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
        app.status_text = "Accounts: [j/k] Navigate  [Enter/Space] Switch Account  [q/Esc] Back".to_string();
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
                        if let Ok(json) = serde_json::to_string(&page) {
                            let _ = app.thumbs.cache().save_feed_cache(cache_key, &json);
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
        AppEvent::StatusMessage(msg) => {
            app.status_text = msg;
        }
        AppEvent::AccountLoaded(res) => {
            if let Ok((name, handle)) = res {
                if app.active_account_index == 1 {
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
        }
        _ => {}
    }
}

fn try_load_feed_cache(app: &mut App, key: &str) {
    if let Some(json) = app.thumbs.cache().load_feed_cache(key, 120) {
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

