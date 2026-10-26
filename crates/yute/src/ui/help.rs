use crate::app::App;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

const ENTRIES: &[(&str, &str)] = &[
    ("Navigation", ""),
    ("Alt-e", "Show and focus sidebar; press again to hide"),
    ("Enter", "Open/play selected entity"),
    ("Shift-Enter", "Item actions: play, copy, save, download"),
    ("arrows", "Move between cards"),
    ("Ctrl-d/u, Ctrl-f/b", "Half/full page"),
    ("Tab / Shift-Tab", "Cycle pane focus"),
    ("Other panes", ""),
    ("Settings: j/k", "Choose setting; h/l/Enter changes it"),
    ("Account", "Session details; :login / :logout"),
    (
        "Command: :",
        "Type commands; Tab completes; Up/Down history",
    ),
    (
        ":help <command>",
        "Command syntax, description and examples",
    ),
    ("q / Esc", "Quit / previous route"),
    ("Ctrl-c / Ctrl-q", "Quit immediately"),
    ("Help: j/k, gg/G", "Scroll help; ? or Esc closes"),
];
pub const HELP_LINES: usize = ENTRIES.len()
    + crate::keymap::BINDINGS.len()
    + crate::keymap::G_BINDINGS.len()
    + crate::keymap::WATCH_BINDINGS.len()
    + 1;

pub fn render_help(f: &mut Frame, app: &App, area: Rect) {
    let popup_w = 70u16.min(area.width.saturating_sub(4));
    let popup_h = 24u16.min(area.height.saturating_sub(2));

    let popup_x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_h)) / 2;

    let popup_rect = Rect::new(popup_x, popup_y, popup_w, popup_h);
    f.render_widget(Clear, popup_rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.accent))
        .title(format!(
            " {:?} · Help ",
            app.nav_history
                .last()
                .map_or(app.screen, |route| route.screen)
        ))
        .style(Style::default().bg(app.theme.surface));

    let inner = block.inner(popup_rect);
    f.render_widget(block, popup_rect);

    let mut lines = Vec::new();
    let generated = crate::keymap::BINDINGS
        .iter()
        .map(|(key, _, description)| (key.to_string(), *description))
        .chain(
            crate::keymap::G_BINDINGS
                .iter()
                .map(|(key, _, description)| (format!("g{key}"), *description)),
        );
    let watch = std::iter::once(("Watch: video details".to_string(), "")).chain(
        crate::keymap::WATCH_BINDINGS
            .iter()
            .map(|(keys, _, description, _)| {
                (
                    keys.chars()
                        .map(|c| {
                            if c == ' ' {
                                "Space".to_string()
                            } else {
                                c.to_string()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("/"),
                    *description,
                )
            }),
    );
    let entries = generated
        .chain(watch)
        .chain(ENTRIES.iter().map(|(key, desc)| (key.to_string(), *desc)));
    for (k, desc) in entries {
        lines.push(Line::from(vec![
            Span::styled(
                format!(" {k:<22}"),
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(desc, Style::default().fg(app.theme.text)),
        ]));
    }

    let max_scroll = HELP_LINES.saturating_sub(inner.height as usize) as u16;
    f.render_widget(
        Paragraph::new(lines).scroll((app.help_scroll.min(max_scroll), 0)),
        inner,
    );
}
