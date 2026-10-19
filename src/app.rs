use crate::backend::auth::AuthContext;
use crate::backend::innertube::InnerTubeBackend;
use crate::backend::models::{Card, Chip, WatchData};
use crate::backend::ytdlp::YtDlpBackend;
use crate::backend::BackendChain;
use crate::config::{resolve_path, Config};
use crate::media::cache::DiskCache;
use crate::media::thumbs::ThumbnailManager;
use crate::player::Player;
use crate::theme::Theme;
use ratatui_image::picker::Picker;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Home,
    Search,
    Watch,
    Subscriptions,
    History,
    WatchLater,
    Playlists,
    Liked,
    LocalVideos,
    Queue,
    Help,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Chips,
    Content,
    SearchInput,
    CommandLine,
}

pub struct App {
    pub config: Config,
    pub theme: Theme,
    pub screen: Screen,
    pub focus: Focus,
    pub sidebar_expanded: bool,
    pub sidebar_index: usize,
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
    pub nav_history: Vec<(Screen, usize)>,
    pub nav_future: Vec<(Screen, usize)>,
    pub backend: Arc<BackendChain>,
    pub should_quit: bool,
    pub account_initial: String,
}

impl App {
    pub fn new(config: Config, picker: Picker) -> Self {
        let theme = Theme::load(&config.general.theme);
        let disk_cache = DiskCache::new(config.cache.max_mb);
        let thumbs = ThumbnailManager::new(picker, disk_cache);
        let player = Player::new(&config);

        let cookies_path = resolve_path(&config.backend.cookies_file);
        let mut auth = AuthContext::load_from_file(&cookies_path).unwrap_or_default();
        auth.hl = config.backend.hl.clone();
        auth.gl = config.backend.gl.clone();
        if !config.backend.client_version.is_empty() {
            auth.client_version = config.backend.client_version.clone();
        }

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

        let inner_backend = Box::new(InnerTubeBackend::new(auth));
        let ytdlp_backend = Box::new(YtDlpBackend::new(Some(cookies_path), cookies_from_browser));
        let chain = Arc::new(BackendChain::new(vec![inner_backend, ytdlp_backend]));

        Self {
            sidebar_expanded: config.grid.sidebar_expanded,
            config,
            theme,
            screen: Screen::Home,
            focus: Focus::Content,
            sidebar_index: 0,
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
            account_initial: "K".to_string(),
        }
    }

    pub fn filtered_items(&self) -> Vec<&Card> {
        if self.filter_text.is_empty() {
            self.items.iter().collect()
        } else {
            let q = self.filter_text.to_lowercase();
            self.items
                .iter()
                .filter(|c| c.title().to_lowercase().contains(&q))
                .collect()
        }
    }

    pub fn selected_card(&self) -> Option<&Card> {
        let list = self.filtered_items();
        list.get(self.selected_index).copied()
    }

    pub fn set_screen(&mut self, screen: Screen) {
        if self.screen != screen {
            self.nav_history.push((self.screen, self.selected_index));
            self.nav_future.clear();
            self.screen = screen;
            self.selected_index = 0;
            self.items.clear();
            self.continuation_token = None;
        }
    }

    pub fn nav_back(&mut self) {
        if let Some((prev_screen, prev_idx)) = self.nav_history.pop() {
            self.nav_future.push((self.screen, self.selected_index));
            self.screen = prev_screen;
            self.selected_index = prev_idx;
        } else if self.screen != Screen::Home {
            self.set_screen(Screen::Home);
        } else {
            self.should_quit = true;
        }
    }

    pub fn nav_forward(&mut self) {
        if let Some((next_screen, next_idx)) = self.nav_future.pop() {
            self.nav_history.push((self.screen, self.selected_index));
            self.screen = next_screen;
            self.selected_index = next_idx;
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
            dirty = true;
        }

        // Automatically return from watch screen when mpv closes
        if was_playing && !self.player.status.is_alive && self.screen == Screen::Watch {
            self.status_text = "Playback finished".to_string();
            self.nav_back();
            dirty = true;
        }

        // Advance spinner if loading
        if self.is_loading {
            self.spinner_frame = (self.spinner_frame + 1) % 4;
            dirty = true;
        }

        dirty
    }
}
