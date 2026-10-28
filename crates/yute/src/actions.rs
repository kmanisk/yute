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
        Card::Channel(c) if crate::backend::models::subscription_tab_params(&c.id).is_some() => {
            "https://www.youtube.com/feed/subscriptions".into()
        }
        Card::Channel(c) => format!("https://www.youtube.com/channel/{}", c.id),
        Card::Playlist(p)
            if p.is_mix && p.first_video_id.as_deref().is_some_and(|id| !id.is_empty()) =>
        {
            format!(
                "https://www.youtube.com/watch?v={}&list={}",
                p.first_video_id.as_deref().unwrap_or_default(),
                p.id.trim_start_matches("VL")
            )
        }
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
    ClearRating {
        video_id: String,
    },
    WatchLater {
        video_id: String,
        remove: bool,
    },
    RemoveHistory {
        video_id: String,
    },
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
    Like,
    Dislike,
    Subscribe,
    WatchLater,
    RemoveHistory,
    QueuePlaylist,
    PlayAll,
    PlaySelect,
    Audio,
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
    PlayAs,
    LoadingFormats,
    VideoFormats,
    AudioFormats,
    LoadingPlaylists,
    Playlists,
    DownloadKind,
    PlaylistVideo,
    PlaylistAudio,
}
pub struct ActionMenu {
    pub request: u64,
    pub card: Card,
    pub cursor: usize,
    pub stage: MenuStage,
    pub playlists: Vec<PlaylistCard>,
    pub message: String,
    pub download: bool,
    pub ffmpeg: bool,
    pub formats: Option<yute_core::media::formats::AvailableFormats>,
    pub cancelled: yute_core::oauth::CancellationToken,
}
impl Drop for ActionMenu {
    fn drop(&mut self) {
        self.cancelled.cancel();
    }
}
impl ActionMenu {
    pub fn media_rows(&self) -> Vec<(String, String)> {
        if self.stage == MenuStage::DownloadKind {
            return vec![
                ("Video".into(), "video".into()),
                ("Audio".into(), "audio".into()),
            ];
        }
        if self.stage == MenuStage::PlaylistVideo {
            return ["best", "1080p", "720p", "480p", "360p"]
                .iter()
                .map(|v| {
                    (
                        format!(
                            "{}{}",
                            v,
                            if *v == "best" {
                                ""
                            } else {
                                " ceiling per video"
                            }
                        ),
                        (*v).into(),
                    )
                })
                .collect();
        }
        if self.stage == MenuStage::PlaylistAudio {
            let mut rows = vec![
                ("Best original audio".into(), "best".into()),
                ("Prefer Opus".into(), "opus".into()),
                ("Prefer AAC".into(), "aac".into()),
            ];
            if self.ffmpeg {
                for v in ["mp3", "flac", "m4a"] {
                    rows.push((format!("{} (convert)", v.to_uppercase()), v.into()));
                }
            }
            return rows;
        }

        use yute_core::media::formats::MediaKind;
        let Some(formats) = &self.formats else {
            return vec![];
        };
        let mut rows = formats
            .formats
            .iter()
            .filter(|f| match self.stage {
                MenuStage::VideoFormats => f.kind != MediaKind::AudioOnly,
                MenuStage::AudioFormats => f.kind == MediaKind::AudioOnly,
                _ => false,
            })
            .collect::<Vec<_>>();
        rows.sort_by_key(|f| {
            std::cmp::Reverse((
                f.height.unwrap_or(0),
                (f.fps.unwrap_or(0.) * 100.) as u32,
                f.audio_bitrate.or(f.bitrate).unwrap_or(0),
            ))
        });
        let mut result = vec![(
            "Best automatically".into(),
            if self.stage == MenuStage::AudioFormats {
                let Some(audio) = formats.best_audio(None) else {
                    return vec![];
                };
                audio.id.clone()
            } else {
                String::new()
            },
        )];
        for f in rows {
            let codec = f
                .video_codec
                .as_deref()
                .or(f.audio_codec.as_deref())
                .unwrap_or("unknown");
            let codec = if codec.starts_with("av01") {
                "AV1"
            } else if codec.starts_with("avc") {
                "H.264"
            } else if codec.starts_with("vp") {
                "VP9"
            } else if codec.starts_with("mp4a") {
                "AAC"
            } else {
                codec
            };
            let rate = f
                .audio_bitrate
                .or(f.bitrate)
                .map(|rate| format!(" · {} kbps", rate / 1000))
                .unwrap_or_default();
            let quality = f
                .height
                .map(|h| {
                    format!(
                        "{h}p{} · ",
                        if f.fps.unwrap_or(0.) >= 59. { "60" } else { "" }
                    )
                })
                .unwrap_or_default();
            let language = f
                .audio_language
                .as_deref()
                .map(|l| format!("{l} · "))
                .unwrap_or_default();
            if let Ok(selector) = formats.selector(&f.id) {
                result.push((
                    format!(
                        "{quality}{language}{codec}{}{rate}",
                        if f.hdr { " · HDR" } else { "" }
                    ),
                    selector,
                ));
            }
        }
        if self.download && self.ffmpeg && self.stage == MenuStage::AudioFormats {
            for kind in ["mp3", "flac", "m4a"] {
                result.push((
                    format!("{} (convert)", kind.to_uppercase()),
                    format!("convert:{kind}"),
                ));
            }
        }
        result
    }
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
    if matches!(card, Card::Video(_))
        && !local
        && !matches!(activation(card), OpenAction::Unavailable(_))
    {
        entries.push(MenuEntry {
            label: "Play / quality...",
            key: 'f',
            action: PlaySelect,
            unavailable: None,
        });
        entries.push(MenuEntry {
            label: "Audio only...",
            key: 'm',
            action: Audio,
            unavailable: None,
        });
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
            label: if matches!(card, Card::Playlist(_)) {
                "Download playlist..."
            } else {
                "Download..."
            },
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
    if authenticated && !local {
        if matches!(card, Card::Video(_)) {
            for (label, key, action) in [
                ("Add/remove Watch Later", 'w', WatchLater),
                ("Like / Unlike", '+', Like),
                ("Dislike / Remove dislike", '-', Dislike),
                ("Remove from history", 'R', RemoveHistory),
            ] {
                entries.push(MenuEntry {
                    label,
                    key,
                    action,
                    unavailable: None,
                });
            }
        }
        if channel_of(card).is_some_and(|channel| {
            crate::backend::models::youtube_channel_id(&channel.id).is_some()
        }) {
            entries.push(MenuEntry {
                label: "Subscribe / Unsubscribe",
                key: 'S',
                action: Subscribe,
                unavailable: None,
            });
        }
    }
    if matches!(card, Card::Playlist(_)) {
        entries.push(MenuEntry {
            label: "Queue playlist",
            key: 'a',
            action: QueuePlaylist,
            unavailable: None,
        });
    }
    entries.sort_by_key(|e| match e.action {
        Play | PlaySelect | Audio | Browse => 0,
        Queue | QueuePlaylist | Save | WatchLater | Like | Dislike => 1,
        Download | CancelDownload | Browser | CopyUrl | CopyTitle => 2,
        _ => 3,
    });
    entries
}
pub fn visible_entries(app: &crate::app::App, card: &Card) -> Vec<MenuEntry> {
    use crate::backend::Backend;
    menu_entries(card, app.is_authenticated(), app.download_active)
        .into_iter()
        .filter(|e| {
            (e.unavailable.is_none() || e.action == ItemAction::Save)
                && (e.action != ItemAction::RemoveHistory
                    || (app.screen == crate::app::Screen::History
                        && app.backend.can_remove_history(card.id())))
        })
        .map(|mut e| {
            match e.action {
                ItemAction::Subscribe => {
                    if let Some(channel) = channel_of(card) {
                        e.label =
                            if app
                                .subscriptions
                                .get(&channel.id)
                                .copied()
                                .unwrap_or_else(|| {
                                    app.subscribed_channels.iter().any(|c| c.id() == channel.id)
                                })
                            {
                                "Unsubscribe"
                            } else {
                                "Subscribe"
                            };
                    }
                }
                ItemAction::Like => {
                    e.label = if app.ratings.get(card.id()) == Some(&1)
                        || app.screen == crate::app::Screen::Liked
                    {
                        "Unlike"
                    } else {
                        "Like"
                    }
                }
                ItemAction::Dislike => {
                    e.label = if app.ratings.get(card.id()) == Some(&-1) {
                        "Remove dislike"
                    } else {
                        "Dislike"
                    }
                }
                ItemAction::WatchLater => {
                    e.label = if app.watch_later.contains(card.id())
                        || app.screen == crate::app::Screen::WatchLater
                    {
                        "Remove Watch Later"
                    } else {
                        "Add Watch Later"
                    }
                }
                _ => (),
            }
            e
        })
        .collect()
}
pub fn menu_rows(menu: &ActionMenu, auth: bool, active: bool) -> Vec<(String, Option<char>)> {
    if menu.stage == MenuStage::Actions {
        return menu_entries(&menu.card, auth, active)
            .into_iter()
            .filter(|e| e.unavailable.is_none() || e.action == ItemAction::Save)
            .map(|e| (e.label.to_string(), Some(e.key)))
            .collect();
    }
    if menu.stage == MenuStage::PlayAs {
        return vec![
            ("Video".into(), None),
            ("Audio only".into(), None),
            ("Best automatically".into(), None),
        ];
    }
    if menu.stage == MenuStage::Playlists {
        return menu
            .playlists
            .iter()
            .map(|p| (p.title.clone(), None))
            .collect();
    }
    menu.media_rows()
        .into_iter()
        .map(|(label, _)| (label, None))
        .collect()
}
pub fn menu_rect(
    app: &crate::app::App,
    area: ratatui::layout::Rect,
) -> Option<ratatui::layout::Rect> {
    use unicode_width::UnicodeWidthStr;
    let menu = app.action_menu.as_ref()?;
    let rows = if menu.stage == MenuStage::Actions {
        visible_entries(app, &menu.card)
            .into_iter()
            .map(|e| (e.label.to_string(), Some(e.key)))
            .collect()
    } else {
        menu_rows(menu, app.is_authenticated(), app.download_active)
    };
    let download_width = if rows.iter().any(|(label, _)| label.contains("Download")) {
        app.config.downloads.directory.len() + 40
    } else {
        0
    };
    let width = rows
        .iter()
        .map(|(s, _)| s.width() + 7)
        .max()
        .unwrap_or(30)
        .max(download_width)
        .max(40)
        .min((area.width as usize * 4 / 5).max(1))
        .min(area.width.saturating_sub(2) as usize) as u16;
    let title_lines = crate::ui::text::wrap_and_truncate_title(
        menu.card.title(),
        width.saturating_sub(4) as usize,
        2,
    )
    .len();
    let height =
        (rows.len().max(1) + title_lines + 4).min(area.height.saturating_sub(2) as usize) as u16;
    Some(ratatui::layout::Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    ))
}
pub fn render_menu(f: &mut ratatui::Frame, app: &crate::app::App) {
    use ratatui::{
        layout::Rect,
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
        .title(if menu.download {
            " Download "
        } else {
            " Actions "
        })
        .style(app.theme.style("popup"))
        .border_style(app.theme.style("border_focused"));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let titles = crate::ui::text::wrap_and_truncate_title(
        menu.card.title(),
        inner.width.saturating_sub(2) as usize,
        2,
    );
    let title_h = titles.len() as u16;
    f.render_widget(
        Paragraph::new(titles.into_iter().map(Line::from).collect::<Vec<_>>())
            .style(app.theme.style("text")),
        Rect::new(
            inner.x + 1,
            inner.y,
            inner.width.saturating_sub(2),
            title_h.min(inner.height),
        ),
    );
    let rows = if menu.stage == MenuStage::Actions {
        visible_entries(app, &menu.card)
            .into_iter()
            .map(|e| (e.label.to_string(), Some(e.key)))
            .collect()
    } else {
        menu_rows(menu, app.is_authenticated(), app.download_active)
    };
    let visible = inner.height.saturating_sub(title_h + 2) as usize;
    let first = menu.cursor.saturating_sub(visible.saturating_sub(1));
    let mut lines = vec![];
    for (i, (label, key)) in rows.iter().enumerate().skip(first).take(visible) {
        let selected = i == menu.cursor;
        let style = app
            .theme
            .style(if selected { "popup_selected" } else { "text" });
        let icon = key
            .and_then(|k| {
                visible_entries(app, &menu.card)
                    .into_iter()
                    .find(|e| e.key == k)
            })
            .map(|e| match e.action {
                ItemAction::Play | ItemAction::PlayAll | ItemAction::PlaySelect => {
                    crate::ui::icons::Icon::Play
                }
                ItemAction::Audio => crate::ui::icons::Icon::Audio,
                ItemAction::Download | ItemAction::CancelDownload => {
                    crate::ui::icons::Icon::Download
                }
                ItemAction::CopyUrl | ItemAction::CopyTitle => crate::ui::icons::Icon::Copy,
                ItemAction::Browser => crate::ui::icons::Icon::Browser,
                ItemAction::Like => crate::ui::icons::Icon::Like,
                ItemAction::Dislike => crate::ui::icons::Icon::Dislike,
                ItemAction::WatchLater => crate::ui::icons::Icon::WatchLater,
                ItemAction::Queue | ItemAction::QueuePlaylist => crate::ui::icons::Icon::Queue,
                ItemAction::Save => crate::ui::icons::Icon::Save,
                _ => crate::ui::icons::Icon::Channel,
            });
        let group = if menu.stage == MenuStage::Actions {
            let entries = visible_entries(app, &menu.card);
            let rank = |a: ItemAction| match a {
                ItemAction::Play
                | ItemAction::PlaySelect
                | ItemAction::Audio
                | ItemAction::Browse => 0,
                ItemAction::Queue
                | ItemAction::QueuePlaylist
                | ItemAction::Save
                | ItemAction::WatchLater
                | ItemAction::Like
                | ItemAction::Dislike => 1,
                ItemAction::Download
                | ItemAction::CancelDownload
                | ItemAction::Browser
                | ItemAction::CopyUrl
                | ItemAction::CopyTitle => 2,
                _ => 3,
            };
            if let Some(entry) = entries.get(i) {
                if i == 0 || rank(entries[i - 1].action) != rank(entry.action) {
                    ["Playback", "Library", "Share", "Account"][rank(entry.action)]
                } else {
                    ""
                }
            } else {
                ""
            }
        } else {
            ""
        };
        let decorated = icon
            .map(|i| i.label(app.config.icons(), label))
            .unwrap_or_else(|| label.clone());
        let text = format!(
            "{} {}{}",
            if selected { ">" } else { " " },
            key.map(|c| format!("{c}  ")).unwrap_or_default(),
            if group.is_empty() {
                decorated
            } else {
                format!("{group} · {decorated}")
            }
        );
        let mut spans = vec![Span::styled(
            crate::ui::text::truncate_str(&text, inner.width as usize),
            style,
        )];
        if label.contains("Download") && inner.width > 50 {
            use unicode_width::UnicodeWidthStr;
            let used = text.width();
            let hint = &app.config.downloads.directory;
            let pad = (inner.width as usize).saturating_sub(used + hint.width());
            spans.push(Span::styled(
                format!("{}{}", " ".repeat(pad), hint),
                app.theme.style("action_hint"),
            ));
        }
        lines.push(Line::from(spans));
    }
    if rows.is_empty() {
        lines.push(Line::from(if menu.stage == MenuStage::LoadingFormats {
            "Resolving formats..."
        } else if menu.stage == MenuStage::LoadingPlaylists {
            "Loading playlists..."
        } else {
            "No available entries"
        }));
    }
    let list = Rect::new(inner.x, inner.y + title_h + 1, inner.width, visible as u16);
    f.render_widget(Paragraph::new(lines), list);
    if inner.height > 0 {
        let indicator = format!(
            "{}{}",
            if first > 0 { "▲ " } else { "" },
            if first + visible < rows.len() {
                "▼ "
            } else {
                ""
            }
        );
        if !menu.message.is_empty() {
            f.render_widget(
                Paragraph::new(crate::ui::text::truncate_str(
                    &menu.message,
                    inner.width as usize,
                ))
                .style(app.theme.style("warning")),
                Rect::new(inner.x, inner.y + title_h, inner.width, 1),
            );
        }
        let footer = format!("{indicator}j/k move · Enter select · Esc close");
        f.render_widget(
            Paragraph::new(crate::ui::text::truncate_str(&footer, inner.width as usize))
                .style(app.theme.style("action_hint")),
            Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
        );
    }
}

/// Argument-only process construction: user/entity text is never interpreted by a shell.
pub fn download_command(
    card: &Card,
    cfg: &crate::config::Config,
    _dir: &std::path::Path,
) -> tokio::process::Command {
    yute_core::downloads::command(
        &yute_core::downloads::Request {
            url: url(card),
            playlist: matches!(card, Card::Playlist(_)),
            mode: yute_core::downloads::Mode::Exact(cfg.playback.resolved_ytdl_format()),
            ffmpeg: false,
        },
        cfg,
    )
    .expect("configured download selector")
}
