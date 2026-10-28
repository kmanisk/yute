use crate::app::App;
use crate::config::ShortsVisibility;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub const SETTINGS_ITEMS: &[(&str, &str)] = &[
    ("Video quality", "Maximum quality for new videos. Use :quality 720p to change an active stream."),
    ("Grid columns", "Automatic sizing adapts to the content pane. Resizing preserves selection."),
    ("Hardware decoding", "auto-safe lets MPV select an available hardware decoder."),
    ("Stream cache", "Readahead and a bounded memory budget. Applies to the active MPV session; rebuffering resumes after 1 second."),
    ("MPV seekbar", "Show MPV's seek overlay in addition to its own bottom controls."),
    ("User MPV profile", "Use your existing MPV configuration. Startup buffering follows your profile in this mode."),
    ("Autoplay next", "Play the next queued video when the current video ends."),
    ("Shorts", "Hidden outside Shorts also hides saved Shorts. Explicit Shorts tabs always work without changing this preference."),
    ("Start as", "Choose the last used account or Anonymous for the next launch."),
    ("Thumbnail cache", "Shared source and encoded thumbnail cache budget on disk. Oldest accessed entries are pruned."),
    ("Thumbnail prefetch", "Preload 0-3 pages after visible cards. Visible requests always have priority."),
    ("Icons", "auto / nerd / unicode / ascii / none. Auto uses Unicode; YUTE_NERD_FONT=1 opts into Nerd Fonts."),
    ("Theme", "terminal/auto/custom; live reload. Invalid files keep the previous theme."),
    ("Graphics", "auto confirms protocols; mux sessions safely use halfblocks. Changes apply live."),
    ("Sidebar", "Show/hide navigation."),
    ("Sidebar channel list", "Names-only subscribed channels in tall sidebars; loaded asynchronously."),
    ("Action bar", "Auto shows contextual actions at 30 rows; off hides it."),
    ("Emoji", "auto substitutes risky sequences; keep preserves; strip removes emoji."),
    ("Download quality", "Per-item ceiling for playlists; exact qualities use the format menu."),
    ("Download audio", "Original streams by default; no automatic conversion."),
    ("Write metadata", "Write yt-dlp JSON sidecars; may include public media metadata."),
    ("Write thumbnail", "Save original image alongside media."),
    ("Write subtitles", "Save available subtitles."),
    ("Keep partial downloads", "Keep resumable yt-dlp partial files."),
    ("Download directory", "Edit [downloads].directory in config.toml; live reload."),    ("Default quality", "Automatic target below the maximum; applies next video."),
    ("Preferred codec", "auto / AV1 / VP9 / H.264; compatible fallback remains available."),
    ("Preferred audio", "Original codec preference; applies next video."),
    ("Audio language", "MPV preferred audio language; applies next video."),
    ("Subtitle language", "MPV subtitle language preference; applies next video."),
    ("History sync", "Send progress only when enabled; cross-client synchronization needs live QA."),
    ("Playlist folders", "Use a per-playlist download directory."),

];
const RESOLUTIONS: &[&str] = &["480p", "720p", "1080p", "1440p", "2160p", "best"];
const CACHE_SECONDS: &[u16] = &[5, 10, 20, 40, 60];

fn slider(index: usize, max: usize) -> String {
    let filled = (index * 12 / max.max(1)).min(12);
    format!("{}●{}", "━".repeat(filled), "─".repeat(12 - filled))
}
fn value(app: &App, index: usize) -> String {
    match index {
        0 => format!("‹ {} ›", app.config.playback.max_resolution),
        1 => {
            if app.config.grid.columns == 0 {
                "‹ Automatic ›".into()
            } else {
                format!("‹ {} columns ›", app.config.grid.columns)
            }
        }
        2 => format!("‹ {} ›", app.config.playback.hwdec),
        3 => format!(
            "{}  {}s / {} MiB",
            slider(
                CACHE_SECONDS
                    .iter()
                    .position(|s| *s >= app.config.playback.stream_cache_seconds)
                    .unwrap_or(4),
                4
            ),
            app.config.playback.stream_cache_seconds,
            app.config.playback.stream_cache_mb()
        ),
        4 => toggle(app.config.playback.osd_bar),
        5 => toggle(app.config.playback.use_user_mpv_config),
        6 => toggle(app.config.playback.autoplay),
        7 => match app.config.content.shorts {
            ShortsVisibility::Never => "‹ Hidden outside Shorts ›",
            ShortsVisibility::Hide => "‹ Hidden in feeds ›",
            ShortsVisibility::Show => "‹ Show ›",
        }
        .into(),
        8 => if app.config.general.start_anonymous {
            "‹ Anonymous ›"
        } else {
            "‹ Last used ›"
        }
        .into(),
        9 => format!("‹ {} MiB ›", app.config.thumbnails.cache_mb),
        10 => format!("‹ {} pages ›", app.config.thumbnails.prefetch_pages),
        11 => format!("‹ {} ›", app.config.icons()),
        12 => app.config.general.theme.clone(),
        13 => app.config.general.graphics.clone(),
        14 => toggle(app.sidebar_expanded),
        15 => toggle(app.config.ui.sidebar_channels),
        16 => {
            if app.config.ui.action_bar == "off" {
                "off".into()
            } else {
                "auto".into()
            }
        }
        17 => app.config.text.emoji.clone(),
        18 => app.config.downloads.video_quality.clone(),
        19 => app.config.downloads.audio_mode.clone(),
        20 => toggle(app.config.downloads.write_metadata),
        21 => toggle(app.config.downloads.write_thumbnail),
        22 => toggle(app.config.downloads.write_subtitles),
        23 => toggle(app.config.downloads.keep_partial),
        24 => app.config.downloads.directory.clone(),
        25 => app.config.playback.default_quality.clone(),
        26 => app.config.playback.preferred_codec.clone(),
        27 => app.config.playback.preferred_audio_codec.clone(),
        28 => app.config.playback.audio_language.clone(),
        29 => app.config.playback.subtitle_language.clone(),
        30 => toggle(app.config.playback.mark_watched),
        31 => toggle(app.config.downloads.playlist_subdirectories),
        _ => String::new(),
    }
}
fn toggle(on: bool) -> String {
    if on { "[on]  off" } else { " on  [off]" }.into()
}

pub fn render_settings(f: &mut Frame, app: &App, area: Rect) {
    if area.is_empty() {
        return;
    }
    let margin = (area.width / 12).min(8);
    let inner = Rect::new(
        area.x + margin,
        area.y,
        area.width.saturating_sub(margin * 2),
        area.height,
    );
    let selected = app.settings_index.min(SETTINGS_ITEMS.len() - 1);
    let mut lines = vec![
        Line::from(Span::styled(
            "Preferences",
            app.theme.style("accent").add_modifier(Modifier::BOLD),
        )),
        Line::default(),
    ];
    for (i, (label, _)) in SETTINGS_ITEMS.iter().enumerate() {
        if i == 3 || i == 7 {
            lines.push(Line::default());
        }
        let active = i == selected;
        let style = Style::default().fg(if active {
            app.theme.accent
        } else {
            app.theme.text
        });
        lines.push(Line::from(vec![
            Span::styled(if active { "▎ " } else { "  " }, style),
            Span::styled(
                format!("{label:<20}"),
                if active {
                    style.add_modifier(Modifier::BOLD)
                } else {
                    style
                },
            ),
            Span::styled(value(app, i), style),
        ]));
    }
    let available = inner.height.saturating_sub(4) as usize;
    // Keep the selected row visible on short terminals without stretching rows.
    let row = 2 + selected + usize::from(selected >= 3) + usize::from(selected >= 7);
    let offset = (row + 1).saturating_sub(available);
    f.render_widget(
        Paragraph::new(lines).scroll((offset as u16, 0)),
        Rect::new(
            inner.x,
            inner.y,
            inner.width,
            inner.height.saturating_sub(4),
        ),
    );
    if inner.height >= 5 {
        let desc = Rect::new(inner.x, inner.bottom() - 4, inner.width, 3);
        f.render_widget(
            Paragraph::new(SETTINGS_ITEMS[selected].1)
                .style(app.theme.style("text_dim"))
                .wrap(ratatui::widgets::Wrap { trim: true })
                .block(
                    Block::default()
                        .borders(Borders::TOP)
                        .border_style(app.theme.style("border")),
                ),
            desc,
        );
        f.render_widget(
            Paragraph::new("j/k choose   h/l adjust   Enter toggle   Esc back")
                .style(app.theme.style("text_dim")),
            Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
        );
    }
}
fn step<T: PartialEq + Clone>(choices: &[T], current: &T, forward: bool) -> T {
    let i = choices.iter().position(|v| v == current).unwrap_or(0);
    choices[if forward {
        (i + 1).min(choices.len() - 1)
    } else {
        i.saturating_sub(1)
    }]
    .clone()
}
pub fn cycle_setting(app: &mut App, forward: bool) {
    match app.settings_index {
        0 => {
            app.config.playback.max_resolution = step(
                RESOLUTIONS,
                &app.config.playback.max_resolution.as_str(),
                forward,
            )
            .into()
        }
        1 => {
            app.config.grid.columns = step(
                &[0, 1, 2, 3, 4, 5, 6, 7, 8],
                &app.config.grid.columns,
                forward,
            )
        }
        2 => {
            app.config.playback.hwdec = step(
                &["auto-safe", "vaapi", "nvdec-copy", "no"],
                &app.config.playback.hwdec.as_str(),
                forward,
            )
            .into()
        }
        3 => {
            app.config.playback.stream_cache_seconds = step(
                CACHE_SECONDS,
                &app.config.playback.stream_cache_seconds,
                forward,
            )
        }
        4 => app.config.playback.osd_bar = !app.config.playback.osd_bar,
        5 => app.config.playback.use_user_mpv_config = !app.config.playback.use_user_mpv_config,
        6 => app.config.playback.autoplay = !app.config.playback.autoplay,
        7 => {
            let index: usize = match app.config.content.shorts {
                ShortsVisibility::Never => 0,
                ShortsVisibility::Hide => 1,
                ShortsVisibility::Show => 2,
            };
            app.config.content.shorts = match if forward {
                (index + 1).min(2)
            } else {
                index.saturating_sub(1)
            } {
                0 => ShortsVisibility::Never,
                1 => ShortsVisibility::Hide,
                _ => ShortsVisibility::Show,
            };
            app.selected_index = 0;
            app.scroll_row = 0;
        }
        8 => app.config.general.start_anonymous = !app.config.general.start_anonymous,
        9 => {
            app.config.thumbnails.cache_mb = step(
                &[32, 64, 128, 256, 512, 1024],
                &app.config.thumbnails.cache_mb,
                forward,
            )
        }
        10 => {
            app.config.thumbnails.prefetch_pages = step(
                &[0, 1, 2, 3],
                &app.config.thumbnails.prefetch_pages,
                forward,
            )
        }
        11 => {
            app.config.ui.icons = Some(
                step(
                    &["auto", "nerd", "unicode", "ascii", "none"],
                    &app.config.icons(),
                    forward,
                )
                .into(),
            )
        }
        12 => {
            let names = crate::theme::Theme::names();
            app.config.general.theme = step(&names, &app.config.general.theme, forward);
        }
        13 => {
            app.config.general.graphics = step(
                &["auto", "kitty", "iterm2", "sixel", "halfblocks", "none"],
                &app.config.general.graphics.as_str(),
                forward,
            )
            .into()
        }
        14 => {
            app.sidebar_expanded = !app.sidebar_expanded;
            app.config.grid.sidebar_expanded = app.sidebar_expanded;
        }
        15 => app.config.ui.sidebar_channels = !app.config.ui.sidebar_channels,
        16 => {
            app.config.ui.action_bar = if app.config.ui.action_bar == "off" {
                "auto".into()
            } else {
                "off".into()
            }
        }
        17 => {
            app.config.text.emoji = step(
                &["auto", "keep", "strip"],
                &app.config.text.emoji.as_str(),
                forward,
            )
            .into()
        }
        18 => {
            app.config.downloads.video_quality = step(
                RESOLUTIONS,
                &app.config.downloads.video_quality.as_str(),
                forward,
            )
            .into()
        }
        19 => {
            app.config.downloads.audio_mode = step(
                &["best", "opus", "aac"],
                &app.config.downloads.audio_mode.as_str(),
                forward,
            )
            .into()
        }
        20 => app.config.downloads.write_metadata = !app.config.downloads.write_metadata,
        21 => app.config.downloads.write_thumbnail = !app.config.downloads.write_thumbnail,
        22 => app.config.downloads.write_subtitles = !app.config.downloads.write_subtitles,
        23 => app.config.downloads.keep_partial = !app.config.downloads.keep_partial,
        24 => {
            app.status_text =
                "Edit [downloads].directory in config.toml; live reload applies it".into();
            return;
        }
        25 => {
            app.config.playback.default_quality = step(
                RESOLUTIONS,
                &app.config.playback.default_quality.as_str(),
                forward,
            )
            .into()
        }
        26 => {
            app.config.playback.preferred_codec = step(
                &["auto", "av1", "vp9", "h264"],
                &app.config.playback.preferred_codec.as_str(),
                forward,
            )
            .into()
        }
        27 => {
            app.config.playback.preferred_audio_codec = step(
                &["auto", "opus", "aac"],
                &app.config.playback.preferred_audio_codec.as_str(),
                forward,
            )
            .into()
        }
        28 => {
            app.config.playback.audio_language = step(
                &["auto", "en", "hi", "ja"],
                &app.config.playback.audio_language.as_str(),
                forward,
            )
            .into()
        }
        29 => {
            app.config.playback.subtitle_language = step(
                &["auto", "en", "hi", "ja"],
                &app.config.playback.subtitle_language.as_str(),
                forward,
            )
            .into()
        }
        30 => app.config.playback.mark_watched = !app.config.playback.mark_watched,
        31 => {
            app.config.downloads.playlist_subdirectories =
                !app.config.downloads.playlist_subdirectories
        }
        _ => return,
    }
    if app.settings_index == 9 {
        app.thumbs.set_budget(app.config.thumbnails.cache_mb);
    }
    app.status_text = format!(
        "{}: {}",
        SETTINGS_ITEMS[app.settings_index].0,
        value(app, app.settings_index)
    );
}
