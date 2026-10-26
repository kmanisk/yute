//! Typed content activation. Neither titles nor renderers choose playback.
use crate::backend::models::{Card, ChannelCard, LiveState, PlaylistCard, VideoCard};

#[derive(Clone, Debug)]
pub enum OpenAction {
    Play(VideoCard),
    OpenChannel(ChannelCard),
    OpenPlaylist(PlaylistCard),
    PlayMix(PlaylistCard),
    Unavailable(&'static str),
}

pub fn activation(card: &Card) -> OpenAction {
    match card {
        Card::Video(video) if video.live == LiveState::Upcoming => {
            OpenAction::Unavailable("This stream is upcoming and cannot be played yet")
        }
        // Reject malformed legacy cache entries rather than passing browse IDs to MPV.
        Card::Video(video) if video.id.starts_with("UC") && video.id.len() != 11 => {
            OpenAction::Unavailable("Cached item has an invalid video identity — refresh this feed")
        }
        Card::Video(video) if video.id.is_empty() => {
            OpenAction::Unavailable("Video identity unavailable — refresh this feed")
        }
        Card::Video(video) => OpenAction::Play(video.clone()),
        Card::Channel(channel) => OpenAction::OpenChannel(channel.clone()),
        Card::Playlist(playlist) if playlist.is_mix => OpenAction::PlayMix(playlist.clone()),
        Card::Playlist(playlist) => OpenAction::OpenPlaylist(playlist.clone()),
    }
}

pub fn channel_of(card: &Card) -> Option<ChannelCard> {
    match card {
        Card::Channel(channel) => Some(channel.clone()),
        Card::Video(video) => video.channel_id.as_ref().map(|id| ChannelCard {
            id: id.clone(),
            title: video.channel.clone(),
            handle: None,
            subscribers: None,
            avatar_url: None,
            verified: video.verified,
        }),
        Card::Playlist(_) => None,
    }
}

pub fn url(card: &Card) -> String {
    match card {
        Card::Channel(c) => format!("https://www.youtube.com/channel/{}", c.id),
        Card::Playlist(p) => format!(
            "https://www.youtube.com/playlist?list={}",
            p.id.trim_start_matches("VL")
        ),
        Card::Video(v) if v.id.starts_with('/') || v.id.starts_with('.') => v.id.clone(),
        Card::Video(v) => format!("https://www.youtube.com/watch?v={}", v.id),
    }
}

#[derive(Clone, Debug)]
pub enum AccountMutation {
    Dislike {
        video_id: String,
    },
    Subscribe {
        channel_id: String,
        subscribed: bool,
    },
    Like {
        video_id: String,
        liked: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemAction {
    Play,
    Browse,
    Browser,
    CopyUrl,
    CopyTitle,
    Queue,
    Save,
    Download,
    CancelDownload,
    Channel,
}
#[derive(Clone, Debug)]
pub struct MenuEntry {
    pub label: &'static str,
    pub key: char,
    pub action: ItemAction,
    pub unavailable: Option<&'static str>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuStage {
    Actions,
    LoadingPlaylists,
    Playlists,
}
pub struct ActionMenu {
    pub request: u64,
    pub card: Card,
    pub cursor: usize,
    pub stage: MenuStage,
    pub playlists: Vec<PlaylistCard>,
    pub message: String,
}
pub fn menu_entries(card: &Card, authenticated: bool, downloading: bool) -> Vec<MenuEntry> {
    use ItemAction::*;
    let local = matches!(card,Card::Video(v) if v.id.starts_with('/') || v.id.starts_with('.'));
    let mut entries = vec![];
    if matches!(card, Card::Channel(_)) {
        entries.push(MenuEntry {
            label: "Open channel",
            key: 'p',
            action: Browse,
            unavailable: None,
        });
    } else {
        entries.push(MenuEntry {
            label: if matches!(card, Card::Playlist(_)) {
                "Play playlist / Mix"
            } else {
                "Play in MPV"
            },
            key: 'p',
            action: Play,
            unavailable: if matches!(activation(card), OpenAction::Unavailable(_)) {
                Some("This item cannot be played yet")
            } else {
                None
            },
        });
        if matches!(card, Card::Playlist(_)) {
            entries.push(MenuEntry {
                label: "Browse playlist",
                key: 'v',
                action: Browse,
                unavailable: None,
            });
        }
    }
    entries.extend([
        MenuEntry {
            label: "Open in browser",
            key: 'o',
            action: Browser,
            unavailable: if local {
                Some("Local file has no YouTube URL")
            } else {
                None
            },
        },
        MenuEntry {
            label: "Copy link",
            key: 'y',
            action: CopyUrl,
            unavailable: None,
        },
        MenuEntry {
            label: "Copy title",
            key: 't',
            action: CopyTitle,
            unavailable: None,
        },
    ]);
    if matches!(card, Card::Video(_)) {
        entries.push(MenuEntry {
            label: "Add to queue",
            key: 'a',
            action: Queue,
            unavailable: if matches!(activation(card), OpenAction::Unavailable(_)) {
                Some("This item is not playable yet")
            } else {
                None
            },
        });
        entries.push(MenuEntry {
            label: "Save to playlist…",
            key: 's',
            action: Save,
            unavailable: if local {
                Some("Local files cannot be saved to YouTube playlists")
            } else if !authenticated {
                Some("Sign in with :login to save to a playlist")
            } else {
                None
            },
        });
    }
    if !matches!(card, Card::Channel(_)) {
        entries.push(MenuEntry {
            label: "Download locally",
            key: 'd',
            action: Download,
            unavailable: if matches!(activation(card), OpenAction::Unavailable(_)) {
                Some("This item is not playable yet")
            } else if local {
                Some("This file is already local")
            } else if downloading {
                Some("A download is already running")
            } else {
                None
            },
        });
    }
    if !matches!(card, Card::Channel(_)) && channel_of(card).is_some() {
        entries.push(MenuEntry {
            label: "Open channel",
            key: 'c',
            action: Channel,
            unavailable: None,
        });
    }
    if downloading {
        entries.push(MenuEntry {
            label: "Cancel current download",
            key: 'x',
            action: CancelDownload,
            unavailable: None,
        });
    }
    entries
}
pub fn menu_rect(
    app: &crate::app::App,
    area: ratatui::layout::Rect,
) -> Option<ratatui::layout::Rect> {
    app.action_menu.as_ref()?;
    let w = 58.min(area.width.saturating_sub(4));
    let h = 16.min(area.height.saturating_sub(2));
    Some(ratatui::layout::Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    ))
}
pub fn render_menu(f: &mut ratatui::Frame, app: &crate::app::App) {
    use ratatui::{
        style::{Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Clear, Paragraph},
    };
    let Some(menu) = &app.action_menu else {
        return;
    };
    let Some(rect) = menu_rect(app, f.area()) else {
        return;
    };
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(if menu.stage == MenuStage::Actions {
            " Actions "
        } else {
            " Save to playlist "
        })
        .border_style(Style::default().fg(app.theme.accent))
        .style(Style::default().bg(app.theme.surface));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let auth = app.is_authenticated();
    let entries = menu_entries(&menu.card, auth, app.download_active);
    let mut lines = vec![
        Line::from(Span::styled(
            crate::ui::text::truncate_str(menu.card.title(), inner.width as usize),
            Style::default()
                .fg(app.theme.text)
                .add_modifier(Modifier::BOLD),
        )),
        Line::default(),
    ];
    if menu.stage == MenuStage::LoadingPlaylists {
        lines.push(Line::from("Loading your playlists…"));
    } else {
        let visible = inner.height.saturating_sub(5).clamp(1, 8) as usize;
        let first = menu.cursor.saturating_sub(visible - 1);
        let rows = if menu.stage == MenuStage::Actions {
            entries
                .iter()
                .map(|e| (format!("{}  {}", e.key, e.label), e.unavailable.is_some()))
                .collect::<Vec<_>>()
        } else {
            menu.playlists
                .iter()
                .map(|p| (p.title.clone(), false))
                .collect()
        };
        if rows.is_empty() {
            lines.push(Line::from("No writable playlists returned by YouTube."));
        }
        for (i, (label, disabled)) in rows.iter().enumerate().skip(first).take(visible) {
            let active = i == menu.cursor;
            lines.push(Line::from(Span::styled(
                format!("{} {}", if active { "›" } else { " " }, label),
                Style::default()
                    .fg(if *disabled {
                        app.theme.text_secondary
                    } else if active {
                        app.theme.accent
                    } else {
                        app.theme.text
                    })
                    .bg(if active {
                        app.theme.surface_hover
                    } else {
                        app.theme.surface
                    }),
            )));
        }
    }
    let message = if !menu.message.is_empty() {
        menu.message.as_str()
    } else if menu.stage == MenuStage::Actions {
        entries
            .get(menu.cursor)
            .and_then(|e| e.unavailable)
            .unwrap_or("Downloads go to ~/Videos/Yute")
    } else {
        "Choose a destination playlist"
    };
    f.render_widget(
        Paragraph::new(lines),
        ratatui::layout::Rect::new(
            inner.x,
            inner.y,
            inner.width,
            inner.height.saturating_sub(3),
        ),
    );
    f.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                message,
                Style::default().fg(app.theme.text_secondary),
            )),
            Line::from("j/k select · Enter action · Esc close"),
        ])
        .wrap(ratatui::widgets::Wrap { trim: true }),
        ratatui::layout::Rect::new(inner.x, inner.bottom().saturating_sub(3), inner.width, 3),
    );
}

/// Argument-only process construction: user/entity text is never interpreted by a shell.
pub fn download_command(
    card: &Card,
    cfg: &crate::config::Config,
    dir: &std::path::Path,
) -> tokio::process::Command {
    let mut command = tokio::process::Command::new("yt-dlp");
    command
        .kill_on_drop(true)
        .args([
            "--ignore-config",
            "--js-runtimes",
            "node",
            "--quiet",
            "--no-warnings",
            "--no-progress",
            "--no-overwrites",
            "--restrict-filenames",
            "--format",
        ])
        .arg(cfg.playback.resolved_ytdl_format())
        .arg("--paths")
        .arg(dir)
        .args(["--output", "%(title).160B [%(id)s].%(ext)s"]);
    command.arg(if matches!(card, Card::Playlist(_)) {
        "--yes-playlist"
    } else {
        "--no-playlist"
    });
    let cookies = crate::config::resolve_path(&cfg.backend.cookies_file);
    if cfg.backend.auth != "guest" && cookies.is_file() {
        command.arg("--cookies").arg(cookies);
    }
    let url = if let Card::Playlist(p) = card {
        let seed = p
            .first_video_id
            .as_deref()
            .or_else(|| p.id.strip_prefix("RD").filter(|s| s.len() == 11));
        if p.is_mix {
            seed.map(|id| format!("https://www.youtube.com/watch?v={id}&list={}", p.id))
                .unwrap_or_else(|| url(card))
        } else {
            url(card)
        }
    } else {
        url(card)
    };
    command
        .arg("--")
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    command
}
