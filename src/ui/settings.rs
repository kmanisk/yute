use crate::app::App;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub const SETTINGS_ITEMS: &[(&str, &str)] = &[
    ("Max Resolution", "Default maximum stream resolution (1080p prevents lag/buffering)"),
    ("Grid Windows", "Number of card columns displayed (3x3 grid layout)"),
    ("Hardware Decoding", "GPU video decoding (auto-safe uses NVDEC/VAAPI for zero CPU stutter)"),
    ("Stream Cache", "Pre-buffer size and readahead to prevent stream pauses"),
    ("OSD Seekbar", "Clean bottom bar vs center screen overlay box"),
    ("Use User MPV Config", "Use ~/.config/mpv/mpv.conf or ytui dedicated profile"),
    ("Default Playback Mode", "Spawn windowed MPV player or inline terminal player"),
    ("Autoplay Next", "Automatically queue and play next recommended video"),
];

pub fn render_settings(f: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Options list
            Constraint::Length(3), // Footer hints
        ])
        .split(area);

    // Title header
    let title_p = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" Settings & Playback Tuning", Style::default().fg(app.theme.text).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled(" Configure video resolution, grid windows, and MPV hardware acceleration", Style::default().fg(app.theme.text_secondary)),
        ]),
    ])
    .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(app.theme.border)));
    f.render_widget(title_p, chunks[0]);

    // Items list
    let mut item_lines = Vec::new();
    for (idx, (label, desc)) in SETTINGS_ITEMS.iter().enumerate() {
        let is_selected = app.settings_index == idx;
        let pointer = if is_selected { "> " } else { "  " };

        let val_str = match idx {
            0 => app.config.playback.max_resolution.clone(),
            1 => {
                if app.config.grid.columns == 3 {
                    "3x3 Windows (3 Columns)".to_string()
                } else if app.config.grid.columns == 0 {
                    "Auto Responsive".to_string()
                } else {
                    format!("{} Columns", app.config.grid.columns)
                }
            }
            2 => app.config.playback.hwdec.clone(),
            3 => "Fast Buffer (150MB / 20s readahead)".to_string(),
            4 => {
                if app.config.playback.osd_bar {
                    "Center Box (Default OSD)".to_string()
                } else {
                    "Bottom OSC Only (Clean)".to_string()
                }
            }
            5 => {
                if app.config.playback.use_user_mpv_config {
                    "Enabled (~/.config/mpv/mpv.conf)".to_string()
                } else {
                    "Disabled (Use ytui Profile)".to_string()
                }
            }
            6 => app.config.playback.default_mode.clone(),
            7 => {
                if app.config.playback.autoplay {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                }
            }
            _ => String::new(),
        };

        let style = if is_selected {
            Style::default()
                .bg(app.theme.surface_hover)
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.theme.text)
        };

        item_lines.push(Line::from(vec![
            Span::styled(pointer, if is_selected { Style::default().fg(app.theme.accent) } else { Style::default() }),
            Span::styled(format!("{label:<24}"), style),
            Span::styled(" : ", Style::default().fg(app.theme.text_secondary)),
            Span::styled(format!("[ {val_str} ]"), if is_selected { Style::default().fg(app.theme.accent).add_modifier(Modifier::BOLD) } else { Style::default().fg(app.theme.text) }),
        ]));

        if is_selected {
            item_lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(format!("  - {desc}"), Style::default().fg(app.theme.text_secondary)),
            ]));
        }

        item_lines.push(Line::from(""));
    }

    let list_p = Paragraph::new(item_lines).block(Block::default().borders(Borders::NONE));
    f.render_widget(list_p, chunks[1]);

    // Footer hints
    let footer_p = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" [j/k] Navigate  [h/l or Enter] Change Value  [q/Esc] Back & Save", Style::default().fg(app.theme.text_secondary)),
        ]),
    ])
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(app.theme.border)));
    f.render_widget(footer_p, chunks[2]);
}

pub fn cycle_setting(app: &mut App, forward: bool) {
    match app.settings_index {
        0 => {
            // Max resolution
            let resolutions = ["1080p", "720p", "480p", "1440p", "2160p"];
            let cur = app.config.playback.max_resolution.as_str();
            let idx = resolutions.iter().position(|&r| r == cur).unwrap_or(0);
            let next_idx = if forward {
                (idx + 1) % resolutions.len()
            } else {
                (idx + resolutions.len() - 1) % resolutions.len()
            };
            let chosen = resolutions[next_idx];
            app.config.playback.max_resolution = chosen.to_string();
            let height = match chosen {
                "720p" => "720",
                "480p" => "480",
                "1440p" => "1440",
                "2160p" => "2160",
                _ => "1080",
            };
            app.config.playback.ytdl_format = format!("bestvideo[height<=?{}]+bestaudio/best", height);
            let _ = app.player.profile.setup_default_configs(None);
            app.status_text = format!("Max resolution set to {chosen}");
        }
        1 => {
            // Grid columns
            let cols = [3, 4, 2, 0];
            let cur = app.config.grid.columns;
            let idx = cols.iter().position(|&c| c == cur).unwrap_or(0);
            let next_idx = if forward {
                (idx + 1) % cols.len()
            } else {
                (idx + cols.len() - 1) % cols.len()
            };
            app.config.grid.columns = cols[next_idx];
            app.status_text = format!("Grid columns set to {}", if cols[next_idx] == 0 { "Auto".to_string() } else { format!("{}", cols[next_idx]) });
        }
        2 => {
            // Hardware acceleration
            let hwdecs = ["auto-safe", "nvdec-copy", "vaapi", "no"];
            let cur = app.config.playback.hwdec.as_str();
            let idx = hwdecs.iter().position(|&h| h == cur).unwrap_or(0);
            let next_idx = if forward {
                (idx + 1) % hwdecs.len()
            } else {
                (idx + hwdecs.len() - 1) % hwdecs.len()
            };
            app.config.playback.hwdec = hwdecs[next_idx].to_string();
            app.status_text = format!("Hardware decoding set to {}", hwdecs[next_idx]);
        }
        3 => {
            // Stream cache
            app.status_text = "Stream cache: Fast Buffer (150MB / 20s readahead)".to_string();
        }
        4 => {
            // OSD Seekbar
            app.config.playback.osd_bar = !app.config.playback.osd_bar;
            app.status_text = format!("OSD bar: {}", if app.config.playback.osd_bar { "Center Box" } else { "Bottom OSC Only" });
        }
        5 => {
            // Use User MPV Config
            app.config.playback.use_user_mpv_config = !app.config.playback.use_user_mpv_config;
            app.status_text = format!("Use user MPV config: {}", app.config.playback.use_user_mpv_config);
        }
        6 => {
            // Default Playback Mode
            if app.config.playback.default_mode == "window" {
                app.config.playback.default_mode = "inline".to_string();
            } else {
                app.config.playback.default_mode = "window".to_string();
            }
            app.status_text = format!("Default playback mode: {}", app.config.playback.default_mode);
        }
        7 => {
            // Autoplay
            app.config.playback.autoplay = !app.config.playback.autoplay;
            app.status_text = format!("Autoplay: {}", app.config.playback.autoplay);
        }
        _ => {}
    }

    let _ = app.config.save();
}
