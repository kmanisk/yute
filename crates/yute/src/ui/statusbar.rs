use crate::app::{App, Focus};
use crate::ui::text::DisplayWidth;
use ratatui::{layout::Rect, widgets::Paragraph, Frame};

pub fn render_statusbar(f: &mut Frame, app: &App, area: Rect) {
    if area.is_empty() {
        return;
    }
    if matches!(
        app.focus,
        Focus::CommandLine | Focus::SearchInput | Focus::CommentInput
    ) {
        super::cmdline::render_input(f, app, area);
        return;
    }
    let count = app.filtered_items().len();
    let pos = if count == 0 {
        0
    } else {
        (app.selected_index + 1).min(count)
    };
    let left = format!(" {pos}/{count}");
    let state = if let Some(error) = &app.feed_error {
        error.as_str()
    } else if app.is_loading_more {
        "Loading more…"
    } else if app.is_loading && !app.items.is_empty() {
        "Refreshing…"
    } else if app.is_loading {
        "Loading…"
    } else if app.status_text.starts_with("Loaded ") || app.status_text == "Ready" {
        ""
    } else {
        &app.status_text
    };
    let status = if state.is_empty() {
        "? help".into()
    } else {
        format!("{state} · ? help")
    };
    let right = status.as_str();
    let right = super::text::truncate_str(right, area.width.saturating_sub(2) as usize);
    let right_width = right.width().min(area.width.saturating_sub(2) as usize) as u16;
    let left_width = if right == "? help" {
        area.width.saturating_sub(right_width + 2)
    } else {
        (left.width() as u16).min(area.width / 2)
    };
    f.render_widget(
        Paragraph::new(left).style(app.theme.style("text_dim")),
        Rect::new(area.x, area.y, left_width, 1),
    );
    let right_area = Rect::new(
        area.x + left_width + 1,
        area.y,
        area.width.saturating_sub(left_width + 2),
        1,
    );
    f.render_widget(
        Paragraph::new(super::text::truncate_str(&right, right_area.width as usize))
            .alignment(ratatui::layout::Alignment::Right)
            .style(app.theme.style("text")),
        right_area,
    );
}
