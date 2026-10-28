//! Semantic icons. Nerd mappings verified against the cmap glyph names in the
//! locally installed IosevkaNerdFont-Regular.ttf (Nerd Fonts Material Design).
//! Auto uses portable Unicode unless YUTE_NERD_FONT=1 explicitly opts in.
#[derive(Clone, Copy, Debug)]
pub enum Icon {
    Home,
    Subscriptions,
    History,
    WatchLater,
    Like,
    Dislike,
    Playlists,
    Queue,
    Local,
    Account,
    Settings,
    Play,
    Audio,
    Download,
    Save,
    Search,
    Channel,
    Browser,
    Copy,
}
impl Icon {
    pub fn glyph(self, mode: &str) -> &'static str {
        let mode = if mode == "auto" {
            if std::env::var("YUTE_NERD_FONT").as_deref() == Ok("1") {
                "nerd"
            } else {
                "unicode"
            }
        } else {
            mode
        };
        match mode {
            "none" => "",
            "nerd" => match self {
                Self::Home => "\u{f02dc}",          // md-home
                Self::Subscriptions => "\u{f0d40}", // md-youtube_subscription
                Self::History => "\u{f02da}",       // md-history
                Self::WatchLater => "\u{f0150}",    // md-clock_outline
                Self::Like => "\u{f0513}",          // md-thumb_up
                Self::Dislike => "\u{f0511}",       // md-thumb_down
                Self::Playlists => "\u{f0411}",     // md-playlist_play
                Self::Queue => "\u{f0412}",         // md-playlist_plus
                Self::Local => "\u{f024b}",         // md-folder
                Self::Account => "\u{f0004}",       // md-account
                Self::Settings => "\u{f0493}",      // md-cog
                Self::Play => "\u{f040a}",          // md-play
                Self::Audio => "\u{f02cb}",         // md-headphones
                Self::Save => "\u{f0193}",          // md-content_save
                Self::Search => "\u{f0349}",        // md-magnify
                Self::Channel => "\u{f0006}",       // md-account_box
                Self::Browser => "\u{f03cc}",       // md-open_in_new
                Self::Copy => "\u{f018f}",          // md-content_copy
                Self::Download => "\u{f01da}",      // md-download
            },
            "unicode" => match self {
                Self::Home => "⌂",
                Self::Subscriptions => "▤",
                Self::History => "↶",
                Self::WatchLater => "◷",
                Self::Like => "+",
                Self::Dislike => "−",
                Self::Playlists => "≡",
                Self::Queue => "≣",
                Self::Local => "▱",
                Self::Account => "♙",
                Self::Settings => "⚙",
                Self::Play => "▶",
                Self::Audio => "♫",
                Self::Save => "+",
                Self::Search => "⌕",
                Self::Channel => "▣",
                Self::Browser => "↗",
                Self::Copy => "⧉",
                Self::Download => "↓",
            },
            _ => match self {
                Self::Home => "H",
                Self::Subscriptions => "S",
                Self::History => "Y",
                Self::WatchLater => "W",
                Self::Like => "+",
                Self::Dislike => "-",
                Self::Playlists => "P",
                Self::Queue => "Q",
                Self::Local => "L",
                Self::Account => "A",
                Self::Settings => "*",
                Self::Play => ">",
                Self::Audio => "~",
                Self::Save => "+",
                Self::Search => "/",
                Self::Channel => "C",
                Self::Browser => "O",
                Self::Copy => "Y",
                Self::Download => "v",
            },
        }
    }
    pub fn label(self, mode: &str, text: &str) -> String {
        let glyph = self.glyph(mode);
        if glyph.is_empty() {
            text.into()
        } else {
            format!("{glyph} {text}")
        }
    }
}
