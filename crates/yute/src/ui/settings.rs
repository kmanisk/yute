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
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD),
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
                .style(Style::default().fg(app.theme.text_secondary))
                .wrap(ratatui::widgets::Wrap { trim: true })
                .block(
                    Block::default()
                        .borders(Borders::TOP)
                        .border_style(Style::default().fg(app.theme.border)),
                ),
            desc,
        );
        f.render_widget(
            Paragraph::new("j/k choose   h/l adjust   Enter toggle   Esc back")
                .style(Style::default().fg(app.theme.text_secondary)),
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
        1 => app.config.grid.columns = step(&[0, 2, 3, 4, 5], &app.config.grid.columns, forward),
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
        _ => return,
    }
    app.status_text = format!(
        "{}: {}",
        SETTINGS_ITEMS[app.settings_index].0,
        value(app, app.settings_index)
    );
}
