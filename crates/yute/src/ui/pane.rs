use crate::theme::Theme;
use ratatui::{
    layout::{Margin, Rect},
    style::Modifier,
    widgets::{Block, Borders},
    Frame,
};
#[derive(Clone, Copy, Debug)]
pub struct Pane<'a> {
    pub title: &'a str,
    pub focused: bool,
}
impl<'a> Pane<'a> {
    pub fn new(title: &'a str, focused: bool) -> Self {
        Self { title, focused }
    }
    pub fn inner(self, area: Rect) -> Rect {
        area.inner(Margin::new(2, if area.height >= 10 { 2 } else { 1 }))
    }
    pub fn render(self, f: &mut Frame, area: Rect, theme: &Theme) -> Rect {
        let title = super::text::truncate_str(self.title, area.width.saturating_sub(6) as usize);
        let style = if self.focused {
            theme.style("border_focused")
        } else {
            theme.style("border").add_modifier(Modifier::DIM)
        };
        f.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {title} "))
                .border_style(style)
                .style(theme.style("popup")),
            area,
        );
        self.inner(area)
    }
}
