//! Persist final protocol cells. Kitty's transmission is separated from reusable
//! Unicode placements so its payload is sent once per image in each process.
use ratatui::{
    buffer::Buffer,
    layout::{Rect, Size},
    widgets::Widget,
};
use ratatui_image::protocol::Protocol;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(Serialize, Deserialize)]
struct Cached {
    version: u8,
    buffer: Buffer,
    transmit: String,
}
pub struct EncodedCover {
    cached: Cached,
    transmitted: AtomicBool,
}
impl EncodedCover {
    pub fn invalidate(&self) {
        self.transmitted.store(false, Ordering::SeqCst);
    }
    pub fn new(protocol: &Protocol, size: Size) -> Self {
        let mut buffer = Buffer::empty(Rect::new(0, 0, size.width, size.height));
        ratatui_image::Image::new(protocol).render(buffer.area, &mut buffer);
        let mut transmit = String::new();
        if matches!(protocol, Protocol::Kitty(_)) {
            let first = &mut buffer[(0, 0)];
            if let Some(index) = first.symbol().find('\u{10eeee}') {
                transmit = first.symbol()[..index].to_owned();
                let symbol = first.symbol()[index..].to_owned();
                first.set_symbol(&symbol);
            }
        }
        Self {
            cached: Cached {
                version: 1,
                buffer,
                transmit,
            },
            transmitted: AtomicBool::new(false),
        }
    }
    pub fn load(path: &std::path::Path, size: Size) -> Option<Self> {
        let metadata = std::fs::metadata(path).ok()?;
        if metadata.len() > 8 * 1024 * 1024 {
            return None;
        }
        let cached: Cached = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        if cached.buffer.area.as_size() != size
            || cached.version != 1
            || cached.buffer.area.x != 0
            || cached.buffer.area.y != 0
            || cached.buffer.content.len() != cached.buffer.area.area() as usize
        {
            return None;
        }
        yute_core::media::thumbs::touch(path);
        Some(Self {
            cached,
            transmitted: AtomicBool::new(false),
        })
    }
    pub fn save(&self, path: &std::path::Path) {
        if let Ok(bytes) = serde_json::to_vec(&self.cached) {
            let _ = yute_core::credentials::atomic_write(path, &bytes);
        }
    }
    fn can_crop(&self) -> bool {
        !self.cached.transmit.is_empty()
            || self
                .cached
                .buffer
                .content
                .iter()
                .all(|cell| !cell.symbol().contains('\x1b'))
    }
}
impl Widget for &EncodedCover {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let size = self.cached.buffer.area;
        if area.is_empty()
            || ((area.width < size.width || area.height < size.height) && !self.can_crop())
        {
            return;
        }
        for y in 0..size.height.min(area.height) {
            for x in 0..size.width.min(area.width) {
                if let Some(target) = buffer.cell_mut((area.x + x, area.y + y)) {
                    *target = self.cached.buffer[(x, y)].clone();
                }
            }
        }
        if !self.cached.transmit.is_empty() && !self.transmitted.swap(true, Ordering::SeqCst) {
            let cell = &mut buffer[(area.x, area.y)];
            let symbol = format!("{}{}", self.cached.transmit, cell.symbol());
            cell.set_symbol(&symbol);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cropped_cells_stay_inside_the_viewport() {
        for kitty in [false, true] {
            let mut picker = ratatui_image::picker::Picker::halfblocks();
            if kitty {
                picker.set_protocol_type(ratatui_image::picker::ProtocolType::Kitty);
            }
            let size = Size::new(8, 4);
            let protocol = picker
                .new_protocol(
                    image::DynamicImage::new_rgb8(80, 40),
                    size,
                    ratatui_image::Resize::Crop(None),
                )
                .unwrap();
            let cover = EncodedCover::new(&protocol, size);
            let mut buffer = Buffer::empty(Rect::new(0, 0, 12, 6));
            (&cover).render(Rect::new(2, 1, 6, 2), &mut buffer);
            assert_eq!(buffer[(3, 1)], cover.cached.buffer[(1, 0)]);
            if kitty {
                assert!(buffer[(2, 1)].symbol().contains("\x1b_G"));
            }
            assert_eq!(buffer[(8, 1)].symbol(), " ");
            assert_eq!(buffer[(2, 3)].symbol(), " ");
        }
    }
    #[test]
    fn kitty_payload_survives_disk_cache_and_transmits_only_once() {
        let mut picker = ratatui_image::picker::Picker::halfblocks();
        picker.set_protocol_type(ratatui_image::picker::ProtocolType::Kitty);
        let size = Size::new(8, 4);
        let protocol = picker
            .new_protocol(
                image::DynamicImage::new_rgb8(80, 40),
                size,
                ratatui_image::Resize::Crop(None),
            )
            .unwrap();
        let cover = EncodedCover::new(&protocol, size);
        let mut first = Buffer::empty(Rect::new(0, 0, 8, 4));
        (&cover).render(first.area, &mut first);
        assert!(first[(0, 0)].symbol().contains("\x1b_G"));
        let mut second = Buffer::empty(first.area);
        (&cover).render(second.area, &mut second);
        assert!(!second[(0, 0)].symbol().contains("\x1b_G"));
        assert!(second[(0, 0)].symbol().contains('\u{10eeee}'));
        // Round-trip serialization resets process-local transmission state.
        let cached =
            serde_json::from_slice::<Cached>(&serde_json::to_vec(&cover.cached).unwrap()).unwrap();
        let restored = EncodedCover {
            cached,
            transmitted: AtomicBool::new(false),
        };
        let mut third = Buffer::empty(first.area);
        (&restored).render(third.area, &mut third);
        assert!(third[(0, 0)].symbol().contains("\x1b_G"));
    }
}
