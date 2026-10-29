//! Semantic styling is constructed at load time; renderers never select palettes.
use anyhow::{bail, Context, Result};
use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;
use std::collections::BTreeMap;

pub const ROLES: &[&str] = &[
    "text",
    "text_dim",
    "border",
    "border_focused",
    "accent",
    "selection",
    "chip_active",
    "chip_idle",
    "badge",
    "badge_live",
    "badge_mix",
    "progress",
    "skeleton",
    "error",
    "warning",
    "scrollbar",
    "popup",
    "popup_selected",
    "status",
    "action_hint",
];
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
    pub badge_fg: Color,
    pub live_badge_bg: Color,
    pub progress_track: Color,
    pub progress_fill: Color,
    pub styles: BTreeMap<String, Style>,
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    fg: Option<String>,
    bg: Option<String>,
    #[serde(default)]
    modifiers: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    roles: BTreeMap<String, Spec>,
}
#[derive(Clone, Copy, Debug)]
pub struct Capabilities {
    pub no_color: bool,
    pub truecolor: bool,
    pub colors256: bool,
}
impl Capabilities {
    pub fn environment() -> Self {
        Self {
            no_color: std::env::var_os("NO_COLOR").is_some(),
            truecolor: matches!(
                std::env::var("COLORTERM").as_deref(),
                Ok("truecolor" | "24bit")
            ),
            colors256: std::env::var("TERM").unwrap_or_default().contains("256"),
        }
    }
}
pub fn parse_color(s: &str) -> Result<Color> {
    let s = s.to_ascii_lowercase();
    Ok(match s.as_str() {
        "reset" => Color::Reset,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "brightblack" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        _ if s.starts_with('#') && s.len() == 7 && s.is_ascii() => Color::Rgb(
            u8::from_str_radix(&s[1..3], 16)?,
            u8::from_str_radix(&s[3..5], 16)?,
            u8::from_str_radix(&s[5..7], 16)?,
        ),
        _ => Color::Indexed(s.parse::<u8>().context("Unknown theme colour")?),
    })
}
fn rgb(c: Color) -> Option<(u8, u8, u8)> {
    match c {
        Color::Rgb(r, g, b) => Some((r, g, b)),
        Color::Indexed(i) if i >= 232 => {
            let v = 8 + 10 * (i - 232);
            Some((v, v, v))
        }
        Color::Indexed(i) if i >= 16 => {
            let i = i - 16;
            let level = |v| if v == 0 { 0 } else { 55 + 40 * v };
            Some((level(i / 36), level(i / 6 % 6), level(i % 6)))
        }
        _ => None,
    }
}
pub fn downgrade(c: Color, caps: Capabilities) -> Color {
    if caps.no_color {
        return Color::Reset;
    }
    const ANSI: [Color; 16] = [
        Color::Black,
        Color::Red,
        Color::Green,
        Color::Yellow,
        Color::Blue,
        Color::Magenta,
        Color::Cyan,
        Color::Gray,
        Color::DarkGray,
        Color::LightRed,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightBlue,
        Color::LightMagenta,
        Color::LightCyan,
        Color::White,
    ];
    let (r, g, b) = match c {
        Color::Rgb(..) if caps.truecolor => return c,
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Indexed(i) if i < 16 && !caps.colors256 => return ANSI[i as usize],
        Color::Indexed(_) if !caps.colors256 => rgb(c).unwrap(),
        _ => return c,
    };
    let distance = |p: (u8, u8, u8)| {
        (i32::from(r) - i32::from(p.0)).pow(2)
            + (i32::from(g) - i32::from(p.1)).pow(2)
            + (i32::from(b) - i32::from(p.2)).pow(2)
    };
    if caps.colors256 {
        return Color::Indexed(
            (16..=255)
                .min_by_key(|i| distance(rgb(Color::Indexed(*i)).unwrap()))
                .unwrap(),
        );
    }
    let values = [
        (0, 0, 0),
        (128, 0, 0),
        (0, 128, 0),
        (128, 128, 0),
        (0, 0, 128),
        (128, 0, 128),
        (0, 128, 128),
        (192, 192, 192),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (0, 0, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    ANSI[(0..16).min_by_key(|i| distance(values[*i])).unwrap()]
}
impl Theme {
    pub fn terminal() -> Self {
        let normal = Style::default().fg(Color::Reset).bg(Color::Reset);
        let mut styles = BTreeMap::new();
        for role in ROLES {
            styles.insert((*role).into(), normal);
        }
        for r in ["text_dim", "chip_idle", "skeleton", "action_hint"] {
            styles.insert(r.into(), normal.add_modifier(Modifier::DIM));
        }
        for r in ["selection", "chip_active", "popup_selected"] {
            styles.insert(
                r.into(),
                normal.add_modifier(Modifier::REVERSED | Modifier::BOLD),
            );
        }
        styles.insert(
            "accent".into(),
            normal.fg(Color::Cyan).add_modifier(Modifier::BOLD),
        );
        styles.insert("border_focused".into(), normal.add_modifier(Modifier::BOLD));
        styles.insert(
            "error".into(),
            normal.fg(Color::Red).add_modifier(Modifier::BOLD),
        );
        styles.insert("warning".into(), normal.fg(Color::Yellow));
        for role in ["badge", "badge_mix"] {
            styles.insert(
                role.into(),
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            );
        }
        styles.insert(
            "badge_live".into(),
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        );
        Self::from_styles(styles)
    }
    fn from_styles(styles: BTreeMap<String, Style>) -> Self {
        let fg = |r: &str| styles[r].fg.unwrap_or(Color::Reset);
        let bg = |r: &str| styles[r].bg.unwrap_or(Color::Reset);
        Self {
            bg: bg("text"),
            surface: bg("popup"),
            surface_hover: bg("selection"),
            border: fg("border"),
            text: fg("text"),
            text_secondary: fg("text_dim"),
            red: fg("error"),
            accent: fg("accent"),
            chip_active_bg: bg("chip_active"),
            chip_active_fg: fg("chip_active"),
            chip_inactive_bg: bg("chip_idle"),
            chip_inactive_fg: fg("chip_idle"),
            badge_bg: bg("badge"),
            badge_fg: fg("badge"),
            live_badge_bg: bg("badge_live"),
            progress_track: fg("skeleton"),
            progress_fill: fg("progress"),
            styles,
        }
    }
    pub fn style(&self, role: &str) -> Style {
        self.styles.get(role).copied().unwrap_or_default()
    }
    pub fn parse(text: &str, caps: Capabilities) -> Result<Self> {
        let file: File = toml::from_str(text).context("Invalid theme TOML")?;
        let mut styles = Self::terminal().styles;
        for (role, spec) in file.roles {
            if !ROLES.contains(&role.as_str()) {
                bail!("Unknown semantic theme role: {role}");
            }
            let mut style = styles[&role];
            if let Some(fg) = spec.fg {
                style = style.fg(parse_color(&fg)?);
            }
            if let Some(bg) = spec.bg {
                style = style.bg(parse_color(&bg)?);
            }
            for m in spec.modifiers {
                style = style.add_modifier(match m.as_str() {
                    "bold" => Modifier::BOLD,
                    "dim" => Modifier::DIM,
                    "reversed" => Modifier::REVERSED,
                    "underlined" => Modifier::UNDERLINED,
                    _ => bail!("Unknown theme modifier"),
                });
            }
            styles.insert(role, style);
        }
        for style in styles.values_mut() {
            style.fg = style.fg.map(|c| downgrade(c, caps));
            style.bg = style.bg.map(|c| downgrade(c, caps));
        }
        Ok(Self::from_styles(styles))
    }
    pub fn resolve(config: &crate::config::Config, light: Option<bool>) -> Result<Self> {
        let name = if config.general.theme == "auto" {
            if light.unwrap_or(false) {
                &config.general.theme_light
            } else {
                &config.general.theme_dark
            }
        } else {
            &config.general.theme
        };
        if name == "terminal" {
            let mut t = Self::terminal();
            if Capabilities::environment().no_color {
                for s in t.styles.values_mut() {
                    s.fg = Some(Color::Reset);
                    s.bg = Some(Color::Reset);
                }
                return Ok(Self::from_styles(t.styles));
            }
            return Ok(t);
        }
        if name.contains('/') || name.contains('\\') || name.contains("..") {
            bail!("Theme name must be a simple filename");
        }
        let path = crate::config::config_dir()
            .join("themes")
            .join(format!("{name}.toml"));
        let text = if path.exists() {
            std::fs::read_to_string(path)?
        } else {
            match name.as_str() {
                "gruvbox-dark" => include_str!("../themes/gruvbox-dark.toml").into(),
                "gruvbox-light" => include_str!("../themes/gruvbox-light.toml").into(),
                "youtube" | "youtube-dark" => include_str!("../themes/youtube-dark.toml").into(),
                "youtube-light" => include_str!("../themes/gruvbox-light.toml").into(),
                _ => bail!("Theme not found: {name}"),
            }
        };
        Self::parse(&text, Capabilities::environment())
    }
    pub fn load(name: &str) -> Self {
        let mut c = crate::config::Config::default();
        c.general.theme = name.into();
        Self::resolve(&c, None).unwrap_or_else(|_| Self::terminal())
    }
    pub fn youtube_dark() -> Self {
        Self::load("youtube-dark")
    }
    pub fn youtube_light() -> Self {
        Self::load("gruvbox-light")
    }
    pub fn contrast_warnings(&self) -> Vec<String> {
        let lum = |v: u8| {
            let v = f64::from(v) / 255.;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        let mut warnings = vec![];
        for (role, s) in &self.styles {
            let ratio = if s.fg == s.bg && s.fg.is_some_and(|c| c != Color::Reset) {
                // The same palette entry has zero contrast even when its RGB is unknown.
                1.
            } else {
                let (Some(fg), Some(bg)) = (s.fg.and_then(rgb), s.bg.and_then(rgb)) else {
                    continue;
                };
                let l = |c: (u8, u8, u8)| 0.2126 * lum(c.0) + 0.7152 * lum(c.1) + 0.0722 * lum(c.2);
                let (a, b) = (l(fg), l(bg));
                (a.max(b) + 0.05) / (a.min(b) + 0.05)
            };
            let min = if ["border", "badge", "badge_live", "badge_mix", "skeleton"]
                .contains(&role.as_str())
            {
                3.
            } else {
                4.5
            };
            if ratio < min {
                warnings.push(format!("{role}: contrast {ratio:.2}:1 below {min}:1"));
            }
        }
        warnings
    }
    pub fn names() -> Vec<String> {
        let mut names = vec![
            "terminal".into(),
            "auto".into(),
            "gruvbox-dark".into(),
            "gruvbox-light".into(),
            "youtube-dark".into(),
        ];
        if let Ok(entries) = std::fs::read_dir(crate::config::config_dir().join("themes")) {
            for entry in entries.flatten() {
                if let Some(name) = entry.path().file_stem().and_then(|s| s.to_str()) {
                    names.push(name.into());
                }
            }
        }
        names.sort();
        names.dedup();
        names
    }
}
pub fn qr_style() -> Style {
    Style::default().fg(Color::Black).bg(Color::White)
}
