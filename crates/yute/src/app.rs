use crate::backend::auth::AuthContext;
use crate::backend::innertube::InnerTubeBackend;
use crate::backend::models::{Card, Chip, WatchData};
use crate::backend::ytdlp::YtDlpBackend;
use crate::backend::{Backend, BackendChain};
use crate::config::{resolve_path, Config};
use crate::media::thumbs::ThumbnailManager;
use crate::player::Player;
use crate::theme::Theme;
use ratatui_image::picker::Picker;
use std::collections::HashSet;
use std::sync::Arc;
use yute_core::media::cache::DiskCache;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Home,
    Search,
    Watch,
    Subscriptions,
    History,
    WatchLater,
    Playlists,
    PlaylistView,
    ChannelView,
    Liked,
    LocalVideos,
    Queue,
    Settings,
    Accounts,
    Help,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Focus {
    Sidebar,
    Chips,
    Content,
    SearchInput,
    CommandLine,
    CommentInput,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ViewMode {
    Grid,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FilterKind {
    All,
    Videos,
    Shorts,
    Live,
    Playlists,
    Channels,
}

impl FilterKind {
    pub fn all() -> &'static [FilterKind] {
        &[
            FilterKind::All,
            FilterKind::Videos,
            FilterKind::Shorts,
            FilterKind::Live,
            FilterKind::Playlists,
            FilterKind::Channels,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            FilterKind::All => "All",
            FilterKind::Videos => "Videos",
            FilterKind::Shorts => "Shorts",
            FilterKind::Live => "Live",
            FilterKind::Playlists => "Playlists",
            FilterKind::Channels => "Channels",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            FilterKind::All => FilterKind::Videos,
            FilterKind::Videos => FilterKind::Shorts,
            FilterKind::Shorts => FilterKind::Live,
            FilterKind::Live => FilterKind::Playlists,
            FilterKind::Playlists => FilterKind::Channels,
            FilterKind::Channels => FilterKind::All,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TabState {
    pub items: Vec<Card>,
    pub continuation: Option<String>,
    pub selected: usize,
    pub scroll: usize,
}

#[derive(Clone, Debug)]
pub struct ChannelState {
    pub channel: crate::backend::models::ChannelCard,
    pub tabs: Vec<crate::backend::models::ChannelLink>,
    pub active: usize,
    pub subscribed: Option<bool>,
    pub pages: std::collections::HashMap<Option<String>, TabState>,
}

#[derive(Clone, Debug)]
pub enum DetailRoute {
    Channel(Box<ChannelState>),
    Playlist(crate::backend::models::PlaylistCard),
}

#[derive(Clone, Debug)]
pub struct NavState {
    pub screen: Screen,
    pub detail_route: Option<DetailRoute>,
    pub selected_index: usize,
    pub scroll_row: usize,
    pub filter_kind: FilterKind,
    pub home_source: FilterKind,
    pub items: Vec<Card>,
    pub search_query: String,
    pub chips: Vec<Chip>,
    pub active_chip_index: usize,
    pub status_text: String,
    pub continuation_token: Option<String>,
    pub filter_text: String,
    pub view_mode: ViewMode,
    pub search_view_mode: ViewMode,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AccountProfile {
    pub id: String,
    pub name: String,
    pub handle: String,
    pub initial: String,
    pub channel_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchPane {
    Description,
    Comments,
    Suggestions,
}
#[derive(Clone)]
pub struct CommentDraft {
    pub video_id: String,
    pub params: String,
    pub reply: bool,
    pub author: Option<String>,
}

pub struct App {
    pub watch_pane: WatchPane,
    pub description_scroll: u16,
    pub watch_link_index: usize,
    pub comments_loading: bool,
    pub comment_input: String,
    pub comment_draft: Option<CommentDraft>,
    pub comment_posting: bool,

    pub home_pages: std::collections::HashMap<FilterKind, TabState>,
    pub channel_items: Vec<Card>,
    pub channels_revision: (u64, usize),

    pub config: Config,
    pub theme: Theme,
    pub screen: Screen,
    pub detail_route: Option<DetailRoute>,
    pub focus: Focus,
    pub sidebar_expanded: bool,
    pub sidebar_index: usize,
    pub settings_index: usize,
    pub chips: Vec<Chip>,
    pub active_chip_index: usize,
    pub items: Vec<Card>,
    pub selected_index: usize,
    pub continuation_token: Option<String>,
    pub search_query: String,
    pub search_input: String,
    pub search_suggestions: Vec<String>,
    pub suggestion_index: Option<usize>,
    pub cmd_input: String,
    pub command_history: crate::commands::History,
    pub completion_base: Option<String>,
    pub completion_index: usize,
    pub command_help: Option<String>,
    pub overlay_area: Option<ratatui::layout::Rect>,
    pub confirming_logout: bool,
    pub playback_history: Vec<crate::backend::models::VideoCard>,
    status_seen: String,
    status_deadline: Option<std::time::Instant>,
    pub status_text: String,
    pub is_loading: bool,
    pub rendered_columns: usize,
    pub rendered_rows: usize,
    pub autofill_pages: u8,
    pub which_key: bool,
    pub input_cursor: Option<usize>,
    pub feed_generation: u64,
    pub metadata_generation: u64,
    pub metadata_pending: HashSet<String>,
    pub metadata_slots: Arc<tokio::sync::Semaphore>,
    pub metadata_attempted: HashSet<String>,
    pub help_scroll: u16,
    pub is_loading_more: bool,
    pub continuation_failed: bool,
    pub player: Player,
    pub thumbs: ThumbnailManager,
    pub watch_data: Option<WatchData>,
    pub show_chat: bool,
    pub show_description: bool,
    pub filter_text: String,
    pub is_filtering: bool,
    pub selected_ids: HashSet<String>,
    pub nav_history: Vec<NavState>,
    pub nav_future: Vec<NavState>,
    pub backend: Arc<BackendChain>,
    pub auth: std::sync::Arc<std::sync::Mutex<AuthContext>>,
    pub auth_generation: Arc<std::sync::atomic::AtomicU64>,
    pub should_quit: bool,
    pub confirming_quit: bool,
    pub account_initial: String,
    pub account_name: String,
    pub account_handle: String,
    pub accounts: Vec<AccountProfile>,
    pub active_account_index: usize,
    pub accounts_cursor: usize,
    pub cell_w_px: f32,
    pub cell_h_px: f32,
    pub view_mode: ViewMode,
    pub search_view_mode: ViewMode,
    pub filter_kind: FilterKind,
    pub home_source: FilterKind,
    pub scroll_row: usize,
    pub watch_tracker: Option<yute_core::player::tracking::WatchTracker>,
    pub watch_right_mode: WatchRightMode,
    pub comments_cursor: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchRightMode {
    UpNext,
    Comments,
}

impl App {
    pub fn new(config: Config, picker: Picker) -> Self {
        let theme = Theme::load(&config.general.theme);
        let font_size = picker.font_size();
        let (cell_w_px, cell_h_px) = if font_size.width > 0 && font_size.height > 0 {
            (font_size.width as f32, font_size.height as f32)
        } else {
            (10.0, 25.0)
        };
        let disk_cache = DiskCache::new(config.cache.max_mb);
        let thumbs = ThumbnailManager::new(picker, disk_cache);
        let player = Player::new(&config);

        let cookies_path = resolve_path(&config.backend.cookies_file);
        let auth_path = resolve_path("~/.config/yute/auth.json");
        let mut auth = AuthContext::default();
        auth.hl = config.backend.hl.clone();
        auth.gl = config.backend.gl.clone();
        if config.backend.client == "web" {
            auth.client_name = "WEB".to_string();
            auth.client_version = "2.20240101.00.00".to_string();
            auth.user_agent = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36".to_string();
        } else {
            if !config.backend.client_name.is_empty() {
                auth.client_name = config.backend.client_name.clone();
            }
            if !config.backend.client_version.is_empty() {
                auth.client_version = config.backend.client_version.clone();
            }
        }
        let _ = auth.load_auth_with_mode(&auth_path, &cookies_path, &config.backend.auth);

        let auth = std::sync::Arc::new(std::sync::Mutex::new(auth));
        let chain = backend_for_session(&config, Arc::clone(&auth));

        let authenticated = {
            let auth = auth.lock().unwrap();
            auth.access_token.is_some() || auth.cookie_header.is_some()
        };
        Self {
            cell_w_px,
            cell_h_px,
            view_mode: ViewMode::Grid,
            search_view_mode: ViewMode::Grid,
            filter_kind: FilterKind::All,
            home_source: FilterKind::All,
            home_pages: Default::default(),
            watch_pane: WatchPane::Description,
            description_scroll: 0,
            watch_link_index: 0,
            comments_loading: false,
            comment_input: String::new(),
            comment_draft: None,
            comment_posting: false,
            channel_items: Vec::new(),
            channels_revision: (u64::MAX, 0),
            scroll_row: 0,
            watch_tracker: None,
            auth: std::sync::Arc::clone(&auth),
            auth_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            sidebar_expanded: config.grid.sidebar_expanded,
            config,
            theme,
            screen: Screen::Home,
            detail_route: None,
            focus: Focus::Content,
            sidebar_index: 0,
            settings_index: 0,
            chips: Vec::new(),
            active_chip_index: 0,
            items: Vec::new(),
            selected_index: 0,
            continuation_token: None,
            search_query: String::new(),
            search_input: String::new(),
            search_suggestions: Vec::new(),
            suggestion_index: None,
            cmd_input: String::new(),
            command_history: Default::default(),
            completion_base: None,
            completion_index: 0,
            command_help: None,
            overlay_area: None,
            confirming_logout: false,
            playback_history: Vec::new(),
            status_seen: String::new(),
            status_deadline: None,
            status_text: "Ready".to_string(),
            is_loading: false,
            rendered_columns: 1,
            rendered_rows: 1,
            autofill_pages: 0,
            which_key: false,
            input_cursor: None,
            feed_generation: 0,
            metadata_generation: 0,
            metadata_pending: HashSet::new(),
            metadata_slots: Arc::new(tokio::sync::Semaphore::new(2)),
            metadata_attempted: HashSet::new(),
            help_scroll: 0,
            is_loading_more: false,
            continuation_failed: false,
            player,
            thumbs,
            watch_data: None,
            show_chat: false,
            show_description: false,
            filter_text: String::new(),
            is_filtering: false,
            selected_ids: HashSet::new(),
            nav_history: Vec::new(),
            nav_future: Vec::new(),
            backend: chain,
            should_quit: false,
            confirming_quit: false,
            account_initial: if authenticated { "S" } else { "G" }.into(),
            account_name: if authenticated { "Signed in" } else { "Guest" }.into(),
            account_handle: String::new(),
            accounts: Vec::new(),
            active_account_index: 0,
            accounts_cursor: 0,
            watch_right_mode: WatchRightMode::UpNext,
            comments_cursor: 0,
        }
    }

    pub fn save_active_account(&self) {
        if let Some(acc) = self.accounts.get(self.active_account_index) {
            let path = resolve_path("~/.config/yute/active_account.json");
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(acc) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    pub fn rebuild_backend(&mut self) {
        self.home_pages.clear();
        self.home_source = FilterKind::All;
        self.channel_items.clear();
        self.channels_revision = (u64::MAX, 0);
        self.backend = backend_for_session(&self.config, Arc::clone(&self.auth));
    }

    pub fn switch_account(&mut self, _index: usize) -> bool {
        self.status_text = "Account switching is unavailable — use :login to authenticate".into();
        false
    }

    /// A new full feed request supersedes older work, including continuation loads.
    pub fn begin_feed_request(&mut self) -> u64 {
        self.autofill_pages = 0;
        self.feed_generation = self.feed_generation.wrapping_add(1);
        self.is_loading = true;
        self.is_loading_more = false;
        self.continuation_failed = false;
        self.feed_generation
    }

    pub fn current_view_mode(&self) -> ViewMode {
        if self.screen == Screen::Search {
            self.search_view_mode
        } else {
            self.view_mode
        }
    }

    pub fn toggle_view_mode(&mut self) {
        if self.screen == Screen::Search {
            self.search_view_mode = match self.search_view_mode {
                ViewMode::Grid => ViewMode::List,
                ViewMode::List => ViewMode::Grid,
            };
        } else {
            self.view_mode = match self.view_mode {
                ViewMode::Grid => ViewMode::List,
                ViewMode::List => ViewMode::Grid,
            };
        }
    }

    pub fn cycle_filter_kind(&mut self) {
        self.filter_kind = self.filter_kind.next();
        self.selected_index = 0;
        self.scroll_row = 0;
        self.status_text = format!("Filter: {}", self.filter_kind.label());
    }

    pub fn refresh_channel_items(&mut self) {
        let revision = (self.feed_generation, self.items.len());
        if self.channels_revision == revision {
            return;
        }
        self.channels_revision = revision;
        self.channel_items.clear();
        let mut ids = self
            .items
            .iter()
            .filter_map(|c| {
                if let Card::Channel(c) = c {
                    Some(c.id.clone())
                } else {
                    None
                }
            })
            .collect::<HashSet<_>>();
        for card in &self.items {
            if let Card::Video(video) = card {
                if let Some(id) = video.channel_id.as_ref().filter(|id| !id.is_empty()) {
                    if !video.channel.is_empty() && ids.insert(id.clone()) {
                        self.channel_items.push(Card::Channel(
                            crate::backend::models::ChannelCard {
                                id: id.clone(),
                                title: video.channel.clone(),
                                handle: None,
                                subscribers: None,
                                avatar_url: None,
                                verified: video.verified,
                            },
                        ));
                    }
                }
            }
        }
    }

    pub fn filtered_items(&self) -> Vec<&Card> {
        let saved = matches!(
            self.screen,
            Screen::History
                | Screen::Liked
                | Screen::WatchLater
                | Screen::PlaylistView
                | Screen::Queue
                | Screen::LocalVideos
        );
        let shorts_tab = matches!(&self.detail_route, Some(DetailRoute::Channel(route)) if route.tabs.get(route.active).is_some_and(|tab| tab.title.eq_ignore_ascii_case("shorts")));
        let allow_shorts = saved
            || shorts_tab
            || matches!(
                self.config.content.shorts,
                crate::config::ShortsVisibility::Show
            );
        self.items
            .iter()
            .chain(
                self.channel_items
                    .iter()
                    .filter(|_| self.filter_kind == FilterKind::Channels),
            )
            .filter(|c| {
                if matches!(
                    self.config.content.shorts,
                    crate::config::ShortsVisibility::Never
                ) || (!allow_shorts && self.filter_kind != FilterKind::Shorts)
                {
                    if let Card::Video(v) = c {
                        if v.badges.iter().any(|b| b == "SHORTS") {
                            return false;
                        }
                    }
                }
                match self.filter_kind {
                    FilterKind::All => true,
                    FilterKind::Videos => match c {
                        Card::Video(v) => {
                            !v.badges.iter().any(|b| b == "SHORTS")
                                && v.live == crate::backend::models::LiveState::None
                        }
                        _ => false,
                    },
                    FilterKind::Shorts => match c {
                        Card::Video(v) => v.badges.iter().any(|b| b == "SHORTS"),
                        _ => false,
                    },
                    FilterKind::Live => match c {
                        Card::Video(v) => v.live == crate::backend::models::LiveState::Live,
                        _ => false,
                    },
                    FilterKind::Playlists => matches!(c, Card::Playlist(_)),
                    FilterKind::Channels => matches!(c, Card::Channel(_)),
                }
            })
            .filter(|c| {
                if self.filter_text.is_empty() {
                    true
                } else {
                    let q = self.filter_text.to_lowercase();
                    c.title().to_lowercase().contains(&q)
                }
            })
            .collect()
    }

    pub fn watch_suggestions(&self) -> Vec<&Card> {
        self.watch_data
            .as_ref()
            .map(|w| {
                w.up_next.iter().filter(|card| {
            !(matches!(self.config.content.shorts, crate::config::ShortsVisibility::Never)
                && matches!(card, Card::Video(v) if v.badges.iter().any(|b| b == "SHORTS")))
        }).collect()
            })
            .unwrap_or_default()
    }

    pub fn selected_card(&self) -> Option<&Card> {
        if self.screen == Screen::Watch {
            return self.watch_suggestions().get(self.selected_index).copied();
        }
        let list = self.filtered_items();
        list.get(self.selected_index).copied()
    }

    pub fn set_screen(&mut self, screen: Screen) {
        self.transition(screen, false);
    }

    pub fn transition(&mut self, screen: Screen, force: bool) {
        if force || self.screen != screen {
            self.feed_generation = self.feed_generation.wrapping_add(1);
            self.nav_history.push(NavState {
                screen: self.screen,
                detail_route: self.detail_route.clone(),
                view_mode: self.view_mode,
                search_view_mode: self.search_view_mode,
                selected_index: self.selected_index,
                scroll_row: self.scroll_row,
                filter_kind: self.filter_kind,
                home_source: self.home_source,
                items: self.items.clone(),
                search_query: self.search_query.clone(),
                chips: self.chips.clone(),
                active_chip_index: self.active_chip_index,
                status_text: self.status_text.clone(),
                continuation_token: self.continuation_token.clone(),
                filter_text: self.filter_text.clone(),
            });
            self.nav_future.clear();
            self.screen = screen;
            if screen == Screen::Home {
                self.home_source = FilterKind::All;
            }
            self.detail_route = None;
            self.filter_kind = FilterKind::All;
            self.filter_text.clear();
            self.selected_index = 0;
            self.scroll_row = 0;
            self.items.clear();
            self.continuation_token = None;
            self.is_loading = false;
            self.is_loading_more = false;
            self.continuation_failed = false;
        }
    }

    pub fn nav_back(&mut self) {
        self.feed_generation = self.feed_generation.wrapping_add(1);
        self.is_loading = false;
        self.is_loading_more = false;
        if let Some(prev) = self.nav_history.pop() {
            self.nav_future.push(NavState {
                screen: self.screen,
                detail_route: self.detail_route.clone(),
                view_mode: self.view_mode,
                search_view_mode: self.search_view_mode,
                selected_index: self.selected_index,
                scroll_row: self.scroll_row,
                filter_kind: self.filter_kind,
                home_source: self.home_source,
                items: self.items.clone(),
                search_query: self.search_query.clone(),
                chips: self.chips.clone(),
                active_chip_index: self.active_chip_index,
                status_text: self.status_text.clone(),
                continuation_token: self.continuation_token.clone(),
                filter_text: self.filter_text.clone(),
            });
            self.screen = prev.screen;
            self.detail_route = prev.detail_route;
            self.view_mode = prev.view_mode;
            self.search_view_mode = prev.search_view_mode;
            self.selected_index = prev.selected_index;
            self.scroll_row = prev.scroll_row;
            self.filter_kind = prev.filter_kind;
            self.home_source = prev.home_source;
            self.items = prev.items;
            self.search_query = prev.search_query;
            self.chips = prev.chips;
            self.active_chip_index = prev.active_chip_index;
            self.status_text = prev.status_text;
            self.continuation_token = prev.continuation_token;
            self.filter_text = prev.filter_text;
            self.sidebar_index =
                crate::ui::sidebar::index_of(self.screen).unwrap_or(self.sidebar_index);
        } else if self.screen != Screen::Home {
            self.set_screen(Screen::Home);
            self.sidebar_index = 0;
        } else {
            self.confirming_quit = true;
        }
    }

    pub fn nav_forward(&mut self) {
        self.feed_generation = self.feed_generation.wrapping_add(1);
        self.is_loading = false;
        self.is_loading_more = false;
        if let Some(next) = self.nav_future.pop() {
            self.nav_history.push(NavState {
                screen: self.screen,
                detail_route: self.detail_route.clone(),
                view_mode: self.view_mode,
                search_view_mode: self.search_view_mode,
                selected_index: self.selected_index,
                scroll_row: self.scroll_row,
                filter_kind: self.filter_kind,
                home_source: self.home_source,
                items: self.items.clone(),
                search_query: self.search_query.clone(),
                chips: self.chips.clone(),
                active_chip_index: self.active_chip_index,
                status_text: self.status_text.clone(),
                continuation_token: self.continuation_token.clone(),
                filter_text: self.filter_text.clone(),
            });
            self.screen = next.screen;
            self.detail_route = next.detail_route;
            self.view_mode = next.view_mode;
            self.search_view_mode = next.search_view_mode;
            self.selected_index = next.selected_index;
            self.scroll_row = next.scroll_row;
            self.filter_kind = next.filter_kind;
            self.home_source = next.home_source;
            self.items = next.items;
            self.search_query = next.search_query;
            self.chips = next.chips;
            self.active_chip_index = next.active_chip_index;
            self.status_text = next.status_text;
            self.continuation_token = next.continuation_token;
            self.filter_text = next.filter_text;
            self.sidebar_index =
                crate::ui::sidebar::index_of(self.screen).unwrap_or(self.sidebar_index);
        }
    }

    pub fn tick(&mut self) -> bool {
        let mut dirty = false;
        if self.status_text != self.status_seen {
            self.status_seen.clone_from(&self.status_text);
            self.status_deadline =
                if self.status_text.is_empty() || self.status_text.starts_with("Login:") {
                    None
                } else {
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(6))
                };
        }
        if self
            .status_deadline
            .is_some_and(|deadline| std::time::Instant::now() >= deadline)
        {
            self.status_text.clear();
            self.status_seen.clear();
            self.status_deadline = None;
            dirty = true;
        }

        // Poll thumbnail loading
        if self.thumbs.poll_loaded() {
            dirty = true;
        }

        // Poll mpv playback status
        let was_playing = self.player.status.is_alive;
        if self.player.poll_updates() {
            if let Some(error) = &self.player.status.playback_error {
                self.status_text = format!("Playback error: {error}");
            } else if self.player.status.stall_error && !self.status_text.contains("stalled") {
                self.status_text =
                    "Playback error: stream stalled (check ~/.cache/yute/mpv.log)".to_string();
            }
            dirty = true;
        }

        // Periodic tracking pings with real mpv time-pos
        if self.player.is_alive() && self.player.status.file_loaded && !self.player.status.paused {
            let pos = self.player.status.time_pos;
            if let Some(ref mut tracker) = self.watch_tracker {
                if !tracker.playback_ping_sent && pos > 0.0 {
                    tracker.playback_ping_sent = true;
                    let backend = Arc::clone(&self.backend);
                    let vid = tracker.video_id.clone();
                    let cpn = tracker.cpn.clone();
                    tokio::spawn(async move {
                        let _ = backend.ping_playback(&vid, &cpn).await;
                    });
                }
                if tracker.should_ping_watchtime(pos) {
                    let (st, et, cmt) = tracker.record_ping(pos);
                    let backend = Arc::clone(&self.backend);
                    let vid = tracker.video_id.clone();
                    let cpn = tracker.cpn.clone();
                    tokio::spawn(async move {
                        let _ = backend.ping_watchtime(&vid, &cpn, st, et, cmt, false).await;
                    });
                }
            }
        }

        // Automatically return from watch screen when mpv closes and send final watchtime ping
        if was_playing && !self.player.status.is_alive {
            if let Some(tracker) = self.watch_tracker.take() {
                let final_pos = self.player.status.time_pos;
                if final_pos > 0.0 {
                    let backend = Arc::clone(&self.backend);
                    tokio::spawn(async move {
                        let _ = backend
                            .ping_watchtime(
                                &tracker.video_id,
                                &tracker.cpn,
                                tracker.last_ping_pos,
                                final_pos,
                                final_pos,
                                true,
                            )
                            .await;
                    });
                }
            }
            if self.screen == Screen::Watch {
                self.nav_back();
                self.status_text = match &self.player.status.playback_error {
                    Some(error) => format!("Playback error: {error}"),
                    None => "Playback finished".to_string(),
                };
                dirty = true;
            }
        }

        dirty
    }
}

fn backend_for_session(
    config: &Config,
    auth: Arc<std::sync::Mutex<AuthContext>>,
) -> Arc<BackendChain> {
    let cookies_from_browser = if config.backend.auth == "guest" {
        None
    } else if !config.backend.cookies_from_browser.is_empty() {
        Some(config.backend.cookies_from_browser.clone())
    } else {
        let path = resolve_path("~/.config/BraveSoftware/Brave-Origin");
        path.exists().then(|| format!("brave:{}", path.display()))
    };
    Arc::new(BackendChain::new(vec![
        Box::new(InnerTubeBackend::from_arc(auth)),
        Box::new(YtDlpBackend::new(
            (config.backend.auth != "guest").then(|| resolve_path(&config.backend.cookies_file)),
            cookies_from_browser,
        )),
    ]))
}
