use crate::{
    app::{App, Focus},
    commands,
};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

pub fn render_input(f: &mut Frame, app: &App, area: Rect) {
    let (mode, prefix, text) = if app.focus == Focus::CommentInput {
        (
            if app.comment_draft.as_ref().is_some_and(|d| d.reply) {
                "REPLY"
            } else {
                "COMMENT"
            },
            "",
            app.comment_input.as_str(),
        )
    } else if app.focus == Focus::CommandLine {
        ("COMMAND", ":", app.cmd_input.as_str())
    } else {
        ("SEARCH", "/", app.search_input.as_str())
    };
    let label = format!(" {mode}  {prefix}");
    let label_width = (label.width() as u16).min(area.width);
    f.render_widget(
        Paragraph::new(label).style(app.theme.style("accent").add_modifier(Modifier::BOLD)),
        Rect::new(area.x, area.y, label_width, area.height),
    );
    let input = Rect::new(
        area.x + label_width,
        area.y,
        area.width.saturating_sub(label_width),
        area.height,
    );
    let requested = app.input_cursor.unwrap_or(text.len()).min(text.len());
    let boundary = unicode_segmentation::UnicodeSegmentation::grapheme_indices(text, true)
        .map(|(offset, _)| offset)
        .chain(std::iter::once(text.len()))
        .take_while(|offset| *offset <= requested)
        .last()
        .unwrap_or(0);
    let cursor_width = text[..boundary].width();
    let scroll = cursor_width.saturating_sub(input.width.saturating_sub(1) as usize);
    f.render_widget(
        Paragraph::new(text)
            .style(app.theme.style("text"))
            .scroll((0, scroll.min(u16::MAX as usize) as u16)),
        input,
    );
    if !input.is_empty() {
        f.set_cursor_position((
            input.x + (cursor_width - scroll).min(input.width.saturating_sub(1) as usize) as u16,
            input.y,
        ));
    }
}

pub fn popup_area(app: &App, area: Rect) -> Option<Rect> {
    if app.which_key {
        let h = 7.min(area.height.saturating_sub(2));
        return Some(Rect::new(
            area.x,
            area.bottom().saturating_sub(h + 1),
            area.width.min(62),
            h,
        ));
    }
    if let Some(text) = &app.command_help {
        let width = area.width.saturating_sub(2).max(1);
        let lines = text
            .lines()
            .map(|line| line.width().max(1).div_ceil(width as usize))
            .sum::<usize>();
        let height = (lines + 2).min(area.height.saturating_sub(2) as usize) as u16;
        return Some(Rect::new(
            area.x + 1,
            area.bottom().saturating_sub(height + 1),
            width,
            height,
        ));
    }
    if app.focus != Focus::CommandLine || app.cmd_input.is_empty() {
        return None;
    }
    let choices = app.command_choices();
    if choices.is_empty() || (choices.len() == 1 && choices[0] == app.cmd_input) {
        return None;
    }
    let height = (choices.len().min(5) as u16 * 2 + 2).min(area.height.saturating_sub(2));
    Some(Rect::new(
        area.x + 1,
        area.bottom().saturating_sub(height + 1),
        area.width.saturating_sub(2).min(80),
        height,
    ))
}

pub fn render_cmdline(f: &mut Frame, app: &App) {
    if app.which_key {
        if let Some(rect) = app.overlay_area {
            f.render_widget(Clear, rect);
            let lines = crate::keymap::G_BINDINGS
                .chunks(2)
                .map(|pair| {
                    pair.iter()
                        .map(|(key, _, name)| format!("{key} {name:<22}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect::<Vec<_>>()
                .join("\n");
            f.render_widget(
                Paragraph::new(lines).style(app.theme.style("text").bg(app.theme.surface)),
                rect,
            );
        }
        return;
    }
    if let Some(text) = &app.command_help {
        let Some(rect) = app.overlay_area else {
            return;
        };
        f.render_widget(Clear, rect);
        f.render_widget(
            Paragraph::new(text.as_str())
                .wrap(ratatui::widgets::Wrap { trim: false })
                .style(app.theme.style("text").bg(app.theme.surface)),
            rect,
        );
        return;
    }
    if app.focus != Focus::CommandLine || app.cmd_input.is_empty() {
        return;
    }
    let choices = app.command_choices();
    let Some(rect) = app.overlay_area else {
        return;
    };
    if choices.is_empty() {
        return;
    }
    f.render_widget(Clear, rect);
    let block = Block::default()
        .title(" Commands ")
        .borders(Borders::ALL)
        .border_style(app.theme.style("accent"))
        .style(Style::default().bg(app.theme.surface));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    let rows = (inner.height / 2).max(1) as usize;
    let start = app.completion_index.saturating_sub(rows - 1);
    let mut lines = Vec::new();
    for (i, name) in choices.iter().enumerate().skip(start).take(rows) {
        let selected = i == app.completion_index;
        let style = if selected {
            app.theme.style("accent").add_modifier(Modifier::BOLD)
        } else {
            app.theme.style("text")
        };
        lines.push(Line::from(Span::styled(
            super::text::truncate_str(
                &format!(" {}{name}", if selected { "▸ " } else { "  " }),
                inner.width as usize,
            ),
            style,
        )));
        let description = commands::spec(name.split_whitespace().next().unwrap_or(name))
            .map_or("Account or command argument", |s| s.description);
        lines.push(Line::from(Span::styled(
            super::text::truncate_str(description, inner.width as usize),
            style.remove_modifier(Modifier::BOLD),
        )));
    }
    f.render_widget(Paragraph::new(lines), inner);
}
