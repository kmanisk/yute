use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, CheckButton, DropDown, Label, Orientation, ScrolledWindow, StringList,
};
use std::cell::RefCell;
use std::rc::Rc;
use yute_core::config::Config;

pub struct SettingsViewWidget;

impl SettingsViewWidget {
    pub fn build(cfg: Rc<RefCell<Config>>) -> ScrolledWindow {
        let content_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(18)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(32)
            .margin_end(32)
            .hexpand(true)
            .build();

        // 1. Header
        let title_lbl = Label::builder()
            .label("Settings (SmartTube Engine & Preferences)")
            .halign(Align::Start)
            .css_classes(["title-1"])
            .build();
        content_box.append(&title_lbl);

        // Section: Playback & Resolution
        let pb_hdr = Label::builder()
            .label("Playback & Video Quality")
            .halign(Align::Start)
            .css_classes(["title-3"])
            .build();
        content_box.append(&pb_hdr);

        let pb_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .css_classes(["yt-desc-box"])
            .build();

        // Max Resolution
        let res_row = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(16)
            .build();
        let res_lbl = Label::builder()
            .label("Preferred Max Resolution")
            .halign(Align::Start)
            .hexpand(true)
            .build();
        let res_options =
            StringList::new(&["1080p", "1440p", "2160p (4K)", "720p", "480p", "360p"]);
        let res_dropdown = DropDown::new(Some(res_options), gtk4::Expression::NONE);

        let cur_res = cfg.borrow().playback.max_resolution.clone();
        let init_pos = match cur_res.as_str() {
            "1440p" => 1,
            "2160p" | "4k" => 2,
            "720p" => 3,
            "480p" => 4,
            "360p" => 5,
            _ => 0,
        };
        res_dropdown.set_selected(init_pos);

        let cfg_res = Rc::clone(&cfg);
        res_dropdown.connect_selected_notify(move |dd| {
            let selected = match dd.selected() {
                1 => "1440p",
                2 => "2160p",
                3 => "720p",
                4 => "480p",
                5 => "360p",
                _ => "1080p",
            };
            let mut conf = cfg_res.borrow_mut();
            conf.playback.max_resolution = selected.to_string();
            let _ = conf.save();
        });

        res_row.append(&res_lbl);
        res_row.append(&res_dropdown);
        pb_box.append(&res_row);

        // Hardware Decoding
        let hw_row = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(16)
            .build();
        let hw_lbl = Label::builder()
            .label("Hardware Decoding (hwdec)")
            .halign(Align::Start)
            .hexpand(true)
            .build();
        let hw_options = StringList::new(&["auto-safe", "vaapi", "nvdec", "cuda", "auto", "no"]);
        let hw_dropdown = DropDown::new(Some(hw_options), gtk4::Expression::NONE);

        let cur_hw = cfg.borrow().playback.hwdec.clone();
        let hw_init_pos = match cur_hw.as_str() {
            "vaapi" => 1,
            "nvdec" => 2,
            "cuda" => 3,
            "auto" => 4,
            "no" => 5,
            _ => 0,
        };
        hw_dropdown.set_selected(hw_init_pos);

        let cfg_hw = Rc::clone(&cfg);
        hw_dropdown.connect_selected_notify(move |dd| {
            let selected = match dd.selected() {
                1 => "vaapi",
                2 => "nvdec",
                3 => "cuda",
                4 => "auto",
                5 => "no",
                _ => "auto-safe",
            };
            let mut conf = cfg_hw.borrow_mut();
            conf.playback.hwdec = selected.to_string();
            let _ = conf.save();
        });

        hw_row.append(&hw_lbl);
        hw_row.append(&hw_dropdown);
        pb_box.append(&hw_row);

        // Mark Watched
        let mw_check = CheckButton::builder()
            .label("Mark watched videos in YouTube watch history")
            .active(cfg.borrow().playback.mark_watched)
            .build();
        let cfg_mw = Rc::clone(&cfg);
        mw_check.connect_toggled(move |btn| {
            let mut conf = cfg_mw.borrow_mut();
            conf.playback.mark_watched = btn.is_active();
            let _ = conf.save();
        });
        pb_box.append(&mw_check);

        content_box.append(&pb_box);

        // Section: SponsorBlock & RYD
        let sb_hdr = Label::builder()
            .label("SponsorBlock & Return YouTube Dislike (RYD)")
            .halign(Align::Start)
            .css_classes(["title-3"])
            .build();
        content_box.append(&sb_hdr);

        let sb_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .css_classes(["yt-desc-box"])
            .build();

        let sb_check = CheckButton::builder()
            .label("Enable SponsorBlock automatic segment skipping")
            .active(true)
            .build();
        sb_box.append(&sb_check);

        let ryd_check = CheckButton::builder()
            .label("Enable Return YouTube Dislike API stats")
            .active(true)
            .build();
        sb_box.append(&ryd_check);

        content_box.append(&sb_box);

        // Section: Network & Streaming Engine (Cronet Analysis)
        let net_hdr = Label::builder()
            .label("Streaming & Network Engine")
            .halign(Align::Start)
            .css_classes(["title-3"])
            .build();
        content_box.append(&net_hdr);

        let net_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .css_classes(["yt-desc-box"])
            .build();

        let net_note = Label::builder()
            .label(
                "Streaming Engine Architecture:\n\
                • SmartTube on Android uses Google Cronet or OkHttp for app-level chunk fetching.\n\
                • On Linux desktop, MPV uses FFmpeg's asynchronous caching demuxers (libavformat).\n\
                • Buffering and latency on Linux are governed by demuxer cache and readahead settings.",
            )
            .halign(Align::Start)
            .wrap(true)
            .css_classes(["dim-label"])
            .build();
        net_box.append(&net_note);

        let buf_row = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(16)
            .build();
        let buf_lbl = Label::builder()
            .label("Demuxer Memory Buffer Cache")
            .halign(Align::Start)
            .hexpand(true)
            .build();
        let buf_options = StringList::new(&["256 MiB (Default)", "512 MiB", "1024 MiB", "128 MiB"]);
        let buf_dropdown = DropDown::new(Some(buf_options), gtk4::Expression::NONE);
        buf_row.append(&buf_lbl);
        buf_row.append(&buf_dropdown);
        net_box.append(&buf_row);

        content_box.append(&net_box);

        // Section: Diagnostics & System Info
        let diag_hdr = Label::builder()
            .label("Diagnostics & Environment")
            .halign(Align::Start)
            .css_classes(["title-3"])
            .build();
        content_box.append(&diag_hdr);

        let diag_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .css_classes(["yt-desc-box"])
            .build();

        let os_lbl = Label::builder()
            .label("OS: CachyOS rolling (x86-64-v3, BORE/EEVDF)")
            .halign(Align::Start)
            .css_classes(["yt-meta"])
            .build();
        let hw_info = Label::builder()
            .label("Hardware: Intel Core i5-13450HX + NVIDIA RTX 5050 Mobile")
            .halign(Align::Start)
            .css_classes(["yt-meta"])
            .build();
        let renderer_lbl = Label::builder()
            .label("Video Pipeline: Embedded libmpv via GtkGLArea (OpenGL Render API)")
            .halign(Align::Start)
            .css_classes(["yt-meta"])
            .build();
        let config_lbl = Label::builder()
            .label(&format!(
                "Config Directory: {}",
                yute_core::config::config_dir().display()
            ))
            .halign(Align::Start)
            .css_classes(["yt-meta"])
            .build();

        diag_box.append(&os_lbl);
        diag_box.append(&hw_info);
        diag_box.append(&renderer_lbl);
        diag_box.append(&config_lbl);

        content_box.append(&diag_box);

        ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .child(&content_box)
            .build()
    }
}
