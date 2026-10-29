use crate::app::{App, FilterKind, Focus};
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn render_chips(f: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let mut spans = Vec::new();
    let mut end = 0u16;
    let mut active_end = 0u16;

    // Use FilterKind chips (All | Videos | Shorts | Live | Playlists | Channels)
    for filter in FilterKind::all() {
        let is_active = *filter == app.filter_kind;
        end += filter.label().len() as u16 + 3;
        if is_active {
            active_end = end;
        }

        let style = if is_active {
            app.theme
                .style("accent")
                .add_modifier(if app.focus == Focus::Chips {
                    Modifier::BOLD | Modifier::UNDERLINED
                } else {
                    Modifier::BOLD | Modifier::UNDERLINED
                })
        } else {
            app.theme.style("text_dim").add_modifier(Modifier::DIM)
        };

        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(filter.label(), style));
    }

    let p = Paragraph::new(Line::from(spans)).scroll((0, active_end.saturating_sub(area.width)));
    f.render_widget(p, area);
}
