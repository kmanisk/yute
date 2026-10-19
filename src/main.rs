#![allow(dead_code)]

use clap::{Parser, Subcommand};
use crossterm::event::{Event, EventStream, KeyCode};
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
use ytui::backend::models::{Card, ChannelTab, SearchFilters};
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

    // Setup terminal
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, crossterm::cursor::Hide)?;
    let term_backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(term_backend)?;

    // Setup panic hook to cleanly restore terminal
    let orig_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
        orig_hook(panic_info);
    }));

    let mut app = App::new(config, picker);
    let (tx_event, mut rx_event) = channel::<AppEvent>(64);

    // Initial load: trigger home feed fetch
    trigger_home_fetch(&app, tx_event.clone(), None);

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
                if !app.search_query.trim().is_empty() {
                    app.set_screen(Screen::Search);
                    app.focus = Focus::Content;
                    app.is_loading = true;
                    app.status_text = format!("Searching for '{}'...", app.search_query);
                    trigger_search_fetch(app, tx.clone(), app.search_query.clone());
                }
            }
            KeyCode::Esc => {
                app.focus = Focus::Content;
            }
            KeyCode::Backspace => {
                app.search_query.pop();
            }
            KeyCode::Char(c) => {
                app.search_query.push(c);
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
        match action {
            Action::Quit | Action::Back => {
                app.nav_back();
            }
            Action::ForceQuit => {
                app.should_quit = true;
            }
            Action::Up => {
                if app.focus == Focus::Sidebar {
                    app.sidebar_index = app.sidebar_index.saturating_sub(1);
                } else {
                    let min_w = app.config.grid.min_card_width.max(28);
                    let cols = (80 / min_w).clamp(1, 6) as usize;
                    app.selected_index = app.selected_index.saturating_sub(cols);
                }
            }
            Action::Down => {
                let total = app.filtered_items().len();
                if app.focus == Focus::Sidebar {
                    app.sidebar_index = (app.sidebar_index + 1).min(8);
                } else if total > 0 {
                    let min_w = app.config.grid.min_card_width.max(28);
                    let cols = (80 / min_w).clamp(1, 6) as usize;
                    app.selected_index = (app.selected_index + cols).min(total - 1);
                }
            }
            Action::Left => {
                app.selected_index = app.selected_index.saturating_sub(1);
            }
            Action::Right => {
                let total = app.filtered_items().len();
                if total > 0 {
                    app.selected_index = (app.selected_index + 1).min(total - 1);
                }
            }
            Action::First => {
                app.selected_index = 0;
            }
            Action::Last => {
                let total = app.filtered_items().len();
                if total > 0 {
                    app.selected_index = total - 1;
                }
            }
            Action::CycleFocusForward => {
                app.focus = match app.focus {
                    Focus::Sidebar => Focus::Chips,
                    Focus::Chips => Focus::Content,
                    Focus::Content => Focus::Sidebar,
                    _ => Focus::Content,
                };
            }
            Action::CycleFocusBackward => {
                app.focus = match app.focus {
                    Focus::Sidebar => Focus::Content,
                    Focus::Content => Focus::Chips,
                    Focus::Chips => Focus::Sidebar,
                    _ => Focus::Content,
                };
            }
            Action::ToggleSidebar => {
                app.sidebar_expanded = !app.sidebar_expanded;
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
                trigger_home_fetch(app, tx.clone(), None);
            }
            Action::NavHome => {
                app.set_screen(Screen::Home);
                trigger_home_fetch(app, tx.clone(), None);
            }
            Action::NavSubs => {
                app.set_screen(Screen::Subscriptions);
                let backend = Arc::clone(&app.backend);
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = backend.subscriptions().await;
                    let _ = tx_c.send(AppEvent::FeedLoaded(res)).await;
                });
            }
            Action::NavHistory => {
                app.set_screen(Screen::History);
                let backend = Arc::clone(&app.backend);
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = backend.history().await;
                    let _ = tx_c.send(AppEvent::FeedLoaded(res)).await;
                });
            }
            Action::NavWatchLater => {
                app.set_screen(Screen::WatchLater);
                let backend = Arc::clone(&app.backend);
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = backend.watch_later().await;
                    let _ = tx_c.send(AppEvent::FeedLoaded(res)).await;
                });
            }
            Action::NavPlaylists => {
                app.set_screen(Screen::Playlists);
                let backend = Arc::clone(&app.backend);
                let tx_c = tx.clone();
                tokio::spawn(async move {
                    let res = backend.playlists().await;
                    let _ = tx_c.send(AppEvent::FeedLoaded(res)).await;
                });
            }
            Action::NavLocal => {
                app.set_screen(Screen::LocalVideos);
                let page = scan_local_videos(&app.config.local.dirs);
                app.items = page.items;
                app.status_text = format!("Loaded {} local videos", app.items.len());
            }
            Action::PlayDefault => {
                play_selected_card(app, None, tx.clone());
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
            Action::StopPlayback => {
                app.player.stop();
            }
            _ => {}
        }
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
            app.set_screen(Screen::Watch);
            trigger_watch_fetch(app, tx, id);
        }
        Card::Playlist(p) => {
            app.status_text = format!("Opening playlist '{}'...", p.title);
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
    if cmd == "q" || cmd == "quit" {
        app.should_quit = true;
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
                        app.chips = page.chips;
                    }
                    app.items = page.items;
                    app.continuation_token = page.continuation;
                    app.status_text = format!("Loaded {} items", app.items.len());
                }
                Err(err) => {
                    app.status_text = format!("Error loading feed: {err}");
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
        _ => {}
    }
}
