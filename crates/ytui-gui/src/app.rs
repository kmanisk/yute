use crate::player_embedded::EmbeddedPlayer;
use crate::video_area::VideoArea;
use gtk4::gdk::Key;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry, EventControllerKey,
    GestureClick, Label, Orientation, Paned, PolicyType, Scale, ScrolledWindow, Stack,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use ytui_core::backend::auth::AuthContext;
use ytui_core::backend::innertube::InnerTubeBackend;
use ytui_core::backend::models::{parse_duration_to_seconds, Card, SearchFilters};
use ytui_core::backend::Backend;
use ytui_core::config::Config;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurrentView {
    Home,
    Subscriptions,
    History,
    Search,
    Watch,
}

pub struct AppState {
    pub view: CurrentView,
    #[allow(dead_code)]
    pub is_loading: bool,
    pub is_fullscreen: bool,
    pub is_theater: bool,
}

pub struct GuiApp {
    window: ApplicationWindow,
    player: Arc<EmbeddedPlayer>,
    backend: Arc<dyn Backend>,
    state: Rc<RefCell<AppState>>,
    video_area: VideoArea,
    // UI elements
    main_stack: Stack,
    feed_flow_box: GtkBox,
    feed_scrolled: ScrolledWindow,
    search_entry: Entry,
    status_label: Label,
    // Watch view elements
    watch_title: Label,
    watch_channel: Label,
    watch_meta: Label,
    watch_desc_box: GtkBox,
    watch_comments_box: GtkBox,
    time_label: Label,
    seek_scale: Scale,
    play_pause_btn: Button,
    watch_paned: Paned,
}

impl GuiApp {
    pub fn new(app: &Application, cfg: Config) -> anyhow::Result<Self> {
        let auth_path = ytui_core::config::config_dir().join("auth.json");
        let auth = AuthContext::load_from_file(&auth_path).unwrap_or_default();
        let backend: Arc<dyn Backend> = Arc::new(InnerTubeBackend::new(auth));

        let player = Arc::new(EmbeddedPlayer::new(&cfg)?);
        let video_area = VideoArea::new(Arc::clone(&player));

        let window = ApplicationWindow::builder()
            .application(app)
            .title("ytui - YouTube Client")
            .default_width(1280)
            .default_height(800)
            .build();

        let state = Rc::new(RefCell::new(AppState {
            view: CurrentView::Home,
            is_loading: false,
            is_fullscreen: false,
            is_theater: false,
        }));

        let search_entry = Entry::builder()
            .placeholder_text("Search YouTube (Press / or Ctrl+L to focus)...")
            .hexpand(true)
            .build();

        let status_label = Label::builder().label("Ready").halign(Align::End).build();

        let feed_flow_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(12)
            .margin_end(12)
            .build();

        let feed_scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .child(&feed_flow_box)
            .build();

        let main_stack = Stack::builder().hexpand(true).vexpand(true).build();

        // Watch view elements
        let watch_title = Label::builder()
            .wrap(true)
            .halign(Align::Start)
            .css_classes(["title-2"])
            .build();

        let watch_channel = Label::builder()
            .halign(Align::Start)
            .css_classes(["dim-label"])
            .build();

        let watch_meta = Label::builder()
            .halign(Align::Start)
            .css_classes(["dim-label"])
            .build();

        let watch_desc_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .build();

        let watch_comments_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .build();

        let time_label = Label::builder().label("0:00 / 0:00").build();

        let seek_scale = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
        seek_scale.set_hexpand(true);

        let play_pause_btn = Button::with_label("Play");

        let watch_paned = Paned::builder()
            .orientation(Orientation::Horizontal)
            .hexpand(true)
            .vexpand(true)
            .build();

        let gui_app = Self {
            window,
            player,
            backend,
            state,
            video_area,
            main_stack,
            feed_flow_box,
            feed_scrolled,
            search_entry,
            status_label,
            watch_title,
            watch_channel,
            watch_meta,
            watch_desc_box,
            watch_comments_box,
            time_label,
            seek_scale,
            play_pause_btn,
            watch_paned,
        };

        gui_app.build_ui();
        gui_app.setup_shortcuts();
        gui_app.setup_polling_timer();
        gui_app.load_home_feed();

        Ok(gui_app)
    }

    fn build_ui(&self) {
        let root_box = GtkBox::builder().orientation(Orientation::Vertical).build();

        // 1. Top Header Bar
        let header_bar = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(12)
            .margin_end(12)
            .build();

        let home_btn = Button::with_label("Home");
        let subs_btn = Button::with_label("Subscriptions");
        let hist_btn = Button::with_label("History");

        header_bar.append(&home_btn);
        header_bar.append(&subs_btn);
        header_bar.append(&hist_btn);
        header_bar.append(&self.search_entry);
        header_bar.append(&self.status_label);

        root_box.append(&header_bar);

        // 2. Feed Page Container
        self.main_stack.add_named(&self.feed_scrolled, Some("feed"));

        // 3. Watch Page Container
        let watch_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .margin_start(12)
            .margin_end(12)
            .margin_bottom(8)
            .build();

        let video_container = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();
        video_container.append(&self.video_area.widget);

        // Controls bar under video
        let controls_bar = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .margin_top(4)
            .build();

        let seek_back_btn = Button::with_label("-5s");
        let seek_fwd_btn = Button::with_label("+5s");
        let fs_btn = Button::with_label("Fullscreen");
        let theater_btn = Button::with_label("Theater");

        controls_bar.append(&self.play_pause_btn);
        controls_bar.append(&seek_back_btn);
        controls_bar.append(&seek_fwd_btn);
        controls_bar.append(&self.seek_scale);
        controls_bar.append(&self.time_label);
        controls_bar.append(&theater_btn);
        controls_bar.append(&fs_btn);

        video_container.append(&controls_bar);

        // Metadata box
        let meta_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .margin_top(6)
            .build();
        meta_box.append(&self.watch_title);
        meta_box.append(&self.watch_channel);
        meta_box.append(&self.watch_meta);

        let left_video_pane = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();
        left_video_pane.append(&video_container);
        left_video_pane.append(&meta_box);

        // Right details pane (Description + Comments)
        let right_details_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .width_request(400)
            .margin_start(8)
            .build();

        let desc_label_hdr = Label::builder()
            .label("Description")
            .halign(Align::Start)
            .css_classes(["heading"])
            .build();
        right_details_box.append(&desc_label_hdr);

        let desc_scroll = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .height_request(240)
            .child(&self.watch_desc_box)
            .build();
        right_details_box.append(&desc_scroll);

        let comments_label_hdr = Label::builder()
            .label("Comments")
            .halign(Align::Start)
            .css_classes(["heading"])
            .margin_top(8)
            .build();
        right_details_box.append(&comments_label_hdr);

        let comments_scroll = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand(true)
            .child(&self.watch_comments_box)
            .build();
        right_details_box.append(&comments_scroll);

        self.watch_paned.set_start_child(Some(&left_video_pane));
        self.watch_paned.set_end_child(Some(&right_details_box));
        self.watch_paned.set_position(820);

        watch_box.append(&self.watch_paned);
        self.main_stack.add_named(&watch_box, Some("watch"));

        root_box.append(&self.main_stack);
        self.window.set_child(Some(&root_box));

        // Connect Navigation Buttons
        let b_home = Arc::clone(&self.backend);
        let fb_home = self.feed_flow_box.clone();
        let p_home = Arc::clone(&self.player);
        let s_home = self.main_stack.clone();
        let st_home = Rc::clone(&self.state);
        let stat_home = self.status_label.clone();
        let wt_h = self.watch_title.clone();
        let wc_h = self.watch_channel.clone();
        let wm_h = self.watch_meta.clone();
        let wd_h = self.watch_desc_box.clone();
        let wcomm_h = self.watch_comments_box.clone();

        home_btn.connect_clicked(move |_| {
            st_home.borrow_mut().view = CurrentView::Home;
            s_home.set_visible_child_name("feed");
            stat_home.set_label("Loading Home...");
            let b = Arc::clone(&b_home);
            let fb = fb_home.clone();
            let p = Arc::clone(&p_home);
            let s = s_home.clone();
            let st = Rc::clone(&st_home);
            let stat = stat_home.clone();
            let wt = wt_h.clone();
            let wc = wc_h.clone();
            let wm = wm_h.clone();
            let wd = wd_h.clone();
            let wcomm = wcomm_h.clone();

            glib::MainContext::default().spawn_local(async move {
                if let Ok(page) = b.home(None).await {
                    Self::populate_feed(&fb, page.items, b, p, s, st, wt, wc, wm, wd, wcomm);
                    stat.set_label("Home");
                }
            });
        });

        let b_subs = Arc::clone(&self.backend);
        let fb_subs = self.feed_flow_box.clone();
        let p_subs = Arc::clone(&self.player);
        let s_subs = self.main_stack.clone();
        let st_subs = Rc::clone(&self.state);
        let stat_subs = self.status_label.clone();
        let wt_s = self.watch_title.clone();
        let wc_s = self.watch_channel.clone();
        let wm_s = self.watch_meta.clone();
        let wd_s = self.watch_desc_box.clone();
        let wcomm_s = self.watch_comments_box.clone();

        subs_btn.connect_clicked(move |_| {
            st_subs.borrow_mut().view = CurrentView::Subscriptions;
            s_subs.set_visible_child_name("feed");
            stat_subs.set_label("Loading Subscriptions...");
            let b = Arc::clone(&b_subs);
            let fb = fb_subs.clone();
            let p = Arc::clone(&p_subs);
            let s = s_subs.clone();
            let st = Rc::clone(&st_subs);
            let stat = stat_subs.clone();
            let wt = wt_s.clone();
            let wc = wc_s.clone();
            let wm = wm_s.clone();
            let wd = wd_s.clone();
            let wcomm = wcomm_s.clone();

            glib::MainContext::default().spawn_local(async move {
                if let Ok(page) = b.subscriptions().await {
                    Self::populate_feed(&fb, page.items, b, p, s, st, wt, wc, wm, wd, wcomm);
                    stat.set_label("Subscriptions");
                }
            });
        });

        let b_hist = Arc::clone(&self.backend);
        let fb_hist = self.feed_flow_box.clone();
        let p_hist = Arc::clone(&self.player);
        let s_hist = self.main_stack.clone();
        let st_hist = Rc::clone(&self.state);
        let stat_hist = self.status_label.clone();
        let wt_hi = self.watch_title.clone();
        let wc_hi = self.watch_channel.clone();
        let wm_hi = self.watch_meta.clone();
        let wd_hi = self.watch_desc_box.clone();
        let wcomm_hi = self.watch_comments_box.clone();

        hist_btn.connect_clicked(move |_| {
            st_hist.borrow_mut().view = CurrentView::History;
            s_hist.set_visible_child_name("feed");
            stat_hist.set_label("Loading History...");
            let b = Arc::clone(&b_hist);
            let fb = fb_hist.clone();
            let p = Arc::clone(&p_hist);
            let s = s_hist.clone();
            let st = Rc::clone(&st_hist);
            let stat = stat_hist.clone();
            let wt = wt_hi.clone();
            let wc = wc_hi.clone();
            let wm = wm_hi.clone();
            let wd = wd_hi.clone();
            let wcomm = wcomm_hi.clone();

            glib::MainContext::default().spawn_local(async move {
                if let Ok(page) = b.history().await {
                    Self::populate_feed(&fb, page.items, b, p, s, st, wt, wc, wm, wd, wcomm);
                    stat.set_label("History");
                }
            });
        });

        // Search submit
        let b_search = Arc::clone(&self.backend);
        let fb_search = self.feed_flow_box.clone();
        let p_search = Arc::clone(&self.player);
        let s_search = self.main_stack.clone();
        let st_search = Rc::clone(&self.state);
        let stat_search = self.status_label.clone();
        let wt_se = self.watch_title.clone();
        let wc_se = self.watch_channel.clone();
        let wm_se = self.watch_meta.clone();
        let wd_se = self.watch_desc_box.clone();
        let wcomm_se = self.watch_comments_box.clone();

        self.search_entry.connect_activate(move |entry| {
            let q = entry.text().trim().to_string();
            if q.is_empty() {
                return;
            }
            st_search.borrow_mut().view = CurrentView::Search;
            s_search.set_visible_child_name("feed");
            stat_search.set_label(&format!("Searching '{q}'..."));

            let b = Arc::clone(&b_search);
            let fb = fb_search.clone();
            let p = Arc::clone(&p_search);
            let s = s_search.clone();
            let st = Rc::clone(&st_search);
            let stat = stat_search.clone();
            let wt = wt_se.clone();
            let wc = wc_se.clone();
            let wm = wm_se.clone();
            let wd = wd_se.clone();
            let wcomm = wcomm_se.clone();

            glib::MainContext::default().spawn_local(async move {
                let filters = SearchFilters::default();
                if let Ok(page) = b.search(&q, &filters, None).await {
                    Self::populate_feed(&fb, page.items, b, p, s, st, wt, wc, wm, wd, wcomm);
                    stat.set_label(&format!("Results for '{q}'"));
                }
            });
        });

        // Connect Playback Buttons
        let p_toggle = Arc::clone(&self.player);
        self.play_pause_btn.connect_clicked(move |_| {
            let _ = p_toggle.toggle_pause();
        });

        let p_back = Arc::clone(&self.player);
        seek_back_btn.connect_clicked(move |_| {
            let _ = p_back.seek(-5.0, true);
        });

        let p_fwd = Arc::clone(&self.player);
        seek_fwd_btn.connect_clicked(move |_| {
            let _ = p_fwd.seek(5.0, true);
        });

        let p_seek = Arc::clone(&self.player);
        self.seek_scale.connect_value_changed(move |scale| {
            let val = scale.value();
            let _ = p_seek.seek(val, false);
        });

        let paned_t = self.watch_paned.clone();
        let state_t = Rc::clone(&self.state);
        theater_btn.connect_clicked(move |_| {
            let mut st = state_t.borrow_mut();
            st.is_theater = !st.is_theater;
            if st.is_theater {
                paned_t.set_orientation(Orientation::Vertical);
                paned_t.set_position(560);
            } else {
                paned_t.set_orientation(Orientation::Horizontal);
                paned_t.set_position(820);
            }
        });

        let win_fs = self.window.clone();
        let state_fs = Rc::clone(&self.state);
        fs_btn.connect_clicked(move |_| {
            let mut st = state_fs.borrow_mut();
            st.is_fullscreen = !st.is_fullscreen;
            if st.is_fullscreen {
                win_fs.fullscreen();
            } else {
                win_fs.unfullscreen();
            }
        });
    }

    fn setup_shortcuts(&self) {
        let controller = EventControllerKey::new();

        let state_ref = Rc::clone(&self.state);
        let player_ref = Arc::clone(&self.player);
        let stack_ref = self.main_stack.clone();
        let search_ref = self.search_entry.clone();
        let window_ref = self.window.clone();
        let paned_ref = self.watch_paned.clone();

        controller.connect_key_pressed(move |_, key, _, modifier| {
            // Focus search
            if (key == Key::slash
                || (key == Key::l && modifier.contains(gtk4::gdk::ModifierType::CONTROL_MASK)))
                && !search_ref.has_focus()
            {
                search_ref.grab_focus();
                return glib::Propagation::Stop;
            }

            // In search entry, pass keys through except Escape
            if search_ref.has_focus() {
                if key == Key::Escape {
                    stack_ref.grab_focus();
                    return glib::Propagation::Stop;
                }
                return glib::Propagation::Proceed;
            }

            match key {
                Key::Escape => {
                    let mut st = state_ref.borrow_mut();
                    if st.is_fullscreen {
                        st.is_fullscreen = false;
                        window_ref.unfullscreen();
                        return glib::Propagation::Stop;
                    }
                    if st.view == CurrentView::Watch {
                        st.view = CurrentView::Home;
                        stack_ref.set_visible_child_name("feed");
                        return glib::Propagation::Stop;
                    }
                    glib::Propagation::Proceed
                }
                // Playback shortcuts
                Key::space | Key::k => {
                    let _ = player_ref.toggle_pause();
                    glib::Propagation::Stop
                }
                Key::Left | Key::j => {
                    let step = if modifier.contains(gtk4::gdk::ModifierType::SHIFT_MASK) {
                        -30.0
                    } else {
                        -5.0
                    };
                    let _ = player_ref.seek(step, true);
                    glib::Propagation::Stop
                }
                Key::Right | Key::l => {
                    let step = if modifier.contains(gtk4::gdk::ModifierType::SHIFT_MASK) {
                        30.0
                    } else {
                        5.0
                    };
                    let _ = player_ref.seek(step, true);
                    glib::Propagation::Stop
                }
                Key::Up => {
                    let cur_vol = player_ref.status.lock().unwrap().volume;
                    let _ = player_ref.set_volume(cur_vol + 5.0);
                    glib::Propagation::Stop
                }
                Key::Down => {
                    let cur_vol = player_ref.status.lock().unwrap().volume;
                    let _ = player_ref.set_volume(cur_vol - 5.0);
                    glib::Propagation::Stop
                }
                Key::m => {
                    let cur_vol = player_ref.status.lock().unwrap().volume;
                    let new_vol = if cur_vol > 0.0 { 0.0 } else { 100.0 };
                    let _ = player_ref.set_volume(new_vol);
                    glib::Propagation::Stop
                }
                Key::f => {
                    let mut st = state_ref.borrow_mut();
                    st.is_fullscreen = !st.is_fullscreen;
                    if st.is_fullscreen {
                        window_ref.fullscreen();
                    } else {
                        window_ref.unfullscreen();
                    }
                    glib::Propagation::Stop
                }
                Key::t => {
                    let mut st = state_ref.borrow_mut();
                    st.is_theater = !st.is_theater;
                    if st.is_theater {
                        paned_ref.set_orientation(Orientation::Vertical);
                        paned_ref.set_position(560);
                    } else {
                        paned_ref.set_orientation(Orientation::Horizontal);
                        paned_ref.set_position(820);
                    }
                    glib::Propagation::Stop
                }
                Key::bracketleft => {
                    let cur = player_ref.status.lock().unwrap().speed;
                    let _ = player_ref.set_speed((cur - 0.25).max(0.25));
                    glib::Propagation::Stop
                }
                Key::bracketright => {
                    let cur = player_ref.status.lock().unwrap().speed;
                    let _ = player_ref.set_speed((cur + 0.25).min(3.0));
                    glib::Propagation::Stop
                }
                Key::c => {
                    let _ = player_ref.cycle_subtitles();
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });

        self.window.add_controller(controller);
    }

    fn setup_polling_timer(&self) {
        let player = Arc::clone(&self.player);
        let time_lbl = self.time_label.clone();
        let scale = self.seek_scale.clone();
        let btn = self.play_pause_btn.clone();

        glib::timeout_add_local(std::time::Duration::from_millis(250), move || {
            let st = player.poll_status();

            let cur_m = (st.time_pos / 60.0).floor() as u64;
            let cur_s = (st.time_pos % 60.0).floor() as u64;
            let dur_m = (st.duration / 60.0).floor() as u64;
            let dur_s = (st.duration % 60.0).floor() as u64;

            time_lbl.set_label(&format!("{cur_m}:{cur_s:02} / {dur_m}:{dur_s:02}"));

            if st.duration > 0.0 {
                scale.set_range(0.0, st.duration);
                scale.set_value(st.time_pos);
            }

            if st.paused {
                btn.set_label("Play");
            } else {
                btn.set_label("Pause");
            }

            glib::ControlFlow::Continue
        });
    }

    fn populate_feed(
        feed_box: &GtkBox,
        items: Vec<Card>,
        backend: Arc<dyn Backend>,
        player: Arc<EmbeddedPlayer>,
        stack: Stack,
        state: Rc<RefCell<AppState>>,
        watch_title: Label,
        watch_channel: Label,
        watch_meta: Label,
        watch_desc_box: GtkBox,
        watch_comments: GtkBox,
    ) {
        while let Some(child) = feed_box.first_child() {
            feed_box.remove(&child);
        }

        for item in items {
            if let Card::Video(v) = item {
                let card_box = GtkBox::builder()
                    .orientation(Orientation::Horizontal)
                    .spacing(12)
                    .margin_top(4)
                    .margin_bottom(4)
                    .focusable(true)
                    .build();

                let info_box = GtkBox::builder()
                    .orientation(Orientation::Vertical)
                    .spacing(2)
                    .build();

                let title_text = if v.badges.contains(&"SHORTS".to_string()) {
                    format!("[SHORTS] {}", v.title)
                } else {
                    v.title.clone()
                };

                let title_lbl = Label::builder()
                    .label(&title_text)
                    .wrap(true)
                    .halign(Align::Start)
                    .css_classes(["heading"])
                    .build();

                let chan_lbl = Label::builder()
                    .label(&v.channel)
                    .halign(Align::Start)
                    .css_classes(["dim-label"])
                    .build();

                let meta_str = format!(
                    "{}  {}",
                    v.duration.as_deref().unwrap_or(""),
                    v.views.as_deref().unwrap_or("")
                );
                let meta_lbl = Label::builder()
                    .label(&meta_str)
                    .halign(Align::Start)
                    .css_classes(["dim-label"])
                    .build();

                info_box.append(&title_lbl);
                info_box.append(&chan_lbl);
                info_box.append(&meta_lbl);

                card_box.append(&info_box);

                let v_clone = v.clone();
                let p_clone = Arc::clone(&player);
                let b_clone = Arc::clone(&backend);
                let s_clone = stack.clone();
                let st_clone = Rc::clone(&state);

                let wt = watch_title.clone();
                let wc = watch_channel.clone();
                let wm = watch_meta.clone();
                let wdesc_box = watch_desc_box.clone();
                let wcomm = watch_comments.clone();

                let click = GestureClick::new();
                click.connect_pressed(move |_, _, _, _| {
                    st_clone.borrow_mut().view = CurrentView::Watch;
                    s_clone.set_visible_child_name("watch");

                    wt.set_label(&v_clone.title);
                    wc.set_label(&v_clone.channel);
                    wm.set_label(&format!(
                        "{} • {}",
                        v_clone.duration.as_deref().unwrap_or(""),
                        v_clone.views.as_deref().unwrap_or("")
                    ));

                    let _ = p_clone.load_video(v_clone.clone(), None);

                    let b = Arc::clone(&b_clone);
                    let p = Arc::clone(&p_clone);
                    let vid = v_clone.id.clone();
                    let wdesc_b = wdesc_box.clone();
                    let wcomm2 = wcomm.clone();
                    let wm2 = wm.clone();
                    let v_dur = v_clone.duration.clone();
                    let v_vw = v_clone.views.clone();

                    glib::MainContext::default().spawn_local(async move {
                        if let Ok(watch_data) = b.watch(&vid).await {
                            // Render RYD likes/dislikes
                            let likes = watch_data.likes.as_deref().unwrap_or("-");
                            let dislikes = watch_data.dislikes.as_deref().unwrap_or("-");
                            wm2.set_label(&format!(
                                "{} • {}  |  Likes: {}  Dislikes: {}",
                                v_dur.as_deref().unwrap_or(""),
                                v_vw.as_deref().unwrap_or(""),
                                likes,
                                dislikes
                            ));

                            // Render Description with clickable timestamps
                            while let Some(c) = wdesc_b.first_child() {
                                wdesc_b.remove(&c);
                            }

                            for line in watch_data.description.lines() {
                                let line_box = GtkBox::builder()
                                    .orientation(Orientation::Horizontal)
                                    .spacing(6)
                                    .build();

                                // Search for timestamp like MM:SS or HH:MM:SS
                                let mut found_ts = None;
                                for word in line.split_whitespace() {
                                    let cleaned =
                                        word.trim_matches(|c: char| !c.is_numeric() && c != ':');
                                    if let Some(sec) = parse_duration_to_seconds(cleaned) {
                                        found_ts = Some((sec, cleaned.to_string()));
                                        break;
                                    }
                                }

                                if let Some((sec, ts_str)) = found_ts {
                                    let ts_btn = Button::builder()
                                        .label(&ts_str)
                                        .css_classes(["flat", "accent"])
                                        .build();

                                    let p_ts = Arc::clone(&p);
                                    ts_btn.connect_clicked(move |_| {
                                        let _ = p_ts.seek(sec as f64, false);
                                    });

                                    line_box.append(&ts_btn);
                                }

                                let text_lbl = Label::builder()
                                    .label(line)
                                    .wrap(true)
                                    .selectable(true)
                                    .halign(Align::Start)
                                    .build();
                                line_box.append(&text_lbl);
                                wdesc_b.append(&line_box);
                            }

                            // Render Comments
                            while let Some(c) = wcomm2.first_child() {
                                wcomm2.remove(&c);
                            }
                            for comment in &watch_data.comments {
                                let c_lbl = Label::builder()
                                    .label(&format!("{}: {}", comment.author, comment.text))
                                    .wrap(true)
                                    .selectable(true)
                                    .halign(Align::Start)
                                    .build();
                                wcomm2.append(&c_lbl);
                            }
                        }
                    });
                });

                card_box.add_controller(click);
                feed_box.append(&card_box);
            }
        }
    }

    pub fn load_home_feed(&self) {
        let backend = Arc::clone(&self.backend);
        let feed_box = self.feed_flow_box.clone();
        let player = Arc::clone(&self.player);
        let stack = self.main_stack.clone();
        let state = Rc::clone(&self.state);
        let watch_title = self.watch_title.clone();
        let watch_channel = self.watch_channel.clone();
        let watch_meta = self.watch_meta.clone();
        let watch_desc_box = self.watch_desc_box.clone();
        let watch_comments = self.watch_comments_box.clone();
        let status = self.status_label.clone();

        status.set_label("Loading Home Feed...");

        glib::MainContext::default().spawn_local(async move {
            if let Ok(page) = backend.home(None).await {
                Self::populate_feed(
                    &feed_box,
                    page.items,
                    backend,
                    player,
                    stack,
                    state,
                    watch_title,
                    watch_channel,
                    watch_meta,
                    watch_desc_box,
                    watch_comments,
                );
                status.set_label("Home");
            }
        });
    }

    pub fn present(&self) {
        self.window.present();
    }
}
