use crate::thumbnail_loader::ThumbnailLoader;
use gtk4::prelude::*;
use gtk4::{Align, AspectFrame, Box as GtkBox, FlowBoxChild, Label, Orientation, Overlay, Picture};
use yute_core::backend::models::{Card, LiveState};

pub struct VideoCardWidget;

impl VideoCardWidget {
    pub fn build(card: &Card, thumbnail_loader: &ThumbnailLoader, compact: bool) -> FlowBoxChild {
        let child = FlowBoxChild::builder()
            .can_focus(true)
            .focusable(true)
            .css_classes(if compact {
                vec!["yt-card-compact".to_string()]
            } else {
                vec!["yt-card".to_string()]
            })
            .hexpand(true)
            .build();

        if compact {
            // Horizontal layout for Related Videos sidebar
            let card_box = GtkBox::builder()
                .orientation(Orientation::Horizontal)
                .spacing(10)
                .margin_top(4)
                .margin_bottom(4)
                .margin_start(4)
                .margin_end(4)
                .hexpand(true)
                .build();

            match card {
                Card::Video(v) => {
                    let picture = Picture::builder()
                        .can_shrink(true)
                        .width_request(168)
                        .height_request(94)
                        .css_classes(["yt-thumb-container-compact"])
                        .build();

                    thumbnail_loader.load_thumbnail(&v.thumb_url, {
                        let pic = picture.clone();
                        move |texture| {
                            pic.set_paintable(Some(&texture));
                        }
                    });

                    let overlay = Overlay::builder()
                        .child(&picture)
                        .width_request(168)
                        .height_request(94)
                        .build();

                    if v.live == LiveState::Live {
                        let live_badge = Label::builder()
                            .label("LIVE")
                            .css_classes(["yt-live-badge"])
                            .halign(Align::End)
                            .valign(Align::End)
                            .margin_end(4)
                            .margin_bottom(4)
                            .build();
                        overlay.add_overlay(&live_badge);
                    } else if let Some(ref dur) = v.duration {
                        if !dur.is_empty() {
                            let dur_badge = Label::builder()
                                .label(dur)
                                .css_classes(["yt-duration-badge"])
                                .halign(Align::End)
                                .valign(Align::End)
                                .margin_end(4)
                                .margin_bottom(4)
                                .build();
                            overlay.add_overlay(&dur_badge);
                        }
                    }

                    card_box.append(&overlay);

                    let text_box = GtkBox::builder()
                        .orientation(Orientation::Vertical)
                        .spacing(3)
                        .hexpand(true)
                        .valign(Align::Start)
                        .build();

                    let title_lbl = Label::builder()
                        .label(&v.title)
                        .wrap(true)
                        .lines(2)
                        .ellipsize(gtk4::pango::EllipsizeMode::End)
                        .halign(Align::Start)
                        .css_classes(["yt-title-compact"])
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
                        .ellipsize(gtk4::pango::EllipsizeMode::End)
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
                        .width_request(168)
                        .height_request(94)
                        .css_classes(["yt-thumb-container-compact"])
                        .build();

                    if !p.thumb_url.is_empty() {
                        thumbnail_loader.load_thumbnail(&p.thumb_url, {
                            let pic = picture.clone();
                            move |texture| {
                                pic.set_paintable(Some(&texture));
                            }
                        });
                    }

                    let overlay = Overlay::builder()
                        .child(&picture)
                        .width_request(168)
                        .height_request(94)
                        .build();

                    let count_str = p.video_count.as_deref().unwrap_or("PLAYLIST");
                    let badge = Label::builder()
                        .label(count_str)
                        .css_classes(["yt-duration-badge"])
                        .halign(Align::End)
                        .valign(Align::End)
                        .margin_end(4)
                        .margin_bottom(4)
                        .build();
                    overlay.add_overlay(&badge);

                    card_box.append(&overlay);

                    let text_box = GtkBox::builder()
                        .orientation(Orientation::Vertical)
                        .spacing(3)
                        .hexpand(true)
                        .valign(Align::Start)
                        .build();

                    let title_lbl = Label::builder()
                        .label(&p.title)
                        .wrap(true)
                        .lines(2)
                        .ellipsize(gtk4::pango::EllipsizeMode::End)
                        .halign(Align::Start)
                        .css_classes(["yt-title-compact"])
                        .build();

                    let channel_lbl = Label::builder()
                        .label(&p.channel)
                        .halign(Align::Start)
                        .ellipsize(gtk4::pango::EllipsizeMode::End)
                        .css_classes(["yt-meta"])
                        .build();

                    text_box.append(&title_lbl);
                    text_box.append(&channel_lbl);
                    card_box.append(&text_box);
                }
                Card::Channel(c) => {
                    let title_lbl = Label::builder()
                        .label(&c.title)
                        .halign(Align::Start)
                        .css_classes(["yt-title-compact"])
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
        } else {
            // Full Grid Card Layout for Home / Search / Subscriptions / History
            let card_box = GtkBox::builder()
                .orientation(Orientation::Vertical)
                .spacing(10)
                .margin_top(6)
                .margin_bottom(10)
                .margin_start(6)
                .margin_end(6)
                .hexpand(true)
                .build();

            match card {
                Card::Video(v) => {
                    let picture = Picture::builder()
                        .can_shrink(true)
                        .hexpand(true)
                        .css_classes(["yt-thumb-container"])
                        .build();

                    thumbnail_loader.load_thumbnail(&v.thumb_url, {
                        let pic = picture.clone();
                        move |texture| {
                            pic.set_paintable(Some(&texture));
                        }
                    });

                    let aspect = AspectFrame::builder()
                        .xalign(0.5)
                        .yalign(0.5)
                        .ratio(16.0 / 9.0)
                        .obey_child(false)
                        .hexpand(true)
                        .child(&picture)
                        .build();

                    let overlay = Overlay::builder().child(&aspect).hexpand(true).build();

                    if v.live == LiveState::Live {
                        let live_badge = Label::builder()
                            .label("LIVE")
                            .css_classes(["yt-live-badge"])
                            .halign(Align::End)
                            .valign(Align::End)
                            .margin_end(8)
                            .margin_bottom(8)
                            .build();
                        overlay.add_overlay(&live_badge);
                    } else if let Some(ref dur) = v.duration {
                        if !dur.is_empty() {
                            let dur_badge = Label::builder()
                                .label(dur)
                                .css_classes(["yt-duration-badge"])
                                .halign(Align::End)
                                .valign(Align::End)
                                .margin_end(8)
                                .margin_bottom(8)
                                .build();
                            overlay.add_overlay(&dur_badge);
                        }
                    }

                    card_box.append(&overlay);

                    // Row beneath thumbnail: Avatar circle + Video info
                    let info_row = GtkBox::builder()
                        .orientation(Orientation::Horizontal)
                        .spacing(12)
                        .hexpand(true)
                        .build();

                    // Channel Avatar circle
                    let initial = v
                        .channel
                        .chars()
                        .next()
                        .unwrap_or('Y')
                        .to_uppercase()
                        .to_string();
                    let avatar = Label::builder()
                        .label(&initial)
                        .width_request(36)
                        .height_request(36)
                        .halign(Align::Center)
                        .valign(Align::Start)
                        .css_classes(["yt-avatar-circle"])
                        .build();
                    info_row.append(&avatar);

                    let text_box = GtkBox::builder()
                        .orientation(Orientation::Vertical)
                        .spacing(3)
                        .hexpand(true)
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

                    info_row.append(&text_box);
                    card_box.append(&info_row);
                }
                Card::Playlist(p) => {
                    let picture = Picture::builder()
                        .can_shrink(true)
                        .hexpand(true)
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

                    let aspect = AspectFrame::builder()
                        .xalign(0.5)
                        .yalign(0.5)
                        .ratio(16.0 / 9.0)
                        .obey_child(false)
                        .hexpand(true)
                        .child(&picture)
                        .build();

                    let overlay = Overlay::builder().child(&aspect).hexpand(true).build();

                    let count_str = p.video_count.as_deref().unwrap_or("PLAYLIST");
                    let badge = Label::builder()
                        .label(&format!("PLAYLIST • {count_str}"))
                        .css_classes(["yt-duration-badge"])
                        .halign(Align::End)
                        .valign(Align::End)
                        .margin_end(8)
                        .margin_bottom(8)
                        .build();
                    overlay.add_overlay(&badge);

                    card_box.append(&overlay);

                    let text_box = GtkBox::builder()
                        .orientation(Orientation::Vertical)
                        .spacing(3)
                        .hexpand(true)
                        .build();

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

                    text_box.append(&title_lbl);
                    text_box.append(&channel_lbl);
                    card_box.append(&text_box);
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
}
