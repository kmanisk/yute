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
use ytui_core::media::cache::DiskCache;

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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ViewMode {
    Grid,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
pub struct NavState {
    pub screen: Screen,
    pub selected_index: usize,
    pub items: Vec<Card>,
    pub search_query: String,
    pub chips: Vec<Chip>,
    pub active_chip_index: usize,
    pub status_text: String,
    pub continuation_token: Option<String>,
    pub filter_text: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct AccountProfile {
    pub id: String,
    pub name: String,
    pub handle: String,
    pub initial: String,
    pub channel_id: Option<String>,
}

pub struct App {
    pub config: Config,
    pub theme: Theme,
    pub screen: Screen,
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
    pub search_suggestions: Vec<String>,
    pub suggestion_index: Option<usize>,
    pub cmd_input: String,
    pub status_text: String,
    pub is_loading: bool,
    pub spinner_frame: usize,
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
    pub scroll_row: usize,
    pub watch_tracker: Option<ytui_core::player::tracking::WatchTracker>,
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
        let auth_path = resolve_path("~/.config/ytui/auth.json");
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

        let cookies_from_browser = if !config.backend.cookies_from_browser.is_empty() {
            Some(config.backend.cookies_from_browser.clone())
        } else {
            let brave_origin = resolve_path("~/.config/BraveSoftware/Brave-Origin");
            if brave_origin.exists() {
                Some(format!("brave:{}", brave_origin.display()))
            } else {
                None
            }
        };

        let auth = std::sync::Arc::new(std::sync::Mutex::new(auth));
        let inner_backend = Box::new(InnerTubeBackend::from_arc(std::sync::Arc::clone(&auth)));
        let ytdlp_backend = Box::new(YtDlpBackend::new(Some(cookies_path), cookies_from_browser));
        let chain = Arc::new(BackendChain::new(vec![inner_backend, ytdlp_backend]));

        let mut app = Self {
            cell_w_px,
            cell_h_px,
            view_mode: ViewMode::Grid,
            search_view_mode: ViewMode::Grid,
            filter_kind: FilterKind::All,
            scroll_row: 0,
            watch_tracker: None,
            auth: std::sync::Arc::clone(&auth),
            sidebar_expanded: config.grid.sidebar_expanded,
            config,
            theme,
            screen: Screen::Home,
            focus: Focus::Content,
            sidebar_index: 0,
            settings_index: 0,
            chips: Vec::new(),
            active_chip_index: 0,
            items: Vec::new(),
            selected_index: 0,
            continuation_token: None,
            search_query: String::new(),
            search_suggestions: Vec::new(),
            suggestion_index: None,
            cmd_input: String::new(),
            status_text: "Ready".to_string(),
            is_loading: false,
            spinner_frame: 0,
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
            account_initial: "I".to_string(),
            account_name: "I am Hunter".to_string(),
            account_handle: "@iamhunter2570".to_string(),
            accounts: vec![
                AccountProfile {
                    id: "hunter".to_string(),
                    name: "I am Hunter".to_string(),
                    handle: "@iamhunter2570".to_string(),
                    initial: "I".to_string(),
                    channel_id: Some("UC3wUMz2AWASPNDptNox9hDw".to_string()),
                },
                AccountProfile {
                    id: "mathew".to_string(),
                    name: "Mathew Manisk".to_string(),
                    handle: "@mathewmanisk5707".to_string(),
                    initial: "M".to_string(),
                    channel_id: Some("113582914790437266954".to_string()),
                },
                AccountProfile {
                    id: "guest".to_string(),
                    name: "Guest (Logged Out)".to_string(),
                    handle: "@guest".to_string(),
                    initial: "G".to_string(),
                    channel_id: None,
                },
            ],
            active_account_index: 0,
            accounts_cursor: 0,
            watch_right_mode: WatchRightMode::UpNext,
            comments_cursor: 0,
        };

        // Load saved active account if exists
        let active_path = resolve_path("~/.config/ytui/active_account.json");
        if let Ok(content) = std::fs::read_to_string(&active_path) {
            if let Ok(saved) = serde_json::from_str::<AccountProfile>(&content) {
                if let Some(pos) = app
                    .accounts
                    .iter()
                    .position(|a| a.id == saved.id || a.handle == saved.handle)
                {
                    app.active_account_index = pos;
                    app.accounts_cursor = pos;
                    app.account_initial = app.accounts[pos].initial.clone();
                    app.account_name = app.accounts[pos].name.clone();
                    app.account_handle = app.accounts[pos].handle.clone();
                }
            }
        }

        app
    }

    pub fn save_active_account(&self) {
        if let Some(acc) = self.accounts.get(self.active_account_index) {
            let path = resolve_path("~/.config/ytui/active_account.json");
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(acc) {
                let _ = std::fs::write(path, json);
            }
        }
    }

    pub fn switch_account(&mut self, index: usize) -> bool {
        if index >= self.accounts.len() {
            return false;
        }
        self.active_account_index = index;
        let acc = &self.accounts[index];
        self.account_name = acc.name.clone();
        self.account_handle = acc.handle.clone();
        self.account_initial = acc.initial.clone();
        self.save_active_account();
        // Clear cached feeds so new account's data is loaded fresh
        let _ = self.thumbs.cache().clear();
        self.items.clear();
        true
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

    pub fn filtered_items(&self) -> Vec<&Card> {
        let allow_shorts = self.config.grid.show_shorts;
        self.items
            .iter()
            .filter(|c| {
                if !allow_shorts && self.filter_kind != FilterKind::Shorts {
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

    pub fn selected_card(&self) -> Option<&Card> {
        let list = self.filtered_items();
        list.get(self.selected_index).copied()
    }

    pub fn set_screen(&mut self, screen: Screen) {
        if self.screen == Screen::Watch && screen != Screen::Watch {
            self.player.stop_playback();
        }
        if self.screen != screen {
            self.nav_history.push(NavState {
                screen: self.screen,
                selected_index: self.selected_index,
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
            self.selected_index = 0;
            self.scroll_row = 0;
            self.items.clear();
            self.continuation_token = None;
        }
    }

    pub fn nav_back(&mut self) {
        if self.screen == Screen::Watch {
            self.player.stop_playback();
        }
        if let Some(prev) = self.nav_history.pop() {
            self.nav_future.push(NavState {
                screen: self.screen,
                selected_index: self.selected_index,
                items: self.items.clone(),
                search_query: self.search_query.clone(),
                chips: self.chips.clone(),
                active_chip_index: self.active_chip_index,
                status_text: self.status_text.clone(),
                continuation_token: self.continuation_token.clone(),
                filter_text: self.filter_text.clone(),
            });
            self.screen = prev.screen;
            self.selected_index = prev.selected_index;
            self.items = prev.items;
            self.search_query = prev.search_query;
            self.chips = prev.chips;
            self.active_chip_index = prev.active_chip_index;
            self.status_text = prev.status_text;
            self.continuation_token = prev.continuation_token;
            self.filter_text = prev.filter_text;
            self.sidebar_index = match self.screen {
                Screen::Home => 0,
                Screen::Subscriptions => 1,
                Screen::History => 2,
                Screen::Playlists => 3,
                Screen::WatchLater => 4,
                Screen::Liked => 5,
                Screen::LocalVideos => 6,
                Screen::Queue => 7,
                Screen::Settings => 8,
                Screen::Accounts => 9,
                _ => self.sidebar_index,
            };
        } else if self.screen != Screen::Home {
            self.set_screen(Screen::Home);
            self.sidebar_index = 0;
        } else {
            self.confirming_quit = true;
        }
    }

    pub fn nav_forward(&mut self) {
        if let Some(next) = self.nav_future.pop() {
            self.nav_history.push(NavState {
                screen: self.screen,
                selected_index: self.selected_index,
                items: self.items.clone(),
                search_query: self.search_query.clone(),
                chips: self.chips.clone(),
                active_chip_index: self.active_chip_index,
                status_text: self.status_text.clone(),
                continuation_token: self.continuation_token.clone(),
                filter_text: self.filter_text.clone(),
            });
            self.screen = next.screen;
            self.selected_index = next.selected_index;
            self.items = next.items;
            self.search_query = next.search_query;
            self.chips = next.chips;
            self.active_chip_index = next.active_chip_index;
            self.status_text = next.status_text;
            self.continuation_token = next.continuation_token;
            self.filter_text = next.filter_text;
            self.sidebar_index = match self.screen {
                Screen::Home => 0,
                Screen::Subscriptions => 1,
                Screen::History => 2,
                Screen::Playlists => 3,
                Screen::WatchLater => 4,
                Screen::Liked => 5,
                Screen::LocalVideos => 6,
                Screen::Queue => 7,
                Screen::Settings => 8,
                Screen::Accounts => 9,
                _ => self.sidebar_index,
            };
        }
    }

    pub fn tick(&mut self) -> bool {
        let mut dirty = false;

        // Poll thumbnail loading
        if self.thumbs.poll_loaded() {
            dirty = true;
        }

        // Poll mpv playback status
        let was_playing = self.player.status.is_alive;
        if self.player.poll_updates() {
            if self.player.status.stall_error && !self.status_text.contains("stalled") {
                self.status_text =
                    "Playback error: stream stalled (check ~/.cache/ytui/mpv.log)".to_string();
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
                self.status_text = "Playback finished".to_string();
                self.nav_back();
                dirty = true;
            }
        }

        // Advance spinner if loading
        if self.is_loading {
            self.spinner_frame = (self.spinner_frame + 1) % 4;
            dirty = true;
        }

        dirty
    }
}
