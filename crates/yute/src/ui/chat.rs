use crate::app::App;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

#[allow(dead_code)]
pub fn render_chat(f: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.theme.border))
        .title(" Live Chat ")
        .style(Style::default().bg(app.theme.surface));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let p = Paragraph::new("Live chat stream polling active...")
        .style(Style::default().fg(app.theme.text_secondary));
    f.render_widget(p, inner);
}
