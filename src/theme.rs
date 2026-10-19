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
    if s.len() == 6 {
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

    pub fn load(name: &str) -> Self {
        let mut base = match name {
            "youtube-light" => Self::youtube_light(),
            _ => Self::youtube_dark(),
        };

        // Check if theme.toml exists in config dir
        if let Some(home) = std::env::var_os("HOME") {
            let path = PathBuf::from(home).join(".config/ytui/theme.toml");
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
