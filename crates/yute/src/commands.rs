//! Small command registry shared by parsing, completion and command help.
use crate::app::Screen;
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Navigate(Screen),
    Search(String),
    Details,
    View(String),
    WatchSection(String),
    Comment(String),
    Set(String, String),
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
    Theme(String),
    Download(String),
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
        Err("Use w or Shift-Enter to add/remove Watch Later; :watch-later opens the library".into())
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

fn set_command(args: &str) -> Result<Command, String> {
    let Some((key, value)) = args.trim().split_once(char::is_whitespace) else {
        return Err("Usage: :set <key> <value>".into());
    };
    let key = key.trim();
    let value = value.trim();
    if value.is_empty() {
        return Err("Setting value cannot be empty".into());
    }
    match key {
        "graphics" => match value {
            "auto" | "kitty" | "iterm2" | "sixel" | "halfblocks" | "off" | "none" => {}
            _ => return Err("graphics must be auto|kitty|iterm2|sixel|halfblocks|off".into()),
        },
        "icons" => match value {
            "auto" | "nerd" | "unicode" | "ascii" | "none" => {}
            _ => return Err("icons must be auto|nerd|unicode|ascii|none".into()),
        },
        "quality" => match value {
            "480" | "480p" | "720" | "720p" | "1080" | "1080p" | "1440" | "1440p" | "2160"
            | "2160p" | "4k" | "best" => {}
            _ => return Err("quality must be 480p|720p|1080p|1440p|2160p|best".into()),
        },
        "columns" => {
            if value != "auto"
                && !value
                    .parse::<u16>()
                    .is_ok_and(|columns| (1..=8).contains(&columns))
            {
                return Err("columns must be auto or 1..8".into());
            }
        }
        "min-card-width" => match value.parse::<u16>() {
            Ok(width) if (28..=40).contains(&width) => {}
            _ => return Err("min-card-width must be 28..40".into()),
        },
        "scrolloff" => match value.parse::<u16>() {
            Ok(rows) if rows <= 12 => {}
            _ => return Err("scrolloff must be 0..12".into()),
        },
        "cache-seconds" => match value {
            "5" | "10" | "20" | "40" | "60" => {}
            _ => return Err("cache-seconds must be 5|10|20|40|60".into()),
        },
        "thumbnail-cache" => match value.parse::<u64>() {
            Ok(mb) if (32..=2048).contains(&mb) => {}
            _ => return Err("thumbnail-cache must be 32..2048 MiB".into()),
        },
        "prefetch-pages" => match value.parse::<usize>() {
            Ok(pages) if pages <= 4 => {}
            _ => return Err("prefetch-pages must be 0..4".into()),
        },
        "codec" => match value {
            "auto" | "av1" | "vp9" | "h264" => {}
            _ => return Err("codec must be auto|av1|vp9|h264".into()),
        },
        "audio-codec" => match value {
            "auto" | "opus" | "aac" => {}
            _ => return Err("audio-codec must be auto|opus|aac".into()),
        },
        "hwdec" => match value {
            "auto-safe" | "auto" | "no" | "nvdec" | "vaapi" | "vulkan" => {}
            _ => return Err("hwdec must be auto-safe|auto|no|nvdec|vaapi|vulkan".into()),
        },
        "action-bar" => match value {
            "auto" | "off" => {}
            _ => return Err("action-bar must be auto|off".into()),
        },
        "emoji" => match value {
            "auto" | "keep" | "strip" => {}
            _ => return Err("emoji must be auto|keep|strip".into()),
        },
        "sidebar-channels" | "start-anonymous" | "autoplay" | "history-sync" | "osd-bar"
        | "user-mpv-config" | "quit-home-only" => match value {
            "on" | "off" | "true" | "false" | "yes" | "no" => {}
            _ => return Err(format!("{key} must be on|off")),
        },
        "language" | "region" | "audio-language" | "subtitle-language" => {
            if value.len() > 32 || value.contains(char::is_whitespace) {
                return Err(format!("{key} must be a short language/region tag"));
            }
        }
        "theme" | "download-dir" | "sidebar-key" | "sidebar-alt-key" | "copy-key"
        | "center-key" | "quit-key" => {}
        _ => return Err("Unknown setting key — :help set".into()),
    }
    Ok(Command::Set(key.into(), value.into()))
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
    Spec { name:"settings", aliases:&["config"], usage:"settings", description:"Open settings.", parse:|a| no_args(a,Command::Navigate(Screen::Settings)) },
    Spec { name:"account", aliases:&["accounts", "acc", "a"], usage:"account [name|index|add|remove <name>|anon|whoami]", description:"Switch accounts, add a TV login, remove a saved account, or inspect the active session.", parse:|a| Ok(Command::Account(a.into())) },
    Spec { name:"anonymous", aliases:&[], usage:"anonymous", description:"Switch to the built-in anonymous account.", parse:|a|no_args(a,Command::Account("anon".into())) },
    Spec { name:"search", aliases:&[], usage:"search <query>", description:"Search YouTube. Example: :search linux kernel", parse:|a| if a.is_empty(){Err("Usage: :search <query>".into())}else{Ok(Command::Search(a.into()))} },
    Spec { name:"details", aliases:&["now","watch","video-info"], usage:"details", description:"Open the dashboard for the currently playing video with description, comments and suggestions.", parse:|a| no_args(a,Command::Details) },
    Spec { name:"view", aliases:&[], usage:"view [toggle|grid|list]", description:"Switch the current feed between grid and list immediately.", parse:|a| match a {""|"toggle"|"grid"|"list"=>Ok(Command::View(if a.is_empty(){"toggle".into()}else{a.into()})),_=>Err("Usage: :view [toggle|grid|list]".into())} },
    Spec { name:"grid", aliases:&[], usage:"grid", description:"Use grid view for the current feed.", parse:|a| no_args(a,Command::View("grid".into())) },
    Spec { name:"list", aliases:&[], usage:"list", description:"Use list view for the current feed.", parse:|a| no_args(a,Command::View("list".into())) },
    Spec { name:"comments", aliases:&[], usage:"comments", description:"Open/focus comments for the currently playing video.", parse:|a| no_args(a,Command::WatchSection("comments".into())) },
    Spec { name:"description", aliases:&["desc"], usage:"description", description:"Open/focus the current video's description and chapters.", parse:|a| no_args(a,Command::WatchSection("description".into())) },
    Spec { name:"suggestions", aliases:&["up-next"], usage:"suggestions", description:"Open/focus suggestions for the currently playing video.", parse:|a| no_args(a,Command::WatchSection("suggestions".into())) },
    Spec { name:"comment", aliases:&[], usage:"comment new|reply|delete|like|dislike", description:"Create, reply to, delete, like or dislike comments in the current Watch comments view.", parse:|a| match a {"new"|"reply"|"delete"|"like"|"dislike"=>Ok(Command::Comment(a.into())),_=>Err("Usage: :comment new|reply|delete|like|dislike".into())} },
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
    Spec { name:"download", aliases:&[], usage:"download [playlist] [video|audio] [best|1080p|720p|480p|360p|opus|aac|mp3|flac|m4a]", description:"Download selected media; playlist video qualities are ceilings.", parse:|a|{if a.split_whitespace().all(|s|matches!(s,"playlist"|"video"|"audio"|"best"|"2160p"|"1440p"|"1080p"|"720p"|"480p"|"360p"|"opus"|"aac"|"mp3"|"flac"|"m4a")){Ok(Command::Download(a.into()))}else{Err("Invalid download mode or quality".into())}} },
    Spec { name:"theme", aliases:&[], usage:"theme terminal|auto|<name>|reload|list|check", description:"Select, reload or validate a semantic theme.", parse:|a| Ok(Command::Theme(if a.is_empty(){"list".into()}else{a.into()})) },
    Spec { name:"graphics", aliases:&[], usage:"graphics auto|kitty|iterm2|sixel|halfblocks|off", description:"Apply terminal graphics mode.", parse:|a| match a {"auto"|"kitty"|"iterm2"|"sixel"|"halfblocks"|"off"|"none"=>Ok(Command::Graphics(a.into())),_=>Err("Usage: :graphics auto|kitty|iterm2|sixel|halfblocks|off".into())} },
    Spec { name:"set", aliases:&[], usage:"set <key> <value>", description:"Change persistent UI/content/playback settings directly. Tab completes supported keys and common values.", parse:set_command },
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
        ("comment", &["new", "reply", "delete", "like", "dislike"]),
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
        (
            "set",
            &[
                "graphics",
                "theme",
                "icons",
                "quality",
                "columns",
                "min-card-width",
                "scrolloff",
                "action-bar",
                "sidebar-channels",
                "start-anonymous",
                "autoplay",
                "history-sync",
                "osd-bar",
                "user-mpv-config",
                "cache-seconds",
                "codec",
                "audio-codec",
                "hwdec",
                "audio-language",
                "subtitle-language",
                "language",
                "region",
                "emoji",
                "thumbnail-cache",
                "prefetch-pages",
                "quit-home-only",
                "download-dir",
                "sidebar-key",
                "sidebar-alt-key",
                "copy-key",
                "center-key",
                "quit-key",
            ],
        ),
        (
            "set graphics",
            &["sixel", "kitty", "iterm2", "halfblocks", "none", "auto"],
        ),
        (
            "set quality",
            &["480p", "720p", "1080p", "1440p", "2160p", "best"],
        ),
        ("set icons", &["auto", "nerd", "unicode", "ascii", "none"]),
        (
            "set columns",
            &["auto", "1", "2", "3", "4", "5", "6", "7", "8"],
        ),
        ("set action-bar", &["auto", "off"]),
        ("set codec", &["auto", "av1", "vp9", "h264"]),
        ("set audio-codec", &["auto", "opus", "aac"]),
        (
            "set hwdec",
            &["auto-safe", "auto", "no", "nvdec", "vaapi", "vulkan"],
        ),
        ("set cache-seconds", &["5", "10", "20", "40", "60"]),
        ("set emoji", &["auto", "keep", "strip"]),
        ("set sidebar-channels", &["on", "off"]),
        ("set start-anonymous", &["on", "off"]),
        ("set autoplay", &["on", "off"]),
        ("set history-sync", &["on", "off"]),
        ("set osd-bar", &["on", "off"]),
        ("set user-mpv-config", &["on", "off"]),
        ("set quit-home-only", &["on", "off"]),
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
                let same_initial = input
                    .chars()
                    .next()
                    .zip(name.chars().next())
                    .is_some_and(|(input, candidate)| input == candidate);
                let mut chars = name.chars();
                if same_initial && input.chars().all(|ch| chars.by_ref().any(|c| c == ch)) {
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
        Ok("Commands\n:help <name> shows syntax and examples.\nNavigate: home subscriptions history watch-later liked playlists queue local settings account\nVideo: details comments description suggestions play pause toggle stop next prev seek volume mute quality\nBrowse: search filter view grid list channel playlist shorts sidebar\nAccount: login logout subscribe unsubscribe like unlike\nTools: download open copy graphics theme set refresh keys quit\nNormal mode: v toggles grid/list, yy copies URL, zz centers, Space-e or Alt-e sidebar. Tab completes commands.".into())
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
