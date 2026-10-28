//! Small command registry shared by parsing, completion and command help.
use crate::app::Screen;
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Navigate(Screen),
    Search(String),
    Channel,
    Playlist,
    Play,
    Pause,
    Toggle,
    Stop,
    Next,
    Prev,
    Seek(f64, bool),
    Volume(f64, bool),
    Mute,
    Quality(Option<String>),
    PlaySelect,
    Audio,
    QueueAdd(bool),
    QueueRemove,
    QueueClear,
    Login,
    Logout,
    Account(String),
    Subscribe(bool),
    Like(bool),
    Open(bool),
    Copy(CopyTarget),
    Refresh,
    Filter(String),
    Shorts(Option<bool>),
    Keys,
    Graphics(String),
    Sidebar(Option<bool>),
    Help(Option<String>),
    Quit(bool),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CopyTarget {
    Url,
    Title,
    Channel,
}

pub struct Spec {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub usage: &'static str,
    pub description: &'static str,
    parse: fn(&str) -> Result<Command, String>,
}
fn no_args(args: &str, command: Command) -> Result<Command, String> {
    if args.is_empty() {
        Ok(command)
    } else {
        Err("Unexpected argument; use :help <command>".into())
    }
}
fn library(args: &str, screen: Screen) -> Result<Command, String> {
    if args.is_empty() {
        Ok(Command::Navigate(screen))
    } else {
        Err(
            "YouTube Watch Later mutations are not implemented; local :queue add is separate"
                .into(),
        )
    }
}
fn finite_number(text: &str) -> Result<f64, String> {
    text.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && n.abs() <= 100_000_000.0)
        .ok_or_else(|| "Expected a finite number".into())
}
fn seek(args: &str) -> Result<Command, String> {
    let relative = args.starts_with('+') || args.starts_with('-');
    if args.is_empty() {
        return Err("Usage: :seek +30, :seek -10 or :seek 1:20".into());
    }
    let value = if args.contains(':') {
        if relative {
            return Err("Use seconds for relative seeks".into());
        }
        let parts = args.split(':').collect::<Vec<_>>();
        if !(2..=3).contains(&parts.len()) {
            return Err("Expected minutes:seconds or hours:minutes:seconds".into());
        }
        let mut seconds = 0.0;
        for (i, part) in parts.iter().enumerate() {
            let n = finite_number(part)?;
            if n < 0.0 || (i > 0 && n >= 60.0) {
                return Err("Invalid time component".into());
            }
            seconds = seconds * 60.0 + n;
        }
        if seconds > 100_000_000.0 {
            return Err("Seek position is too large".into());
        }
        seconds
    } else {
        finite_number(args)?
    };
    if !relative && value < 0.0 {
        return Err("Absolute position cannot be negative".into());
    }
    Ok(Command::Seek(value, relative))
}
fn volume(args: &str) -> Result<Command, String> {
    let relative = args.starts_with('+') || args.starts_with('-');
    let value = finite_number(args)?;
    if value.abs() > 100.0 || (!relative && value < 0.0) {
        return Err("Volume must be 0–100, or a relative change within ±100".into());
    }
    Ok(Command::Volume(value, relative))
}
fn queue(args: &str) -> Result<Command, String> {
    match args {
        "" => Ok(Command::Navigate(Screen::Queue)),
        "add" => Ok(Command::QueueAdd(false)),
        "next" => Ok(Command::QueueAdd(true)),
        "remove" => Ok(Command::QueueRemove),
        "clear" => Ok(Command::QueueClear),
        _ => Err("Usage: :queue [add|next|remove|clear]".into()),
    }
}

pub static REGISTRY: &[Spec] = &[
    Spec { name:"play-select", aliases:&[], usage:"play-select", description:"Choose available video or audio streams for the selected video.", parse:|a| if a.is_empty() {Ok(Command::PlaySelect)} else {Err("Usage: :play-select".into())} },
    Spec { name:"audio", aliases:&[], usage:"audio", description:"Choose original audio streams for this playback only.", parse:|a| if a.is_empty() {Ok(Command::Audio)} else {Err("Usage: :audio".into())} },
    Spec { name:"quality", aliases:&[], usage:"quality [480p|720p|1080p|1440p|2160p|best]", description:"Show quality or reload the active stream at its current position with a new limit.", parse:|a| match a { "" => Ok(Command::Quality(None)), "480"|"480p"|"720"|"720p"|"1080"|"1080p"|"1440"|"1440p"|"2160"|"2160p"|"4k"|"best" => Ok(Command::Quality(Some(if a=="4k" {"2160p".into()} else if a.chars().all(|c|c.is_ascii_digit()) {format!("{a}p")} else {a.into()}))), _=> Err("Usage: :quality 480p|720p|1080p|1440p|2160p|best".into()) } },
    Spec { name:"home", aliases:&[], usage:"home", description:"Open Home.", parse:|a| no_args(a,Command::Navigate(Screen::Home)) },
    Spec { name:"subscriptions", aliases:&["subs"], usage:"subscriptions", description:"Open the subscriptions feed.", parse:|a| no_args(a,Command::Navigate(Screen::Subscriptions)) },
    Spec { name:"history", aliases:&[], usage:"history", description:"Open account watch history.", parse:|a| no_args(a,Command::Navigate(Screen::History)) },
    Spec { name:"watch-later", aliases:&["wl"], usage:"watch-later", description:"Open YouTube Watch Later. Mutations are unavailable.", parse:|a| library(a,Screen::WatchLater) },
    Spec { name:"liked", aliases:&[], usage:"liked", description:"Open liked videos.", parse:|a| no_args(a,Command::Navigate(Screen::Liked)) },
    Spec { name:"playlists", aliases:&[], usage:"playlists", description:"Open your library playlists.", parse:|a| no_args(a,Command::Navigate(Screen::Playlists)) },
    Spec { name:"queue", aliases:&[], usage:"queue [add|next|remove|clear]", description:"Open the local queue; add selected video, prepend it, remove selected queue entry, or clear. Example: :queue add", parse:queue },
    Spec { name:"local", aliases:&[], usage:"local", description:"Browse configured local videos.", parse:|a| no_args(a,Command::Navigate(Screen::LocalVideos)) },
    Spec { name:"settings", aliases:&[], usage:"settings", description:"Open settings.", parse:|a| no_args(a,Command::Navigate(Screen::Settings)) },
    Spec { name:"account", aliases:&["accounts", "acc", "a"], usage:"account [name|index|add|remove <name>|anon|whoami]", description:"Switch accounts, add a TV login, remove a saved account, or inspect the active session.", parse:|a| Ok(Command::Account(a.into())) },
    Spec { name:"anonymous", aliases:&[], usage:"anonymous", description:"Switch to the built-in anonymous account.", parse:|a|no_args(a,Command::Account("anon".into())) },
    Spec { name:"search", aliases:&[], usage:"search <query>", description:"Search YouTube. Example: :search linux kernel", parse:|a| if a.is_empty(){Err("Usage: :search <query>".into())}else{Ok(Command::Search(a.into()))} },
    Spec { name:"channel", aliases:&[], usage:"channel", description:"Open the selected item's channel when its channel ID is available.", parse:|a| no_args(a,Command::Channel) },
    Spec { name:"playlist", aliases:&[], usage:"playlist", description:"Open the selected playlist, without autoplay.", parse:|a| no_args(a,Command::Playlist) },
    Spec { name:"play", aliases:&[], usage:"play", description:"Activate the selected item; resume current playback if no item is selected.", parse:|a| no_args(a,Command::Play) },
    Spec { name:"pause", aliases:&[], usage:"pause", description:"Pause yute-managed MPV.", parse:|a| no_args(a,Command::Pause) },
    Spec { name:"toggle", aliases:&[], usage:"toggle", description:"Toggle pause in yute-managed MPV.", parse:|a| no_args(a,Command::Toggle) },
    Spec { name:"stop", aliases:&[], usage:"stop", description:"Stop playback.", parse:|a| no_args(a,Command::Stop) },
    Spec { name:"next", aliases:&[], usage:"next", description:"Play and consume the next queued video.", parse:|a| no_args(a,Command::Next) },
    Spec { name:"prev", aliases:&[], usage:"prev", description:"Return to the previously played video in this session.", parse:|a| no_args(a,Command::Prev) },
    Spec { name:"seek", aliases:&[], usage:"seek <seconds|+seconds|-seconds|m:ss|h:mm:ss>", description:"Seek in MPV. Example: :seek 1:20", parse:seek },
    Spec { name:"volume", aliases:&["vol"], usage:"volume <0..100|+change|-change>", description:"Set or adjust MPV volume. Example: :volume +5", parse:volume },
    Spec { name:"mute", aliases:&[], usage:"mute", description:"Toggle MPV mute.", parse:|a| no_args(a,Command::Mute) },
    Spec { name:"login", aliases:&["auth"], usage:"login", description:"Start YouTube TV device-code login.", parse:|a| no_args(a,Command::Login) },
    Spec { name:"logout", aliases:&[], usage:"logout [name|index|prefix]", description:"Confirm removal of the selected saved account, or the active account when omitted.", parse:|a| if a.trim().is_empty() { Ok(Command::Logout) } else { Ok(Command::Account(format!("remove {}", a.trim()))) } },
    Spec { name:"subscribe", aliases:&[], usage:"subscribe", description:"Subscribe to the selected channel using the authenticated backend.", parse:|a| no_args(a,Command::Subscribe(true)) },
    Spec { name:"unsubscribe", aliases:&[], usage:"unsubscribe", description:"Unsubscribe from the selected channel using the authenticated backend.", parse:|a| no_args(a,Command::Subscribe(false)) },
    Spec { name:"like", aliases:&[], usage:"like", description:"Like the selected video using the authenticated backend.", parse:|a| no_args(a,Command::Like(true)) },
    Spec { name:"unlike", aliases:&[], usage:"unlike", description:"Remove the selected video's rating using the authenticated backend.", parse:|a| no_args(a,Command::Like(false)) },
    Spec { name:"open", aliases:&[], usage:"open [video|channel]", description:"Open the selected entity's URL in the system browser.", parse:|a| match a {""|"video"=>Ok(Command::Open(false)),"channel"=>Ok(Command::Open(true)),_=>Err("Usage: :open [video|channel]".into())} },
    Spec { name:"copy", aliases:&[], usage:"copy [url|title|channel]", description:"Copy selected metadata using the existing clipboard tools.", parse:|a| match a {""|"url"=>Ok(Command::Copy(CopyTarget::Url)),"title"=>Ok(Command::Copy(CopyTarget::Title)),"channel"=>Ok(Command::Copy(CopyTarget::Channel)),_=>Err("Usage: :copy [url|title|channel]".into())} },
    Spec { name:"refresh", aliases:&[], usage:"refresh [hard]", description:"Refresh the current route while keeping existing content.", parse:|a| match a {""|"hard"=>Ok(Command::Refresh),_=>Err("Usage: :refresh [hard]".into())} },
    Spec { name:"goto", aliases:&[], usage:"goto <route>", description:"Navigate to a named route.", parse:|a| {
        if a == "goto" || a.is_empty() { return Err("Usage: :goto home|subscriptions|history|watch-later|liked|playlists|queue|local|settings|account".into()); }
        match spec(a).map(|s| (s.parse)("")) { Some(Ok(command @ Command::Navigate(_))) => Ok(command), _ => Err("Unknown route; use :help goto".into()) }
    } },
    Spec { name:"keys", aliases:&[], usage:"keys", description:"Show keyboard bindings.", parse:|a| no_args(a, Command::Keys) },
    Spec { name:"shorts", aliases:&[], usage:"shorts [on|off|toggle]", description:"Show or hide Shorts in browsing feeds without refetching.", parse:|a| match a {"on"=>Ok(Command::Shorts(Some(true))),"off"=>Ok(Command::Shorts(Some(false))),""|"toggle"=>Ok(Command::Shorts(None)),_=>Err("Usage: :shorts on|off|toggle".into())} },
    Spec { name:"filter", aliases:&[], usage:"filter [all|videos|shorts|live|playlists|channels|query]", description:"Choose a content type, or filter loaded titles locally.", parse:|a| Ok(Command::Filter(a.into())) },
    Spec { name:"set", aliases:&[], usage:"set graphics <sixel|kitty|halfblocks|none|auto>", description:"Save a graphics preference for the next launch.", parse:|a| {let parts=a.split_whitespace().collect::<Vec<_>>(); match parts.as_slice(){["graphics", mode @ ("sixel"|"kitty"|"halfblocks"|"none"|"auto")]=>Ok(Command::Graphics((*mode).into())),_=>Err("Usage: :set graphics <sixel|kitty|halfblocks|none|auto>".into())}} },
    Spec { name:"sidebar", aliases:&[], usage:"sidebar [show|hide]", description:"Toggle or set sidebar visibility.", parse:|a| match a {""|"toggle"=>Ok(Command::Sidebar(None)),"show"=>Ok(Command::Sidebar(Some(true))),"hide"=>Ok(Command::Sidebar(Some(false))),_=>Err("Usage: :sidebar [show|hide]".into())} },
    Spec { name:"help", aliases:&[], usage:"help [command]", description:"Show command help; ? shows keyboard shortcuts.", parse:|a| if a.split_whitespace().count()>1{Err("Usage: :help [command]".into())}else{Ok(Command::Help((!a.is_empty()).then(||a.into())))} },
    Spec { name:"quit", aliases:&["q"], usage:"quit", description:"Confirm application quit. Normal-mode q goes back.", parse:|a| no_args(a,Command::Quit(false)) },
    Spec { name:"quit!", aliases:&["q!"], usage:"quit!", description:"Quit immediately.", parse:|a| no_args(a,Command::Quit(true)) },
];

pub fn spec(name: &str) -> Option<&'static Spec> {
    REGISTRY
        .iter()
        .find(|s| s.name == name || s.aliases.contains(&name))
}
pub fn parse(input: &str) -> Result<Command, String> {
    let input = input
        .trim()
        .strip_prefix(':')
        .unwrap_or(input.trim())
        .trim();
    let (name, args) = input.split_once(char::is_whitespace).unwrap_or((input, ""));
    let spec = spec(name).ok_or_else(|| {
        if name.is_empty() {
            "Enter a command; :help lists commands".into()
        } else {
            format!("Unknown command '{name}' — use :help")
        }
    })?;
    (spec.parse)(args.trim())
}
pub fn completions(input: &str) -> Vec<String> {
    completions_with_accounts(input, &[])
}

pub fn completions_with_accounts(input: &str, accounts: &[String]) -> Vec<String> {
    let input = input.trim_start();
    let subcommands: &[(&str, &[&str])] = &[
        ("account", &["add", "remove", "anon", "whoami"]),
        ("queue", &["add", "next", "remove", "clear"]),
        (
            "filter",
            &["all", "videos", "shorts", "live", "playlists", "channels"],
        ),
        (
            "quality",
            &["480p", "720p", "1080p", "1440p", "2160p", "best"],
        ),
        ("sidebar", &["show", "hide", "toggle"]),
        ("shorts", &["on", "off", "toggle"]),
        ("set", &["graphics"]),
        (
            "set graphics",
            &["sixel", "kitty", "halfblocks", "none", "auto"],
        ),
        ("copy", &["url", "title", "channel"]),
        ("open", &["video", "channel"]),
    ];
    let (head, tail) = input.split_once(' ').unwrap_or((input, ""));
    if matches!(head, "account" | "accounts" | "acc" | "a") && input.contains(' ') {
        return ["add", "remove", "anon", "whoami"]
            .iter()
            .map(|s| s.to_string())
            .chain(accounts.iter().cloned())
            .filter(|s| s.starts_with(tail))
            .map(|s| format!("{head} {s}"))
            .collect();
    }
    let candidates: Vec<String> = if input.starts_with("goto ") {
        REGISTRY
            .iter()
            .filter(|s| s.name != "goto")
            .filter(|s| matches!((s.parse)(""), Ok(Command::Navigate(_))))
            .map(|s| format!("goto {}", s.name))
            .collect()
    } else if let Some((prefix, values)) = subcommands
        .iter()
        .rev()
        .find(|(prefix, _)| input.starts_with(&format!("{prefix} ")))
    {
        values.iter().map(|v| format!("{prefix} {v}")).collect()
    } else {
        REGISTRY.iter().map(|s| s.name.to_string()).collect()
    };
    let mut matches: Vec<_> = candidates
        .into_iter()
        .filter_map(|name| {
            if name.starts_with(input) {
                return Some((0, name));
            }
            if !input.contains(' ') {
                if spec(input).is_some_and(|s| s.name == name) {
                    return Some((0, name));
                }
                let mut chars = name.chars();
                if input.chars().all(|ch| chars.by_ref().any(|c| c == ch)) {
                    return Some((1, name));
                }
            }
            None
        })
        .collect();
    matches.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.len().cmp(&b.1.len()))
            .then_with(|| a.1.cmp(&b.1))
    });
    matches.into_iter().map(|(_, name)| name).collect()
}
pub fn help(topic: Option<&str>) -> Result<String, String> {
    if let Some(topic) = topic {
        let s = spec(topic).ok_or_else(|| format!("No command named '{topic}'"))?;
        Ok(format!(":{}\n{}", s.usage, s.description))
    } else {
        Ok("Commands\n:help <name> shows syntax and an example.\nNavigate: home subscriptions history watch-later liked playlists queue local settings account\nUse: search channel playlist play pause toggle stop next prev seek volume mute\nMore: login logout subscribe unsubscribe like unlike open copy refresh filter set sidebar quit\nTab completes names; Up/Down recalls session commands. Esc closes.".into())
    }
}

#[derive(Default)]
pub struct History {
    entries: VecDeque<String>,
    cursor: Option<usize>,
    draft: String,
}
impl History {
    pub fn record(&mut self, input: &str) {
        let input = input.trim();
        if !input.is_empty() && self.entries.back().is_none_or(|s| s != input) {
            if self.entries.len() == 100 {
                self.entries.pop_front();
            }
            self.entries.push_back(input.into());
        }
        self.reset();
    }
    pub fn reset(&mut self) {
        self.cursor = None;
        self.draft.clear();
    }
    pub fn previous(&mut self, current: &str) -> String {
        if self.entries.is_empty() {
            return current.into();
        }
        let i = if let Some(i) = self.cursor {
            i.saturating_sub(1)
        } else {
            self.draft = current.into();
            self.entries.len() - 1
        };
        self.cursor = Some(i);
        self.entries[i].clone()
    }
    pub fn next_entry(&mut self) -> String {
        if let Some(i) = self.cursor {
            if i + 1 < self.entries.len() {
                self.cursor = Some(i + 1);
                return self.entries[i + 1].clone();
            }
        }
        self.cursor = None;
        self.draft.clone()
    }
}
