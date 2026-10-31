use crate::{
    app::App,
    ui::{
        pane::Pane,
        text::{truncate_str, DisplayWidth},
    },
};
use ratatui::{
    layout::Rect,
    widgets::{Clear, Paragraph},
    Frame,
};
use yute_core::search::{Date, Duration, Feature, Kind, Options};
#[derive(Clone, Debug, Default)]
pub struct Modal {
    pub draft: Options,
    pub section: usize,
    pub cursor: [usize; 5],
}
pub const SECTIONS: [&str; 5] = ["Type", "Duration", "Upload date", "Features", "Prioritize"];
pub fn rows(section: usize) -> &'static [&'static str] {
    match section {
        0 => &["All", "Videos", "Channels", "Playlists", "Movies"],
        1 => &["Any", "Short (<4 min)", "Medium (4–20 min)", "Over 20 min"],
        2 => &["Any", "Today", "This week", "This month", "This year"],
        3 => &["Live", "4K", "HDR"],
        _ => &["Relevance", "Popularity"],
    }
}
impl Modal {
    pub fn choose(&mut self) {
        let i = self.cursor[self.section];
        match self.section {
            0 => {
                self.draft.kind = match i {
                    1 => Some(Kind::Video),
                    2 => Some(Kind::Channel),
                    3 => Some(Kind::Playlist),
                    4 => Some(Kind::Movie),
                    _ => None,
                }
            }
            1 => {
                self.draft.duration = match i {
                    1 => Some(Duration::Short),
                    2 => Some(Duration::Medium),
                    3 => Some(Duration::Long),
                    _ => None,
                }
            }
            2 => {
                self.draft.date = match i {
                    1 => Some(Date::Today),
                    2 => Some(Date::Week),
                    3 => Some(Date::Month),
                    4 => Some(Date::Year),
                    _ => None,
                }
            }
            3 => {
                let f = [Feature::Live, Feature::FourK, Feature::Hdr][i];
                if self.draft.features.contains(&f) {
                    self.draft.features.retain(|x| *x != f);
                } else {
                    self.draft.features.push(f);
                }
            }
            _ => self.draft.popularity = i == 1,
        }
    }
}
pub fn rect(size: Rect) -> Rect {
    let w = size.width.saturating_sub(4).min(130);
    let h = size.height.saturating_sub(2).min(14);
    Rect::new(
        size.x + (size.width - w) / 2,
        size.y + (size.height - h) / 2,
        w,
        h,
    )
}
pub fn render(f: &mut Frame, app: &App) {
    let Some(m) = &app.search_modal else {
        return;
    };
    let area = rect(f.area());
    f.render_widget(Clear, area);
    let inner = Pane::new("Search filters", true).render(f, area, &app.theme);
    if inner.height < 3 {
        return;
    }
    let wide = inner.width >= 90;
    if !wide {
        f.render_widget(
            Paragraph::new(truncate_str(&SECTIONS.join(" · "), inner.width as usize))
                .style(app.theme.style("accent")),
            Rect::new(inner.x, inner.y, inner.width, 1),
        );
    }
    for (section, label) in SECTIONS.iter().enumerate() {
        if !wide && section != m.section {
            continue;
        }
        let width = if wide { inner.width / 5 } else { inner.width };
        let x = if wide {
            inner.x + section as u16 * width
        } else {
            inner.x
        };
        let y = inner.y + u16::from(!wide);
        let available = Rect::new(x, y, width, inner.height.saturating_sub(2));
        f.render_widget(
            Paragraph::new(*label).style(app.theme.style("accent")),
            Rect::new(x, y, width, 1),
        );
        for (i, text) in rows(section)
            .iter()
            .enumerate()
            .take(available.height.saturating_sub(1) as usize)
        {
            let selected = section == m.section && i == m.cursor[section];
            let style = if selected {
                app.theme.style("selection")
            } else {
                app.theme.style("text")
            };
            f.render_widget(
                Paragraph::new(truncate_str(
                    &format!("{} {text}", if selected { ">" } else { " " }),
                    width as usize,
                ))
                .style(style),
                Rect::new(x, y + 1 + i as u16, width, 1),
            );
        }
    }
    let choices = m.draft.labels().join(" · ");
    f.render_widget(
        Paragraph::new(truncate_str(&choices, inner.width as usize))
            .style(app.theme.style("text_dim")),
        Rect::new(inner.x, inner.bottom() - 2, inner.width, 1),
    );
    let footer = "h/l section · Space select · x clear · a apply · Esc cancel";
    let _ = footer.width();
    f.render_widget(
        Paragraph::new(truncate_str(footer, inner.width as usize))
            .style(app.theme.style("action_hint")),
        Rect::new(inner.x, inner.bottom() - 1, inner.width, 1),
    );
}
pub fn active(app: &App) -> String {
    let (_, automatic) = app.search_options.biased(
        &app.search_query,
        &app.config.search,
        app.music_bias_disabled,
    );
    let mut labels = app.search_options.labels();
    if automatic {
        labels.push("auto: music 3–20m (M remove)".into());
    }
    if !labels.is_empty() {
        labels.push("Backspace remove · f filters".into());
    }
    labels.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn modal_fits_and_renders_at_requested_sizes() {
        for (w, h) in [(80, 24), (200, 60)] {
            let mut app = App::new(
                crate::config::Config::default(),
                ratatui_image::picker::Picker::halfblocks(),
            );
            app.search_modal = Some(Modal::default());
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(w, h)).unwrap();
            terminal.draw(|f| render(f, &app)).unwrap();
            let area = rect(Rect::new(0, 0, w, h));
            assert_eq!(terminal.backend().buffer()[(area.x, area.y)].symbol(), "┌");
            assert_eq!(
                terminal.backend().buffer()[(area.right() - 1, area.bottom() - 1)].symbol(),
                "┘"
            );
            let text = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect::<String>();
            assert!(text.contains("Videos"));
            assert!(text.contains("apply"));
        }
    }
    #[test]
    fn selecting_features_is_multichoice() {
        let mut m = Modal {
            section: 3,
            ..Default::default()
        };
        m.choose();
        m.cursor[3] = 1;
        m.choose();
        assert_eq!(m.draft.features.len(), 2);
        m.cursor[3] = 0;
        m.choose();
        assert_eq!(m.draft.features, vec![Feature::FourK]);
    }
}
