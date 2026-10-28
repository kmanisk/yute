use crate::async_runtime::spawn_task;
use crate::player_embedded::EmbeddedPlayer;
use crate::settings_view::SettingsViewWidget;
use crate::thumbnail_loader::ThumbnailLoader;
use crate::video_area::VideoArea;
use crate::video_card::VideoCardWidget;
use gtk4::gdk::Key;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, AspectFrame, Box as GtkBox, Button, Entry,
    EventControllerKey, FlowBox, Label, Orientation, Paned, PolicyType, Scale, ScrolledWindow,
    SelectionMode, Stack,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use yute_core::backend::auth::AuthContext;
use yute_core::backend::innertube::InnerTubeBackend;
use yute_core::backend::models::{
    parse_duration_to_seconds, Card, SearchFilters, VideoCard, WatchData,
};
use yute_core::backend::ytdlp::YtDlpBackend;
use yute_core::backend::{Backend, BackendChain};
use yute_core::config::Config;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CurrentView {
    Home,
    Subscriptions,
    History,
    Playlists,
    Search,
    Watch,
    Settings,
}

pub struct AppState {
    pub view: CurrentView,
    pub is_loading: bool,
    pub is_fullscreen: bool,
    pub is_theater: bool,
}

#[derive(Clone)]
pub struct GuiApp {
    inner: Rc<GuiAppInner>,
}

pub struct GuiAppInner {
    pub window: ApplicationWindow,
    pub player: Arc<EmbeddedPlayer>,
    pub backend: Arc<dyn Backend>,
    pub state: Rc<RefCell<AppState>>,
    pub thumbnail_loader: ThumbnailLoader,
    pub config: Rc<RefCell<Config>>,
    #[allow(dead_code)]
    pub video_area: VideoArea,
    // UI elements
    pub main_stack: Stack,
    pub feed_flow_box: FlowBox,
    pub search_entry: Entry,
    pub status_label: Label,
    pub current_cards: Rc<RefCell<Vec<Card>>>,
    // Watch view elements
    pub watch_title: Label,
    pub watch_channel: Label,
    pub watch_meta: Label,
    pub watch_likes_lbl: Label,
    pub watch_desc_box: GtkBox,
    pub watch_comments_box: GtkBox,
    pub related_flow_box: FlowBox,
    pub related_cards: Rc<RefCell<Vec<Card>>>,
    pub time_label: Label,
    pub seek_scale: Scale,
    pub play_pause_btn: Button,
    pub speed_btn: Button,
    pub watch_paned: Paned,
}

impl GuiApp {
    pub fn new(app: &Application, cfg: Config) -> anyhow::Result<Self> {
        let auth = AuthContext::from_config(&cfg);
        let credentials_config = cfg.clone();

        let auth_arc = Arc::new(Mutex::new(auth));
        let inner_backend = Box::new(InnerTubeBackend::from_arc(Arc::clone(&auth_arc)));
        let ytdlp_backend = Box::new(YtDlpBackend::new(None, None));
        let backend: Arc<dyn Backend> =
            Arc::new(BackendChain::new(vec![inner_backend, ytdlp_backend]));

        let player = Arc::new(EmbeddedPlayer::new(&cfg)?);
        let video_area = VideoArea::new(Arc::clone(&player));
        let thumbnail_loader = ThumbnailLoader::new_with_budget(150, cfg.thumbnails.cache_mb);
        let config = Rc::new(RefCell::new(cfg));

        let window = ApplicationWindow::builder()
            .application(app)
            .title("yute - YouTube")
            .default_width(1360)
            .default_height(860)
            .build();

        let state = Rc::new(RefCell::new(AppState {
            view: CurrentView::Home,
            is_loading: false,
            is_fullscreen: false,
            is_theater: false,
        }));

        let search_entry = Entry::builder()
            .placeholder_text("Search YouTube (Press / or Ctrl+L)...")
            .hexpand(true)
            .build();

        let status_label = Label::builder().label("Ready").halign(Align::End).build();

        let feed_flow_box = FlowBox::builder()
            .selection_mode(SelectionMode::Single)
            .homogeneous(true)
            .min_children_per_line(2)
            .max_children_per_line(5)
            .column_spacing(18)
            .row_spacing(24)
            .margin_top(16)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();

        let main_stack = Stack::builder().hexpand(true).vexpand(true).build();

        let watch_title = Label::builder()
            .wrap(true)
            .halign(Align::Start)
            .css_classes(["title-2"])
            .build();

        let watch_channel = Label::builder()
            .halign(Align::Start)
            .css_classes(["title-4"])
            .build();

        let watch_meta = Label::builder()
            .halign(Align::Start)
            .css_classes(["dim-label"])
            .build();

        let watch_likes_lbl = Label::builder()
            .label("Likes: -  |  Dislikes: -")
            .css_classes(["yt-action-pill"])
            .build();

        let watch_desc_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .css_classes(["yt-desc-box"])
            .build();

        let watch_comments_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .css_classes(["yt-comments-box"])
            .build();

        let related_flow_box = FlowBox::builder()
            .selection_mode(SelectionMode::Single)
            .homogeneous(true)
            .min_children_per_line(1)
            .max_children_per_line(1)
            .column_spacing(8)
            .row_spacing(10)
            .build();

        let time_label = Label::builder().label("0:00 / 0:00").build();

        let seek_scale = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
        seek_scale.set_hexpand(true);

        let play_pause_btn = Button::with_label("Play");
        let speed_btn = Button::with_label("1.0x");

        let watch_paned = Paned::builder()
            .orientation(Orientation::Horizontal)
            .hexpand(true)
            .vexpand(true)
            .build();

        let inner = Rc::new(GuiAppInner {
            window,
            player,
            backend,
            state,
            thumbnail_loader,
            config,
            video_area,
            main_stack,
            feed_flow_box,
            search_entry,
            status_label,
            current_cards: Rc::new(RefCell::new(Vec::new())),
            watch_title,
            watch_channel,
            watch_meta,
            watch_likes_lbl,
            watch_desc_box,
            watch_comments_box,
            related_flow_box,
            related_cards: Rc::new(RefCell::new(Vec::new())),
            time_label,
            seek_scale,
            play_pause_btn,
            speed_btn,
            watch_paned,
        });

        let app_inst = Self { inner };
        app_inst.build_ui();
        app_inst.setup_shortcuts();
        app_inst.setup_polling_timer();
        let ready_app = app_inst.clone();
        spawn_task(
            async move {
                tokio::task::spawn_blocking(move || {
                    yute_core::accounts::Accounts::load_session(
                        &credentials_config,
                        AuthContext::from_config(&credentials_config),
                    )
                })
                .await
            },
            move |result| {
                if let Ok(Ok(accounts)) = result {
                    *auth_arc.lock().unwrap() = accounts.current_context();
                }
                ready_app.load_home_feed();
            },
        );

        Ok(app_inst)
    }

    fn build_ui(&self) {
        let root_box = GtkBox::builder().orientation(Orientation::Vertical).build();

        // 1. Top Header Bar
        let header_bar = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(12)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(16)
            .margin_end(16)
            .build();

        // Brand Logo
        let logo_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .valign(Align::Center)
            .build();
        let logo_badge = Label::builder()
            .label("YT")
            .css_classes(["yt-logo-badge"])
            .build();
        let logo_text = Label::builder()
            .label("yute")
            .css_classes(["yt-logo"])
            .build();
        logo_box.append(&logo_badge);
        logo_box.append(&logo_text);
        header_bar.append(&logo_box);

        // Center Search Container
        let search_container = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .hexpand(true)
            .margin_start(40)
            .margin_end(40)
            .build();

        search_container.append(&self.inner.search_entry);
        let search_btn = Button::with_label("Search");
        search_container.append(&search_btn);
        header_bar.append(&search_container);

        // Right status and Settings shortcut
        header_bar.append(&self.inner.status_label);
        let settings_top_btn = Button::with_label("Settings");
        header_bar.append(&settings_top_btn);

        root_box.append(&header_bar);

        // Main Body Layout: Left Sidebar + Right Content Stack
        let body_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .hexpand(true)
            .vexpand(true)
            .build();

        // 2. Left Navigation Sidebar
        let sidebar_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .width_request(190)
            .margin_top(8)
            .margin_start(8)
            .margin_end(8)
            .css_classes(["yt-sidebar"])
            .build();

        let home_btn = Button::builder()
            .label("  Home")
            .css_classes(["yt-nav-btn"])
            .halign(Align::Fill)
            .build();
        let subs_btn = Button::builder()
            .label("  Subscriptions")
            .css_classes(["yt-nav-btn"])
            .halign(Align::Fill)
            .build();
        let hist_btn = Button::builder()
            .label("  History")
            .css_classes(["yt-nav-btn"])
            .halign(Align::Fill)
            .build();
        let play_btn = Button::builder()
            .label("  Playlists")
            .css_classes(["yt-nav-btn"])
            .halign(Align::Fill)
            .build();
        let sett_btn = Button::builder()
            .label("  Settings")
            .css_classes(["yt-nav-btn"])
            .halign(Align::Fill)
            .build();

        sidebar_box.append(&home_btn);
        sidebar_box.append(&subs_btn);
        sidebar_box.append(&hist_btn);
        sidebar_box.append(&play_btn);
        sidebar_box.append(&sett_btn);

        body_box.append(&sidebar_box);

        // 3. Browse Feed Page (Responsive FlowBox grid)
        let feed_scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .child(&self.inner.feed_flow_box)
            .build();

        self.inner
            .main_stack
            .add_named(&feed_scrolled, Some("feed"));

        // 4. Watch Page Container
        let watch_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .margin_start(16)
            .margin_end(16)
            .margin_bottom(12)
            .build();

        let video_container = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .hexpand(true)
            .build();

        let video_aspect = AspectFrame::builder()
            .xalign(0.5)
            .yalign(0.5)
            .ratio(16.0 / 9.0)
            .obey_child(false)
            .hexpand(true)
            .height_request(450)
            .css_classes(["yt-video-wrapper"])
            .child(&self.inner.video_area.widget)
            .build();
        video_container.append(&video_aspect);

        // Controls bar under video
        let controls_bar = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .margin_top(6)
            .build();

        let seek_back_btn = Button::with_label("-5s");
        let seek_fwd_btn = Button::with_label("+5s");
        let sub_cycle_btn = Button::with_label("CC");
        let fs_btn = Button::with_label("Fullscreen");
        let theater_btn = Button::with_label("Theater");

        controls_bar.append(&self.inner.play_pause_btn);
        controls_bar.append(&seek_back_btn);
        controls_bar.append(&seek_fwd_btn);
        controls_bar.append(&self.inner.seek_scale);
        controls_bar.append(&self.inner.time_label);
        controls_bar.append(&self.inner.speed_btn);
        controls_bar.append(&sub_cycle_btn);
        controls_bar.append(&theater_btn);
        controls_bar.append(&fs_btn);

        video_container.append(&controls_bar);

        // Primary Video Details Section
        let meta_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(8)
            .margin_top(8)
            .build();

        meta_box.append(&self.inner.watch_title);

        // Channel + Actions Row
        let channel_row = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(12)
            .build();
        channel_row.append(&self.inner.watch_channel);

        let subscribe_btn = Button::builder()
            .label("Subscribe")
            .css_classes(["yt-sub-btn"])
            .build();
        channel_row.append(&subscribe_btn);

        let spacer = GtkBox::builder().hexpand(true).build();
        channel_row.append(&spacer);

        channel_row.append(&self.inner.watch_likes_lbl);
        meta_box.append(&channel_row);
        meta_box.append(&self.inner.watch_meta);

        // Description Section
        let desc_label_hdr = Label::builder()
            .label("Description")
            .halign(Align::Start)
            .css_classes(["title-4"])
            .margin_top(8)
            .build();
        meta_box.append(&desc_label_hdr);
        meta_box.append(&self.inner.watch_desc_box);

        // Comments Section
        let comments_label_hdr = Label::builder()
            .label("Comments")
            .halign(Align::Start)
            .css_classes(["title-4"])
            .margin_top(12)
            .build();
        meta_box.append(&comments_label_hdr);
        meta_box.append(&self.inner.watch_comments_box);

        let left_video_pane = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .child(&{
                let vbox = GtkBox::builder()
                    .orientation(Orientation::Vertical)
                    .spacing(8)
                    .build();
                vbox.append(&video_container);
                vbox.append(&meta_box);
                vbox
            })
            .build();

        // Right details pane (Related Videos)
        let right_details_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(10)
            .width_request(380)
            .margin_start(12)
            .build();

        let related_label_hdr = Label::builder()
            .label("Related Videos")
            .halign(Align::Start)
            .css_classes(["title-4"])
            .build();
        right_details_box.append(&related_label_hdr);

        let related_scroll = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .vexpand(true)
            .child(&self.inner.related_flow_box)
            .build();
        right_details_box.append(&related_scroll);

        self.inner
            .watch_paned
            .set_start_child(Some(&left_video_pane));
        self.inner
            .watch_paned
            .set_end_child(Some(&right_details_box));
        self.inner.watch_paned.set_position(900);

        watch_box.append(&self.inner.watch_paned);
        self.inner.main_stack.add_named(&watch_box, Some("watch"));

        // 5. Settings Page Container
        let settings_page = SettingsViewWidget::build(Rc::clone(&self.inner.config));
        self.inner
            .main_stack
            .add_named(&settings_page, Some("settings"));

        body_box.append(&self.inner.main_stack);
        root_box.append(&body_box);

        self.inner.window.set_child(Some(&root_box));

        // Connect Navigation Buttons
        let app_home = self.clone();
        home_btn.connect_clicked(move |_| {
            app_home.load_home_feed();
        });

        let app_subs = self.clone();
        subs_btn.connect_clicked(move |_| {
            app_subs.load_subscriptions();
        });

        let app_hist = self.clone();
        hist_btn.connect_clicked(move |_| {
            app_hist.load_history();
        });

        let app_play = self.clone();
        play_btn.connect_clicked(move |_| {
            app_play.load_playlists();
        });

        let app_sett = self.clone();
        sett_btn.connect_clicked(move |_| {
            app_sett.open_settings();
        });

        let app_sett_top = self.clone();
        settings_top_btn.connect_clicked(move |_| {
            app_sett_top.open_settings();
        });

        // Search submit (Enter in entry or Search button click)
        let app_search1 = self.clone();
        self.inner.search_entry.connect_activate(move |entry| {
            app_search1.search_query(&entry.text());
        });

        let app_search2 = self.clone();
        let search_ent = self.inner.search_entry.clone();
        search_btn.connect_clicked(move |_| {
            app_search2.search_query(&search_ent.text());
        });

        // Connect Row Activated in Feed Grid
        let app_flow = self.clone();
        self.inner
            .feed_flow_box
            .connect_child_activated(move |_, child| {
                let idx = child.index() as usize;
                if let Some(item) = app_flow.inner.current_cards.borrow().get(idx).cloned() {
                    match item {
                        Card::Video(v) => {
                            app_flow.open_watch_video(v);
                        }
                        Card::Playlist(p) => {
                            app_flow.load_playlist(&p.id);
                        }
                        _ => {}
                    }
                }
            });

        // Connect Row Activated in Related Videos Grid
        let app_rel = self.clone();
        self.inner
            .related_flow_box
            .connect_child_activated(move |_, child| {
                let idx = child.index() as usize;
                if let Some(item) = app_rel.inner.related_cards.borrow().get(idx).cloned() {
                    if let Card::Video(v) = item {
                        app_rel.open_watch_video(v);
                    }
                }
            });

        // Connect Playback Buttons
        let p_toggle = Arc::clone(&self.inner.player);
        self.inner.play_pause_btn.connect_clicked(move |_| {
            let _ = p_toggle.toggle_pause();
        });

        let p_back = Arc::clone(&self.inner.player);
        seek_back_btn.connect_clicked(move |_| {
            let _ = p_back.seek(-5.0, true);
        });

        let p_fwd = Arc::clone(&self.inner.player);
        seek_fwd_btn.connect_clicked(move |_| {
            let _ = p_fwd.seek(5.0, true);
        });

        let p_seek = Arc::clone(&self.inner.player);
        self.inner.seek_scale.connect_value_changed(move |scale| {
            let val = scale.value();
            let _ = p_seek.seek(val, false);
        });

        let p_spd = Arc::clone(&self.inner.player);
        let spd_lbl = self.inner.speed_btn.clone();
        self.inner.speed_btn.connect_clicked(move |_| {
            let cur = p_spd.status.lock().unwrap().speed;
            let next_spd = if cur >= 2.0 {
                0.75
            } else if cur >= 1.5 {
                2.0
            } else if cur >= 1.25 {
                1.5
            } else if cur >= 1.0 {
                1.25
            } else {
                1.0
            };
            let _ = p_spd.set_speed(next_spd);
            spd_lbl.set_label(&format!("{next_spd:.2}x"));
        });

        let p_sub = Arc::clone(&self.inner.player);
        sub_cycle_btn.connect_clicked(move |_| {
            let _ = p_sub.cycle_subtitles();
        });

        let paned_t = self.inner.watch_paned.clone();
        let state_t = Rc::clone(&self.inner.state);
        theater_btn.connect_clicked(move |_| {
            let mut st = state_t.borrow_mut();
            st.is_theater = !st.is_theater;
            if st.is_theater {
                paned_t.set_orientation(Orientation::Vertical);
                paned_t.set_position(600);
            } else {
                paned_t.set_orientation(Orientation::Horizontal);
                paned_t.set_position(900);
            }
        });

        let win_fs = self.inner.window.clone();
        let state_fs = Rc::clone(&self.inner.state);
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

        let state_ref = Rc::clone(&self.inner.state);
        let player_ref = Arc::clone(&self.inner.player);
        let stack_ref = self.inner.main_stack.clone();
        let search_ref = self.inner.search_entry.clone();
        let flow_ref = self.inner.feed_flow_box.clone();
        let cards_store_ref = Rc::clone(&self.inner.current_cards);
        let related_flow_ref = self.inner.related_flow_box.clone();
        let related_cards_ref = Rc::clone(&self.inner.related_cards);
        let window_ref = self.inner.window.clone();
        let paned_ref = self.inner.watch_paned.clone();
        let app_shortcuts = self.clone();
        let last_g_pressed = Rc::new(RefCell::new(None::<std::time::Instant>));

        controller.connect_key_pressed(move |_, key, _, modifier| {
            // 1. Focus search bar
            if (key == Key::slash
                || (key == Key::l && modifier.contains(gtk4::gdk::ModifierType::CONTROL_MASK)))
                && !search_ref.has_focus()
            {
                search_ref.grab_focus();
                return glib::Propagation::Stop;
            }

            // 2. In search entry, pass keys through except Escape
            if search_ref.has_focus() {
                if key == Key::Escape {
                    flow_ref.grab_focus();
                    return glib::Propagation::Stop;
                }
                return glib::Propagation::Proceed;
            }

            let is_watch_view = state_ref.borrow().view == CurrentView::Watch;

            // In Watch view: check if related_flow_box has focus
            let in_related = is_watch_view
                && (related_flow_ref.has_focus()
                    || related_flow_ref
                        .selected_children()
                        .iter()
                        .any(|c| c.has_focus()));

            if in_related {
                match key {
                    Key::j | Key::Down => {
                        let total = related_cards_ref.borrow().len();
                        if total > 0 {
                            let cur_idx = related_flow_ref
                                .selected_children()
                                .first()
                                .map(|c| c.index() as usize)
                                .unwrap_or(0);
                            let target = (cur_idx + 1).min(total - 1);
                            if let Some(child) = related_flow_ref.child_at_index(target as i32) {
                                related_flow_ref.select_child(&child);
                                child.grab_focus();
                            }
                        }
                        return glib::Propagation::Stop;
                    }
                    Key::k | Key::Up => {
                        let cur_idx = related_flow_ref
                            .selected_children()
                            .first()
                            .map(|c| c.index() as usize)
                            .unwrap_or(0);
                        let target = cur_idx.saturating_sub(1);
                        if let Some(child) = related_flow_ref.child_at_index(target as i32) {
                            related_flow_ref.select_child(&child);
                            child.grab_focus();
                        }
                        return glib::Propagation::Stop;
                    }
                    Key::Return | Key::KP_Enter => {
                        if let Some(child) = related_flow_ref.selected_children().first() {
                            child.activate();
                        }
                        return glib::Propagation::Stop;
                    }
                    Key::Escape => {
                        window_ref.grab_focus();
                        return glib::Propagation::Stop;
                    }
                    _ => {}
                }
            }

            // Tab toggles focus between player area and related videos in Watch view
            if key == Key::Tab && is_watch_view {
                if in_related {
                    window_ref.grab_focus();
                } else if let Some(first_rel) = related_flow_ref.child_at_index(0) {
                    related_flow_ref.select_child(&first_rel);
                    first_rel.grab_focus();
                }
                return glib::Propagation::Stop;
            }

            match key {
                Key::Escape => {
                    let mut st = state_ref.borrow_mut();
                    if st.is_fullscreen {
                        st.is_fullscreen = false;
                        window_ref.unfullscreen();
                        return glib::Propagation::Stop;
                    }
                    if st.view == CurrentView::Watch || st.view == CurrentView::Settings {
                        st.view = CurrentView::Home;
                        stack_ref.set_visible_child_name("feed");
                        flow_ref.grab_focus();
                        return glib::Propagation::Stop;
                    }
                    glib::Propagation::Proceed
                }
                // Quick View Switching from keyboard
                Key::_1 => {
                    app_shortcuts.load_home_feed();
                    glib::Propagation::Stop
                }
                Key::_2 => {
                    app_shortcuts.load_subscriptions();
                    glib::Propagation::Stop
                }
                Key::_3 => {
                    app_shortcuts.load_history();
                    glib::Propagation::Stop
                }
                Key::_4 => {
                    app_shortcuts.load_playlists();
                    glib::Propagation::Stop
                }
                Key::_5 => {
                    app_shortcuts.open_settings();
                    glib::Propagation::Stop
                }

                // Playback shortcuts (only active when in Watch view)
                Key::space | Key::k if is_watch_view => {
                    let _ = player_ref.toggle_pause();
                    glib::Propagation::Stop
                }
                Key::Left | Key::j if is_watch_view => {
                    let step = if modifier.contains(gtk4::gdk::ModifierType::SHIFT_MASK) {
                        -30.0
                    } else {
                        -5.0
                    };
                    let _ = player_ref.seek(step, true);
                    glib::Propagation::Stop
                }
                Key::Right | Key::l if is_watch_view => {
                    let step = if modifier.contains(gtk4::gdk::ModifierType::SHIFT_MASK) {
                        30.0
                    } else {
                        5.0
                    };
                    let _ = player_ref.seek(step, true);
                    glib::Propagation::Stop
                }
                Key::Up | Key::plus | Key::equal if is_watch_view => {
                    let cur_vol = player_ref.status.lock().unwrap().volume;
                    let _ = player_ref.set_volume((cur_vol + 5.0).min(100.0));
                    glib::Propagation::Stop
                }
                Key::Down | Key::minus if is_watch_view => {
                    let cur_vol = player_ref.status.lock().unwrap().volume;
                    let _ = player_ref.set_volume((cur_vol - 5.0).max(0.0));
                    glib::Propagation::Stop
                }
                Key::m if is_watch_view => {
                    let cur_vol = player_ref.status.lock().unwrap().volume;
                    let new_vol = if cur_vol > 0.0 { 0.0 } else { 100.0 };
                    let _ = player_ref.set_volume(new_vol);
                    glib::Propagation::Stop
                }
                Key::f if is_watch_view => {
                    let mut st = state_ref.borrow_mut();
                    st.is_fullscreen = !st.is_fullscreen;
                    if st.is_fullscreen {
                        window_ref.fullscreen();
                    } else {
                        window_ref.unfullscreen();
                    }
                    glib::Propagation::Stop
                }
                Key::t if is_watch_view => {
                    let mut st = state_ref.borrow_mut();
                    st.is_theater = !st.is_theater;
                    if st.is_theater {
                        paned_ref.set_orientation(Orientation::Vertical);
                        paned_ref.set_position(600);
                    } else {
                        paned_ref.set_orientation(Orientation::Horizontal);
                        paned_ref.set_position(900);
                    }
                    glib::Propagation::Stop
                }
                Key::bracketleft if is_watch_view => {
                    let cur = player_ref.status.lock().unwrap().speed;
                    let _ = player_ref.set_speed((cur - 0.25).max(0.25));
                    glib::Propagation::Stop
                }
                Key::bracketright if is_watch_view => {
                    let cur = player_ref.status.lock().unwrap().speed;
                    let _ = player_ref.set_speed((cur + 0.25).min(3.0));
                    glib::Propagation::Stop
                }
                Key::c if is_watch_view => {
                    let _ = player_ref.cycle_subtitles();
                    glib::Propagation::Stop
                }

                // Browse Feed Navigation (Vim Keys: j, k, h, l, Enter, gg, G, r)
                Key::j | Key::Down if !is_watch_view => {
                    let total = cards_store_ref.borrow().len();
                    if total > 0 {
                        let cur_idx = flow_ref
                            .selected_children()
                            .first()
                            .map(|c| c.index() as usize)
                            .unwrap_or(0);
                        let w = flow_ref.width();
                        let min_cols = flow_ref.min_children_per_line() as usize;
                        let max_cols = flow_ref.max_children_per_line() as usize;
                        let cols = if w > 0 {
                            ((w / 340) as usize).clamp(min_cols, max_cols)
                        } else {
                            min_cols
                        };
                        let target = (cur_idx + cols).min(total.saturating_sub(1));
                        if let Some(child) = flow_ref.child_at_index(target as i32) {
                            flow_ref.select_child(&child);
                            child.grab_focus();
                        }
                    }
                    glib::Propagation::Stop
                }
                Key::k | Key::Up if !is_watch_view => {
                    let total = cards_store_ref.borrow().len();
                    if total > 0 {
                        let cur_idx = flow_ref
                            .selected_children()
                            .first()
                            .map(|c| c.index() as usize)
                            .unwrap_or(0);
                        let w = flow_ref.width();
                        let min_cols = flow_ref.min_children_per_line() as usize;
                        let max_cols = flow_ref.max_children_per_line() as usize;
                        let cols = if w > 0 {
                            ((w / 340) as usize).clamp(min_cols, max_cols)
                        } else {
                            min_cols
                        };
                        let target = cur_idx.saturating_sub(cols);
                        if let Some(child) = flow_ref.child_at_index(target as i32) {
                            flow_ref.select_child(&child);
                            child.grab_focus();
                        }
                    }
                    glib::Propagation::Stop
                }
                Key::h | Key::Left if !is_watch_view => {
                    let total = cards_store_ref.borrow().len();
                    if total > 0 {
                        let cur_idx = flow_ref
                            .selected_children()
                            .first()
                            .map(|c| c.index() as usize)
                            .unwrap_or(0);
                        let target = cur_idx.saturating_sub(1);
                        if let Some(child) = flow_ref.child_at_index(target as i32) {
                            flow_ref.select_child(&child);
                            child.grab_focus();
                        }
                    }
                    glib::Propagation::Stop
                }
                Key::l | Key::Right if !is_watch_view => {
                    let total = cards_store_ref.borrow().len();
                    if total > 0 {
                        let cur_idx = flow_ref
                            .selected_children()
                            .first()
                            .map(|c| c.index() as usize)
                            .unwrap_or(0);
                        let target = (cur_idx + 1).min(total.saturating_sub(1));
                        if let Some(child) = flow_ref.child_at_index(target as i32) {
                            flow_ref.select_child(&child);
                            child.grab_focus();
                        }
                    }
                    glib::Propagation::Stop
                }
                Key::Return | Key::KP_Enter if !is_watch_view => {
                    if let Some(child) = flow_ref.selected_children().first() {
                        child.activate();
                    }
                    glib::Propagation::Stop
                }
                Key::g if !is_watch_view => {
                    let now = std::time::Instant::now();
                    let mut g_lock = last_g_pressed.borrow_mut();
                    let is_double_g = if let Some(t) = *g_lock {
                        now.duration_since(t).as_millis() < 500
                    } else {
                        false
                    };
                    if is_double_g {
                        *g_lock = None;
                        if let Some(child) = flow_ref.child_at_index(0) {
                            flow_ref.select_child(&child);
                            child.grab_focus();
                        }
                    } else {
                        *g_lock = Some(now);
                    }
                    glib::Propagation::Stop
                }
                Key::G if !is_watch_view => {
                    let total = cards_store_ref.borrow().len();
                    if total > 0 {
                        let last_idx = (total - 1) as i32;
                        if let Some(child) = flow_ref.child_at_index(last_idx) {
                            flow_ref.select_child(&child);
                            child.grab_focus();
                        }
                    }
                    glib::Propagation::Stop
                }
                Key::r if !is_watch_view => {
                    match state_ref.borrow().view {
                        CurrentView::Home => app_shortcuts.load_home_feed(),
                        CurrentView::Subscriptions => app_shortcuts.load_subscriptions(),
                        CurrentView::History => app_shortcuts.load_history(),
                        CurrentView::Playlists => app_shortcuts.load_playlists(),
                        CurrentView::Search => {
                            let text = search_ref.text();
                            if !text.is_empty() {
                                app_shortcuts.search_query(&text);
                            }
                        }
                        _ => {}
                    }
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });

        self.inner.window.add_controller(controller);
    }

    fn setup_polling_timer(&self) {
        let player = Arc::clone(&self.inner.player);
        let time_lbl = self.inner.time_label.clone();
        let scale = self.inner.seek_scale.clone();
        let btn = self.inner.play_pause_btn.clone();

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

    fn populate_grid(
        flow_box: &FlowBox,
        cards_store: &Rc<RefCell<Vec<Card>>>,
        items: Vec<Card>,
        thumbnail_loader: &ThumbnailLoader,
        compact: bool,
    ) {
        while let Some(child) = flow_box.first_child() {
            flow_box.remove(&child);
        }

        let mut cards = Vec::new();

        for item in &items {
            let card_widget = VideoCardWidget::build(item, thumbnail_loader, compact);
            flow_box.insert(&card_widget, -1);
            cards.push(item.clone());
        }

        *cards_store.borrow_mut() = cards;

        if let Some(first_child) = flow_box.child_at_index(0) {
            flow_box.select_child(&first_child);
        }
    }

    pub fn load_home_feed(&self) {
        self.inner.state.borrow_mut().view = CurrentView::Home;
        self.inner.main_stack.set_visible_child_name("feed");
        self.inner.status_label.set_label("Loading Home Feed...");
        self.inner.state.borrow_mut().is_loading = true;

        let backend = Arc::clone(&self.inner.backend);
        let flow_box = self.inner.feed_flow_box.clone();
        let cards_store = Rc::clone(&self.inner.current_cards);
        let loader = self.inner.thumbnail_loader.clone();
        let status = self.inner.status_label.clone();
        let state = Rc::clone(&self.inner.state);

        spawn_task(async move { backend.home(None).await }, move |result| {
            state.borrow_mut().is_loading = false;
            match result {
                Ok(page) => {
                    let count = page.items.len();
                    tracing::info!("Loaded {count} items for home feed");
                    Self::populate_grid(&flow_box, &cards_store, page.items, &loader, false);
                    status.set_label(&format!("Home ({count} videos)"));
                    flow_box.grab_focus();
                }
                Err(err) => {
                    tracing::error!("Failed to load home feed: {err:#}");
                    status.set_label(&format!("Home error: {err}"));
                }
            }
        });
    }

    pub fn load_subscriptions(&self) {
        self.inner.state.borrow_mut().view = CurrentView::Subscriptions;
        self.inner.main_stack.set_visible_child_name("feed");
        self.inner
            .status_label
            .set_label("Loading Subscriptions...");
        self.inner.state.borrow_mut().is_loading = true;

        let backend = Arc::clone(&self.inner.backend);
        let flow_box = self.inner.feed_flow_box.clone();
        let cards_store = Rc::clone(&self.inner.current_cards);
        let loader = self.inner.thumbnail_loader.clone();
        let status = self.inner.status_label.clone();
        let state = Rc::clone(&self.inner.state);

        spawn_task(
            async move { backend.subscriptions().await },
            move |result| {
                state.borrow_mut().is_loading = false;
                match result {
                    Ok(page) => {
                        let count = page.items.len();
                        tracing::info!("Loaded {count} items for subscriptions");
                        Self::populate_grid(&flow_box, &cards_store, page.items, &loader, false);
                        status.set_label(&format!("Subscriptions ({count} videos)"));
                        flow_box.grab_focus();
                    }
                    Err(err) => {
                        tracing::error!("Failed to load subscriptions: {err:#}");
                        status.set_label(&format!("Subscriptions error: {err}"));
                    }
                }
            },
        );
    }

    pub fn load_history(&self) {
        self.inner.state.borrow_mut().view = CurrentView::History;
        self.inner.main_stack.set_visible_child_name("feed");
        self.inner.status_label.set_label("Loading History...");
        self.inner.state.borrow_mut().is_loading = true;

        let backend = Arc::clone(&self.inner.backend);
        let flow_box = self.inner.feed_flow_box.clone();
        let cards_store = Rc::clone(&self.inner.current_cards);
        let loader = self.inner.thumbnail_loader.clone();
        let status = self.inner.status_label.clone();
        let state = Rc::clone(&self.inner.state);

        spawn_task(async move { backend.history().await }, move |result| {
            state.borrow_mut().is_loading = false;
            match result {
                Ok(page) => {
                    let count = page.items.len();
                    tracing::info!("Loaded {count} items for history");
                    Self::populate_grid(&flow_box, &cards_store, page.items, &loader, false);
                    status.set_label(&format!("History ({count} videos)"));
                    flow_box.grab_focus();
                }
                Err(err) => {
                    tracing::error!("Failed to load history: {err:#}");
                    status.set_label(&format!("History error: {err}"));
                }
            }
        });
    }

    pub fn load_playlists(&self) {
        self.inner.state.borrow_mut().view = CurrentView::Playlists;
        self.inner.main_stack.set_visible_child_name("feed");
        self.inner.status_label.set_label("Loading Playlists...");
        self.inner.state.borrow_mut().is_loading = true;

        let backend = Arc::clone(&self.inner.backend);
        let flow_box = self.inner.feed_flow_box.clone();
        let cards_store = Rc::clone(&self.inner.current_cards);
        let loader = self.inner.thumbnail_loader.clone();
        let status = self.inner.status_label.clone();
        let state = Rc::clone(&self.inner.state);

        spawn_task(async move { backend.playlists().await }, move |result| {
            state.borrow_mut().is_loading = false;
            match result {
                Ok(page) => {
                    let count = page.items.len();
                    tracing::info!("Loaded {count} items for playlists");
                    Self::populate_grid(&flow_box, &cards_store, page.items, &loader, false);
                    status.set_label(&format!("Playlists ({count})"));
                    flow_box.grab_focus();
                }
                Err(err) => {
                    tracing::error!("Failed to load playlists: {err:#}");
                    status.set_label(&format!("Playlists error: {err}"));
                }
            }
        });
    }

    pub fn load_playlist(&self, playlist_id: &str) {
        self.inner.state.borrow_mut().view = CurrentView::Home;
        self.inner.main_stack.set_visible_child_name("feed");
        self.inner.status_label.set_label("Loading Playlist...");
        self.inner.state.borrow_mut().is_loading = true;

        let backend = Arc::clone(&self.inner.backend);
        let flow_box = self.inner.feed_flow_box.clone();
        let cards_store = Rc::clone(&self.inner.current_cards);
        let loader = self.inner.thumbnail_loader.clone();
        let status = self.inner.status_label.clone();
        let state = Rc::clone(&self.inner.state);
        let pid = playlist_id.to_string();
        let pid_task = pid.clone();

        spawn_task(
            async move { backend.playlist(&pid_task).await },
            move |result| {
                state.borrow_mut().is_loading = false;
                match result {
                    Ok(page) => {
                        let count = page.items.len();
                        tracing::info!("Loaded {count} items for playlist {pid}");
                        Self::populate_grid(&flow_box, &cards_store, page.items, &loader, false);
                        status.set_label(&format!("Playlist ({count} videos)"));
                        flow_box.grab_focus();
                    }
                    Err(err) => {
                        tracing::error!("Failed to load playlist: {err:#}");
                        status.set_label(&format!("Playlist error: {err}"));
                    }
                }
            },
        );
    }

    pub fn search_query(&self, q: &str) {
        let q_trimmed = q.trim().to_string();
        if q_trimmed.is_empty() {
            return;
        }

        self.inner.state.borrow_mut().view = CurrentView::Search;
        self.inner.main_stack.set_visible_child_name("feed");
        self.inner
            .status_label
            .set_label(&format!("Searching '{q_trimmed}'..."));
        self.inner.state.borrow_mut().is_loading = true;

        let backend = Arc::clone(&self.inner.backend);
        let flow_box = self.inner.feed_flow_box.clone();
        let cards_store = Rc::clone(&self.inner.current_cards);
        let loader = self.inner.thumbnail_loader.clone();
        let status = self.inner.status_label.clone();
        let state = Rc::clone(&self.inner.state);
        let query = q_trimmed.clone();
        let q_task = query.clone();

        spawn_task(
            async move {
                let filters = SearchFilters::default();
                backend.search(&q_task, &filters, None).await
            },
            move |result| {
                state.borrow_mut().is_loading = false;
                match result {
                    Ok(page) => {
                        let count = page.items.len();
                        tracing::info!("Loaded {count} results for '{query}'");
                        Self::populate_grid(&flow_box, &cards_store, page.items, &loader, false);
                        status.set_label(&format!("Results for '{query}' ({count})"));
                        flow_box.grab_focus();
                    }
                    Err(err) => {
                        tracing::error!("Failed search for '{query}': {err:#}");
                        status.set_label(&format!("Search error: {err}"));
                    }
                }
            },
        );
    }

    pub fn open_settings(&self) {
        self.inner.state.borrow_mut().view = CurrentView::Settings;
        self.inner.main_stack.set_visible_child_name("settings");
        self.inner.status_label.set_label("Settings");
    }

    pub fn open_watch_video(&self, v: VideoCard) {
        self.inner.state.borrow_mut().view = CurrentView::Watch;
        self.inner.main_stack.set_visible_child_name("watch");

        self.inner.watch_title.set_label(&v.title);
        self.inner.watch_channel.set_label(&v.channel);
        self.inner.watch_meta.set_label(&format!(
            "{} • {}",
            v.duration.as_deref().unwrap_or(""),
            v.views.as_deref().unwrap_or("")
        ));
        self.inner
            .watch_likes_lbl
            .set_label("Likes: ...  |  Dislikes: ...");

        // Clear previous description, comments, related videos
        while let Some(c) = self.inner.watch_desc_box.first_child() {
            self.inner.watch_desc_box.remove(&c);
        }
        while let Some(c) = self.inner.watch_comments_box.first_child() {
            self.inner.watch_comments_box.remove(&c);
        }
        while let Some(c) = self.inner.related_flow_box.first_child() {
            self.inner.related_flow_box.remove(&c);
        }

        // Start playback immediately in embedded libmpv
        let _ = self.inner.player.load_video(v.clone(), None);

        // Update YouTube watch history
        let b_mark = Arc::clone(&self.inner.backend);
        let vid_mark = v.id.clone();
        crate::async_runtime::runtime().spawn(async move {
            let _ = b_mark.mark_watched(&vid_mark).await;
        });

        let b = Arc::clone(&self.inner.backend);
        let p = Arc::clone(&self.inner.player);
        let vid = v.id.clone();
        let v_dur = v.duration.clone();
        let v_vw = v.views.clone();
        let wm = self.inner.watch_meta.clone();
        let wlikes = self.inner.watch_likes_lbl.clone();
        let wdesc = self.inner.watch_desc_box.clone();
        let wcomm = self.inner.watch_comments_box.clone();
        let related_flow = self.inner.related_flow_box.clone();
        let related_store = Rc::clone(&self.inner.related_cards);
        let loader = self.inner.thumbnail_loader.clone();

        spawn_task(
            async move { b.watch(&vid).await },
            move |result: Result<WatchData, anyhow::Error>| {
                if let Ok(watch_data) = result {
                    let likes = watch_data.likes.as_deref().unwrap_or("-");
                    let dislikes = watch_data.dislikes.as_deref().unwrap_or("-");
                    wm.set_label(&format!(
                        "{} • {}",
                        v_dur.as_deref().unwrap_or(""),
                        v_vw.as_deref().unwrap_or("")
                    ));
                    wlikes.set_label(&format!("Likes: {likes}  |  Dislikes: {dislikes}"));

                    // 1. Render Description with clickable timestamps and chapters
                    while let Some(c) = wdesc.first_child() {
                        wdesc.remove(&c);
                    }

                    // Chapters header if available
                    if !watch_data.chapters.is_empty() {
                        let ch_box = GtkBox::builder()
                            .orientation(Orientation::Horizontal)
                            .spacing(6)
                            .build();
                        for (sec, title) in &watch_data.chapters {
                            let ch_btn = Button::builder()
                                .label(&format!("{sec}s: {title}"))
                                .css_classes(["flat", "accent"])
                                .build();
                            let p_ch = Arc::clone(&p);
                            let s_pos = *sec as f64;
                            ch_btn.connect_clicked(move |_| {
                                let _ = p_ch.seek(s_pos, false);
                            });
                            ch_box.append(&ch_btn);
                        }
                        wdesc.append(&ch_box);
                    }

                    for line in watch_data.description.lines() {
                        let line_box = GtkBox::builder()
                            .orientation(Orientation::Horizontal)
                            .spacing(6)
                            .build();

                        let mut found_ts = None;
                        for word in line.split_whitespace() {
                            let cleaned = word.trim_matches(|c: char| !c.is_numeric() && c != ':');
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
                        wdesc.append(&line_box);
                    }

                    // 2. Render Comments
                    while let Some(c) = wcomm.first_child() {
                        wcomm.remove(&c);
                    }
                    for comment in &watch_data.comments {
                        let c_box = GtkBox::builder()
                            .orientation(Orientation::Vertical)
                            .spacing(2)
                            .margin_top(4)
                            .margin_bottom(6)
                            .build();

                        let auth_line = GtkBox::builder()
                            .orientation(Orientation::Horizontal)
                            .spacing(8)
                            .build();
                        let a_lbl = Label::builder()
                            .label(&comment.author)
                            .halign(Align::Start)
                            .css_classes(["yt-comment-author"])
                            .build();
                        auth_line.append(&a_lbl);

                        if let Some(ref pub_time) = comment.published {
                            let pub_lbl = Label::builder()
                                .label(pub_time)
                                .halign(Align::Start)
                                .css_classes(["yt-meta"])
                                .build();
                            auth_line.append(&pub_lbl);
                        }
                        c_box.append(&auth_line);

                        let text_lbl = Label::builder()
                            .label(&comment.text)
                            .wrap(true)
                            .selectable(true)
                            .halign(Align::Start)
                            .css_classes(["yt-comment-text"])
                            .build();
                        c_box.append(&text_lbl);

                        if let Some(ref likes) = comment.likes {
                            let likes_lbl = Label::builder()
                                .label(&format!("Likes: {likes}"))
                                .halign(Align::Start)
                                .css_classes(["yt-meta"])
                                .build();
                            c_box.append(&likes_lbl);
                        }

                        wcomm.append(&c_box);
                    }

                    // 3. Render Related Videos into right pane
                    if !watch_data.up_next.is_empty() {
                        Self::populate_grid(
                            &related_flow,
                            &related_store,
                            watch_data.up_next,
                            &loader,
                            true,
                        );
                    }
                }
            },
        );
    }

    pub fn present(&self) {
        self.inner.window.present();
    }
}
