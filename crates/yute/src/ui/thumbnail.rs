//! Text overlays must opt back into Ratatui diffing after graphics mark cells Skip.
use ratatui::{buffer::CellDiffOption, layout::Rect, widgets::Paragraph, Frame};
use std::num::NonZeroU16;

pub fn visible(app: &crate::app::App, area: Rect) -> bool {
    !area.is_empty()
        && crate::terminal::images_enabled(&app.config.general.graphics)
        && !app
            .overlay_area
            .is_some_and(|overlay| overlay.intersects(area))
}

pub fn badge(f: &mut Frame, widget: Paragraph<'_>, area: Rect) {
    if area.is_empty() {
        return;
    }
    let prefix = f.buffer_mut().cell((area.x, area.y)).and_then(|cell| {
        let symbol = cell.symbol();
        // A tiny badge can cover the image's first cell, which owns the protocol
        // payload. Retain its escape sequences instead of deleting the image.
        symbol
            .rfind("\x1b\\")
            .map(|end| symbol[..end + 2].to_string())
    });
    f.render_widget(widget, area);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = f.buffer_mut().cell_mut((x, y)) {
                cell.set_diff_option(CellDiffOption::AlwaysUpdate);
            }
        }
    }
    if let Some(prefix) = prefix {
        if let Some(cell) = f.buffer_mut().cell_mut((area.x, area.y)) {
            let symbol = format!(
                "{prefix}\x1b[{};{}H{}",
                area.y + 1,
                area.x + 1,
                cell.symbol()
            );
            cell.set_symbol(&symbol)
                .set_diff_option(CellDiffOption::ForcedWidth(NonZeroU16::MIN));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, layout::Size, Terminal};
    use ratatui_image::{
        picker::{Picker, ProtocolType},
        Image, Resize,
    };
    #[test]
    fn sixel_badge_preserves_image_payload_and_is_not_skipped() {
        let mut picker = Picker::halfblocks();
        picker.set_protocol_type(ProtocolType::Sixel);
        let image = image::DynamicImage::new_rgb8(40, 40);
        let protocol = picker
            .new_protocol(image, Size::new(8, 4), Resize::Scale(None))
            .unwrap();
        let mut terminal = Terminal::new(TestBackend::new(12, 8)).unwrap();
        terminal
            .draw(|f| {
                f.render_widget(Image::new(&protocol), Rect::new(0, 0, 8, 4));
                badge(f, Paragraph::new("SHORTS"), Rect::new(0, 0, 6, 1));
                badge(f, Paragraph::new("1:20"), Rect::new(4, 3, 4, 1));
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert!(buffer[(0, 0)].symbol().contains("\x1bP"));
        assert!(buffer[(0, 0)].symbol().ends_with('S'));
        assert_ne!(buffer[(4, 3)].diff_option, CellDiffOption::Skip);
        assert_eq!(buffer[(4, 3)].symbol(), "1");
    }
}
