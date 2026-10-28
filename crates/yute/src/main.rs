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
use yute::app::{App, Focus, Screen};
use yute::backend::models::{Card, SearchFilters, VideoCard};
use yute::backend::Backend;
use yute::config::Config;
use yute::events::AppEvent;
use yute::keymap::{Action, KeymapState};
use yute::local::scan_local_videos;
use yute::player::ipc::IpcCommand;
use yute::player::mpvconf::MpvProfile;
use yute::ui;

#[derive(Parser, Debug)]
#[command(
    name = "yute",
    version,
    about = "Fast, keyboard-driven YouTube terminal client in Rust"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Dump raw JSON for a given screen to cache/debug/
    #[arg(long)]
    dump_raw: Option<String>,

    /// Maximum video resolution for this session (also available as :quality).
    #[arg(long, value_parser=["480p","720p","1080p","1440p","2160p","best"])]
    quality: Option<String>,

    /// Enable verbose file logging
    #[arg(long)]
    log: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Diagnose connectivity, session, tools and terminal without displaying secrets.
    Doctor,
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

fn main() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(run());
    // An OS keyring may be waiting for a desktop unlock prompt. Quitting must
    // not wait indefinitely for a non-cancellable blocking credential call.
    runtime.shutdown_timeout(Duration::from_millis(100));
    result
}

async fn run() -> anyhow::Result<()> {
    yute_core::trace::start();
    init_performance_trace()?;
    let cli = Cli::parse();

    if let Some(Commands::Doctor) = cli.command {
        return run_doctor().await;
    }

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

    let mut config = Config::load();
    if let Some(quality) = cli.quality {
        config.playback.max_resolution = quality;
    }

    // No stdin query before first paint; cached font and terminal hints are enough.
    let picker = yute::terminal::initial_picker(&config.general.graphics);

    // Request unambiguous terminal key events
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

    hydrate_accounts(&mut app, tx_event.clone());
    tokio::spawn(yute_core::http::prewarm());
    tokio::task::spawn_blocking(|| {
        let _ = yute_core::config::migrate_legacy_config();
    });
    let size_tx = tx_event.clone();
    tokio::spawn(async move {
        if let Some((width, height)) = yute::terminal::cell_size().await {
            let _ = size_tx.send(AppEvent::TerminalCells(width, height)).await;
        }
    });

    // Initial load: load disk cache immediately for fast launch, then fetch fresh
    try_load_feed_cache(&mut app, "feed_home");
    app.is_loading = true;
    if app.account_store.active_id() == yute_core::accounts::ANONYMOUS {
        trigger_home_fetch(&mut app, tx_event.clone(), None);
    }

    // Prune cache in background to prevent disk bloat
    let cache_clone = app.thumbs.cache().clone();
    tokio::task::spawn_blocking(move || {
        let _ = cache_clone.prune_thumbnails();
        let _ = cache_clone.prune_feeds(3);
    });

    let thumbnail_ready = app.thumbs.notifier();
    let mut keymap = KeymapState::new();
    let mut reader = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(250));
    let mut dirty = true;
    let mut first_frame = true;
    let mut first_home = true;
    let mut graphics_layout = String::new();

    while !app.should_quit {
        if dirty {
            app.refresh_channel_items();
            let next_layout = format!(
                "{:?}:{:?}:{:?}:{}:{}:{:?}:{}:{}:{}:{:?}",
                terminal.size()?,
                app.screen,
                app.focus,
                app.selected_index / app.rendered_columns.max(1),
                app.sidebar_expanded,
                app.overlay_area,
                app.confirming_quit,
                app.confirming_logout,
                app.scroll_row,
                (
                    app.action_menu.is_some(),
                    app.account_picker,
                    app.login_modal.is_some(),
                    app.command_help.is_some()
                )
            );
            if next_layout != graphics_layout {
                // Delete Kitty placements and erase Sixel cells before a structural
                // change. Cached encodings remain available for the next draw.
                use std::io::Write;
                terminal
                    .backend_mut()
                    .write_all(b"\x1b_Ga=d,d=a,q=2;\x1b\\")?;
                let size = terminal.size()?;
                terminal.resize(ratatui::layout::Rect::new(0, 0, size.width, size.height))?;
                graphics_layout = next_layout;
            }
            terminal.draw(|f| ui::draw(f, &mut app))?;
            if let Some(start) = app.trace_account_frame.take() {
                yute_core::trace::duration(
                    "account_switch_visible",
                    start.elapsed(),
                    app.items.len(),
                );
            }
            if first_frame {
                yute_core::trace::since_start("first_frame");
                first_frame = false;
            }
            if first_home && app.screen == Screen::Home && !app.items.is_empty() {
                yute_core::trace::since_start("home_rendered");
                first_home = false;
            }
            if app.screen == Screen::Subscriptions && !app.items.is_empty() {
                if let Some(start) = app.trace_subscriptions.take() {
                    yute_core::trace::duration(
                        "subscriptions_first_page",
                        start.elapsed(),
                        app.items.len(),
                    );
                }
                if app.continuation_token.is_none() {
                    if let Some(start) = app.trace_subscriptions_complete.take() {
                        yute_core::trace::duration(
                            "subscriptions_complete",
                            start.elapsed(),
                            app.items.len(),
                        );
                    }
                }
            }
            maybe_continue_feed(&mut app, tx_event.clone());
            maybe_load_card_metadata(&mut app, tx_event.clone());
            dirty = false;
        }

        tokio::select! {
            _ = thumbnail_ready.notified() => {dirty |= app.thumbs.poll_loaded();}
            _ = ticker.tick(), if app.needs_tick() || keymap.has_pending() => {
                dirty |= app.tick();
                let hint = keymap.which_key_visible();
                if hint != app.which_key { app.which_key = hint; dirty = true; }
            }

            event_opt = reader.next() => {
                if let Some(Ok(event)) = event_opt {
                    match event {
                        Event::Key(key) => {
                            if key.kind != crossterm::event::KeyEventKind::Press {
                                continue;
                            }
                            handle_key_event(&mut app, &mut keymap, key, tx_event.clone()).await;
                            dirty = true;
                            maybe_continue_feed(&mut app, tx_event.clone());
                        }
                        Event::Resize(_, _) | Event::FocusGained => {
                            refresh_terminal_cells(&mut app).await;
                            graphics_layout.clear();
                            dirty = true;
                        }
                        _ => {}
                    }
                }
            }

            app_event_opt = rx_event.recv() => {
                if let Some(app_event) = app_event_opt {
                    handle_app_event(&mut app, app_event, tx_event.clone());
                    dirty = true;
                    maybe_continue_feed(&mut app, tx_event.clone());
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

fn trigger_home_fetch(app: &mut App, tx: Sender<AppEvent>, chip_token: Option<String>) {
    let backend = Arc::clone(&app.backend);
    let generation = app.begin_feed_request();
    tokio::spawn(async move {
        let res = backend.home(chip_token.as_deref()).await;
        let _ = tx.send(AppEvent::FeedLoaded(generation, res)).await;
    });
}

fn trigger_search_fetch(app: &mut App, tx: Sender<AppEvent>, query: String) {
    let backend = Arc::clone(&app.backend);
    let generation = app.begin_feed_request();
    tokio::spawn(async move {
        let res = backend
            .search(&query, &SearchFilters::default(), None)
            .await;
        let _ = tx.send(AppEvent::FeedLoaded(generation, res)).await;
    });
}

fn trigger_watch_fetch(app: &App, tx: Sender<AppEvent>, video_id: String) {
    let generation = app
        .auth_generation
        .load(std::sync::atomic::Ordering::SeqCst);
    let backend = Arc::clone(&app.backend);
    tokio::spawn(async move {
        let res = backend.watch(&video_id).await;
        let _ = tx
            .send(AppEvent::AccountResult(
                generation,
                Box::new(AppEvent::WatchDataLoaded(video_id, res)),
            ))
            .await;
    });
}

fn init_performance_trace() -> anyhow::Result<()> {
    if !yute_core::trace::enabled() {
        return Ok(());
    }
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let dir = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| yute::config::resolve_path("~/.local/state"))
        .join("yute");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(dir.join(format!("trace-{}.log", std::process::id())))?;
    use tracing_subscriber::prelude::*;
    let layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .without_time()
        .with_writer(std::sync::Mutex::new(file))
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_filter(tracing_subscriber::filter::filter_fn(|m| {
            m.target() == "yute_perf"
        }));
    let _ = tracing_subscriber::registry().with(layer).try_init();
    Ok(())
}

fn hydrate_accounts(app: &mut App, tx: Sender<AppEvent>) {
    if app.credentials_pending {
        return;
    }
    app.credentials_pending = true;
    let generation = app
        .auth_generation
        .load(std::sync::atomic::Ordering::SeqCst);
    let config = app.config.clone();
    let auth = app.auth.lock().unwrap().clone();
    tokio::spawn(async move {
        let mut work = tokio::task::spawn_blocking(move || {
            yute_core::accounts::Accounts::load_session(&config, auth)
        });
        let result = tokio::select! {
            result=&mut work=>result,
            _=tokio::time::sleep(Duration::from_secs(2))=>{
                let _=tx.send(AppEvent::CredentialsWaiting(generation)).await;
                work.await
            }
        }
        .unwrap_or_else(|_| Err(anyhow::anyhow!("Credential worker failed")));
        let _ = tx
            .send(AppEvent::CredentialsLoaded(generation, result))
            .await;
    });
}

fn refresh_account_profiles(app: &App, tx: Sender<AppEvent>) {
    if app.auth.lock().unwrap().access_token.is_none() {
        return;
    }
    let auth = app.auth.clone();
    let generation = app
        .auth_generation
        .load(std::sync::atomic::Ordering::SeqCst);
    tokio::spawn(async move {
        let result = yute_core::accounts::fetch_profiles(auth).await;
        let _ = tx.send(AppEvent::AccountProfiles(generation, result)).await;
    });
}
fn activate_account(app: &mut App, index: usize, tx: Sender<AppEvent>) {
    if app.switch_account(index) {
        let _ = app.config.save();
        try_load_feed_cache(app, "feed_home");
        trigger_home_fetch(app, tx, None);
    }
}
fn handle_account_picker(app: &mut App, key: crossterm::event::KeyEvent, tx: Sender<AppEvent>) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.account_picker = false;
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.accounts_cursor =
                (app.accounts_cursor + 1).min(app.accounts.len().saturating_sub(1))
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.accounts_cursor = app.accounts_cursor.saturating_sub(1)
        }
        KeyCode::Enter => activate_account(app, app.accounts_cursor, tx),
        KeyCode::Char(c @ '1'..='9') => {
            let index = c as usize - '1' as usize;
            if index < app.accounts.len() {
                activate_account(app, index, tx);
            }
        }
        KeyCode::Char('a') => {
            app.account_picker = false;
            trigger_oauth_device_flow(app, tx);
        }
        KeyCode::Char('d') => {
            if app.accounts_cursor != 0 {
                app.pending_remove = Some(app.accounts_cursor);
                app.confirming_logout = true;
            }
        }
        _ => (),
    }
}

fn handle_command_input(app: &mut App, key: crossterm::event::KeyEvent, tx: Sender<AppEvent>) {
    use crossterm::event::KeyModifiers;
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let choices = app.command_choices();
    match key.code {
        KeyCode::Esc | KeyCode::Char('c') if key.code == KeyCode::Esc || ctrl => {
            app.focus = Focus::Content;
            app.cmd_input.clear();
            app.command_history.reset();
            app.completion_base = None;
            app.completion_index = 0;
            app.input_cursor = None;
        }
        KeyCode::Backspace if app.cmd_input.is_empty() => {
            app.focus = Focus::Content;
            app.completion_base = None;
            app.input_cursor = None;
        }
        KeyCode::Enter => {
            let choosing = app
                .completion_base
                .as_ref()
                .is_some_and(|base| app.cmd_input.starts_with(base))
                || choices.iter().any(|choice| choice == &app.cmd_input);
            if choosing || yute::commands::parse(&app.cmd_input).is_err() {
                if let Some(choice) =
                    choices.get(app.completion_index.min(choices.len().saturating_sub(1)))
                {
                    app.cmd_input = choice.clone();
                }
            }
            app.focus = Focus::Content;
            execute_command(app, tx);
            app.cmd_input.clear();
            app.completion_base = None;
            app.completion_index = 0;
            app.input_cursor = None;
            app.command_history.reset();
        }
        KeyCode::Char('r' | 's') if ctrl => {
            app.cmd_input = if key.code == KeyCode::Char('r') {
                app.command_history.previous(&app.cmd_input)
            } else {
                app.command_history.next_entry()
            };
            app.completion_base = None;
            app.completion_index = 0;
            app.input_cursor = None;
        }
        KeyCode::Up
        | KeyCode::Down
        | KeyCode::Tab
        | KeyCode::BackTab
        | KeyCode::Char('n' | 'p')
            if !matches!(key.code, KeyCode::Char(_)) || ctrl =>
        {
            if choices.is_empty()
                || (app.cmd_input.is_empty() && matches!(key.code, KeyCode::Up | KeyCode::Down))
            {
                app.cmd_input = if key.code == KeyCode::Up {
                    app.command_history.previous(&app.cmd_input)
                } else if key.code == KeyCode::Down {
                    app.command_history.next_entry()
                } else {
                    app.cmd_input.clone()
                };
            } else {
                let previous = matches!(
                    key.code,
                    KeyCode::Up | KeyCode::BackTab | KeyCode::Char('p')
                );
                let started = app.completion_base.is_some();
                app.completion_base
                    .get_or_insert_with(|| app.cmd_input.clone());
                app.completion_index = if previous {
                    (app.completion_index + choices.len() - 1) % choices.len()
                } else if started {
                    (app.completion_index + 1) % choices.len()
                } else {
                    0
                };
                if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
                    app.cmd_input = choices[app.completion_index].clone();
                }
            }
            app.input_cursor = None;
        }
        _ => {
            edit_input(app, key);
        }
    }
}

async fn refresh_terminal_cells(app: &mut App) {
    if let Some((width, height)) = yute::terminal::cell_size().await {
        app.cell_w_px = width as f32;
        app.cell_h_px = height as f32;
        app.thumbs.refresh_cell_size(width, height);
        yute::terminal::remember_cells(width, height);
    }
}

async fn handle_key_event(
    app: &mut App,
    keymap: &mut KeymapState,
    key: crossterm::event::KeyEvent,
    tx: Sender<AppEvent>,
) {
    let key = yute::keymap::normalize_key(key);
    if let Some(modal) = &app.login_modal {
        if key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('c')
                && key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL))
        {
            modal.cancelled.cancel();
            app.login_modal = None;
            app.auth_generation
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            app.status_text = "Sign-in cancelled".into();
        }
        return;
    }
    if app.focus == Focus::CommandLine {
        handle_command_input(app, key, tx);
        return;
    }
    if app.action_menu.is_some() {
        handle_item_menu(app, key, tx);
        return;
    }
    if key.code == KeyCode::Char('e')
        && key.modifiers == crossterm::event::KeyModifiers::ALT
        && !app.confirming_quit
        && !app.confirming_logout
    {
        focus_sidebar(app);
        return;
    }
    if app.focus == Focus::CommentInput {
        if key.code == KeyCode::Esc {
            app.focus = Focus::Content;
            app.comment_draft = None;
            return;
        }
        if key.code == KeyCode::Enter
            || (key.code == KeyCode::Char('s')
                && key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL))
        {
            post_comment_draft(app, tx);
            return;
        }
        if !app.comment_posting {
            edit_input(app, key);
        }
        return;
    }
    if app.focus == Focus::CommandLine {
        handle_command_input(app, key, tx);
        return;
    }
    if app.focus == Focus::SearchInput && edit_input(app, key) {
        return;
    }
    if app.confirming_logout {
        app.confirming_logout = false;
        if matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')) {
            if let Some(index) = app.pending_remove.take() {
                if index == app.active_account_index {
                    activate_account(app, 0, tx.clone());
                }
                if let Some(account) = app.accounts.get(index) {
                    let id = account.id.clone();
                    match app.account_store.removal_job(index) {
                        Ok(job) => {
                            tokio::spawn(async move {
                                let result =
                                    tokio::task::spawn_blocking(job).await.unwrap_or_else(|_| {
                                        Err(anyhow::anyhow!("Credential worker failed"))
                                    });
                                let _ = tx.send(AppEvent::AccountRemoved(id, result)).await;
                            });
                            app.status_text = "Removing saved account…".into();
                        }
                        Err(_) => app.status_text = "Cannot remove this account".into(),
                    }
                }
            } else {
                logout(app);
            }
        } else {
            app.pending_remove = None;
        }
        return;
    }
    if app.account_picker {
        handle_account_picker(app, key, tx);
        return;
    }
    if app.screen == Screen::Accounts
        && app.focus == Focus::Content
        && matches!(key.code, KeyCode::Char('a' | 'd' | '1'..='9'))
    {
        handle_account_picker(app, key, tx);
        return;
    }

    if key
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
    {
        app.player.stop();
        app.should_quit = true;
        return;
    }
    if app.command_help.is_some() {
        if matches!(key.code, KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter) {
            app.command_help = None;
        } else if key.code == KeyCode::Char(':') {
            app.command_help = None;
            app.focus = Focus::CommandLine;
            app.cmd_input.clear();
        }
        return;
    }
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

    if app.focus == Focus::CommandLine {
        handle_command_input(app, key, tx);
        return;
    }
    if app.focus == Focus::SearchInput {
        match key.code {
            KeyCode::Enter => {
                let query = app
                    .suggestion_index
                    .and_then(|i| app.search_suggestions.get(i))
                    .cloned()
                    .unwrap_or_else(|| app.search_input.trim().to_string());
                if !query.is_empty() {
                    app.focus = Focus::Content;
                    dispatch_command(app, yute::commands::Command::Search(query), tx);
                }
            }
            KeyCode::Esc => app.focus = Focus::Content,
            KeyCode::Backspace => {
                app.search_input.pop();
                app.suggestion_index = None;
            }
            KeyCode::Up if !app.search_suggestions.is_empty() => {
                app.suggestion_index = Some(app.suggestion_index.unwrap_or(0).saturating_sub(1))
            }
            KeyCode::Down if !app.search_suggestions.is_empty() => {
                app.suggestion_index = Some(
                    (app.suggestion_index.map_or(0, |i| i + 1))
                        .min(app.search_suggestions.len() - 1),
                )
            }
            KeyCode::Char('u')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                app.search_input.clear()
            }
            KeyCode::Char('w')
                if key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                delete_word(&mut app.search_input)
            }
            KeyCode::Char(c)
                if !key.modifiers.intersects(
                    crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::ALT,
                ) =>
            {
                app.search_input.push(c);
                app.suggestion_index = None;
            }
            _ => {}
        }
        return;
    }

    app.which_key = false;
    let in_watch = app.screen == Screen::Watch && app.focus != Focus::Sidebar;
    let Some((action, count)) = keymap.handle_key(key, in_watch) else {
        return;
    };

    if action == Action::ItemActions {
        open_item_menu(app);
        return;
    }
    if action == Action::Quit {
        app.should_quit = true;
        return;
    }
    if matches!(action, Action::NavBackHistory | Action::NavForwardHistory) {
        if action == Action::NavBackHistory {
            app.nav_back();
        } else {
            app.nav_forward();
        }
        ensure_screen_loaded(app, tx);
        return;
    }
    if app.screen == Screen::Watch
        && app.focus != Focus::Sidebar
        && handle_watch_action(app, action, count, tx.clone())
    {
        return;
    }
    if app.screen == Screen::Help {
        match action {
            Action::Down => {
                app.help_scroll = app
                    .help_scroll
                    .saturating_add(count as u16)
                    .min(ui::help::HELP_LINES.saturating_sub(1) as u16)
            }
            Action::Up => app.help_scroll = app.help_scroll.saturating_sub(count as u16),
            Action::PageDown | Action::HalfPageDown => {
                app.help_scroll = app
                    .help_scroll
                    .saturating_add(10)
                    .min(ui::help::HELP_LINES.saturating_sub(1) as u16)
            }
            Action::PageUp | Action::HalfPageUp => {
                app.help_scroll = app.help_scroll.saturating_sub(10)
            }
            Action::First => app.help_scroll = 0,
            Action::Last => app.help_scroll = ui::help::HELP_LINES.saturating_sub(1) as u16,
            Action::Back | Action::Help => app.nav_back(),
            Action::ForceQuit => {
                app.player.stop();
                app.should_quit = true;
            }
            _ => {}
        }
        return;
    }

    let destination = match action {
        Action::NavHome => Some(Screen::Home),
        Action::NavWatch => Some(Screen::Watch),
        Action::NavSubs => Some(Screen::Subscriptions),
        Action::NavHistory => Some(Screen::History),
        Action::NavWatchLater => Some(Screen::WatchLater),
        Action::NavLiked => Some(Screen::Liked),
        Action::NavPlaylists => Some(Screen::Playlists),
        Action::NavQueue => Some(Screen::Queue),
        Action::NavLocal => Some(Screen::LocalVideos),
        Action::NavAccounts => Some(Screen::Accounts),
        Action::NavSettings => Some(Screen::Settings),
        _ => None,
    };
    if let Some(screen) = destination {
        navigate_to_screen(app, screen, tx);
        return;
    }
    if action == Action::CommandLine {
        app.focus = Focus::CommandLine;
        app.cmd_input.clear();
        app.completion_base = None;
        app.completion_index = 0;
        app.command_history.reset();
        return;
    }
    if action == Action::FocusSearch {
        app.focus = Focus::SearchInput;
        app.search_input.clear();
        app.suggestion_index = None;
        return;
    }
    use yute::commands::{Command as AppCommand, CopyTarget};
    let command = match action {
        Action::NavChannel => Some(AppCommand::Channel),
        Action::OpenBrowser => Some(AppCommand::Open(false)),
        Action::YankUrl => Some(AppCommand::Copy(CopyTarget::Url)),
        Action::QueueAppend => Some(AppCommand::QueueAdd(false)),
        Action::QueuePrepend => Some(AppCommand::QueueAdd(true)),
        Action::NextItem => Some(AppCommand::Next),
        Action::PrevItem => Some(AppCommand::Prev),
        Action::ToggleMute => Some(AppCommand::Mute),
        _ => None,
    };
    if let Some(command) = command {
        dispatch_command(app, command, tx);
        return;
    }
    if app.focus == Focus::Sidebar {
        match action {
            Action::Up => {
                app.sidebar_index = app.sidebar_index.saturating_sub(count);
                return;
            }
            Action::Down => {
                app.sidebar_index = app
                    .sidebar_index
                    .saturating_add(count)
                    .min(ui::sidebar::SIDEBAR_ITEMS.len().saturating_sub(1));
                return;
            }
            Action::First => {
                app.sidebar_index = 0;
                return;
            }
            Action::Last => {
                app.sidebar_index = ui::sidebar::SIDEBAR_ITEMS.len().saturating_sub(1);
                return;
            }
            Action::Back => {
                app.focus = Focus::Content;
                return;
            }
            _ => {}
        }
    }
    if matches!(action, Action::NextFilter | Action::PrevFilter) {
        if !matches!(
            app.screen,
            Screen::Watch
                | Screen::Settings
                | Screen::Accounts
                | Screen::Help
                | Screen::ChannelView
        ) {
            let kinds = yute::app::FilterKind::all();
            let current = kinds
                .iter()
                .position(|kind| *kind == app.filter_kind)
                .unwrap_or(0);
            let next = if action == Action::NextFilter {
                (current + 1) % kinds.len()
            } else {
                (current + kinds.len() - 1) % kinds.len()
            };
            set_content_filter(app, kinds[next], tx.clone());
        }
        return;
    }
    if app.screen == Screen::ChannelView && matches!(action, Action::PrevChip | Action::NextChip) {
        switch_channel_tab(app, action == Action::NextChip, tx);
        return;
    }
    if action == Action::FocusSidebar {
        focus_sidebar(app);
        return;
    }
    if action == Action::ToggleSidebar {
        app.sidebar_expanded = !app.sidebar_expanded;
        app.focus = if app.sidebar_expanded {
            Focus::Sidebar
        } else {
            Focus::Content
        };
        return;
    }
    if action == Action::Help {
        app.help_scroll = 0;
        app.set_screen(Screen::Help);
        return;
    }

    if app.focus == Focus::Chips {
        match action {
            Action::PlayDefault | Action::Down | Action::Back => {
                app.focus = Focus::Content;
                return;
            }
            Action::Left | Action::Right => {
                if app.screen == Screen::ChannelView {
                    switch_channel_tab(app, action == Action::Right, tx);
                } else {
                    let filters = yute::app::FilterKind::all();
                    let current = filters
                        .iter()
                        .position(|filter| *filter == app.filter_kind)
                        .unwrap_or(0);
                    let next = if action == Action::Right {
                        (current + 1) % filters.len()
                    } else {
                        (current + filters.len() - 1) % filters.len()
                    };
                    set_content_filter(app, filters[next], tx.clone());
                }
                return;
            }
            _ => {}
        }
    }
    for _ in 0..count {
        if app.screen == Screen::Settings && app.focus != Focus::Sidebar {
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
                    apply_stream_cache(app);
                    if app.config.save().is_err() {
                        app.status_text = "Could not save setting".into();
                    }
                }
                Action::Right | Action::PlayDefault => {
                    ui::settings::cycle_setting(app, true);
                    apply_stream_cache(app);
                    if app.config.save().is_err() {
                        app.status_text = "Could not save setting".into();
                    }
                }
                Action::Back | Action::Quit => {
                    app.nav_back();
                    ensure_screen_loaded(app, tx.clone());
                }
                Action::ForceQuit => {
                    app.should_quit = true;
                }
                Action::FocusSearch => {
                    app.search_input.clear();
                    app.suggestion_index = None;
                    app.focus = Focus::SearchInput;
                }
                _ => {}
            }
            return;
        }

        if app.screen == Screen::Accounts && app.focus != Focus::Sidebar {
            match action {
                Action::Up => {
                    app.accounts_cursor = app.accounts_cursor.saturating_sub(1);
                }
                Action::Down => {
                    app.accounts_cursor =
                        (app.accounts_cursor + 1).min(app.accounts.len().saturating_sub(1));
                }
                Action::PlayDefault | Action::Right => {
                    activate_account(app, app.accounts_cursor, tx.clone());
                }
                Action::Back | Action::Quit => {
                    app.nav_back();
                    ensure_screen_loaded(app, tx.clone());
                }
                Action::ForceQuit => {
                    app.should_quit = true;
                }
                Action::FocusSearch => {
                    app.search_input.clear();
                    app.suggestion_index = None;
                    app.focus = Focus::SearchInput;
                }
                _ => {}
            }
            return;
        }

        let grid_cols = app.rendered_columns;

        match action {
            Action::Quit => {
                app.should_quit = true;
            }
            Action::Back => {
                if app.screen == Screen::Watch {
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
                    if app.watch_right_mode == yute::app::WatchRightMode::Comments {
                        app.comments_cursor = app.comments_cursor.saturating_sub(1);
                    } else {
                        app.selected_index = app.selected_index.saturating_sub(1);
                    }
                } else if app.focus == Focus::Sidebar {
                    app.sidebar_index = app.sidebar_index.saturating_sub(1);
                } else {
                    app.selected_index = app.selected_index.saturating_sub(grid_cols);
                }
            }
            Action::Down => {
                if app.screen == Screen::Watch {
                    if app.watch_right_mode == yute::app::WatchRightMode::Comments {
                        let total_comments = app
                            .watch_data
                            .as_ref()
                            .map(|w| w.comments.len())
                            .unwrap_or(0);
                        if total_comments > 0 {
                            app.comments_cursor =
                                (app.comments_cursor + 1).min(total_comments.saturating_sub(1));
                        }
                    } else if let Some(watch) = &app.watch_data {
                        app.selected_index =
                            (app.selected_index + 1).min(watch.up_next.len().saturating_sub(1));
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
                if app.focus == Focus::Content && app.selected_index % grid_cols > 0 {
                    app.selected_index = app.selected_index.saturating_sub(1);
                }
            }
            Action::Right => {
                let total = app.filtered_items().len();
                if app.focus == Focus::Content
                    && app.selected_index % grid_cols + 1 < grid_cols
                    && app.selected_index + 1 < total
                {
                    app.selected_index += 1;
                }
            }
            Action::PageDown | Action::PageUp | Action::HalfPageDown | Action::HalfPageUp => {
                let rows = if matches!(action, Action::HalfPageDown | Action::HalfPageUp) {
                    (app.rendered_rows / 2).max(1)
                } else {
                    app.rendered_rows
                };
                let step = rows
                    .saturating_mul(app.rendered_columns)
                    .saturating_mul(count);
                if matches!(action, Action::PageUp | Action::HalfPageUp) {
                    app.selected_index = app.selected_index.saturating_sub(step);
                } else {
                    app.selected_index = app
                        .selected_index
                        .saturating_add(step)
                        .min(app.filtered_items().len().saturating_sub(1));
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
            Action::CycleFocusForward | Action::CycleFocusBackward => {
                let mut panes = Vec::with_capacity(3);
                if app.sidebar_expanded {
                    panes.push(Focus::Sidebar);
                }
                let show_filters = !matches!(
                    app.screen,
                    Screen::Watch | Screen::Settings | Screen::Accounts | Screen::Help
                );
                if show_filters {
                    panes.push(Focus::Chips);
                }
                panes.push(Focus::Content);
                let current = panes
                    .iter()
                    .position(|focus| *focus == app.focus)
                    .unwrap_or(0);
                let next = if action == Action::CycleFocusForward {
                    (current + 1) % panes.len()
                } else {
                    (current + panes.len() - 1) % panes.len()
                };
                app.focus = panes[next];
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
                app.search_input.clear();
                app.suggestion_index = None;
                app.focus = Focus::SearchInput;
            }
            Action::Filter => {
                set_content_filter(app, app.filter_kind.next(), tx.clone());
            }
            Action::ToggleViewMode => {
                app.toggle_view_mode();
                app.status_text = format!("View: {:?}", app.current_view_mode());
            }
            Action::CommandLine => {
                app.focus = Focus::CommandLine;
                app.cmd_input.clear();
                app.completion_base = None;
                app.completion_index = 0;
                app.command_history.reset();
            }
            Action::Refresh | Action::HardRefresh => {
                if app.continuation_failed && action == Action::Refresh {
                    app.continuation_failed = false;
                    maybe_continue_feed(app, tx.clone());
                } else {
                    refresh_current_feed(app, tx.clone());
                }
            }
            Action::PlayDefault => {
                if app.focus == Focus::Sidebar {
                    activate_sidebar_item(app, tx.clone());
                } else {
                    play_selected_card(app, tx.clone());
                }
            }
            Action::PlayWindow => {
                play_selected_card(app, tx.clone());
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
                dispatch_command(app, yute::commands::Command::Like(true), tx.clone())
            }
            Action::DislikeVideo => {
                if !authenticated(app) {
                    app.status_text = "Login required for ratings — use :login".into();
                } else if let Some(video) = &app.player.current_video {
                    perform_mutation(
                        app,
                        yute::actions::AccountMutation::Dislike {
                            video_id: video.id.clone(),
                        },
                        tx.clone(),
                    );
                }
            }
            Action::ToggleSubscribe => {
                let current =
                    if let Some(yute::app::DetailRoute::Channel(route)) = &app.detail_route {
                        route.subscribed
                    } else {
                        app.watch_data.as_ref().and_then(|w| w.is_subscribed)
                    };
                if let Some(current) = current {
                    dispatch_command(
                        app,
                        yute::commands::Command::Subscribe(!current),
                        tx.clone(),
                    );
                } else {
                    app.status_text =
                        "Subscription state unavailable — use :subscribe or :unsubscribe".into();
                }
            }
            Action::ToggleComments => {
                ensure_comments(app, tx.clone());
            }

            _ => {}
        }
    }
}

fn set_content_filter(app: &mut App, kind: yute::app::FilterKind, tx: Sender<AppEvent>) {
    use yute::app::{FilterKind, TabState};
    let source = if matches!(kind, FilterKind::Shorts | FilterKind::Live) {
        kind
    } else {
        FilterKind::All
    };
    let mut fetch = false;
    let mut restored = false;
    if app.screen == Screen::Home && source != app.home_source {
        if !app.items.is_empty() {
            app.home_pages.insert(
                app.home_source,
                TabState {
                    items: std::mem::take(&mut app.items),
                    continuation: app.continuation_token.take(),
                    selected: app.selected_index,
                    scroll: app.scroll_row,
                },
            );
        }
        app.feed_generation = app.feed_generation.wrapping_add(1);
        app.home_source = source;
        app.is_loading = false;
        app.is_loading_more = false;
        app.continuation_failed = false;
        if let Some(page) = app.home_pages.remove(&source) {
            app.items = page.items;
            app.continuation_token = page.continuation;
            app.selected_index = page.selected;
            app.scroll_row = page.scroll;
            restored = true;
        } else {
            app.items.clear();
            fetch = true;
        }
    }
    app.filter_kind = kind;
    app.filter_text.clear();
    if !restored {
        app.selected_index = 0;
        app.scroll_row = 0;
    }
    app.refresh_channel_items();
    app.status_text = format!("Filter: {}", kind.label());
    if fetch {
        let backend = Arc::clone(&app.backend);
        let generation = app.begin_feed_request();
        tokio::spawn(async move {
            let result = match source {
                FilterKind::Shorts => {
                    backend
                        .home_category(yute::backend::models::HomeCategory::Shorts)
                        .await
                }
                FilterKind::Live => {
                    backend
                        .home_category(yute::backend::models::HomeCategory::Live)
                        .await
                }
                _ => backend.home(None).await,
            };
            let _ = tx.send(AppEvent::FeedLoaded(generation, result)).await;
        });
    }
}

fn open_item_menu(app: &mut App) {
    let card = if app.screen == Screen::Watch && app.watch_pane != yute::app::WatchPane::Suggestions
    {
        app.player.current_video.clone().map(Card::Video)
    } else if app.focus == Focus::Content {
        app.selected_card().cloned()
    } else {
        None
    };
    let Some(card) = card else {
        app.status_text = "Select a video, playlist or channel first".into();
        return;
    };
    app.menu_request = app.menu_request.wrapping_add(1);
    app.action_menu = Some(yute::actions::ActionMenu {
        request: app.menu_request,
        card,
        cursor: 0,
        stage: yute::actions::MenuStage::Actions,
        formats: None,
        cancelled: yute_core::oauth::CancellationToken::new(),
        playlists: vec![],
        message: String::new(),
    });
}
fn resolve_menu_formats(app: &mut App, audio: bool, tx: Sender<AppEvent>) {
    let Some(menu) = &mut app.action_menu else {
        return;
    };
    menu.stage = yute::actions::MenuStage::LoadingFormats;
    menu.cursor = 0;
    menu.message.clear();
    let id = menu.card.id().to_string();
    let request = menu.request;
    let cancelled = menu.cancelled.clone();
    let resolver = app.format_resolver.clone();
    tokio::spawn(async move {
        let result = resolver.resolve(&id, cancelled).await;
        let _ = tx.send(AppEvent::MenuFormats(request, audio, result)).await;
    });
}
fn handle_item_menu(app: &mut App, key: crossterm::event::KeyEvent, tx: Sender<AppEvent>) {
    use crossterm::event::KeyModifiers;
    use yute::actions::{menu_entries, ItemAction, MenuStage};
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
    {
        app.player.stop();
        app.should_quit = true;
        return;
    }
    if matches!(key.code, KeyCode::Esc | KeyCode::Char('q')) {
        app.action_menu = None;
        return;
    }
    let auth = authenticated(app);
    let menu = app.action_menu.as_mut().unwrap();
    let entries = menu_entries(&menu.card, auth, app.download_active);
    let count = match menu.stage {
        MenuStage::Actions => entries.len(),
        MenuStage::PlayAs => 3,
        MenuStage::VideoFormats | MenuStage::AudioFormats => menu.media_rows().len(),
        _ => menu.playlists.len(),
    };
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            menu.cursor = (menu.cursor + 1).min(count.saturating_sub(1));
            menu.message.clear();
            return;
        }
        KeyCode::Char('k') | KeyCode::Up => {
            menu.cursor = menu.cursor.saturating_sub(1);
            menu.message.clear();
            return;
        }
        KeyCode::Enter => {}
        KeyCode::Char(c) if menu.stage == MenuStage::Actions => {
            if let Some(i) = entries.iter().position(|e| e.key == c) {
                menu.cursor = i;
            } else {
                return;
            }
        }
        _ => return,
    }
    if matches!(
        menu.stage,
        MenuStage::LoadingPlaylists | MenuStage::LoadingFormats
    ) {
        return;
    }
    if menu.stage == MenuStage::PlayAs {
        match menu.cursor {
            0 | 1 => {
                let audio = menu.cursor == 1;
                resolve_menu_formats(app, audio, tx);
            }
            _ => {
                let card = menu.card.clone();
                app.action_menu = None;
                dispatch_open(app, yute::actions::activation(&card), tx);
            }
        }
        return;
    }
    if matches!(
        menu.stage,
        MenuStage::VideoFormats | MenuStage::AudioFormats
    ) {
        let Some((_, selector)) = menu.media_rows().get(menu.cursor).cloned() else {
            return;
        };
        let Card::Video(video) = menu.card.clone() else {
            return;
        };
        let mut config = app.config.clone();
        if !selector.is_empty() {
            config.playback.max_resolution = "manual".into();
            config.playback.ytdl_format = selector;
        }
        app.action_menu = None;
        app.status_text = match app.player.play_video(video, &config) {
            Ok(()) => "Starting external MPV with selected format".into(),
            Err(_) => "Could not start MPV; run yute doctor".into(),
        };
        return;
    }
    if menu.stage == MenuStage::Playlists {
        let Some(playlist) = menu.playlists.get(menu.cursor) else {
            return;
        };
        let Card::Video(video) = &menu.card else {
            return;
        };
        let id = video.id.clone();
        let playlist = playlist.id.clone();
        let backend = Arc::clone(&app.backend);
        let generation = app
            .auth_generation
            .load(std::sync::atomic::Ordering::SeqCst);
        app.action_menu = None;
        app.status_text = "Saving to playlist…".into();
        tokio::spawn(async move {
            let result = backend.save_to_playlist(&playlist, &id).await;
            let _ = tx.send(AppEvent::PlaylistSaved(generation, result)).await;
        });
        return;
    }
    let Some(entry) = entries.get(menu.cursor) else {
        return;
    };
    if let Some(reason) = entry.unavailable {
        menu.message = reason.into();
        return;
    }
    let action = entry.action;
    let card = menu.card.clone();
    let request = menu.request;
    if action == ItemAction::PlaySelect {
        menu.stage = MenuStage::PlayAs;
        menu.cursor = 0;
        return;
    }
    if action == ItemAction::Audio {
        resolve_menu_formats(app, true, tx);
        return;
    }
    if action == ItemAction::Save {
        menu.stage = MenuStage::LoadingPlaylists;
        menu.message.clear();
        let backend = Arc::clone(&app.backend);
        let id = card.id().to_string();
        tokio::spawn(async move {
            let result = backend.playlist_destinations(&id).await;
            let _ = tx.send(AppEvent::MenuPlaylists(request, result)).await;
        });
        return;
    }
    app.action_menu = None;
    match action {
        ItemAction::PlaySelect | ItemAction::Audio => unreachable!("handled before closing menu"),
        ItemAction::Play => match card {
            Card::Playlist(p) => dispatch_open(app, yute::actions::OpenAction::PlayMix(p), tx),
            _ => dispatch_open(app, yute::actions::activation(&card), tx),
        },
        ItemAction::Browse => dispatch_open(app, yute::actions::activation(&card), tx),
        ItemAction::Browser => open_system_url(yute::actions::url(&card), tx),
        ItemAction::CopyUrl => copy_to_clipboard(yute::actions::url(&card), tx),
        ItemAction::CopyTitle => copy_to_clipboard(card.title().into(), tx),
        ItemAction::Queue => {
            if let Card::Video(v) = card {
                app.player.queue.push_back(v);
                sync_queue_view(app);
                app.status_text = "Added to queue".into();
            }
        }
        ItemAction::Channel => {
            if let Some(c) = yute::actions::channel_of(&card) {
                dispatch_open(app, yute::actions::OpenAction::OpenChannel(c), tx);
            }
        }
        ItemAction::Download => start_download(app, card, tx),
        ItemAction::CancelDownload => {
            app.download_request = app.download_request.wrapping_add(1);
            if let Some(task) = app.download_task.take() {
                task.abort();
            }
            app.download_active = false;
            app.status_text = "Download cancelled".into();
        }
        ItemAction::Save => {}
    }
}
fn open_system_url(url: String, tx: Sender<AppEvent>) {
    tokio::spawn(async move {
        let result = tokio::process::Command::new("xdg-open")
            .arg(url)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;
        let _ = tx
            .send(AppEvent::StatusMessage(
                if result.is_ok_and(|s| s.success()) {
                    "Opened in system browser"
                } else {
                    "Could not open URL — check xdg-open"
                }
                .into(),
            ))
            .await;
    });
}
fn start_download(app: &mut App, card: Card, tx: Sender<AppEvent>) {
    if app.download_active {
        return;
    }
    app.download_active = true;
    app.status_text = "Downloading with yt-dlp → ~/Videos/Yute".into();
    let config = app.config.clone();
    app.download_request = app.download_request.wrapping_add(1);
    let request = app.download_request;
    let task = tokio::spawn(async move {
        let dir = yute::config::resolve_path("~/Videos/Yute");
        let result = async {
            tokio::fs::create_dir_all(&dir).await?;
            let mut command = yute::actions::download_command(&card, &config, &dir);
            command.status().await.map(|s| s.success())
        }
        .await;
        let _ = tx
            .send(AppEvent::DownloadFinished(request, result.unwrap_or(false)))
            .await;
    });
    app.download_task = Some(task.abort_handle());
}

fn handle_watch_action(app: &mut App, action: Action, count: usize, tx: Sender<AppEvent>) -> bool {
    use yute::app::WatchPane;
    match action {
        Action::CycleFocusForward | Action::CycleFocusBackward | Action::Left | Action::Right => {
            let panes = [
                WatchPane::Description,
                WatchPane::Comments,
                WatchPane::Suggestions,
            ];
            let current = panes
                .iter()
                .position(|pane| *pane == app.watch_pane)
                .unwrap_or(0);
            let backward = matches!(action, Action::CycleFocusBackward | Action::Left);
            app.watch_pane = panes[(current + if backward { 2 } else { 1 }) % 3];
            if app.watch_pane == WatchPane::Comments {
                ensure_comments(app, tx);
            }
        }
        Action::ToggleDescription => app.watch_pane = WatchPane::Description,
        Action::ToggleComments => {
            app.watch_pane = WatchPane::Comments;
            ensure_comments(app, tx);
        }
        Action::FocusSuggestions => app.watch_pane = WatchPane::Suggestions,
        Action::Up | Action::Down => {
            let down = action == Action::Down;
            match app.watch_pane {
                WatchPane::Description => {
                    let step = count.min(1000) as u16;
                    app.description_scroll = if down {
                        app.description_scroll.saturating_add(step)
                    } else {
                        app.description_scroll.saturating_sub(step)
                    };
                }
                WatchPane::Comments => {
                    let len = app.watch_data.as_ref().map_or(0, |w| w.comments.len());
                    app.comments_cursor = if down {
                        app.comments_cursor
                            .saturating_add(count)
                            .min(len.saturating_sub(1))
                    } else {
                        app.comments_cursor.saturating_sub(count)
                    };
                    ensure_comments(app, tx);
                }
                WatchPane::Suggestions => {
                    let len = app.watch_suggestions().len();
                    app.selected_index = if down {
                        app.selected_index
                            .saturating_add(count)
                            .min(len.saturating_sub(1))
                    } else {
                        app.selected_index.saturating_sub(count)
                    };
                }
            }
        }
        Action::PlayDefault => match app.watch_pane {
            WatchPane::Suggestions => play_selected_card(app, tx),
            WatchPane::Comments => compose_comment(app, true),
            WatchPane::Description => open_description_link(app, tx),
        },
        Action::ComposeComment => compose_comment(app, false),
        Action::ReplyComment => {
            if app.watch_pane == WatchPane::Comments {
                compose_comment(app, true);
            } else if let Some(id) = app.player.current_video_id().map(str::to_string) {
                trigger_watch_fetch(app, tx, id);
            }
        }
        Action::OpenDescriptionLink => open_description_link(app, tx),
        Action::PrevChip | Action::NextChip => {
            let len = ui::watch::description_links(app).len();
            if len > 0 {
                app.watch_link_index = if action == Action::NextChip {
                    (app.watch_link_index + 1) % len
                } else {
                    (app.watch_link_index + len - 1) % len
                };
            }
        }
        Action::LikeVideo | Action::DislikeVideo => {
            if app.watch_pane == WatchPane::Comments {
                react_comment(app, action == Action::LikeVideo, tx);
            } else if action == Action::LikeVideo {
                let liked = app.watch_data.as_ref().and_then(|w| w.is_liked) == Some(true);
                dispatch_command(app, yute::commands::Command::Like(!liked), tx);
            } else if app.watch_data.as_ref().and_then(|w| w.is_disliked) == Some(true) {
                dispatch_command(app, yute::commands::Command::Like(false), tx);
            } else if !authenticated(app) {
                app.status_text = "Login required for ratings — use :login".into();
            } else if let Some(video_id) = app.player.current_video_id().map(str::to_string) {
                perform_mutation(
                    app,
                    yute::actions::AccountMutation::Dislike { video_id },
                    tx,
                );
            }
        }
        _ => return false,
    }
    true
}

fn ensure_comments(app: &mut App, tx: Sender<AppEvent>) {
    if app.comments_loading {
        return;
    }
    let Some(watch) = &app.watch_data else {
        return;
    };
    if !watch.comments.is_empty() && app.comments_cursor + 2 < watch.comments.len() {
        return;
    }
    let Some(token) = watch.comments_token.clone() else {
        return;
    };
    let id = watch.video.id.clone();
    let backend = Arc::clone(&app.backend);
    app.comments_loading = true;
    let account_generation = app
        .auth_generation
        .load(std::sync::atomic::Ordering::SeqCst);
    tokio::spawn(async move {
        let result = backend.fetch_comments(&token).await;
        let _ = tx
            .send(AppEvent::AccountResult(
                account_generation,
                Box::new(AppEvent::CommentsLoaded(id, result)),
            ))
            .await;
    });
}
fn compose_comment(app: &mut App, reply: bool) {
    if !authenticated(app) {
        app.status_text = "Sign in with :login before posting comments".into();
        return;
    }
    let Some(watch) = &app.watch_data else {
        return;
    };
    let (params, author) = if reply {
        let comment = watch.comments.get(app.comments_cursor);
        (
            comment.and_then(|c| c.reply_params.clone()),
            comment.map(|c| c.author.clone()),
        )
    } else {
        (watch.comment_create_params.clone(), None)
    };
    let Some(params) = params else {
        app.status_text = "Posting is unavailable for this YouTube client — use :open".into();
        return;
    };
    app.comment_draft = Some(yute::app::CommentDraft {
        video_id: watch.video.id.clone(),
        params,
        reply,
        author,
    });
    app.focus = Focus::CommentInput;
    app.input_cursor = None;
    app.status_text = "Enter posts · Esc cancels".into();
}
fn post_comment_draft(app: &mut App, tx: Sender<AppEvent>) {
    if app.comment_posting {
        return;
    }
    let Some(draft) = app.comment_draft.clone() else {
        return;
    };
    if app.screen != Screen::Watch || app.player.current_video_id() != Some(draft.video_id.as_str())
    {
        app.status_text = "Open the original video before sending this draft".into();
        return;
    }
    if app.comment_input.trim().is_empty() || app.comment_input.chars().count() > 10000 {
        app.status_text = "Comment must contain 1 to 10000 characters".into();
        return;
    }
    let text = app.comment_input.clone();
    let backend = Arc::clone(&app.backend);
    let generation = app.feed_generation;
    app.comment_posting = true;
    app.status_text = "Posting comment…".into();
    tokio::spawn(async move {
        let result = backend
            .submit_comment(&draft.params, &text, draft.reply)
            .await;
        let _ = tx
            .send(AppEvent::CommentUpdated(
                generation,
                draft.video_id,
                true,
                result,
            ))
            .await;
    });
}
fn react_comment(app: &mut App, like: bool, tx: Sender<AppEvent>) {
    if !authenticated(app) {
        app.status_text = "Login required for comment reactions — use :login".into();
        return;
    }
    let Some(watch) = &app.watch_data else {
        return;
    };
    let action = watch.comments.get(app.comments_cursor).and_then(|c| {
        if like {
            c.like_action.clone()
        } else {
            c.dislike_action.clone()
        }
    });
    let Some(action) = action else {
        app.status_text = "Comment reactions unavailable for this client — use :open".into();
        return;
    };
    let id = watch.video.id.clone();
    let generation = app.feed_generation;
    let backend = Arc::clone(&app.backend);
    tokio::spawn(async move {
        let result = backend.comment_action(&action).await;
        let _ = tx
            .send(AppEvent::CommentUpdated(generation, id, false, result))
            .await;
    });
}
fn open_description_link(app: &mut App, tx: Sender<AppEvent>) {
    let links = ui::watch::description_links(app);
    let Some((_, url)) = links.get(app.watch_link_index % links.len().max(1)) else {
        app.status_text = "No description links available".into();
        return;
    };
    let url = url.clone();
    tokio::spawn(async move {
        let result = tokio::process::Command::new("xdg-open")
            .arg(url)
            .status()
            .await;
        let message = if result.is_ok_and(|s| s.success()) {
            "Opened description link"
        } else {
            "Could not open link — check xdg-open"
        };
        let _ = tx.send(AppEvent::StatusMessage(message.into())).await;
    });
}
fn mutation_error_message(error: &anyhow::Error) -> String {
    let diagnostic = error.to_string().to_ascii_lowercase();
    if diagnostic.contains("401") || diagnostic.contains("403") || diagnostic.contains("expired") {
        "YouTube rejected this session — use :login, then retry".into()
    } else if diagnostic.contains("unavailable") || diagnostic.contains("unsupported") {
        "Action unavailable for this YouTube client — use :open".into()
    } else {
        "YouTube could not save the action — check :account and retry".into()
    }
}

fn focus_sidebar(app: &mut App) {
    if app.sidebar_expanded && app.focus == Focus::Sidebar {
        app.sidebar_expanded = false;
        app.focus = Focus::Content;
    } else {
        app.sidebar_expanded = true;
        app.focus = Focus::Sidebar;
    }
}

fn activate_sidebar_item(app: &mut App, tx: Sender<AppEvent>) {
    if let Some((_, screen, _)) = ui::sidebar::SIDEBAR_ITEMS.get(app.sidebar_index) {
        navigate_to_screen(app, *screen, tx);
    }
}

fn navigate_to_screen(app: &mut App, screen: Screen, tx: Sender<AppEvent>) {
    app.focus = Focus::Content;
    if screen == app.screen && !app.items.is_empty() {
        return;
    }
    app.set_screen(screen);
    if app.anonymous_library() {
        app.items.clear();
        app.is_loading = false;
        app.status_text = "Sign in with :account add".into();
        return;
    }
    if let Some(index) = ui::sidebar::index_of(screen) {
        app.sidebar_index = index;
    }
    match screen {
        Screen::LocalVideos => {
            app.items = scan_local_videos(&app.config.local.dirs).items;
            return;
        }
        Screen::Queue => {
            sync_queue_view(app);
            return;
        }
        Screen::Settings | Screen::Accounts => {
            app.status_text.clear();
            return;
        }
        _ => {}
    }
    let cache = match screen {
        Screen::Home => match app.home_source {
            yute::app::FilterKind::Shorts => "feed_home_shorts",
            yute::app::FilterKind::Live => "feed_home_live",
            _ => "feed_home",
        },
        Screen::Subscriptions => "feed_subs",
        Screen::History => "feed_history",
        Screen::WatchLater => "feed_watchlater",
        Screen::Liked => "feed_liked",
        Screen::Playlists => "feed_playlists",
        _ => return,
    };
    try_load_feed_cache(app, cache);
    let backend = Arc::clone(&app.backend);
    let generation = app.begin_feed_request();
    tokio::spawn(async move {
        let result = match screen {
            Screen::Home => backend.home(None).await,
            Screen::Subscriptions => backend.subscriptions().await,
            Screen::History => backend.history().await,
            Screen::WatchLater => backend.watch_later().await,
            Screen::Liked => backend.playlist("LL").await,
            Screen::Playlists => backend.playlists().await,
            _ => unreachable!(),
        };
        let _ = tx.send(AppEvent::FeedLoaded(generation, result)).await;
    });
}

fn sync_queue_view(app: &mut App) {
    if app.screen == Screen::Queue {
        app.items = app
            .player
            .queue
            .items()
            .iter()
            .cloned()
            .map(Card::Video)
            .collect();
        app.selected_index = app.selected_index.min(app.items.len().saturating_sub(1));
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

fn play_selected_card(app: &mut App, tx: Sender<AppEvent>) {
    let Some(card) = app.selected_card() else {
        return;
    };
    dispatch_open(app, yute::actions::activation(card), tx);
}

fn dispatch_open(app: &mut App, action: yute::actions::OpenAction, tx: Sender<AppEvent>) {
    use yute::actions::OpenAction;
    match action {
        OpenAction::Play(video) => start_playback(app, video, tx),
        OpenAction::PlayMix(playlist) => {
            app.transition(Screen::PlaylistView, true);
            let id = playlist.id.clone();
            app.detail_route = Some(yute::app::DetailRoute::Playlist(playlist));
            let backend = Arc::clone(&app.backend);
            let generation = app.begin_feed_request();
            tokio::spawn(async move {
                let result = backend.playlist(&id).await;
                let _ = tx.send(AppEvent::MixLoaded(generation, result)).await;
            });
        }
        OpenAction::OpenPlaylist(playlist) => {
            app.transition(Screen::PlaylistView, true);
            let id = playlist.id.clone();
            app.detail_route = Some(yute::app::DetailRoute::Playlist(playlist));
            let backend = Arc::clone(&app.backend);
            let generation = app.begin_feed_request();
            tokio::spawn(async move {
                let result = backend.playlist(&id).await;
                let _ = tx.send(AppEvent::FeedLoaded(generation, result)).await;
            });
        }
        OpenAction::OpenChannel(channel) => {
            app.transition(Screen::ChannelView, true);
            app.detail_route = Some(yute::app::DetailRoute::Channel(Box::new(
                yute::app::ChannelState {
                    channel,
                    tabs: Vec::new(),
                    active: 0,
                    subscribed: None,
                    pages: Default::default(),
                },
            )));
            fetch_channel(app, tx);
        }
        OpenAction::Unavailable(message) => app.status_text = message.into(),
    }
}

fn start_playback(app: &mut App, video: VideoCard, tx: Sender<AppEvent>) {
    let id = video.id.clone();
    let previous = app.player.current_video.clone();
    if let Err(_error) = app.player.play_video(video, &app.config) {
        app.status_text = "Could not start MPV — check that mpv and yt-dlp are installed".into();
        return;
    }
    if let Some(previous) = previous.filter(|v| v.id != id) {
        if app.playback_history.len() >= 50 {
            app.playback_history.remove(0);
        }
        app.playback_history.push(previous);
    }
    app.watch_data = None;
    app.watch_pane = yute::app::WatchPane::Description;
    app.description_scroll = 0;
    app.watch_link_index = 0;
    app.comments_cursor = 0;
    app.comments_loading = false;
    app.comment_draft = None;
    app.comment_input.clear();
    app.comment_posting = false;
    app.watch_tracker = Some(yute_core::player::tracking::WatchTracker::new(&id));
    app.set_screen(Screen::Watch);
    app.selected_index = 0;
    trigger_watch_fetch(app, tx, id);
}

fn fetch_channel(app: &mut App, tx: Sender<AppEvent>) {
    let Some(yute::app::DetailRoute::Channel(route)) = &app.detail_route else {
        return;
    };
    let id = route.channel.id.clone();
    let params = route.tabs.get(route.active).and_then(|t| t.params.clone());
    let backend = Arc::clone(&app.backend);
    let generation = app.begin_feed_request();
    tokio::spawn(async move {
        let result = backend.channel_page(&id, params.as_deref()).await;
        let _ = tx
            .send(AppEvent::ChannelLoaded(generation, id, result))
            .await;
    });
}

fn switch_channel_tab(app: &mut App, forward: bool, tx: Sender<AppEvent>) {
    let Some(yute::app::DetailRoute::Channel(route)) = &mut app.detail_route else {
        return;
    };
    if route.tabs.is_empty() || app.is_loading {
        return;
    }
    let old = route.tabs[route.active].params.clone();
    route.pages.insert(
        old,
        yute::app::TabState {
            items: std::mem::take(&mut app.items),
            continuation: app.continuation_token.take(),
            selected: app.selected_index,
            scroll: app.scroll_row,
        },
    );
    route.active = if forward {
        (route.active + 1) % route.tabs.len()
    } else {
        (route.active + route.tabs.len() - 1) % route.tabs.len()
    };
    app.filter_kind = yute::app::FilterKind::All;
    app.filter_text.clear();
    let params = route.tabs[route.active].params.clone();
    if let Some(page) = route.pages.remove(&params) {
        app.items = page.items;
        app.continuation_token = page.continuation;
        app.selected_index = page.selected;
        app.scroll_row = page.scroll;
        app.feed_generation = app.feed_generation.wrapping_add(1);
        app.is_loading_more = false;
    } else {
        app.selected_index = 0;
        app.scroll_row = 0;
        fetch_channel(app, tx);
    }
}

fn execute_command(app: &mut App, tx: Sender<AppEvent>) {
    let input = app.cmd_input.clone();
    match yute::commands::parse(&input) {
        Ok(command) => {
            app.command_history.record(&input);
            dispatch_command(app, command, tx);
        }
        Err(error) => app.status_text = error,
    }
}

fn handle_app_event(app: &mut App, event: AppEvent, tx: Sender<AppEvent>) {
    match event {
        AppEvent::AccountResult(generation, event) => {
            if generation
                == app
                    .auth_generation
                    .load(std::sync::atomic::Ordering::SeqCst)
            {
                handle_app_event(app, *event, tx);
            }
        }
        AppEvent::MenuFormats(request, audio, result) => {
            let Some(menu) = &mut app.action_menu else {
                return;
            };
            if menu.request != request || menu.stage != yute::actions::MenuStage::LoadingFormats {
                return;
            }
            match result {
                Ok(formats) => {
                    menu.formats = Some(formats);
                    menu.stage = if audio {
                        yute::actions::MenuStage::AudioFormats
                    } else {
                        yute::actions::MenuStage::VideoFormats
                    };
                    menu.message = "Manual selection overrides defaults for this playback".into();
                }
                Err(error) => {
                    menu.stage = yute::actions::MenuStage::Actions;
                    menu.message = error.to_string();
                }
            }
        }
        AppEvent::MenuPlaylists(request, result) => {
            let Some(menu) = &mut app.action_menu else {
                return;
            };
            if menu.request != request || menu.stage != yute::actions::MenuStage::LoadingPlaylists {
                return;
            }
            menu.stage = yute::actions::MenuStage::Playlists;
            menu.cursor = 0;
            match result {
                Ok(page) => {
                    menu.playlists = page
                        .items
                        .into_iter()
                        .filter_map(|c| {
                            if let Card::Playlist(p) = c {
                                Some(p)
                            } else {
                                None
                            }
                        })
                        .collect()
                }
                Err(_) => menu.message = "Could not load playlists · Esc closes, then retry".into(),
            }
        }
        AppEvent::PlaylistSaved(generation, result) => {
            if generation
                != app
                    .auth_generation
                    .load(std::sync::atomic::Ordering::SeqCst)
            {
                return;
            }
            app.status_text = match result {
                Ok(()) => "Saved to playlist".into(),
                Err(error) => mutation_error_message(&error),
            };
        }
        AppEvent::DownloadFinished(request, ok) => {
            if request != app.download_request {
                return;
            }
            app.download_task = None;
            app.download_active = false;
            app.status_text = if ok {
                "Download complete · ~/Videos/Yute · :local to browse"
            } else {
                "Download failed · check yt-dlp, FFmpeg, disk space and account access"
            }
            .into();
        }

        AppEvent::MutationFinished(generation, mutation, result) => {
            if generation != app.feed_generation {
                return;
            }
            match result {
                Err(error) => app.status_text = mutation_error_message(&error),
                Ok(()) => match mutation {
                    yute::actions::AccountMutation::Dislike { video_id } => {
                        if let Some(watch) = &mut app.watch_data {
                            if watch.video.id == video_id {
                                watch.is_disliked = Some(true);
                                watch.is_liked = Some(false);
                            }
                        }
                        app.status_text = "Disliked video".into();
                    }
                    yute::actions::AccountMutation::Subscribe {
                        channel_id,
                        subscribed,
                    } => {
                        if let Some(yute::app::DetailRoute::Channel(route)) = &mut app.detail_route
                        {
                            if route.channel.id == channel_id {
                                route.subscribed = Some(subscribed);
                            }
                        }
                        if let Some(watch) = &mut app.watch_data {
                            if watch.video.channel_id.as_deref() == Some(&channel_id) {
                                watch.is_subscribed = Some(subscribed);
                            }
                        }
                        app.status_text = if subscribed {
                            "Subscribed"
                        } else {
                            "Unsubscribed"
                        }
                        .into();
                    }
                    yute::actions::AccountMutation::Like { video_id, liked } => {
                        if let Some(watch) = &mut app.watch_data {
                            if watch.video.id == video_id {
                                watch.is_liked = Some(liked);
                                watch.is_disliked = Some(false);
                            }
                        }
                        app.status_text = if liked {
                            "Liked video"
                        } else {
                            "Removed rating"
                        }
                        .into();
                    }
                },
            }
        }
        AppEvent::ChannelLoaded(generation, id, result) => {
            if generation != app.feed_generation {
                return;
            }
            app.is_loading = false;
            let Some(yute::app::DetailRoute::Channel(route)) = &mut app.detail_route else {
                return;
            };
            if route.channel.id != id {
                return;
            }
            match result {
                Ok(page) => {
                    let selected_id = app
                        .items
                        .get(app.selected_index)
                        .map(|c| c.id().to_string());
                    if !page.channel.title.is_empty() {
                        route.channel.title = page.channel.title;
                    }
                    if page.channel.handle.is_some() {
                        route.channel.handle = page.channel.handle;
                    }
                    if page.channel.subscribers.is_some() {
                        route.channel.subscribers = page.channel.subscribers;
                    }
                    if page.channel.avatar_url.is_some() {
                        route.channel.avatar_url = page.channel.avatar_url;
                    }
                    if !page.tabs.is_empty() {
                        route.active = page.tabs.iter().position(|t| t.selected).unwrap_or(0);
                        route.tabs = page.tabs;
                    }
                    route.subscribed = page.subscribed;
                    app.items = page.feed.items;
                    if let Some(id) = selected_id {
                        if let Some(index) = app.items.iter().position(|c| c.id() == id) {
                            app.selected_index = index;
                        }
                    }
                    app.continuation_token = page.feed.continuation;
                    app.selected_index = app.selected_index.min(app.items.len().saturating_sub(1));
                    app.status_text.clear();
                }
                Err(error) => app.status_text = feed_error_message(&error).into(),
            }
        }
        AppEvent::MixLoaded(generation, result) => {
            if generation != app.feed_generation || app.screen != Screen::PlaylistView {
                return;
            }
            let videos = result
                .as_ref()
                .ok()
                .map(|page| {
                    page.items
                        .iter()
                        .filter_map(|card| match yute::actions::activation(card) {
                            yute::actions::OpenAction::Play(video) => Some(video),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            handle_app_event(app, AppEvent::FeedLoaded(generation, result), tx.clone());
            if let Some(first) = videos.first() {
                start_playback(app, first.clone(), tx);
                if app.player.is_alive() && app.player.current_video_id() == Some(first.id.as_str())
                {
                    // Keep existing user queue entries after the real Mix batch.
                    for video in videos.iter().skip(1).rev() {
                        app.player.queue.push_front(video.clone());
                    }
                }
            } else if !app.is_loading {
                app.status_text = "Could not load this Mix — press r to retry".into();
            }
        }
        AppEvent::CardMetadataLoaded(generation, id, video) => {
            if generation != app.feed_generation || app.screen != Screen::Home {
                return;
            }
            app.metadata_pending.remove(&id);
            if let Some(metadata) = video {
                if metadata.id != id {
                    return;
                }
                if let Some(Card::Video(video)) = app.items.iter_mut().find(|card| card.id() == id)
                {
                    if video.views.is_none() {
                        video.views = metadata.views;
                    }
                    if video.published.is_none() {
                        video.published = metadata.published;
                    }
                }
            }
            if app.metadata_pending.is_empty() {
                let page = yute::backend::models::FeedPage {
                    chips: app.chips.clone(),
                    items: app.items.clone(),
                    continuation: app.continuation_token.clone(),
                };
                if let Ok(json) = serde_json::to_string(&page) {
                    let _ = app
                        .thumbs
                        .cache()
                        .save_feed_cache(&feed_cache_key(app, "feed_home"), &json);
                }
            }
        }
        AppEvent::FeedLoaded(generation, res) => {
            if generation != app.feed_generation {
                return;
            }
            app.is_loading = false;
            match res {
                Ok(page) => {
                    if !page.chips.is_empty() {
                        app.chips = page.chips.clone();
                    }
                    let selected_id = app.selected_card().map(|c| c.id().to_string());
                    app.items = page.items.clone();
                    if let Some(id) = selected_id {
                        if let Some(index) = app.filtered_items().iter().position(|c| c.id() == id)
                        {
                            app.selected_index = index;
                        }
                    }
                    app.selected_index = app
                        .selected_index
                        .min(app.filtered_items().len().saturating_sub(1));
                    app.continuation_token = page.continuation.clone();
                    app.continuation_failed = false;
                    app.status_text = format!("Loaded {} items", app.items.len());

                    let cache_key = match app.screen {
                        Screen::Home => match app.home_source {
                            yute::app::FilterKind::Shorts => "feed_home_shorts",
                            yute::app::FilterKind::Live => "feed_home_live",
                            _ => "feed_home",
                        },
                        Screen::Subscriptions => "feed_subs",
                        Screen::History => "feed_history",
                        Screen::Playlists => "feed_playlists",
                        Screen::WatchLater => "feed_watchlater",
                        Screen::Liked => "feed_liked",
                        _ => "",
                    };
                    if !cache_key.is_empty() && !page.items.is_empty() {
                        let final_key = feed_cache_key(app, cache_key);
                        if let Ok(json) = serde_json::to_string(&page) {
                            let _ = app.thumbs.cache().save_feed_cache(&final_key, &json);
                        }
                    }
                }
                Err(err) => {
                    app.status_text = feed_error_message(&err).to_string();
                }
            }
        }
        AppEvent::FeedContinued(generation, screen, account, token, result) => {
            let _ = account;
            if generation != app.feed_generation
                || screen != app.screen
                || app.continuation_token.as_deref() != Some(&token)
            {
                return;
            }
            app.is_loading_more = false;
            match result {
                Ok(page) => {
                    let before = app.items.len();
                    let mut ids: std::collections::HashSet<String> =
                        app.items.iter().map(|c| c.id().to_string()).collect();
                    app.items.extend(
                        page.items
                            .into_iter()
                            .filter(|c| ids.insert(c.id().to_string())),
                    );
                    app.continuation_token = page
                        .continuation
                        .filter(|next| next != &token && app.items.len() > before);
                    app.status_text = format!("Loaded {} more items", app.items.len() - before);
                }
                Err(error) => {
                    app.continuation_failed = true;
                    app.status_text = feed_error_message(&error).to_string();
                }
            }
        }
        AppEvent::WatchDataLoaded(id, mut res) => {
            if app.player.current_video_id() != Some(id.as_str()) {
                return;
            }
            if let Ok(ref mut data) = res {
                data.up_next.retain(|card| card.id() != id);
            }
            if let Ok(wd) = res {
                if app.player.current_video_id() == Some(wd.video.id.as_str()) {
                    app.watch_data = Some(wd);
                    ensure_comments(app, tx);
                }
            }
        }
        AppEvent::CommentsLoaded(id, result) => {
            if app.player.current_video_id() != Some(id.as_str()) {
                return;
            }
            app.comments_loading = false;
            match result {
                Ok(batch) => {
                    if let Some(watch) = &mut app.watch_data {
                        let mut ids = watch
                            .comments
                            .iter()
                            .map(|c| c.id.clone())
                            .collect::<std::collections::HashSet<_>>();
                        watch.comments.extend(
                            batch
                                .comments
                                .into_iter()
                                .filter(|c| ids.insert(c.id.clone())),
                        );
                        watch.comments_token = batch
                            .continuation
                            .filter(|token| watch.comments_token.as_ref() != Some(token));
                        if batch.create_params.is_some() {
                            watch.comment_create_params = batch.create_params;
                        }
                    }
                }
                Err(_) => app.status_text = "Comments could not load — press c to retry".into(),
            }
        }
        AppEvent::CommentUpdated(generation, id, posted, result) => {
            if generation != app.feed_generation
                || app.player.current_video_id() != Some(id.as_str())
            {
                return;
            }
            app.comment_posting = false;
            match result {
                Ok(()) => {
                    app.focus = Focus::Content;
                    app.comment_draft = None;
                    app.comment_input.clear();
                    app.status_text = if posted {
                        "Comment posted"
                    } else {
                        "Comment reaction saved"
                    }
                    .into();
                    trigger_watch_fetch(app, tx, id);
                }
                Err(error) => {
                    app.status_text = mutation_error_message(&error);
                    if app.comment_draft.is_some() {
                        app.focus = Focus::CommentInput;
                    }
                }
            }
        }
        AppEvent::StatusMessage(msg) => {
            app.status_text = msg;
        }
        AppEvent::TerminalCells(width, height) => {
            app.cell_w_px = width as f32;
            app.cell_h_px = height as f32;
            app.thumbs.refresh_cell_size(width, height);
            yute::terminal::remember_cells(width, height);
        }
        AppEvent::CredentialsWaiting(generation) => {
            if generation
                == app
                    .auth_generation
                    .load(std::sync::atomic::Ordering::SeqCst)
            {
                app.status_text =
                    "Unlock the system keyring to load saved accounts · Search remains available"
                        .into();
            }
        }
        AppEvent::CredentialsLoaded(generation, result) => {
            app.credentials_pending = false;
            if generation
                != app
                    .auth_generation
                    .load(std::sync::atomic::Ordering::SeqCst)
            {
                return;
            }
            app.credentials_loaded = result.is_ok();
            match result {
                Ok(accounts) => {
                    let next = accounts.current_context();
                    let changed = app.account_store.active_id() != accounts.active_id()
                        || app.auth.lock().unwrap().access_token != next.access_token;
                    app.accounts = accounts.profiles.clone();
                    app.active_account_index = accounts.active_index();
                    app.account_name = app.accounts[app.active_account_index].name.clone();
                    app.account_handle = app.accounts[app.active_account_index].handle.clone();
                    app.account_store = accounts;
                    if changed {
                        app.auth.lock().unwrap().cancelled.cancel();
                        app.auth = Arc::new(std::sync::Mutex::new(next));
                        app.rebuild_backend();
                        app.feed_generation = app.feed_generation.wrapping_add(1);
                        app.items.clear();
                        app.continuation_token = None;
                        if app.screen == Screen::Home {
                            try_load_feed_cache(app, "feed_home");
                            trigger_home_fetch(app, tx.clone(), None);
                        } else if app.screen == Screen::Search && !app.search_query.is_empty() {
                            trigger_search_fetch(app, tx.clone(), app.search_query.clone());
                        } else {
                            ensure_screen_loaded(app, tx.clone());
                        }
                    }
                    refresh_account_profiles(app, tx);
                }
                Err(_) => {
                    app.status_text =
                        "Credentials locked or unreadable; unlock the keyring or check YUTE_KEYFILE"
                            .into()
                }
            }
        }
        AppEvent::AccountRemoved(id, result) => {
            if result.is_ok() {
                let was_active = app.account_store.active_id() == id;
                app.account_store.forget(&id);
                app.accounts = app.account_store.profiles.clone();
                if was_active {
                    activate_account(app, 0, tx.clone());
                    app.account_store.forget(&id);
                }
                app.active_account_index = app.account_store.active_index();
                app.accounts_cursor = app.active_account_index;
                app.status_text = "Account removed locally".into();
            } else {
                app.status_text = "Could not remove saved credentials".into();
            }
        }
        AppEvent::LoginDisplay(generation, event) => {
            if generation
                != app
                    .auth_generation
                    .load(std::sync::atomic::Ordering::SeqCst)
            {
                return;
            }
            if let Some(modal) = &mut app.login_modal {
                modal.update(event);
            }
        }
        AppEvent::LoginStatus(generation, message) => {
            if generation
                == app
                    .auth_generation
                    .load(std::sync::atomic::Ordering::SeqCst)
            {
                app.login_modal = None;
                app.status_text = message;
            }
        }
        AppEvent::LoginSucceeded(generation, name, handle) => {
            if generation
                != app
                    .auth_generation
                    .load(std::sync::atomic::Ordering::SeqCst)
            {
                return;
            }
            app.login_modal = None;
            app.config.backend.auth = "tv_oauth".into();
            hydrate_accounts(app, tx.clone());
            let saved = app.config.save().is_ok();
            if let Ok(mut auth) = app.auth.lock() {
                auth.auth_file_path = Some(yute_core::credentials::path());
            }
            app.account_name = if name.is_empty() {
                "Signed in".into()
            } else {
                name
            };
            app.account_handle = handle;
            app.nav_history.clear();
            app.nav_future.clear();
            app.screen = Screen::Home;
            app.detail_route = None;
            app.items.clear();
            app.selected_index = 0;
            app.scroll_row = 0;
            app.filter_kind = yute::app::FilterKind::All;
            app.filter_text.clear();
            app.status_text = if saved {
                "Signed in"
            } else {
                "Signed in; could not save login mode"
            }
            .into();
            refresh_account_profiles(app, tx.clone());
            trigger_home_fetch(app, tx, None);
        }
        AppEvent::AccountProfiles(generation, result) => {
            if generation
                != app
                    .auth_generation
                    .load(std::sync::atomic::Ordering::SeqCst)
            {
                return;
            }
            if let Ok(profiles) = result {
                app.account_store
                    .import_profiles(profiles, &app.auth.lock().unwrap());
                app.accounts = app.account_store.profiles.clone();
                app.active_account_index = app.account_store.active_index();
                if let Some(profile) = app.accounts.get(app.active_account_index) {
                    app.account_name = profile.name.clone();
                    app.account_handle = profile.handle.clone();
                    app.auth.lock().unwrap().page_id = profile.channel_id.clone();
                }
                app.account_store.persist_background();
            }
        }
        AppEvent::AccountLoaded(generation, res) => {
            if generation
                != app
                    .auth_generation
                    .load(std::sync::atomic::Ordering::SeqCst)
            {
                return;
            }
            if let Ok((name, handle)) = res {
                if !name.is_empty() {
                    app.account_name = name;
                }
                if !handle.is_empty() {
                    app.account_handle = handle;
                }
                app.account_store
                    .update_profile(&app.account_name, &app.account_handle);
                app.accounts = app.account_store.profiles.clone();
                if let Some(ch) = app.account_name.chars().next() {
                    app.account_initial = ch.to_uppercase().to_string();
                }
            }
        }
        _ => {}
    }
}

fn try_load_feed_cache(app: &mut App, key: &str) {
    if !app.items.is_empty() {
        return;
    }
    let final_key = feed_cache_key(app, key);
    let ttl_sec = 0; // Stale-while-revalidate: age never prevents first paint.
    let json = app.thumbs.cache().load_feed_cache(&final_key, ttl_sec);
    if let Some(json) = json {
        if let Ok(page) = serde_json::from_str::<yute::backend::models::FeedPage>(&json) {
            if !page.items.is_empty() {
                app.items = page
                    .items
                    .into_iter()
                    .filter(|c| !matches!(c, Card::Video(v) if v.id.starts_with("UC") && v.id.len() != 11))
                    .collect();
                app.continuation_token = page.continuation;
                if !page.chips.is_empty() {
                    app.chips = page.chips;
                }
                app.status_text = format!("Loaded {} items (cached)", app.items.len());
            }
        }
    }
}

fn ensure_screen_loaded(app: &mut App, tx: Sender<AppEvent>) {
    if app.anonymous_library() {
        app.is_loading = false;
        app.items.clear();
        app.status_text = "Sign in with :account add".into();
        return;
    }
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
                let generation = app.begin_feed_request();
                tokio::spawn(async move {
                    let res = backend.subscriptions().await;
                    let _ = tx.send(AppEvent::FeedLoaded(generation, res)).await;
                });
            }
        }
        Screen::History => {
            try_load_feed_cache(app, "feed_history");
            if app.items.is_empty() {
                app.is_loading = true;
                app.status_text = "Loading History...".to_string();
                let backend = Arc::clone(&app.backend);
                let generation = app.begin_feed_request();
                tokio::spawn(async move {
                    let res = backend.history().await;
                    let _ = tx.send(AppEvent::FeedLoaded(generation, res)).await;
                });
            }
        }
        Screen::Playlists => {
            try_load_feed_cache(app, "feed_playlists");
            if app.items.is_empty() {
                app.is_loading = true;
                app.status_text = "Loading Playlists...".to_string();
                let backend = Arc::clone(&app.backend);
                let generation = app.begin_feed_request();
                tokio::spawn(async move {
                    let res = backend.playlists().await;
                    let _ = tx.send(AppEvent::FeedLoaded(generation, res)).await;
                });
            }
        }
        Screen::WatchLater => {
            try_load_feed_cache(app, "feed_watchlater");
            if app.items.is_empty() {
                app.is_loading = true;
                app.status_text = "Loading Watch Later...".to_string();
                let backend = Arc::clone(&app.backend);
                let generation = app.begin_feed_request();
                tokio::spawn(async move {
                    let res = backend.watch_later().await;
                    let _ = tx.send(AppEvent::FeedLoaded(generation, res)).await;
                });
            }
        }
        Screen::Liked => {
            try_load_feed_cache(app, "feed_liked");
            if app.items.is_empty() {
                app.is_loading = true;
                app.status_text = "Loading Liked Videos...".to_string();
                let backend = Arc::clone(&app.backend);
                let generation = app.begin_feed_request();
                tokio::spawn(async move {
                    let res = backend.playlist("LL").await;
                    let _ = tx.send(AppEvent::FeedLoaded(generation, res)).await;
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
    if let Some(old) = app.login_modal.take() {
        old.cancelled.cancel();
    }
    let cancelled = yute_core::oauth::CancellationToken::new();
    app.login_modal = Some(ui::signin::LoginModal::new(cancelled.clone()));
    app.account_picker = false;
    app.status_text = "Requesting TV sign-in code…".into();
    let generation = app
        .auth_generation
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        + 1;
    let language = (app.config.backend.hl.clone(), app.config.backend.gl.clone());
    tokio::spawn(async move {
        let (events, mut updates) = tokio::sync::mpsc::unbounded_channel();
        let flow = yute_core::oauth::device_login(
            yute::config::resolve_path("~/.config/yute/auth.json"),
            language,
            cancelled,
            events,
        );
        tokio::pin!(flow);
        loop {
            tokio::select! {
                Some(event)=updates.recv()=>{ if tx.send(AppEvent::LoginDisplay(generation,event)).await.is_err() { return; } },
                result=&mut flow=>{
                    let event=match result {
                        Ok(account)=>AppEvent::LoginSucceeded(generation,account.name,account.handle),
                        Err(error)=>AppEvent::LoginStatus(generation,error.to_string()),
                    };
                    let _=tx.send(event).await; break;
                }
            }
        }
    });
}

async fn run_login() -> anyhow::Result<()> {
    let config = Config::load();
    let cancelled = yute_core::oauth::CancellationToken::new();
    let (events, mut updates) = tokio::sync::mpsc::unbounded_channel();
    let flow = yute_core::oauth::device_login(
        yute::config::resolve_path("~/.config/yute/auth.json"),
        (config.backend.hl.clone(), config.backend.gl.clone()),
        cancelled.clone(),
        events,
    );
    tokio::pin!(flow);
    loop {
        tokio::select! {
            _=tokio::signal::ctrl_c()=>{cancelled.cancel(); anyhow::bail!("Sign-in cancelled");},
            Some(event)=updates.recv()=>{ if let yute_core::oauth::LoginEvent::Code{user_code,verification_url,..}=event {
                println!("Open {verification_url} and enter {user_code}. Ctrl+C cancels.");
                println!("{}",ui::signin::qr_text(&verification_url));
            }},
            result=&mut flow=>{
                let account=result?;
                let mut config=config.clone(); config.backend.auth="tv_oauth".into(); config.save()?;
                println!("Signed in as {}. Credentials saved.",account.name);
                return Ok(());
            }
        }
    }
}

async fn run_dump_tv(query: &str) -> anyhow::Result<()> {
    use serde_json::json;
    use std::fs;
    use std::io::Write;
    use yute::backend::auth::AuthContext;
    use yute::backend::innertube::InnerTubeBackend;
    use yute::config::resolve_path;

    let debug_dir = resolve_path("~/.cache/yute/debug");
    let _ = fs::create_dir_all(&debug_dir);

    let auth_path = resolve_path("~/.config/yute/auth.json");
    let cookies_path = resolve_path("~/.config/yute/cookies.txt");
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
    let page = yute::backend::parse::parse_feed_page(&home_val);
    if let Some(card) = page.items.first() {
        sample_video_id = card.id().to_string();
    } else {
        let s_page = yute::backend::parse::parse_feed_page(&search_val);
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
    let s_page = yute::backend::parse::parse_feed_page(&search_val);
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

// Keep diagnostics concise; never display request URLs, headers or response bodies.
fn feed_error_message(error: &anyhow::Error) -> &'static str {
    let diagnostic = format!("{error:#}").to_ascii_lowercase();
    if diagnostic.contains("401")
        || diagnostic.contains("unauthorized")
        || diagnostic.contains("needs re-login")
    {
        "YouTube session expired — use :login to reconnect"
    } else if diagnostic.contains("403") {
        "YouTube rejected this session/client — check Accounts, then press r to retry"
    } else if diagnostic.contains("429") {
        "YouTube rate limited this session — wait, then press r to retry"
    } else if diagnostic.contains("timed out") || diagnostic.contains("timeout") {
        "YouTube timed out — check connection, then press r to retry"
    } else {
        "YouTube feed could not load — press r to retry"
    }
}

/// The flat Home extractor sometimes omits counts. Resolve only visible missing
/// metadata, twice concurrently, without starting a player or reporting a watch.
fn maybe_load_card_metadata(app: &mut App, tx: Sender<AppEvent>) {
    if app.screen != Screen::Home || app.home_source != yute::app::FilterKind::All || app.is_loading
    {
        return;
    }
    if app.metadata_generation != app.feed_generation {
        app.metadata_generation = app.feed_generation;
        app.metadata_pending.clear();
        app.metadata_attempted.clear();
    }
    let slots = 2usize.saturating_sub(app.metadata_pending.len());
    if slots == 0 || app.metadata_attempted.len() >= 200 {
        return;
    }
    let start = app.scroll_row.saturating_mul(app.rendered_columns);
    let ids = app
        .filtered_items()
        .into_iter()
        .skip(start)
        .take(app.rendered_columns.saturating_mul(app.rendered_rows))
        .filter_map(|card| match card {
            Card::Video(video)
                if video.views.is_none() && !app.metadata_attempted.contains(&video.id) =>
            {
                Some(video.id.clone())
            }
            _ => None,
        })
        .take(slots)
        .collect::<Vec<_>>();
    for id in ids {
        let Ok(permit) = Arc::clone(&app.metadata_slots).try_acquire_owned() else {
            break;
        };
        app.metadata_pending.insert(id.clone());
        app.metadata_attempted.insert(id.clone());
        let backend = Arc::clone(&app.backend);
        let generation = app.feed_generation;
        let tx = tx.clone();
        tokio::spawn(async move {
            let data = tokio::time::timeout(Duration::from_secs(12), backend.video_metadata(&id))
                .await
                .ok()
                .and_then(Result::ok)
                .filter(|video| video.id == id);
            let _ = tx
                .send(AppEvent::CardMetadataLoaded(generation, id, data))
                .await;
            drop(permit);
        });
    }
}

fn maybe_continue_feed(app: &mut App, tx: Sender<AppEvent>) {
    if app.is_loading
        || app.is_loading_more
        || app.continuation_failed
        || (app.focus != Focus::Content && app.screen != Screen::Subscriptions)
        || !matches!(
            app.screen,
            Screen::Home
                | Screen::Subscriptions
                | Screen::History
                | Screen::WatchLater
                | Screen::Liked
                | Screen::Playlists
                | Screen::PlaylistView
                | Screen::ChannelView
                | Screen::Search
        )
    {
        return;
    }
    let count = app.filtered_items().len();
    if app.screen != Screen::Subscriptions && app.selected_index + app.rendered_columns * 2 < count
    {
        return;
    }
    let Some(token) = app.continuation_token.clone() else {
        return;
    };
    if app.screen != Screen::Subscriptions && count < app.rendered_columns * app.rendered_rows {
        if app.autofill_pages >= 3 {
            return;
        }
        app.autofill_pages += 1;
    }
    app.is_loading_more = true;
    let backend = Arc::clone(&app.backend);
    let screen = app.screen;
    let account = app.account_handle.clone();
    let generation = app.feed_generation;
    let query = app.search_query.clone();
    tokio::spawn(async move {
        let result = if screen == Screen::Search {
            backend
                .search(&query, &SearchFilters::default(), Some(&token))
                .await
        } else {
            backend.continue_feed(&token).await
        };
        let _ = tx
            .send(AppEvent::FeedContinued(
                generation, screen, account, token, result,
            ))
            .await;
    });
}

fn refresh_current_feed(app: &mut App, tx: Sender<AppEvent>) {
    if app.is_loading || app.is_loading_more {
        return;
    }
    if app.screen == Screen::Home
        && matches!(
            app.home_source,
            yute::app::FilterKind::Shorts | yute::app::FilterKind::Live
        )
    {
        let category = if app.home_source == yute::app::FilterKind::Shorts {
            yute::backend::models::HomeCategory::Shorts
        } else {
            yute::backend::models::HomeCategory::Live
        };
        let backend = Arc::clone(&app.backend);
        let generation = app.begin_feed_request();
        tokio::spawn(async move {
            let result = backend.home_category(category).await;
            let _ = tx.send(AppEvent::FeedLoaded(generation, result)).await;
        });
        return;
    }

    match &app.detail_route {
        Some(yute::app::DetailRoute::Channel(_)) => {
            fetch_channel(app, tx);
            return;
        }
        Some(yute::app::DetailRoute::Playlist(playlist)) => {
            let id = playlist.id.clone();
            let backend = Arc::clone(&app.backend);
            let generation = app.begin_feed_request();
            tokio::spawn(async move {
                let result = backend.playlist(&id).await;
                let _ = tx.send(AppEvent::FeedLoaded(generation, result)).await;
            });
            return;
        }
        None => {}
    }
    let screen = app.screen;
    if !matches!(
        screen,
        Screen::Home
            | Screen::Subscriptions
            | Screen::History
            | Screen::WatchLater
            | Screen::Liked
            | Screen::Playlists
            | Screen::Search
    ) {
        app.status_text = "Open a feed to refresh it".to_string();
        return;
    }
    app.is_loading = true;
    app.status_text = "Refreshing…".to_string();
    let backend = Arc::clone(&app.backend);
    let query = app.search_query.clone();
    let generation = app.begin_feed_request();
    tokio::spawn(async move {
        let result = match screen {
            Screen::Home => backend.home(None).await,
            Screen::Subscriptions => backend.subscriptions().await,
            Screen::History => backend.history().await,
            Screen::WatchLater => backend.watch_later().await,
            Screen::Liked => backend.playlist("LL").await,
            Screen::Playlists => backend.playlists().await,
            Screen::Search => {
                backend
                    .search(&query, &SearchFilters::default(), None)
                    .await
            }
            _ => unreachable!(),
        };
        let _ = tx.send(AppEvent::FeedLoaded(generation, result)).await;
    });
}

fn apply_stream_cache(app: &App) {
    if app.settings_index == 0 && app.player.is_alive() {
        app.player.send_command(IpcCommand::SetProperty(
            "ytdl-format",
            serde_json::json!(app.config.playback.resolved_ytdl_format()),
        ));
    }
    if app.settings_index == 3 && app.player.is_alive() {
        for (name, value) in [
            (
                "cache-secs",
                serde_json::json!(app.config.playback.stream_cache_seconds),
            ),
            (
                "demuxer-readahead-secs",
                serde_json::json!(app.config.playback.stream_cache_seconds),
            ),
            (
                "demuxer-max-bytes",
                serde_json::json!(u64::from(app.config.playback.stream_cache_mb()) * 1024 * 1024),
            ),
        ] {
            app.player
                .send_command(IpcCommand::SetProperty(name, value));
        }
    }
}

fn dispatch_command(app: &mut App, command: yute::commands::Command, tx: Sender<AppEvent>) {
    use yute::commands::{Command, CopyTarget};
    match command {
        Command::PlaySelect | Command::Audio => {
            open_item_menu(app);
            if let Some(menu) = &mut app.action_menu {
                if !matches!(menu.card, Card::Video(_))
                    || menu.card.id().starts_with('/')
                    || menu.card.id().starts_with('.')
                {
                    app.action_menu = None;
                    app.status_text = "Select a YouTube video to choose formats".into();
                    return;
                }
                if command == Command::Audio {
                    resolve_menu_formats(app, true, tx);
                } else {
                    menu.stage = yute::actions::MenuStage::PlayAs;
                }
            }
        }
        Command::Quality(quality) => {
            let Some(quality) = quality else {
                app.status_text = format!(
                    "Quality: {} · :quality 720p",
                    app.config.playback.max_resolution
                );
                return;
            };
            app.config.playback.max_resolution = quality.clone();
            if app.config.save().is_err() {
                app.status_text = "Could not save quality".into();
                return;
            }
            if app.player.is_alive() {
                let format = app.config.playback.resolved_ytdl_format();
                app.player.send_command(IpcCommand::SetProperty(
                    "ytdl-format",
                    serde_json::json!(format),
                ));
                if let Some(id) = app.player.current_video_id() {
                    let url = if id.starts_with('/') || id.starts_with('.') {
                        id.to_string()
                    } else {
                        format!("https://www.youtube.com/watch?v={id}")
                    };
                    app.player.send_command(IpcCommand::ChangeQuality(
                        format,
                        url,
                        app.player.status.time_pos,
                        app.player.status.paused,
                    ));
                    app.status_text = format!(
                        "Reloading at {} · quality {quality}",
                        ui::text::truncate_str(&format!("{:.0}s", app.player.status.time_pos), 12)
                    );
                }
            } else {
                app.status_text = format!("Quality {quality} saved for the next video");
            }
        }
        Command::Navigate(screen) => navigate_to_screen(app, screen, tx),
        Command::Search(query) => {
            app.transition(Screen::Search, true);
            app.search_query = query.clone();
            app.focus = Focus::Content;
            trigger_search_fetch(app, tx, query);
        }
        Command::Channel => {
            if let Some(channel) = app.selected_card().and_then(yute::actions::channel_of) {
                dispatch_open(app, yute::actions::OpenAction::OpenChannel(channel), tx);
            } else {
                app.status_text = "Channel ID unavailable for this item".into();
            }
        }
        Command::Playlist => {
            if let Some(Card::Playlist(playlist)) = app.selected_card().cloned() {
                dispatch_open(app, yute::actions::OpenAction::OpenPlaylist(playlist), tx);
            } else {
                app.status_text = "Select a playlist to open it".into();
            }
        }
        Command::Play => {
            if app.selected_card().is_some() {
                play_selected_card(app, tx);
            } else {
                send_player_command(app, IpcCommand::Play);
            }
        }
        Command::Pause => send_player_command(app, IpcCommand::Pause),
        Command::Toggle => send_player_command(app, IpcCommand::TogglePause),
        Command::Stop => {
            app.watch_tracker = None;
            app.player.stop_playback();
            app.status_text = "Playback stopped".into();
        }
        Command::Next => {
            if let Some(video) = app.player.queue.pop_next() {
                sync_queue_view(app);
                dispatch_open(app, yute::actions::activation(&Card::Video(video)), tx);
            } else {
                app.status_text = "Queue is empty — use :queue add".into();
            }
        }
        Command::Prev => {
            let mut history = std::mem::take(&mut app.playback_history);
            if let Some(video) = history.pop() {
                start_playback(app, video, tx);
            } else {
                app.status_text = "No previous playback in this session".into();
            }
            app.playback_history = history;
        }
        Command::Seek(value, relative) => {
            send_player_command(app, IpcCommand::Seek(value, relative))
        }
        Command::Volume(value, true) => send_player_command(app, IpcCommand::AdjustVolume(value)),
        Command::Volume(value, false) => send_player_command(app, IpcCommand::SetVolume(value)),
        Command::Mute => send_player_command(app, IpcCommand::ToggleMute),
        Command::QueueAdd(front) => match app.selected_card().map(yute::actions::activation) {
            Some(yute::actions::OpenAction::Play(video)) => {
                if front {
                    app.player.queue.push_front(video);
                } else {
                    app.player.queue.push_back(video);
                }
                sync_queue_view(app);
                app.status_text = if front {
                    "Queued next"
                } else {
                    "Added to queue"
                }
                .into();
            }
            _ => app.status_text = "Select a playable video to queue it".into(),
        },
        Command::QueueRemove => {
            if app.screen != Screen::Queue {
                app.status_text = "Open :queue and select an entry to remove".into();
            } else if let Some(card) = app.selected_card() {
                let index = app.items.iter().position(|item| std::ptr::eq(item, card));
                if let Some(index) = index {
                    app.player.queue.remove(index);
                    sync_queue_view(app);
                    app.status_text = "Removed from queue".into();
                }
            }
        }
        Command::QueueClear => {
            app.player.queue.clear();
            sync_queue_view(app);
            app.status_text = "Queue cleared".into();
        }
        Command::Login => trigger_oauth_device_flow(app, tx),
        Command::Logout => {
            if app.active_account_index == 0 {
                app.status_text = "Anonymous has no saved credentials".into();
            } else {
                app.pending_remove = Some(app.active_account_index);
                app.confirming_logout = true;
            }
        }
        Command::Account(args) => match args.trim() {
            "" | "switch" => {
                app.accounts_cursor = app.active_account_index;
                app.account_picker = true;
            }
            "add" => trigger_oauth_device_flow(app, tx),
            "whoami" => {
                app.command_help = Some(format!(
                    "{}\n{}\n{}",
                    app.account_name,
                    app.account_handle,
                    if authenticated(app) {
                        "Signed in"
                    } else {
                        "Anonymous"
                    }
                ))
            }
            input if input.starts_with("remove ") => match app
                .account_store
                .resolve(input.trim_start_matches("remove ").trim())
            {
                Ok(index) if index > 0 => {
                    app.pending_remove = Some(index);
                    app.confirming_logout = true;
                }
                _ => app.status_text = "Choose a saved account to remove".into(),
            },
            input => match app.account_store.resolve(input) {
                Ok(index) => activate_account(app, index, tx),
                Err(_) => app.status_text = "Account not found or prefix is ambiguous".into(),
            },
        },
        Command::Subscribe(value) => {
            if !authenticated(app) {
                app.status_text = "Login required for subscriptions — use :login".into();
                return;
            }
            let channel = if let Some(yute::app::DetailRoute::Channel(route)) = &app.detail_route {
                Some(route.channel.clone())
            } else {
                app.selected_card().and_then(yute::actions::channel_of)
            };
            if let Some(channel) = channel {
                perform_mutation(
                    app,
                    yute::actions::AccountMutation::Subscribe {
                        channel_id: channel.id,
                        subscribed: value,
                    },
                    tx,
                );
            } else {
                app.status_text = "Channel ID unavailable — open a channel result first".into();
            }
        }
        Command::Like(value) => {
            if !authenticated(app) {
                app.status_text = "Login required for ratings — use :login".into();
                return;
            }
            let video = if app.screen == Screen::Watch {
                app.player.current_video.clone()
            } else {
                match app.selected_card() {
                    Some(Card::Video(v)) => Some(v.clone()),
                    _ => None,
                }
            };
            if let Some(video) = video.filter(|v| !v.id.starts_with('/') && !v.id.starts_with('.'))
            {
                perform_mutation(
                    app,
                    yute::actions::AccountMutation::Like {
                        video_id: video.id,
                        liked: value,
                    },
                    tx,
                );
            } else {
                app.status_text = "Select a YouTube video to rate it".into();
            }
        }
        Command::Open(channel) => {
            let selected = if app.screen == Screen::Watch
                && app.watch_pane != yute::app::WatchPane::Suggestions
            {
                app.player.current_video.clone().map(Card::Video)
            } else {
                app.selected_card().cloned()
            };
            let card = if channel {
                selected
                    .as_ref()
                    .and_then(yute::actions::channel_of)
                    .map(Card::Channel)
            } else {
                selected
            };
            if let Some(card) = card {
                open_system_url(yute::actions::url(&card), tx);
            } else {
                app.status_text = "No matching selected entity to open".into();
            }
        }
        Command::Copy(target) => {
            let Some(card) = app.selected_card() else {
                app.status_text = "Select an item to copy".into();
                return;
            };
            let text = match target {
                CopyTarget::Url => yute::actions::url(card),
                CopyTarget::Title => card.title().to_string(),
                CopyTarget::Channel => match card {
                    Card::Video(v) => v.channel.clone(),
                    Card::Playlist(p) => p.channel.clone(),
                    Card::Channel(c) => c.title.clone(),
                },
            };
            copy_to_clipboard(text, tx);
        }
        Command::Refresh => refresh_current_feed(app, tx),
        Command::Keys => {
            app.set_screen(Screen::Help);
        }
        Command::Shorts(show) => {
            let current = matches!(
                app.config.content.shorts,
                yute::config::ShortsVisibility::Show
            );
            let id = app.selected_card().map(|c| c.id().to_string());
            app.config.content.shorts = if show.unwrap_or(!current) {
                yute::config::ShortsVisibility::Show
            } else {
                yute::config::ShortsVisibility::Hide
            };
            app.selected_index = id
                .and_then(|id| app.filtered_items().iter().position(|c| c.id() == id))
                .unwrap_or(0);
            let _ = app.config.save();
            app.status_text = "Shorts visibility saved".into();
        }
        Command::Filter(query) => {
            if let Some(kind) = yute::app::FilterKind::all()
                .iter()
                .find(|kind| kind.label().eq_ignore_ascii_case(&query))
            {
                set_content_filter(app, *kind, tx);
                return;
            } else {
                app.filter_text = query;
            }
            app.selected_index = 0;
            app.scroll_row = 0;
            app.status_text.clear();
        }
        Command::Graphics(mode) => {
            let old = std::mem::replace(&mut app.config.general.graphics, mode);
            if app.config.save().is_ok() {
                app.status_text = "Graphics preference saved — requires restart".into();
            } else {
                app.config.general.graphics = old;
                app.status_text = "Could not save graphics preference".into();
            }
        }
        Command::Sidebar(show) => {
            app.sidebar_expanded = show.unwrap_or(!app.sidebar_expanded);
            app.focus = if app.sidebar_expanded {
                Focus::Sidebar
            } else {
                Focus::Content
            };
        }
        Command::Help(topic) => match yute::commands::help(topic.as_deref()) {
            Ok(text) => app.command_help = Some(text),
            Err(error) => app.status_text = error,
        },
        Command::Quit(force) => {
            if force {
                app.player.stop();
                app.should_quit = true;
            } else {
                app.confirming_quit = true;
            }
        }
    }
}

fn send_player_command(app: &mut App, command: IpcCommand) {
    if app.player.is_alive() {
        app.player.send_command(command);
    } else {
        app.status_text = "MPV is not active — select a video and use :play".into();
    }
}
fn authenticated(app: &App) -> bool {
    app.is_authenticated()
}

fn perform_mutation(app: &mut App, mutation: yute::actions::AccountMutation, tx: Sender<AppEvent>) {
    let backend = Arc::clone(&app.backend);
    let generation = app.feed_generation;
    app.status_text = "Updating YouTube…".into();
    tokio::spawn(async move {
        use yute::actions::AccountMutation;
        let result = match &mutation {
            AccountMutation::Dislike { video_id } => {
                backend.dislike_video(video_id).await.and_then(|confirmed| {
                    if confirmed {
                        Ok(())
                    } else {
                        Err(anyhow::anyhow!("Dislike was not confirmed"))
                    }
                })
            }
            AccountMutation::Subscribe {
                channel_id,
                subscribed: true,
            } => backend.subscribe_channel(channel_id).await,
            AccountMutation::Subscribe {
                channel_id,
                subscribed: false,
            } => backend.unsubscribe_channel(channel_id).await,
            AccountMutation::Like {
                video_id,
                liked: true,
            } => backend.like_video(video_id).await.and_then(|confirmed| {
                if confirmed {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("Like was not confirmed"))
                }
            }),
            AccountMutation::Like {
                video_id,
                liked: false,
            } => backend.remove_rating(video_id).await,
        };
        let _ = tx
            .send(AppEvent::MutationFinished(generation, mutation, result))
            .await;
    });
}
fn copy_to_clipboard(text: String, tx: Sender<AppEvent>) {
    tokio::spawn(async move {
        use tokio::io::AsyncWriteExt;
        let (program, args) = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            ("wl-copy", vec!["--type", "text/plain;charset=utf-8"])
        } else {
            ("xclip", vec!["-selection", "clipboard"])
        };
        let result: std::io::Result<_> = async {
            let mut child = tokio::process::Command::new(program)
                .args(args)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()?;
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(text.as_bytes()).await?;
            }
            child.wait().await
        }
        .await;
        let message = if result.is_ok_and(|s| s.success()) {
            "Copied"
        } else {
            "Clipboard unavailable — install/use wl-copy on Wayland or xclip on X11"
        };
        let _ = tx.send(AppEvent::StatusMessage(message.into())).await;
    });
}
fn logout(app: &mut App) {
    let mut config = app.config.clone();
    config.backend.auth = "guest".into();
    if config.save().is_err() {
        app.status_text = "Could not save guest mode; session was not changed".into();
        return;
    }
    app.config = config;
    app.auth_generation
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    app.player.stop();
    app.watch_tracker = None;
    {
        let mut auth = app.auth.lock().unwrap();
        auth.access_token = None;
        auth.refresh_token = None;
        auth.token_expires_at = None;
        auth.cookies.clear();
        auth.cookie_header = None;
        auth.sapisid = None;
        auth.visitor_data = None;
        // Guest responses must not rewrite the retained encrypted login.
        auth.auth_file_path = None;
    }
    app.rebuild_backend();
    app.account_name = "Guest".into();
    app.account_handle.clear();
    app.account_initial = "G".into();
    app.nav_history.clear();
    app.nav_future.clear();
    app.items.clear();
    app.continuation_token = None;
    app.detail_route = None;
    app.screen = Screen::Home;
    app.filter_kind = yute::app::FilterKind::All;
    app.filter_text.clear();
    app.selected_index = 0;
    app.scroll_row = 0;
    app.feed_generation = app.feed_generation.wrapping_add(1);
    app.is_loading = false;
    app.is_loading_more = false;
    app.status_text =
        "Signed out locally; saved login remains encrypted. Use :login to reconnect".into();
}

// Cursor is a UTF-8 byte boundary; rendering converts it to terminal columns.
fn edit_input(app: &mut App, key: crossterm::event::KeyEvent) -> bool {
    use crossterm::event::KeyModifiers;
    let text = if app.focus == Focus::CommentInput {
        &mut app.comment_input
    } else if app.focus == Focus::CommandLine {
        &mut app.cmd_input
    } else {
        &mut app.search_input
    };
    let cursor = app.input_cursor.get_or_insert(text.len());
    *cursor = (*cursor).min(text.len());
    while !text.is_char_boundary(*cursor) {
        *cursor = cursor.saturating_sub(1);
    }
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Home | KeyCode::Char('a') if key.code == KeyCode::Home || ctrl => *cursor = 0,
        KeyCode::End | KeyCode::Char('e') if key.code == KeyCode::End || ctrl => {
            *cursor = text.len()
        }
        KeyCode::Left => *cursor = text[..*cursor].char_indices().last().map_or(0, |(i, _)| i),
        KeyCode::Right => *cursor += text[*cursor..].chars().next().map_or(0, char::len_utf8),
        KeyCode::Backspace => {
            if *cursor > 0 {
                let start = text[..*cursor].char_indices().last().map_or(0, |(i, _)| i);
                text.drain(start..*cursor);
                *cursor = start;
            }
        }
        KeyCode::Delete => {
            if let Some(ch) = text[*cursor..].chars().next() {
                text.drain(*cursor..*cursor + ch.len_utf8());
            }
        }
        KeyCode::Char('u') if ctrl => {
            text.clear();
            *cursor = 0;
        }
        KeyCode::Char('w') if ctrl => {
            let mut prefix = text[..*cursor].to_string();
            delete_word(&mut prefix);
            text.drain(prefix.len()..*cursor);
            *cursor = prefix.len();
        }
        KeyCode::Char(c)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            text.insert(*cursor, c);
            *cursor += c.len_utf8();
        }
        _ => {
            app.input_cursor = None;
            return false;
        }
    }
    app.completion_base = None;
    app.completion_index = 0;
    app.suggestion_index = None;
    true
}

fn delete_word(input: &mut String) {
    let end = input
        .trim_end()
        .char_indices()
        .rev()
        .find(|(_, ch)| ch.is_whitespace())
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    input.truncate(end);
}

fn feed_cache_key(app: &App, key: &str) -> String {
    let key = if key == "feed_home" {
        "feed_home_tv_default"
    } else {
        key
    };
    format!(
        "v3_{}_{}_{}",
        app.config.backend.auth,
        app.account_store.cache_namespace(),
        key
    )
}

async fn run_doctor() -> anyhow::Result<()> {
    use yute_core::doctor::{display_text, Status};
    let config = Config::load();
    let checks = yute_core::doctor::run(&config).await;
    let failed = checks.iter().any(|check| check.status == Status::Failure);
    for check in checks {
        println!("{:?} · {}: {}", check.status, check.name, check.detail);
    }
    println!(
        "Terminal: {} · program={}",
        display_text(&std::env::var("TERM").unwrap_or_else(|_| "unset".into())),
        display_text(&std::env::var("TERM_PROGRAM").unwrap_or_else(|_| "unset".into()))
    );
    println!(
        "Graphics: {:?} (configured={}, environment hint; not actively negotiated)",
        yute::terminal::initial_picker(&config.general.graphics).protocol_type(),
        display_text(&config.general.graphics)
    );
    match crossterm::terminal::window_size() {
        Ok(size) => println!(
            "Terminal dimensions: {}x{} cells, {}x{} pixels",
            size.columns, size.rows, size.width, size.height
        ),
        Err(_) => {
            println!("Warning · terminal dimensions unavailable (stdout may not be a terminal)")
        }
    }
    match yute::terminal::cell_size().await {
        Some((w, h)) => println!("Terminal cell dimensions: {w}x{h} pixels"),
        None => println!(
            "Warning · terminal cell dimensions unavailable; rendering uses cached/default hints"
        ),
    }
    if failed {
        anyhow::bail!("doctor found failures; see individual checks above");
    }
    Ok(())
}

#[cfg(test)]
mod feed_ui_tests {
    use super::*;
    use yute::backend::models::{FeedPage, LiveState};

    #[tokio::test]
    async fn shifted_jk_switch_content_tabs_without_leaking_from_text_modes() {
        let (mut app, cards) = app_and_cards();
        app.items = cards;
        let mut keys = KeymapState::new();
        let (tx, _) = tokio::sync::mpsc::channel(16);
        for (ch, expected) in [
            ('j', yute::app::FilterKind::Videos),
            ('j', yute::app::FilterKind::Shorts),
            ('k', yute::app::FilterKind::Videos),
        ] {
            handle_key_event(
                &mut app,
                &mut keys,
                crossterm::event::KeyEvent::new(
                    KeyCode::Char(ch),
                    crossterm::event::KeyModifiers::SHIFT,
                ),
                tx.clone(),
            )
            .await;
            assert_eq!(app.filter_kind, expected);
        }
        app.focus = Focus::CommandLine;
        handle_key_event(
            &mut app,
            &mut keys,
            crossterm::event::KeyEvent::new(
                KeyCode::Char('j'),
                crossterm::event::KeyModifiers::SHIFT,
            ),
            tx,
        )
        .await;
        assert_eq!(app.cmd_input, "J");
        assert_eq!(app.filter_kind, yute::app::FilterKind::Videos);
    }

    #[tokio::test]
    async fn missing_counts_only_queue_visible_cards_twice_concurrently() {
        let (mut app, cards) = app_and_cards();
        app.screen = Screen::Home;
        app.items = cards;
        app.backend = Arc::new(yute::backend::BackendChain::new(vec![]));
        app.rendered_columns = 3;
        app.rendered_rows = 1;
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        maybe_load_card_metadata(&mut app, tx.clone());
        assert_eq!(app.metadata_pending.len(), 2);
        assert_eq!(app.metadata_slots.available_permits(), 0);
        maybe_load_card_metadata(&mut app, tx);
        assert_eq!(app.metadata_attempted.len(), 2);
        for _ in 0..2 {
            assert!(matches!(
                rx.recv().await,
                Some(AppEvent::CardMetadataLoaded(_, _, None))
            ));
        }
        assert!(!app.player.is_alive());
    }

    #[test]
    fn metadata_responses_preserve_selection_and_reject_stale_generations() {
        let (mut app, cards) = app_and_cards();
        app.screen = Screen::Home;
        app.items = cards;
        app.selected_index = 3;
        app.scroll_row = 1;
        let mut video = match &app.items[0] {
            Card::Video(v) => v.clone(),
            _ => unreachable!(),
        };
        video.views = Some("1234 views".into());
        let (tx, _) = tokio::sync::mpsc::channel(16);
        app.metadata_pending.insert("other-pending".into()); // Keep this fixture out of the real Home cache.
        let generation = app.feed_generation;
        handle_app_event(
            &mut app,
            AppEvent::CardMetadataLoaded(
                generation.wrapping_add(1),
                video.id.clone(),
                Some(video.clone()),
            ),
            tx.clone(),
        );
        assert!(matches!(&app.items[0],Card::Video(v) if v.views.is_none()));
        handle_app_event(
            &mut app,
            AppEvent::CardMetadataLoaded(generation, video.id.clone(), Some(video)),
            tx,
        );
        assert!(matches!(&app.items[0],Card::Video(v) if v.views.as_deref()==Some("1234 views")));
        assert_eq!(app.selected_index, 3);
        assert_eq!(app.scroll_row, 1);
        assert!(!app.player.is_alive());
    }

    #[tokio::test]
    async fn text_modes_consume_navigation_and_edit_unicode() {
        for focus in [Focus::CommandLine, Focus::SearchInput] {
            let (mut app, cards) = app_and_cards();
            app.items = cards;
            app.focus = focus;
            let mut keymap = KeymapState::new();
            let (tx, _) = tokio::sync::mpsc::channel(16);
            for ch in "qhistory/:?हिन्दी".chars() {
                handle_key_event(
                    &mut app,
                    &mut keymap,
                    crossterm::event::KeyEvent::new(
                        KeyCode::Char(ch),
                        crossterm::event::KeyModifiers::NONE,
                    ),
                    tx.clone(),
                )
                .await;
            }
            assert!(!app.should_quit);
            assert_eq!(app.screen, Screen::Search);
            assert_eq!(app.selected_index, 0);
            handle_key_event(
                &mut app,
                &mut keymap,
                crossterm::event::KeyEvent::new(
                    KeyCode::Char('a'),
                    crossterm::event::KeyModifiers::CONTROL,
                ),
                tx.clone(),
            )
            .await;
            handle_key_event(
                &mut app,
                &mut keymap,
                crossterm::event::KeyEvent::new(
                    KeyCode::Char('X'),
                    crossterm::event::KeyModifiers::NONE,
                ),
                tx.clone(),
            )
            .await;
            let text = if focus == Focus::CommandLine {
                &app.cmd_input
            } else {
                &app.search_input
            };
            assert!(text.starts_with('X'));
            assert!(text.contains("qhistory/:?"));
        }
    }

    fn app_and_cards() -> (App, Vec<Card>) {
        let mut app = App::new(
            Config::default(),
            ratatui_image::picker::Picker::halfblocks(),
        );
        // Search avoids writing fixture data to the user's Home disk cache.
        app.screen = Screen::Search;
        let cards = (0..5)
            .map(|id| {
                Card::Video(VideoCard {
                    id: format!("fixture-{id}"),
                    title: format!("Fixture {id}"),
                    channel: "Channel".into(),
                    channel_id: None,
                    verified: false,
                    duration: None,
                    views: None,
                    published: None,
                    thumb_url: String::new(),
                    progress_pct: None,
                    live: LiveState::None,
                    badges: Vec::new(),
                    resume_time_sec: None,
                })
            })
            .collect();
        (app, cards)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn sidebar_toggle_and_focus_work_from_settings() {
        let (mut app, _) = app_and_cards();
        app.screen = Screen::Settings;
        app.sidebar_expanded = false;
        app.focus = Focus::Content;
        let mut keymap = KeymapState::new();
        let (tx, _) = channel(2);
        let key = crossterm::event::KeyEvent::new(
            KeyCode::Char('b'),
            crossterm::event::KeyModifiers::NONE,
        );
        handle_key_event(&mut app, &mut keymap, key, tx.clone()).await;
        assert!(app.sidebar_expanded);
        assert_eq!(app.focus, Focus::Sidebar);
        let next = crossterm::event::KeyEvent::new(
            KeyCode::Char('j'),
            crossterm::event::KeyModifiers::NONE,
        );
        handle_key_event(&mut app, &mut keymap, next, tx.clone()).await;
        assert_eq!(app.sidebar_index, 1);
        assert_eq!(app.settings_index, 0);
        handle_key_event(&mut app, &mut keymap, key, tx).await;
        assert!(!app.sidebar_expanded);
        assert_eq!(app.focus, Focus::Content);
    }

    #[tokio::test]
    async fn command_selection_overrides_a_completed_command_and_consumes_alt_shortcuts() {
        use crossterm::event::{KeyEvent, KeyModifiers};
        let (mut app, _) = app_and_cards();
        app.screen = Screen::Settings;
        app.focus = Focus::CommandLine;
        app.cmd_input = "h".into();
        let (tx, _) = channel(4);
        let mut keys = KeymapState::new();
        let sidebar = app.sidebar_expanded;
        handle_key_event(
            &mut app,
            &mut keys,
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT),
            tx.clone(),
        )
        .await;
        assert_eq!(app.focus, Focus::CommandLine);
        assert_eq!(app.sidebar_expanded, sidebar);
        for code in [KeyCode::Tab, KeyCode::Down, KeyCode::Enter] {
            handle_key_event(
                &mut app,
                &mut keys,
                KeyEvent::new(code, KeyModifiers::NONE),
                tx.clone(),
            )
            .await;
        }
        assert_eq!(app.screen, Screen::Home);
        assert!(app.command_help.is_none());
    }

    #[tokio::test]
    async fn command_enter_accepts_alias_fuzzy_and_never_opens_underlying_card() {
        use crossterm::event::{KeyEvent, KeyModifiers};
        for input in ["acc", "a", "acount"] {
            let (mut app, cards) = app_and_cards();
            app.items = cards;
            app.focus = Focus::CommandLine;
            app.cmd_input = input.into();
            let (tx, _) = channel(4);
            handle_key_event(
                &mut app,
                &mut KeymapState::new(),
                KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
                tx,
            )
            .await;
            assert!(app.account_picker);
            assert!(!app.player.is_alive());
        }
        let (mut app, _) = app_and_cards();
        app.focus = Focus::CommandLine;
        let (tx, _) = channel(4);
        handle_command_input(
            &mut app,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            tx,
        );
        assert_eq!(app.focus, Focus::Content);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn command_mode_completion_history_and_local_filter_are_connected() {
        use crossterm::event::{KeyEvent, KeyModifiers};
        let (mut app, _) = app_and_cards();
        app.screen = Screen::Settings;
        let mut keymap = KeymapState::new();
        let (tx, _) = channel(4);
        handle_key_event(
            &mut app,
            &mut keymap,
            KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE),
            tx.clone(),
        )
        .await;
        assert_eq!(app.focus, Focus::CommandLine);
        app.cmd_input = "his".into();
        handle_key_event(
            &mut app,
            &mut keymap,
            KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
            tx.clone(),
        )
        .await;
        assert_eq!(app.cmd_input, "history");
        app.cmd_input = "filter rust".into();
        handle_key_event(
            &mut app,
            &mut keymap,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            tx.clone(),
        )
        .await;
        assert_eq!(app.filter_text, "rust");
        handle_key_event(
            &mut app,
            &mut keymap,
            KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE),
            tx.clone(),
        )
        .await;
        handle_key_event(
            &mut app,
            &mut keymap,
            KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
            tx.clone(),
        )
        .await;
        assert_eq!(app.cmd_input, "filter rust");
        handle_key_event(
            &mut app,
            &mut keymap,
            KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
            tx,
        )
        .await;
        assert_eq!(app.cmd_input, "");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn mixed_entity_enter_routes_without_creating_a_player() {
        use yute::backend::models::{ChannelCard, PlaylistCard};
        let (mut app, _) = app_and_cards();
        app.backend = Arc::new(yute::backend::BackendChain::new(vec![]));
        app.items = vec![Card::Channel(ChannelCard {
            id: "UCfixture".into(),
            title: "Fixture Channel".into(),
            handle: None,
            subscribers: None,
            avatar_url: None,
            verified: false,
        })];
        let (tx, mut rx) = channel(4);
        play_selected_card(&mut app, tx.clone());
        assert_eq!(app.screen, Screen::ChannelView);
        assert!(app.player.current_video.is_none());
        let event = rx.recv().await.unwrap();
        handle_app_event(&mut app, event, tx.clone());
        app.nav_back();
        assert_eq!(app.screen, Screen::Search);
        app.items = vec![Card::Playlist(PlaylistCard {
            id: "PLfixture".into(),
            title: "Playlist".into(),
            channel: "".into(),
            video_count: None,
            thumb_url: "".into(),
            is_mix: false,
            first_video_id: Some("abcdefghijk".into()),
        })];
        play_selected_card(&mut app, tx.clone());
        assert_eq!(app.screen, Screen::PlaylistView);
        assert!(app.player.current_video.is_none());
        let event = rx.recv().await.unwrap();
        handle_app_event(&mut app, event, tx);
    }

    #[test]
    fn switching_cached_channel_tabs_preserves_each_position() {
        use yute::app::{ChannelState, DetailRoute, TabState};
        use yute::backend::models::{ChannelCard, ChannelLink};
        let (mut app, cards) = app_and_cards();
        app.screen = Screen::ChannelView;
        app.items = cards[..2].to_vec();
        app.selected_index = 1;
        app.scroll_row = 3;
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            Some("shorts".into()),
            TabState {
                items: cards.clone(),
                continuation: Some("next-short".into()),
                selected: 4,
                scroll: 2,
            },
        );
        app.detail_route = Some(DetailRoute::Channel(Box::new(ChannelState {
            channel: ChannelCard {
                id: "UCfixture".into(),
                title: "Fixture".into(),
                handle: None,
                subscribers: None,
                avatar_url: None,
                verified: false,
            },
            tabs: vec![
                ChannelLink {
                    title: "Videos".into(),
                    params: Some("videos".into()),
                    selected: true,
                },
                ChannelLink {
                    title: "Shorts".into(),
                    params: Some("shorts".into()),
                    selected: false,
                },
            ],
            active: 0,
            subscribed: None,
            pages,
        })));
        let (tx, _) = channel(2);
        switch_channel_tab(&mut app, true, tx.clone());
        assert_eq!(app.selected_index, 4);
        assert_eq!(app.scroll_row, 2);
        assert_eq!(app.continuation_token.as_deref(), Some("next-short"));
        switch_channel_tab(&mut app, false, tx);
        assert_eq!(app.selected_index, 1);
        assert_eq!(app.scroll_row, 3);
        assert_eq!(app.items.len(), 2);
        assert!(!app.is_loading);
    }

    #[test]
    fn removing_a_duplicate_queue_entry_removes_the_selected_instance() {
        let (mut app, cards) = app_and_cards();
        let Card::Video(first) = cards[0].clone() else {
            unreachable!()
        };
        let mut second = first.clone();
        second.title = "Second instance".into();
        app.player.queue.push_back(first.clone());
        app.player.queue.push_back(second);
        app.screen = Screen::Queue;
        sync_queue_view(&mut app);
        app.selected_index = 1;
        let (tx, _) = channel(2);
        dispatch_command(&mut app, yute::commands::Command::QueueRemove, tx);
        assert_eq!(app.player.queue.len(), 1);
        assert_eq!(app.player.queue.items()[0].title, first.title);
    }

    #[test]
    fn guest_cache_and_stale_login_results_cannot_restore_an_old_session() {
        let (mut app, _) = app_and_cards();
        let (tx, _) = channel(2);
        app.config.backend.auth = "tv_oauth".into();
        let authenticated_key = feed_cache_key(&app, "feed_home");
        app.config.backend.auth = "guest".into();
        assert_ne!(feed_cache_key(&app, "feed_home"), authenticated_key);
        app.auth_generation
            .store(1, std::sync::atomic::Ordering::SeqCst);
        let name = app.account_name.clone();
        handle_app_event(
            &mut app,
            AppEvent::LoginSucceeded(0, "Obsolete".into(), "@obsolete".into()),
            tx,
        );
        assert_eq!(app.account_name, name);
        assert_eq!(app.config.backend.auth, "guest");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn enter_on_filter_navigation_never_starts_mpv() {
        use crossterm::event::{KeyEvent, KeyModifiers};
        let (mut app, cards) = app_and_cards();
        app.items = cards;
        app.focus = Focus::Chips;
        let mut keymap = KeymapState::new();
        let (tx, _) = channel(2);
        handle_key_event(
            &mut app,
            &mut keymap,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            tx,
        )
        .await;
        assert!(app.player.current_video.is_none());
        assert_eq!(app.focus, Focus::Content);
        assert_eq!(app.screen, Screen::Search);
    }

    #[test]
    fn word_deletion_respects_unicode_character_boundaries() {
        let mut input = "search\u{00a0}Rust".to_string();
        delete_word(&mut input);
        assert_eq!(input, "search\u{00a0}");
        delete_word(&mut input);
        assert!(input.is_empty());
    }

    #[test]
    fn refresh_keeps_selected_identity_when_items_reorder() {
        let (mut app, cards) = app_and_cards();
        app.items = cards[..3].to_vec();
        app.selected_index = 1;
        app.is_loading = true;
        let (tx, _) = channel(2);
        let page = FeedPage {
            items: vec![cards[1].clone(), cards[0].clone()],
            ..Default::default()
        };
        handle_app_event(&mut app, AppEvent::FeedLoaded(0, Ok(page)), tx);
        assert_eq!(app.selected_card().unwrap().id(), "fixture-1");
        assert_eq!(app.selected_index, 0);
        assert!(!app.is_loading);
    }

    #[test]
    fn continuation_appends_unique_items_without_moving_selection() {
        let (mut app, cards) = app_and_cards();
        app.items = cards[..3].to_vec();
        app.selected_index = 2;
        app.scroll_row = 1;
        app.continuation_token = Some("page-two".into());
        app.is_loading_more = true;
        let account = app.account_handle.clone();
        let (tx, _) = channel(2);
        let page = FeedPage {
            items: cards[2..].to_vec(),
            continuation: Some("page-three".into()),
            ..Default::default()
        };
        handle_app_event(
            &mut app,
            AppEvent::FeedContinued(0, Screen::Search, account, "page-two".into(), Ok(page)),
            tx,
        );
        assert_eq!(app.items.len(), 5);
        assert_eq!(app.selected_index, 2);
        assert_eq!(app.scroll_row, 1);
        assert_eq!(app.continuation_token.as_deref(), Some("page-three"));
        assert!(!app.is_loading_more);
    }

    #[test]
    fn stale_response_cannot_replace_new_route_or_end_its_loading_state() {
        let (mut app, cards) = app_and_cards();
        app.feed_generation = 2;
        app.is_loading = true;
        app.items = cards[..2].to_vec();
        let (tx, _) = channel(2);
        handle_app_event(
            &mut app,
            AppEvent::FeedLoaded(1, Ok(FeedPage::default())),
            tx,
        );
        assert_eq!(app.items.len(), 2);
        assert!(app.is_loading);
    }

    #[test]
    fn refresh_failure_keeps_feed_and_hides_untrusted_diagnostics() {
        let (mut app, cards) = app_and_cards();
        app.items = cards;
        app.selected_index = 3;
        app.is_loading = true;
        let (tx, _) = channel(2);
        handle_app_event(
            &mut app,
            AppEvent::FeedLoaded(
                0,
                Err(anyhow::anyhow!(
                    "HTTP 401 Authorization: Bearer fixture-secret"
                )),
            ),
            tx,
        );
        assert_eq!(app.items.len(), 5);
        assert_eq!(app.selected_index, 3);
        assert!(app.status_text.contains("session expired"));
        assert!(!app.status_text.contains("fixture-secret"));
        assert!(!app.is_loading);
    }
    #[tokio::test]
    async fn sidebar_focus_history_and_quit_do_not_leak_into_text() {
        use crossterm::event::{KeyEvent, KeyModifiers};
        let (mut app, cards) = app_and_cards();
        app.items = cards;
        app.rendered_columns = 3;
        let mut keys = KeymapState::new();
        let (tx, _) = channel(16);
        app.selected_index = 0;
        handle_key_event(
            &mut app,
            &mut keys,
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE),
            tx.clone(),
        )
        .await;
        assert_eq!(app.focus, Focus::Content);
        assert_eq!(app.selected_index, 0);
        for _ in 0..2 {
            handle_key_event(
                &mut app,
                &mut keys,
                KeyEvent::new(KeyCode::Char('e'), KeyModifiers::ALT),
                tx.clone(),
            )
            .await;
        }
        assert!(!app.sidebar_expanded);
        assert_eq!(app.focus, Focus::Content);
        for focus in [Focus::CommandLine, Focus::SearchInput, Focus::CommentInput] {
            app.focus = focus;
            handle_key_event(
                &mut app,
                &mut keys,
                KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
                tx.clone(),
            )
            .await;
            assert!(!app.should_quit);
        }
        assert_eq!(app.cmd_input, "q");
        assert_eq!(app.search_input, "q");
        assert_eq!(app.comment_input, "q");
        app.focus = Focus::Content;
        app.transition(Screen::Settings, false);
        handle_key_event(
            &mut app,
            &mut keys,
            KeyEvent::new(KeyCode::Char('h'), KeyModifiers::SHIFT),
            tx.clone(),
        )
        .await;
        assert_eq!(app.screen, Screen::Search);
        handle_key_event(
            &mut app,
            &mut keys,
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::SHIFT),
            tx.clone(),
        )
        .await;
        assert_eq!(app.screen, Screen::Settings);
        handle_key_event(
            &mut app,
            &mut keys,
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            tx,
        )
        .await;
        assert!(app.should_quit);
    }

    #[tokio::test]
    async fn cached_home_categories_restore_position_and_channel_identities() {
        use yute::app::{FilterKind, TabState};
        let (mut app, mut cards) = app_and_cards();
        app.screen = Screen::Home;
        if let Card::Video(v) = &mut cards[0] {
            v.channel_id = Some("UCfixture-owner".into());
        }
        app.items = cards.clone();
        app.selected_index = 3;
        app.home_pages.insert(
            FilterKind::Live,
            TabState {
                items: cards,
                continuation: None,
                selected: 2,
                scroll: 0,
            },
        );
        let (tx, _) = channel(16);
        set_content_filter(&mut app, FilterKind::Live, tx.clone());
        assert_eq!(app.selected_index, 2);
        set_content_filter(&mut app, FilterKind::All, tx.clone());
        assert_eq!(app.selected_index, 3);
        set_content_filter(&mut app, FilterKind::Channels, tx);
        assert_eq!(app.filtered_items().len(), 1);
        assert!(matches!(app.selected_card(),Some(Card::Channel(c)) if c.id=="UCfixture-owner"));
    }
    #[tokio::test]
    async fn explicit_shorts_navigation_overrides_hiding_without_changing_preference() {
        use crossterm::event::{KeyEvent, KeyModifiers};
        use yute::app::{FilterKind, TabState};
        let (mut app, mut cards) = app_and_cards();
        let mut short = cards[0].clone();
        if let Card::Video(v) = &mut short {
            v.id = "fixture-short".into();
            v.badges.push("SHORTS".into());
        }
        cards.push(short.clone());
        app.items = cards;
        app.screen = Screen::Home;
        app.filter_kind = FilterKind::Videos;
        app.config.content.shorts = yute::config::ShortsVisibility::Never;
        app.home_pages.insert(
            FilterKind::Shorts,
            TabState {
                items: vec![short],
                continuation: None,
                selected: 0,
                scroll: 0,
            },
        );
        let (tx, _) = channel(16);
        let mut keys = KeymapState::new();
        handle_key_event(
            &mut app,
            &mut keys,
            KeyEvent::new(KeyCode::Char('j'), KeyModifiers::SHIFT),
            tx.clone(),
        )
        .await;
        assert_eq!(app.filter_kind, FilterKind::Shorts);
        assert_eq!(app.home_source, FilterKind::Shorts);
        assert_eq!(app.selected_card().unwrap().id(), "fixture-short");
        assert!(!app.is_loading);
        assert!(matches!(
            app.config.content.shorts,
            yute::config::ShortsVisibility::Never
        ));
        set_content_filter(&mut app, FilterKind::All, tx);
        assert_eq!(app.items.len(), 6);
        assert_eq!(app.filtered_items().len(), 5);
        assert!(!app
            .filtered_items()
            .iter()
            .any(|c| c.id() == "fixture-short"));
    }
    #[tokio::test]
    async fn item_menu_captures_target_and_consumes_keys_without_normal_navigation() {
        use crossterm::event::{KeyEvent, KeyModifiers};
        let (mut app, cards) = app_and_cards();
        app.items = cards;
        app.focus = Focus::Content;
        let mut keys = KeymapState::new();
        let (tx, _) = channel(16);
        handle_key_event(
            &mut app,
            &mut keys,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT),
            tx.clone(),
        )
        .await;
        assert!(app.action_menu.is_some());
        assert!(!app.player.is_alive());
        let target = app.action_menu.as_ref().unwrap().card.id().to_string();
        app.selected_index = 2;
        handle_key_event(
            &mut app,
            &mut keys,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            tx.clone(),
        )
        .await;
        assert!(app.action_menu.is_none());
        assert_eq!(app.player.queue.pop_next().unwrap().id, target);
        open_item_menu(&mut app);
        handle_key_event(
            &mut app,
            &mut keys,
            KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
            tx.clone(),
        )
        .await;
        assert!(!app.should_quit);
        assert!(app.action_menu.is_none());
        app.focus = Focus::CommandLine;
        app.cmd_input = "help".into();
        handle_key_event(
            &mut app,
            &mut keys,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT),
            tx,
        )
        .await;
        assert!(app.action_menu.is_none());
        assert!(app.command_help.is_some());
    }
    #[tokio::test]
    async fn cancelling_download_ignores_already_queued_completion_from_old_job() {
        use yute::actions::{menu_entries, ItemAction};
        let (mut app, cards) = app_and_cards();
        app.items = cards;
        app.focus = Focus::Content;
        app.download_active = true;
        app.download_request = 1;
        let task = tokio::spawn(std::future::pending::<()>());
        app.download_task = Some(task.abort_handle());
        open_item_menu(&mut app);
        app.action_menu.as_mut().unwrap().cursor = menu_entries(&app.items[0], false, true)
            .iter()
            .position(|e| e.action == ItemAction::CancelDownload)
            .unwrap();
        let (tx, _) = channel(16);
        handle_item_menu(
            &mut app,
            crossterm::event::KeyEvent::new(KeyCode::Enter, crossterm::event::KeyModifiers::NONE),
            tx.clone(),
        );
        tokio::task::yield_now().await;
        assert!(!app.download_active);
        assert!(task.await.unwrap_err().is_cancelled());
        app.download_request = 3;
        app.download_active = true;
        handle_app_event(&mut app, AppEvent::DownloadFinished(1, true), tx);
        assert!(app.download_active);
    }
}
