use ratatui::style::Color;
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Theme {
    pub bg: Color,
    pub surface: Color,
    pub surface_hover: Color,
    pub border: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub red: Color,
    pub accent: Color,
    pub chip_active_bg: Color,
    pub chip_active_fg: Color,
    pub chip_inactive_bg: Color,
    pub chip_inactive_fg: Color,
    pub badge_bg: Color,
    pub live_badge_bg: Color,
    #[allow(dead_code)]
    pub progress_track: Color,
    #[allow(dead_code)]
    pub progress_fill: Color,
}

fn hex(s: &str, fallback: Color) -> Color {
    let s = s.trim().trim_matches('"').trim_start_matches('#');
    if s.len() == 6 && s.is_ascii() {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&s[0..2], 16),
            u8::from_str_radix(&s[2..4], 16),
            u8::from_str_radix(&s[4..6], 16),
        ) {
            return Color::Rgb(r, g, b);
        }
    }
    fallback
}

impl Theme {
    pub fn youtube_dark() -> Self {
        Self {
            bg: hex("#0F0F0F", Color::Rgb(15, 15, 15)),
            surface: hex("#272727", Color::Rgb(39, 39, 39)),
            surface_hover: hex("#3F3F3F", Color::Rgb(63, 63, 63)),
            border: hex("#303030", Color::Rgb(48, 48, 48)),
            text: hex("#F1F1F1", Color::Rgb(241, 241, 241)),
            text_secondary: hex("#AAAAAA", Color::Rgb(170, 170, 170)),
            red: hex("#FF0000", Color::Rgb(255, 0, 0)),
            accent: hex("#FF0000", Color::Rgb(255, 0, 0)),
            chip_active_bg: hex("#F1F1F1", Color::Rgb(241, 241, 241)),
            chip_active_fg: hex("#0F0F0F", Color::Rgb(15, 15, 15)),
            chip_inactive_bg: hex("#272727", Color::Rgb(39, 39, 39)),
            chip_inactive_fg: hex("#F1F1F1", Color::Rgb(241, 241, 241)),
            badge_bg: hex("#000000", Color::Rgb(0, 0, 0)),
            live_badge_bg: hex("#CC0000", Color::Rgb(204, 0, 0)),
            progress_track: hex("#606060", Color::Rgb(96, 96, 96)),
            progress_fill: hex("#FF0000", Color::Rgb(255, 0, 0)),
        }
    }

    pub fn youtube_light() -> Self {
        Self {
            bg: hex("#FFFFFF", Color::Rgb(255, 255, 255)),
            surface: hex("#F2F2F2", Color::Rgb(242, 242, 242)),
            surface_hover: hex("#E5E5E5", Color::Rgb(229, 229, 229)),
            border: hex("#CCCCCC", Color::Rgb(204, 204, 204)),
            text: hex("#0F0F0F", Color::Rgb(15, 15, 15)),
            text_secondary: hex("#606060", Color::Rgb(96, 96, 96)),
            red: hex("#CC0000", Color::Rgb(204, 0, 0)),
            accent: hex("#CC0000", Color::Rgb(204, 0, 0)),
            chip_active_bg: hex("#0F0F0F", Color::Rgb(15, 15, 15)),
            chip_active_fg: hex("#FFFFFF", Color::Rgb(255, 255, 255)),
            chip_inactive_bg: hex("#0000000D", Color::Rgb(230, 230, 230)),
            chip_inactive_fg: hex("#0F0F0F", Color::Rgb(15, 15, 15)),
            badge_bg: hex("#000000", Color::Rgb(0, 0, 0)),
            live_badge_bg: hex("#CC0000", Color::Rgb(204, 0, 0)),
            progress_track: hex("#CCCCCC", Color::Rgb(204, 204, 204)),
            progress_fill: hex("#CC0000", Color::Rgb(204, 0, 0)),
        }
    }

    pub fn system_theme() -> Option<Self> {
        let home = std::env::var_os("HOME")?;
        let colors_sh = PathBuf::from(home).join(".local/state/theme/colors.sh");
        if !colors_sh.exists() {
            return None;
        }

        let content = fs::read_to_string(colors_sh).ok()?;
        let mut map = std::collections::HashMap::new();
        for line in content.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("export ") {
                if let Some((k, v)) = rest.split_once('=') {
                    let v = v.trim_matches('"').trim_matches('\'');
                    map.insert(k.trim().to_string(), v.to_string());
                }
            }
        }

        // Terminal default background (transparent, merges with terminal like btop/cava)
        let bg = Color::Reset;
        let fg = map
            .get("THEME_FG")
            .map(|s| hex(s, Color::White))
            .unwrap_or(Color::White);
        let accent = map
            .get("THEME_ACCENT")
            .map(|s| hex(s, Color::Cyan))
            .unwrap_or(Color::Cyan);
        let sel = map
            .get("THEME_SEL")
            .map(|s| hex(s, Color::DarkGray))
            .unwrap_or(Color::DarkGray);
        let muted = map
            .get("THEME_MUTED")
            .map(|s| hex(s, Color::Gray))
            .unwrap_or(Color::Gray);
        let red = map
            .get("THEME_RED")
            .map(|s| hex(s, Color::Red))
            .unwrap_or(Color::Red);

        Some(Self {
            bg,
            surface: Color::Reset,
            surface_hover: sel,
            border: muted,
            text: fg,
            text_secondary: match (muted, fg) {
                (Color::Rgb(r, g, b), Color::Rgb(fr, fg, fb)) => Color::Rgb(
                    ((r as u16 + fr as u16) / 2) as u8,
                    ((g as u16 + fg as u16) / 2) as u8,
                    ((b as u16 + fb as u16) / 2) as u8,
                ),
                _ => Color::Gray,
            },
            red,
            accent,
            chip_active_bg: accent,
            chip_active_fg: Color::Black,
            chip_inactive_bg: Color::Reset,
            chip_inactive_fg: fg,
            badge_bg: Color::Reset,
            live_badge_bg: red,
            progress_track: muted,
            progress_fill: accent,
        })
    }

    pub fn load(name: &str) -> Self {
        let mut base = if name == "youtube-light" {
            Self::youtube_light()
        } else if let Some(sys) = Self::system_theme() {
            sys
        } else if name == "youtube-dark" {
            Self::youtube_dark()
        } else {
            Self::youtube_dark()
        };

        // Check if theme.toml exists in config dir
        if let Some(home) = std::env::var_os("HOME") {
            let path = PathBuf::from(home).join(".config/yute/theme.toml");
            if path.exists() {
                if let Ok(content) = fs::read_to_string(path) {
                    base.apply_overrides(&content);
                }
            }
        }
        base
    }

    fn apply_overrides(&mut self, text: &str) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let key = k.trim();
                let val = v.trim().trim_matches('"').trim_matches('\'');
                match key {
                    "bg" => self.bg = hex(val, self.bg),
                    "surface" => self.surface = hex(val, self.surface),
                    "surface_hover" => self.surface_hover = hex(val, self.surface_hover),
                    "border" => self.border = hex(val, self.border),
                    "text" => self.text = hex(val, self.text),
                    "text_secondary" => self.text_secondary = hex(val, self.text_secondary),
                    "red" => self.red = hex(val, self.red),
                    "accent" => self.accent = hex(val, self.accent),
                    _ => {}
                }
            }
        }
    }
}
