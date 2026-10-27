use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    // Navigation
    Up,
    Down,
    Left,
    Right,
    First,
    Last,
    PageDown,
    PageUp,
    HalfPageDown,
    HalfPageUp,
    CycleFocusForward,
    CycleFocusBackward,
    ToggleSidebar,
    FocusSidebar,
    PrevChip,
    NextChip,
    FocusSearch,
    Filter,
    PrevFilter,
    NextFilter,
    Refresh,
    HardRefresh,
    NavHome,
    NavWatch,
    NavSubs,
    NavHistory,
    NavWatchLater,
    NavPlaylists,
    NavLiked,
    NavLocal,
    NavQueue,
    NavSettings,
    NavAccounts,
    NavChannel,
    NavBackHistory,
    NavForwardHistory,
    Back,
    Quit,
    ForceQuit,
    CommandLine,
    Help,
    Redraw,

    // Cards / lists
    PlayDefault,
    ItemActions,
    PlayWindow,
    OpenBrowser,
    QueueAppend,
    QueuePrepend,
    ToggleSelect,
    ToggleViewMode,
    SelectAll,
    ToggleInfo,
    YankUrl,
    YankTitleAndUrl,
    MarkWatched,
    AddToWatchLater,

    // Watch screen / mpv
    PauseResume,
    SeekBack5,
    SeekForward5,
    SeekBack30,
    SeekForward30,
    NextItem,
    PrevItem,
    VolumeUp,
    VolumeDown,
    ToggleMute,
    SpeedDown,
    SpeedUp,
    PrevChapter,
    NextChapter,
    CycleSubtitles,
    ToggleSponsorBlock,
    ToggleChat,
    ToggleDescription,
    ToggleFullscreen,
    ToggleAutoplay,
    ToggleStats,
    StopPlayback,
    LikeVideo,
    DislikeVideo,
    ToggleSubscribe,
    ToggleComments,
    FocusSuggestions,
    ComposeComment,
    ReplyComment,
    OpenDescriptionLink,
}

/// Normal bindings shared by dispatch and keyboard help.
pub const BINDINGS: &[(char, Action, &str)] = &[
    ('h', Action::Left, "Previous column"),
    ('j', Action::Down, "Next row"),
    ('k', Action::Up, "Previous row"),
    ('l', Action::Right, "Next column"),
    ('G', Action::Last, "Last item"),
    ('b', Action::ToggleSidebar, "Toggle sidebar"),
    ('H', Action::NavBackHistory, "Back in route history"),
    ('L', Action::NavForwardHistory, "Forward in route history"),
    ('/', Action::FocusSearch, "Search"),
    (':', Action::CommandLine, "Command line"),
    ('?', Action::Help, "Keyboard help"),
    ('q', Action::Quit, "Quit application"),
    ('f', Action::Filter, "Cycle content type"),
    ('J', Action::NextFilter, "Next content tab"),
    ('K', Action::PrevFilter, "Previous content tab"),
    ('r', Action::Refresh, "Refresh/retry"),
    ('a', Action::QueueAppend, "Add to queue"),
    ('y', Action::YankUrl, "Copy URL"),
    ('c', Action::NavChannel, "Open channel"),
    ('v', Action::ToggleViewMode, "Grid/list"),
    ('[', Action::PrevChip, "Previous channel tab"),
    (']', Action::NextChip, "Next channel tab"),
];
pub const G_BINDINGS: &[(char, Action, &str)] = &[
    ('g', Action::First, "First item"),
    ('h', Action::NavHome, "Home"),
    ('s', Action::NavSubs, "Subscriptions"),
    ('y', Action::NavHistory, "History"),
    ('w', Action::NavWatchLater, "Watch Later"),
    ('l', Action::NavLiked, "Liked"),
    ('p', Action::NavPlaylists, "Playlists"),
    ('q', Action::NavQueue, "Queue"),
    ('v', Action::NavLocal, "Local"),
    ('a', Action::NavAccounts, "Account"),
    (',', Action::NavSettings, "Settings"),
    ('n', Action::NavWatch, "Now Playing"),
    ('c', Action::NavChannel, "Channel"),
];

/// Aliases, action, help text and whether a numeric repeat applies.
pub const WATCH_BINDINGS: &[(&str, Action, &str, bool)] = &[
    ("j", Action::Down, "Move/scroll down", true),
    ("k", Action::Up, "Move/scroll up", true),
    ("h", Action::Left, "Previous Watch section", false),
    ("l", Action::Right, "Next Watch section", false),
    ("d", Action::ToggleDescription, "Description", false),
    ("c", Action::ToggleComments, "Comments", false),
    ("w", Action::FocusSuggestions, "Video suggestions", false),
    (
        "+=",
        Action::LikeVideo,
        "Like video / focused comment",
        false,
    ),
    (
        "-",
        Action::DislikeVideo,
        "Dislike video / focused comment",
        false,
    ),
    ("n", Action::ComposeComment, "Write a comment", false),
    (
        "r",
        Action::ReplyComment,
        "Reply to selected comment",
        false,
    ),
    (
        "O",
        Action::OpenDescriptionLink,
        "Open selected description link",
        false,
    ),
    (
        "o",
        Action::OpenBrowser,
        "Open current video in browser",
        false,
    ),
    ("[", Action::PrevChip, "Previous description link", false),
    ("]", Action::NextChip, "Next description link", false),
];

pub struct KeymapState {
    count_buf: String,
    prefix_char: Option<char>,
    prefix_at: Option<std::time::Instant>,
}

impl Default for KeymapState {
    fn default() -> Self {
        Self {
            count_buf: String::new(),
            prefix_char: None,
            prefix_at: None,
        }
    }
}

impl KeymapState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_key(&mut self, key: KeyEvent, in_watch: bool) -> Option<(Action, usize)> {
        if self.prefix_at.is_some_and(|at| at.elapsed().as_secs() >= 4) {
            self.prefix_char = None;
            self.prefix_at = None;
        }
        let key = normalize_key(key);
        let code = key.code;
        let mods = key.modifiers;

        // Quit now
        if (mods == KeyModifiers::CONTROL && code == KeyCode::Char('c'))
            || (mods == KeyModifiers::CONTROL && code == KeyCode::Char('q'))
        {
            return Some((Action::ForceQuit, 1));
        }

        if mods == KeyModifiers::ALT && code == KeyCode::Char('e') {
            return Some((Action::FocusSidebar, 1));
        }
        if !mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
            match code {
                KeyCode::Char('H') => return Some((Action::NavBackHistory, 1)),
                KeyCode::Char('L') => return Some((Action::NavForwardHistory, 1)),
                _ => {}
            }
        }

        // Multi-key 'g' handler
        if let Some(prefix) = self.prefix_char {
            self.prefix_char = None;
            let count = self.take_count();
            if prefix == 'g' {
                if let KeyCode::Char(c) = code {
                    if c == 'o' {
                        return Some((Action::NavSettings, count));
                    }
                    return G_BINDINGS
                        .iter()
                        .find(|(key, _, _)| *key == c)
                        .map(|(_, action, _)| (*action, count));
                }
            }
            return None;
        }

        // Digit prefix for counts (1..9, or 0 if count_buf is not empty)
        if mods.is_empty() {
            if let KeyCode::Char(c) = code {
                if c.is_ascii_digit() && (c != '0' || !self.count_buf.is_empty()) {
                    self.count_buf.push(c);
                    return None;
                }
            }
        }

        let count = self.take_count();

        // Control combinations
        if mods == KeyModifiers::CONTROL {
            match code {
                KeyCode::Char('d') => return Some((Action::HalfPageDown, count)),
                KeyCode::Char('u') => return Some((Action::HalfPageUp, count)),
                KeyCode::Char('f') => return Some((Action::PageDown, count)),
                KeyCode::Char('b') => return Some((Action::PageUp, count)),
                KeyCode::Char('o') => return Some((Action::NavBackHistory, 1)),
                KeyCode::Char('i') => return Some((Action::NavForwardHistory, 1)),
                KeyCode::Char('l') => return Some((Action::Redraw, 1)),
                _ => {}
            }
        }

        if code == KeyCode::Enter && mods.contains(KeyModifiers::SHIFT) {
            return Some((Action::ItemActions, 1));
        }

        // Watch mode bindings
        if in_watch {
            if let KeyCode::Char(c) = code {
                if let Some((_, action, _, repeat)) = WATCH_BINDINGS
                    .iter()
                    .find(|(keys, _, _, _)| keys.contains(c))
                {
                    return Some((*action, if *repeat { count } else { 1 }));
                }
            }
            match code {
                KeyCode::Down => return Some((Action::Down, count)),
                KeyCode::Up => return Some((Action::Up, count)),
                KeyCode::Enter => return Some((Action::PlayDefault, 1)),
                KeyCode::Esc => return Some((Action::Back, 1)),
                _ => {}
            }
        }

        if !mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) {
            if let KeyCode::Char(c) = code {
                if let Some((_, action, _)) = BINDINGS.iter().find(|(key, _, _)| *key == c) {
                    return Some((*action, count));
                }
            }
        }

        // Standard browsing bindings
        match code {
            KeyCode::Left => Some((Action::Left, count)),
            KeyCode::Down => Some((Action::Down, count)),
            KeyCode::Up => Some((Action::Up, count)),
            KeyCode::Right => Some((Action::Right, count)),
            KeyCode::Char('g') => {
                self.prefix_char = Some('g');
                self.prefix_at = Some(std::time::Instant::now());
                None
            }
            KeyCode::Tab => {
                if mods.contains(KeyModifiers::SHIFT) {
                    Some((Action::CycleFocusBackward, 1))
                } else {
                    Some((Action::CycleFocusForward, 1))
                }
            }
            KeyCode::BackTab => Some((Action::CycleFocusBackward, 1)),
            KeyCode::Char('R') => Some((Action::HardRefresh, 1)),
            KeyCode::Char('H') => Some((Action::NavBackHistory, 1)),
            KeyCode::Char('S') => Some((Action::NavSubs, 1)),
            KeyCode::Char('Y') => Some((Action::NavHistory, 1)),
            KeyCode::Char('P') => Some((Action::NavPlaylists, 1)),
            KeyCode::Char('W') => Some((Action::NavWatchLater, 1)),
            KeyCode::Char('L') => Some((Action::NavForwardHistory, 1)),
            KeyCode::Char('V') => Some((Action::NavLocal, 1)),
            KeyCode::Char('Q') => Some((Action::NavQueue, 1)),
            KeyCode::Char('O') => Some((Action::NavSettings, 1)),
            KeyCode::Esc => Some((Action::Back, 1)),
            KeyCode::Enter => Some((Action::PlayDefault, 1)),
            KeyCode::Char('w') => Some((Action::PlayWindow, 1)),
            KeyCode::Char('o') => Some((Action::OpenBrowser, 1)),
            KeyCode::Char('A') | KeyCode::Char('C') => Some((Action::NavAccounts, 1)),
            KeyCode::Char('x') => Some((Action::ToggleSelect, 1)),
            KeyCode::Char('i') => Some((Action::ToggleInfo, 1)),
            KeyCode::Char('s') => Some((Action::ToggleSubscribe, 1)),
            KeyCode::Char('m') => Some((Action::MarkWatched, 1)),
            _ => None,
        }
    }

    pub fn has_pending(&self) -> bool {
        self.prefix_at.is_some()
    }

    pub fn which_key_visible(&mut self) -> bool {
        let Some(at) = self.prefix_at else {
            return false;
        };
        if at.elapsed().as_secs() >= 4 {
            self.prefix_char = None;
            self.prefix_at = None;
            return false;
        }
        self.prefix_char == Some('g') && at.elapsed().as_millis() >= 400
    }

    fn take_count(&mut self) -> usize {
        if self.count_buf.is_empty() {
            1
        } else {
            let cnt = self.count_buf.parse::<usize>().unwrap_or(1).max(1);
            self.count_buf.clear();
            cnt
        }
    }
}

/// Terminals may report shifted ASCII as lowercase+SHIFT or uppercase.
pub fn normalize_key(mut key: KeyEvent) -> KeyEvent {
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        if let KeyCode::Char(c) = key.code {
            key.code = KeyCode::Char(match c {
                ';' => ':',
                '/' => '?',
                c if c.is_ascii_lowercase() => c.to_ascii_uppercase(),
                c => c,
            });
        }
    }
    key
}
