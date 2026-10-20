use crate::thumbnail_loader::ThumbnailLoader;
use gtk4::prelude::*;
use gtk4::{Align, Box as GtkBox, FlowBoxChild, Label, Orientation, Overlay, Picture};
use ytui_core::backend::models::{Card, LiveState};

pub struct VideoCardWidget;

impl VideoCardWidget {
    pub fn build(
        card: &Card,
        thumbnail_loader: &ThumbnailLoader,
        compact: bool,
    ) -> FlowBoxChild {
        let child = FlowBoxChild::builder()
            .can_focus(true)
            .focusable(true)
            .css_classes(["yt-card"])
            .build();

        let card_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .margin_top(4)
            .margin_bottom(6)
            .margin_start(4)
            .margin_end(4)
            .build();

        let (thumb_w, thumb_h) = if compact {
            (240, 135)
        } else {
            (300, 169)
        };

        match card {
            Card::Video(v) => {
                // 1. Thumbnail with 16:9 aspect ratio and duration overlay
                let picture = Picture::builder()
                    .can_shrink(true)
                    .keep_aspect_ratio(true)
                    .width_request(thumb_w)
                    .height_request(thumb_h)
                    .css_classes(["yt-thumb-container"])
                    .build();

                thumbnail_loader.load_thumbnail(&v.thumb_url, {
                    let pic = picture.clone();
                    move |texture| {
                        pic.set_paintable(Some(&texture));
                    }
                });

                let overlay = Overlay::builder().child(&picture).build();

                // Duration / Live Badge
                if v.live == LiveState::Live {
                    let live_badge = Label::builder()
                        .label("LIVE")
                        .css_classes(["yt-live-badge"])
                        .halign(Align::End)
                        .valign(Align::End)
                        .margin_end(6)
                        .margin_bottom(6)
                        .build();
                    overlay.add_overlay(&live_badge);
                } else if let Some(ref dur) = v.duration {
                    if !dur.is_empty() {
                        let dur_badge = Label::builder()
                            .label(dur)
                            .css_classes(["yt-duration-badge"])
                            .halign(Align::End)
                            .valign(Align::End)
                            .margin_end(6)
                            .margin_bottom(6)
                            .build();
                        overlay.add_overlay(&dur_badge);
                    }
                }

                card_box.append(&overlay);

                // 2. Metadata Section
                let text_box = GtkBox::builder()
                    .orientation(Orientation::Vertical)
                    .spacing(2)
                    .width_request(thumb_w)
                    .build();

                let mut title_prefix = String::new();
                if v.badges.iter().any(|b| b.eq_ignore_ascii_case("shorts")) {
                    title_prefix.push_str("[SHORTS] ");
                }

                let title_lbl = Label::builder()
                    .label(&format!("{title_prefix}{}", v.title))
                    .wrap(true)
                    .lines(2)
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .halign(Align::Start)
                    .css_classes(["yt-title"])
                    .build();

                let channel_lbl = Label::builder()
                    .label(&v.channel)
                    .halign(Align::Start)
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .css_classes(["yt-meta"])
                    .build();

                let mut meta_parts = Vec::new();
                if let Some(ref views) = v.views {
                    if !views.is_empty() {
                        meta_parts.push(views.clone());
                    }
                }
                if let Some(ref pub_time) = v.published {
                    if !pub_time.is_empty() {
                        meta_parts.push(pub_time.clone());
                    }
                }

                let meta_lbl = Label::builder()
                    .label(&meta_parts.join(" • "))
                    .halign(Align::Start)
                    .css_classes(["yt-meta"])
                    .build();

                text_box.append(&title_lbl);
                text_box.append(&channel_lbl);
                if !meta_parts.is_empty() {
                    text_box.append(&meta_lbl);
                }

                card_box.append(&text_box);
            }
            Card::Playlist(p) => {
                let picture = Picture::builder()
                    .can_shrink(true)
                    .keep_aspect_ratio(true)
                    .width_request(thumb_w)
                    .height_request(thumb_h)
                    .css_classes(["yt-thumb-container"])
                    .build();

                if !p.thumb_url.is_empty() {
                    thumbnail_loader.load_thumbnail(&p.thumb_url, {
                        let pic = picture.clone();
                        move |texture| {
                            pic.set_paintable(Some(&texture));
                        }
                    });
                }

                let overlay = Overlay::builder().child(&picture).build();
                let count_str = p.video_count.as_deref().unwrap_or("PLAYLIST");
                let badge = Label::builder()
                    .label(&format!("PLAYLIST • {count_str}"))
                    .css_classes(["yt-duration-badge"])
                    .halign(Align::End)
                    .valign(Align::End)
                    .margin_end(6)
                    .margin_bottom(6)
                    .build();
                overlay.add_overlay(&badge);

                card_box.append(&overlay);

                let title_lbl = Label::builder()
                    .label(&p.title)
                    .wrap(true)
                    .lines(2)
                    .halign(Align::Start)
                    .css_classes(["yt-title"])
                    .build();

                let channel_lbl = Label::builder()
                    .label(&p.channel)
                    .halign(Align::Start)
                    .css_classes(["yt-meta"])
                    .build();

                card_box.append(&title_lbl);
                card_box.append(&channel_lbl);
            }
            Card::Channel(c) => {
                let title_lbl = Label::builder()
                    .label(&format!("[CHANNEL] {}", c.title))
                    .halign(Align::Start)
                    .css_classes(["yt-title"])
                    .build();
                let sub_lbl = Label::builder()
                    .label(c.subscribers.as_deref().unwrap_or("Channel"))
                    .halign(Align::Start)
                    .css_classes(["yt-meta"])
                    .build();

                card_box.append(&title_lbl);
                card_box.append(&sub_lbl);
            }
        }

        child.set_child(Some(&card_box));
        child
    }
}
