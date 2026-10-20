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
    PrevChip,
    NextChip,
    FocusSearch,
    Filter,
    Refresh,
    HardRefresh,
    NavHome,
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
    PlayInline,
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
}

pub struct KeymapState {
    count_buf: String,
    prefix_char: Option<char>,
}

impl Default for KeymapState {
    fn default() -> Self {
        Self {
            count_buf: String::new(),
            prefix_char: None,
        }
    }
}

impl KeymapState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle_key(&mut self, key: KeyEvent, in_watch: bool) -> Option<(Action, usize)> {
        let code = key.code;
        let mods = key.modifiers;

        // Quit now
        if (mods == KeyModifiers::CONTROL && code == KeyCode::Char('c'))
            || (mods == KeyModifiers::CONTROL && code == KeyCode::Char('q'))
        {
            return Some((Action::ForceQuit, 1));
        }

        // Multi-key 'g' handler
        if let Some(prefix) = self.prefix_char {
            self.prefix_char = None;
            let count = self.take_count();
            if prefix == 'g' {
                match code {
                    KeyCode::Char('g') => return Some((Action::First, count)),
                    KeyCode::Char('h') => return Some((Action::NavHome, 1)),
                    KeyCode::Char('s') => return Some((Action::NavSubs, 1)),
                    KeyCode::Char('y') => return Some((Action::NavHistory, 1)),
                    KeyCode::Char('w') => return Some((Action::NavWatchLater, 1)),
                    KeyCode::Char('p') => return Some((Action::NavPlaylists, 1)),
                    KeyCode::Char('l') => return Some((Action::NavLiked, 1)),
                    KeyCode::Char('v') => return Some((Action::NavLocal, 1)),
                    KeyCode::Char('q') => return Some((Action::NavQueue, 1)),
                    KeyCode::Char('o') => return Some((Action::NavSettings, 1)),
                    KeyCode::Char('a') => return Some((Action::NavAccounts, 1)),
                    KeyCode::Char('c') => return Some((Action::NavChannel, 1)),
                    _ => return None,
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

        // Watch mode bindings
        if in_watch {
            match code {
                KeyCode::Char(' ') | KeyCode::Char('k') | KeyCode::Char('K') => {
                    return Some((Action::PauseResume, 1))
                }
                KeyCode::Char('h') => return Some((Action::SeekBack5, count)),
                KeyCode::Char('l') => return Some((Action::SeekForward5, count)),
                KeyCode::Char('H') => return Some((Action::SeekBack30, count)),
                KeyCode::Char('L') => return Some((Action::SeekForward30, count)),
                KeyCode::Char('j') | KeyCode::Down => return Some((Action::Down, count)),
                KeyCode::Up => return Some((Action::Up, count)),
                KeyCode::Enter => {
                    if mods.contains(KeyModifiers::SHIFT) {
                        return Some((Action::PlayInline, 1));
                    } else {
                        return Some((Action::PlayDefault, 1));
                    }
                }
                KeyCode::Char('i') | KeyCode::Char('I') => return Some((Action::ToggleStats, 1)),
                KeyCode::Char('n') => return Some((Action::NextItem, 1)),
                KeyCode::Char('N') => return Some((Action::PrevItem, 1)),
                KeyCode::Char('+') | KeyCode::Char('=') => return Some((Action::LikeVideo, 1)),
                KeyCode::Char('-') => return Some((Action::DislikeVideo, 1)),
                KeyCode::Char('s') => return Some((Action::ToggleSubscribe, 1)),
                KeyCode::Char('B') => return Some((Action::ToggleSponsorBlock, 1)),
                KeyCode::Char('c') => return Some((Action::ToggleComments, 1)),
                KeyCode::Char('C') => return Some((Action::ToggleChat, 1)),
                KeyCode::Char('[') => return Some((Action::VolumeUp, 1)),
                KeyCode::Char('\'') => return Some((Action::VolumeDown, 1)),
                KeyCode::Char('{') => return Some((Action::PrevChapter, 1)),
                KeyCode::Char('}') => return Some((Action::NextChapter, 1)),
                KeyCode::Char('M') | KeyCode::Char('m') => return Some((Action::ToggleMute, 1)),
                KeyCode::Char(',') => return Some((Action::SeekBack5, count)),
                KeyCode::Char('.') => return Some((Action::SeekForward5, count)),
                KeyCode::Char('<') => return Some((Action::SpeedDown, 1)),
                KeyCode::Char('>') => return Some((Action::SpeedUp, 1)),
                KeyCode::Char('t') | KeyCode::Char('v') => {
                    return Some((Action::CycleSubtitles, 1))
                }
                KeyCode::Char('d') => return Some((Action::ToggleDescription, 1)),
                KeyCode::Char('f') => return Some((Action::ToggleFullscreen, 1)),
                KeyCode::Char('P') => return Some((Action::ToggleAutoplay, 1)),
                KeyCode::Char('S') => return Some((Action::StopPlayback, 1)),
                KeyCode::Char('q') | KeyCode::Esc => return Some((Action::Back, 1)),
                _ => {}
            }
        }

        // Standard browsing bindings
        match code {
            KeyCode::Char('h') | KeyCode::Left => Some((Action::Left, count)),
            KeyCode::Char('j') | KeyCode::Down => Some((Action::Down, count)),
            KeyCode::Char('k') | KeyCode::Up => Some((Action::Up, count)),
            KeyCode::Char('l') | KeyCode::Right => Some((Action::Right, count)),
            KeyCode::Char('g') => {
                self.prefix_char = Some('g');
                None
            }
            KeyCode::Char('G') => Some((Action::Last, 1)),
            KeyCode::Tab => {
                if mods.contains(KeyModifiers::SHIFT) {
                    Some((Action::CycleFocusBackward, 1))
                } else {
                    Some((Action::CycleFocusForward, 1))
                }
            }
            KeyCode::BackTab => Some((Action::CycleFocusBackward, 1)),
            KeyCode::Char('b') => Some((Action::ToggleSidebar, 1)),
            KeyCode::Char('[') => Some((Action::PrevChip, 1)),
            KeyCode::Char(']') => Some((Action::NextChip, 1)),
            KeyCode::Char('/') => Some((Action::FocusSearch, 1)),
            KeyCode::Char('f') => Some((Action::Filter, 1)),
            KeyCode::Char('r') => Some((Action::Refresh, 1)),
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
            KeyCode::Char('q') | KeyCode::Esc => Some((Action::Back, 1)),
            KeyCode::Char(':') => Some((Action::CommandLine, 1)),
            KeyCode::Char('?') => Some((Action::Help, 1)),
            KeyCode::Enter => {
                if mods.contains(KeyModifiers::SHIFT) {
                    Some((Action::PlayInline, 1))
                } else {
                    Some((Action::PlayDefault, 1))
                }
            }
            KeyCode::Char('w') => Some((Action::PlayWindow, 1)),
            KeyCode::Char('o') => Some((Action::OpenBrowser, 1)),
            KeyCode::Char('A') | KeyCode::Char('C') => Some((Action::NavAccounts, 1)),
            KeyCode::Char('a') => Some((Action::QueueAppend, 1)),
            KeyCode::Char('v') => Some((Action::ToggleViewMode, 1)),
            KeyCode::Char('x') => Some((Action::ToggleSelect, 1)),
            KeyCode::Char('i') => Some((Action::ToggleInfo, 1)),
            KeyCode::Char('y') => Some((Action::YankUrl, 1)),
            KeyCode::Char('m') => Some((Action::MarkWatched, 1)),
            _ => None,
        }
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
