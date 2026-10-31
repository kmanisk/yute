use crate::app::App;
use crate::config::ShortsVisibility;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub const SETTINGS_ITEMS: &[(&str, &str)] = &[
    ("Video quality", "Maximum quality for new videos. Use :quality 720p to change an active stream."),
    ("Grid columns", "Automatic sizing adapts to the content pane. Resizing preserves selection."),
    ("Hardware decoding", "auto-safe lets MPV select an available hardware decoder."),
    ("Stream cache", "Readahead and a bounded memory budget. Applies to the active MPV session; rebuffering resumes after 1 second."),
    ("MPV seekbar", "Full draggable MPV seekbar. Off keeps slim controls and avoids DASH seek storms."),
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
    ("Download directory", "Enter edits the yt-dlp directory used by Downloads and the offline Local library."),
    ("Default quality", "Automatic target below the maximum; applies next video."),
    ("Preferred codec", "auto / AV1 / VP9 / H.264; compatible fallback remains available."),
    ("Preferred audio", "Original codec preference; applies next video."),
    ("Audio language", "Enter accepts any MPV language tag; applies next video."),
    ("Subtitle language", "Enter accepts any MPV language tag; applies next video."),
    ("History sync", "Send progress only when enabled; cross-client synchronization needs live QA."),
    ("Playlist folders", "Use a per-playlist download directory."),
    ("Scroll context", "Vim-style scrolloff rows kept around selection when the viewport allows it; zz centers explicitly."),
    ("Sidebar key", "Primary sidebar sequence, e.g. 'space e'."),
    ("Sidebar alt key", "Modified sidebar alias, e.g. 'alt-e'."),
    ("Copy URL key", "Sequence for copying the selected/current video URL, e.g. 'yy'."),
    ("Center key", "Sequence for centering the current row, e.g. 'zz'."),
    ("Quit key", "Deliberate normal-mode quit binding; empty leaves q unbound."),
    ("Scripts", "auto uses native text for passing probes; T toggles the selected title. Transliteration is approximate."),
    ("Mixes in search", "Hide Mix/radio results in normal Search; Playlists still shows them."),
    ("Music search bias", "auto requests Videos and a metadata-based 3–20 minute range; M removes this for the query."),
    ("Network engine", "auto: direct, one refresh then finite-range transport after a persistent stall. mpv: direct only. proxy: finite ranges for timed streams."),
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
        32 => format!("‹ {} rows ›", app.config.ui.scrolloff),
        33 => app.config.keys.sidebar.clone(),
        34 => app.config.keys.sidebar_alt.clone(),
        35 => app.config.keys.copy_url.clone(),
        36 => app.config.keys.center.clone(),
        37 => app.config.keys.quit.clone(),
        38 => app.config.text.scripts.clone(),
        39 => if app.config.search.mixes {
            "show"
        } else {
            "hide"
        }
        .into(),
        40 => app.config.search.music_bias.clone(),
        41 => app.config.playback.network_engine.clone(),
        _ => String::new(),
    }
}
fn toggle(on: bool) -> String {
    if on { "[on]  off" } else { " on  [off]" }.into()
}

pub const GROUPS: [&str; 8] = [
    "Playback",
    "Network",
    "Appearance",
    "Content",
    "Search",
    "Downloads",
    "Keys",
    "Account",
];
pub fn group(index: usize) -> usize {
    match index {
        3 | 41 => 1,
        1 | 9..=17 | 32 | 38 => 2,
        7 => 3,
        39 | 40 => 4,
        18..=24 | 31 => 5,
        33..=37 => 6,
        8 => 7,
        _ => 0,
    }
}
pub fn indices(app: &App) -> Vec<usize> {
    let g = group(app.settings_index);
    let q = app.settings_query.to_lowercase();
    SETTINGS_ITEMS
        .iter()
        .enumerate()
        .filter(|(i, (name, help))| {
            if q.is_empty() {
                group(*i) == g
            } else {
                format!("{name} {help}").to_lowercase().contains(&q)
            }
        })
        .map(|(i, _)| i)
        .collect()
}
pub fn move_selection(app: &mut App, down: bool) {
    if app.settings_group_focus {
        let g = group(app.settings_index);
        let g = if down { (g + 1) % 8 } else { (g + 7) % 8 };
        if let Some(i) = (0..SETTINGS_ITEMS.len()).find(|i| group(*i) == g) {
            app.settings_index = i;
        }
        return;
    }
    let indices = indices(app);
    let at = indices
        .iter()
        .position(|i| *i == app.settings_index)
        .unwrap_or(0);
    if let Some(i) = indices.get(if down {
        (at + 1).min(indices.len().saturating_sub(1))
    } else {
        at.saturating_sub(1)
    }) {
        app.settings_index = *i;
    }
}
pub fn render_settings(f: &mut Frame, app: &App, area: Rect) {
    use super::{
        pane::Pane,
        text::{truncate_str, DisplayWidth},
    };
    if area.width < 8 || area.height < 5 {
        return;
    }
    let help_h = 4.min(area.height);
    let body = Rect::new(
        area.x,
        area.y,
        area.width,
        area.height.saturating_sub(help_h + 1),
    );
    let selected = app.settings_index.min(SETTINGS_ITEMS.len() - 1);
    let compact = area.width < 70;
    let group_w = if compact { 0 } else { 18.min(body.width / 3) };
    if !compact || app.settings_group_focus {
        let a = if compact {
            body
        } else {
            Rect::new(body.x, body.y, group_w, body.height)
        };
        let inner = Pane::new("Settings", app.settings_group_focus).render(f, a, &app.theme);
        let g = group(selected);
        let offset = g.saturating_add(1).saturating_sub(inner.height as usize);
        for (row, (i, label)) in GROUPS
            .iter()
            .enumerate()
            .skip(offset)
            .take(inner.height as usize)
            .enumerate()
        {
            f.render_widget(
                Paragraph::new(truncate_str(label, inner.width as usize)).style(if i == g {
                    app.theme.style("selection")
                } else {
                    app.theme.style("text_dim")
                }),
                Rect::new(inner.x, inner.y + row as u16, inner.width, 1),
            );
        }
    }
    if !compact || !app.settings_group_focus {
        let x = body.x + group_w + u16::from(!compact);
        let right = Rect::new(x, body.y, body.right().saturating_sub(x), body.height);
        let title = if app.settings_filtering {
            format!("Find: {}", app.settings_query)
        } else if app.settings_query.is_empty() {
            format!("Preferences · {}", GROUPS[group(selected)])
        } else {
            format!("Matches: {}", app.settings_query)
        };
        let inner = Pane::new(&title, !app.settings_group_focus).render(f, right, &app.theme);
        let indices = indices(app);
        let at = indices.iter().position(|i| *i == selected).unwrap_or(0);
        let rows = inner.height as usize;
        let offset = match app.settings_anchor {
            1 => at.saturating_sub(rows / 2),
            2 => at,
            3 => at.saturating_add(1).saturating_sub(rows),
            _ => at.saturating_add(1).saturating_sub(rows),
        }
        .min(indices.len().saturating_sub(rows));
        if indices.is_empty() {
            f.render_widget(
                Paragraph::new("No matching settings").style(app.theme.style("text_dim")),
                inner,
            );
        }
        for (row, i) in indices.into_iter().skip(offset).take(rows).enumerate() {
            let val = truncate_str(&value(app, i), inner.width as usize / 2);
            let label_w = (inner.width as usize).saturating_sub(val.width() + 2);
            let label = truncate_str(SETTINGS_ITEMS[i].0, label_w);
            let pad = (inner.width as usize).saturating_sub(label.width() + val.width());
            let line = format!("{label}{}{val}", " ".repeat(pad));
            f.render_widget(
                Paragraph::new(line).style(if i == selected {
                    app.theme.style("selection")
                } else {
                    app.theme.style("text")
                }),
                Rect::new(inner.x, inner.y + row as u16, inner.width, 1),
            );
        }
    }
    let help = Rect::new(area.x, area.bottom() - help_h, area.width, help_h);
    let inner =
        Pane::new("Help · / filter · Tab groups · zz/zt/zb", false).render(f, help, &app.theme);
    f.render_widget(
        Paragraph::new(SETTINGS_ITEMS[selected].1)
            .style(app.theme.style("text_dim"))
            .wrap(ratatui::widgets::Wrap { trim: true }),
        inner,
    );
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
        32 => {
            app.config.ui.scrolloff =
                step(&[0, 1, 2, 3, 4, 5, 6], &app.config.ui.scrolloff, forward)
        }
        33..=37 => {
            app.status_text = "Press Enter to edit this key binding".into();
            return;
        }
        38 => {
            app.config.text.scripts = step(
                &["auto", "native", "transliterate"],
                &app.config.text.scripts.as_str(),
                forward,
            )
            .into()
        }
        39 => app.config.search.mixes = !app.config.search.mixes,
        40 => {
            app.config.search.music_bias = if app.config.search.music_bias == "auto" {
                "off"
            } else {
                "auto"
            }
            .into()
        }
        41 => {
            app.config.playback.network_engine = step(
                &["auto", "mpv", "proxy"],
                &app.config.playback.network_engine.as_str(),
                forward,
            )
            .into()
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

pub fn open_editor(app: &mut App) -> bool {
    use crate::app::SettingEditor;
    let setting = app.settings_index;
    let title = SETTINGS_ITEMS
        .get(setting)
        .map(|item| item.0)
        .unwrap_or("Setting")
        .to_string();
    let choices: Option<Vec<String>> = match setting {
        0 | 18 | 25 => Some(RESOLUTIONS.iter().map(|s| (*s).into()).collect()),
        1 => Some(
            (0..=8)
                .map(|n| {
                    if n == 0 {
                        "Automatic".into()
                    } else {
                        n.to_string()
                    }
                })
                .collect(),
        ),
        2 => Some(
            ["auto-safe", "vaapi", "nvdec-copy", "no"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        ),
        3 => Some(CACHE_SECONDS.iter().map(|n| n.to_string()).collect()),
        7 => Some(
            ["Hidden outside Shorts", "Hidden in feeds", "Show"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        ),
        8 => Some(
            ["Last used", "Anonymous"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        ),
        9 => Some(
            [32, 64, 128, 256, 512, 1024]
                .into_iter()
                .map(|n| n.to_string())
                .collect(),
        ),
        10 => Some((0..=3).map(|n| n.to_string()).collect()),
        11 => Some(
            ["auto", "nerd", "unicode", "ascii", "none"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        ),
        12 => Some(crate::theme::Theme::names()),
        13 => Some(
            ["auto", "kitty", "iterm2", "sixel", "halfblocks", "none"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        ),
        16 => Some(["auto", "off"].into_iter().map(str::to_string).collect()),
        17 => Some(
            ["auto", "keep", "strip"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        ),
        19 => Some(
            ["best", "opus", "aac"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        ),
        26 => Some(
            ["auto", "av1", "vp9", "h264"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        ),
        27 => Some(
            ["auto", "opus", "aac"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        ),
        32 => Some((0..=6).map(|n| n.to_string()).collect()),
        41 => Some(
            ["auto", "mpv", "proxy"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        ),
        _ => None,
    };
    if let Some(options) = choices {
        let current = value(app, setting)
            .replace(['‹', '›'], "")
            .trim()
            .to_string();
        let cursor = options
            .iter()
            .position(|option| current.contains(option))
            .unwrap_or(0);
        app.setting_editor = Some(SettingEditor::Choice {
            setting,
            title,
            options,
            cursor,
        });
        return true;
    }
    let text = match setting {
        24 => Some(app.config.downloads.directory.clone()),
        28 => Some(app.config.playback.audio_language.clone()),
        29 => Some(app.config.playback.subtitle_language.clone()),
        33 => Some(app.config.keys.sidebar.clone()),
        34 => Some(app.config.keys.sidebar_alt.clone()),
        35 => Some(app.config.keys.copy_url.clone()),
        36 => Some(app.config.keys.center.clone()),
        37 => Some(app.config.keys.quit.clone()),
        _ => None,
    };
    if let Some(value) = text {
        app.setting_editor = Some(SettingEditor::Text {
            setting,
            title,
            value,
        });
        return true;
    }
    false
}

fn apply_value(app: &mut App, setting: usize, value: &str) -> anyhow::Result<()> {
    match setting {
        0 => app.config.playback.max_resolution = value.into(),
        1 => {
            app.config.grid.columns = if value == "Automatic" {
                0
            } else {
                value.parse()?
            }
        }
        2 => app.config.playback.hwdec = value.into(),
        3 => app.config.playback.stream_cache_seconds = value.parse()?,
        7 => {
            app.config.content.shorts = match value {
                "Hidden outside Shorts" => ShortsVisibility::Never,
                "Hidden in feeds" => ShortsVisibility::Hide,
                _ => ShortsVisibility::Show,
            }
        }
        8 => app.config.general.start_anonymous = value == "Anonymous",
        9 => app.config.thumbnails.cache_mb = value.parse()?,
        10 => app.config.thumbnails.prefetch_pages = value.parse()?,
        11 => app.config.ui.icons = Some(value.into()),
        12 => app.config.general.theme = value.into(),
        13 => app.config.general.graphics = value.into(),
        16 => app.config.ui.action_bar = value.into(),
        17 => app.config.text.emoji = value.into(),
        18 => app.config.downloads.video_quality = value.into(),
        19 => app.config.downloads.audio_mode = value.into(),
        24 => {
            if value.trim().is_empty() {
                anyhow::bail!("Download directory cannot be empty");
            }
            app.config.downloads.directory = value.trim().into();
        }
        25 => app.config.playback.default_quality = value.into(),
        26 => app.config.playback.preferred_codec = value.into(),
        27 => app.config.playback.preferred_audio_codec = value.into(),
        28 => app.config.playback.audio_language = value.trim().into(),
        29 => app.config.playback.subtitle_language = value.trim().into(),
        32 => app.config.ui.scrolloff = value.parse::<u16>()?.min(12),
        33 => app.config.keys.sidebar = value.trim().into(),
        34 => app.config.keys.sidebar_alt = value.trim().into(),
        35 => app.config.keys.copy_url = value.trim().into(),
        36 => app.config.keys.center = value.trim().into(),
        37 => app.config.keys.quit = value.trim().into(),
        41 => {
            yute_core::player::network::Engine::parse(value)?;
            app.config.playback.network_engine = value.into();
        }
        _ => anyhow::bail!("This setting uses the toggle control"),
    }
    Ok(())
}

/// Handle the modal editor. Returns Some(saved) when it consumed the key.
pub fn handle_editor_key(app: &mut App, key: crossterm::event::KeyEvent) -> Option<bool> {
    use crate::app::SettingEditor;
    use crossterm::event::{KeyCode, KeyModifiers};
    let editor = app.setting_editor.as_mut()?;
    match editor {
        SettingEditor::Choice {
            options, cursor, ..
        } => match key.code {
            KeyCode::Esc => {
                app.setting_editor = None;
                return Some(false);
            }
            KeyCode::Up | KeyCode::Char('k') => *cursor = cursor.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                *cursor = (*cursor + 1).min(options.len().saturating_sub(1))
            }
            KeyCode::Home => *cursor = 0,
            KeyCode::End => *cursor = options.len().saturating_sub(1),
            KeyCode::Enter => {
                let SettingEditor::Choice {
                    setting,
                    options,
                    cursor,
                    ..
                } = app.setting_editor.take().unwrap()
                else {
                    unreachable!()
                };
                let Some(value) = options.get(cursor).cloned() else {
                    return Some(false);
                };
                match apply_value(app, setting, &value).and_then(|_| app.config.save()) {
                    Ok(()) => {
                        if setting == 9 {
                            app.thumbs.set_budget(app.config.thumbnails.cache_mb);
                        }
                        app.status_text = format!("{}: {}", SETTINGS_ITEMS[setting].0, value);
                        return Some(true);
                    }
                    Err(_) => {
                        app.status_text = "Could not save setting".into();
                        return Some(false);
                    }
                }
            }
            _ => {}
        },
        SettingEditor::Text { value, .. } => match key.code {
            KeyCode::Esc => {
                app.setting_editor = None;
                return Some(false);
            }
            KeyCode::Backspace => {
                value.pop();
            }
            KeyCode::Char('u') if key.modifiers == KeyModifiers::CONTROL => value.clear(),
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                value.push(c)
            }
            KeyCode::Enter => {
                let SettingEditor::Text { setting, value, .. } = app.setting_editor.take().unwrap()
                else {
                    unreachable!()
                };
                match apply_value(app, setting, &value).and_then(|_| app.config.save()) {
                    Ok(()) => {
                        app.status_text = format!("{} updated", SETTINGS_ITEMS[setting].0);
                        return Some(true);
                    }
                    Err(_) => {
                        app.status_text = "Invalid setting value; previous value kept".into();
                        return Some(false);
                    }
                }
            }
            _ => {}
        },
    }
    Some(false)
}

pub fn editor_area(app: &App, outer: Rect) -> Option<Rect> {
    use crate::app::SettingEditor;
    let editor = app.setting_editor.as_ref()?;
    let width = outer.width.saturating_sub(8).min(68).max(24);
    let height = match editor {
        SettingEditor::Choice { options, .. } => (options.len() as u16 + 4)
            .min(outer.height.saturating_sub(4))
            .max(6),
        SettingEditor::Text { .. } => 7.min(outer.height.saturating_sub(2)).max(5),
    };
    Some(Rect::new(
        outer.x + outer.width.saturating_sub(width) / 2,
        outer.y + outer.height.saturating_sub(height) / 2,
        width,
        height,
    ))
}

pub fn render_editor(f: &mut Frame, app: &App) {
    use crate::app::SettingEditor;
    use ratatui::widgets::Clear;
    let Some(editor) = &app.setting_editor else {
        return;
    };
    let Some(rect) = editor_area(app, f.area()) else {
        return;
    };
    let title = match editor {
        SettingEditor::Choice { title, .. } | SettingEditor::Text { title, .. } => title.as_str(),
    };
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "))
        .border_style(app.theme.style("border_focused"))
        .style(app.theme.style("popup"));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    match editor {
        SettingEditor::Choice {
            options, cursor, ..
        } => {
            let visible = inner.height.saturating_sub(1) as usize;
            let start = cursor.saturating_sub(visible.saturating_sub(1));
            let lines = options
                .iter()
                .enumerate()
                .skip(start)
                .take(visible)
                .map(|(i, option)| {
                    let selected = i == *cursor;
                    Line::from(Span::styled(
                        format!("{} {}", if selected { "▎" } else { " " }, option),
                        if selected {
                            app.theme.style("accent").add_modifier(Modifier::BOLD)
                        } else {
                            app.theme.style("text")
                        },
                    ))
                })
                .collect::<Vec<_>>();
            f.render_widget(Paragraph::new(lines), inner);
        }
        SettingEditor::Text { value, .. } => {
            f.render_widget(
                Paragraph::new(vec![
                    Line::from("Type a value, Enter saves, Esc cancels"),
                    Line::default(),
                    Line::from(vec![
                        Span::styled("▎ ", app.theme.style("accent")),
                        Span::styled(
                            value,
                            app.theme.style("text").add_modifier(Modifier::UNDERLINED),
                        ),
                    ]),
                ])
                .style(app.theme.style("text_dim")),
                inner,
            );
        }
    }
}
