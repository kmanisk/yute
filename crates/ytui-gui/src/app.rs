use crate::async_runtime::spawn_task;
use crate::player_embedded::EmbeddedPlayer;
use crate::video_area::VideoArea;
use gtk4::gdk::Key;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, Entry, EventControllerKey,
    Label, ListBox, ListBoxRow, Orientation, Paned, PolicyType, Scale, ScrolledWindow,
    SelectionMode, Stack,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use ytui_core::backend::auth::AuthContext;
use ytui_core::backend::innertube::InnerTubeBackend;
use ytui_core::backend::models::{parse_duration_to_seconds, Card, LiveState, SearchFilters, VideoCard};
use ytui_core::backend::ytdlp::YtDlpBackend;
use ytui_core::backend::{Backend, BackendChain};
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
    #[allow(dead_code)]
    pub video_area: VideoArea,
    // UI elements
    pub main_stack: Stack,
    pub feed_list_box: ListBox,
    pub feed_scrolled: ScrolledWindow,
    pub search_entry: Entry,
    pub status_label: Label,
    pub current_cards: Rc<RefCell<Vec<Card>>>,
    // Watch view elements
    pub watch_title: Label,
    pub watch_channel: Label,
    pub watch_meta: Label,
    pub watch_desc_box: GtkBox,
    pub watch_comments_box: GtkBox,
    pub time_label: Label,
    pub seek_scale: Scale,
    pub play_pause_btn: Button,
    pub watch_paned: Paned,
}

impl GuiApp {
    pub fn new(app: &Application, cfg: Config) -> anyhow::Result<Self> {
        let auth_path = ytui_core::config::config_dir().join("auth.json");
        let cookies_path = ytui_core::config::config_dir().join("cookies.txt");

        let mut auth = AuthContext::default();
        let _ = auth.load_auth_with_mode(&auth_path, &cookies_path, &cfg.backend.auth);

        let cookies_from_browser = if !cfg.backend.cookies_from_browser.is_empty() {
            Some(cfg.backend.cookies_from_browser.clone())
        } else {
            let brave_origin = ytui_core::config::resolve_path("~/.config/BraveSoftware/Brave-Origin");
            if brave_origin.exists() {
                Some(format!("brave:{}", brave_origin.display()))
            } else {
                None
            }
        };

        let auth_arc = Arc::new(Mutex::new(auth));
        let inner_backend = Box::new(InnerTubeBackend::from_arc(Arc::clone(&auth_arc)));
        let ytdlp_backend = Box::new(YtDlpBackend::new(Some(cookies_path), cookies_from_browser));
        let backend: Arc<dyn Backend> = Arc::new(BackendChain::new(vec![inner_backend, ytdlp_backend]));

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

        let feed_list_box = ListBox::builder()
            .selection_mode(SelectionMode::Single)
            .show_separators(true)
            .can_focus(true)
            .focusable(true)
            .build();

        let feed_scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(PolicyType::Never)
            .vscrollbar_policy(PolicyType::Automatic)
            .hexpand(true)
            .vexpand(true)
            .child(&feed_list_box)
            .build();

        let main_stack = Stack::builder().hexpand(true).vexpand(true).build();

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

        let inner = Rc::new(GuiAppInner {
            window,
            player,
            backend,
            state,
            video_area,
            main_stack,
            feed_list_box,
            feed_scrolled,
            search_entry,
            status_label,
            current_cards: Rc::new(RefCell::new(Vec::new())),
            watch_title,
            watch_channel,
            watch_meta,
            watch_desc_box,
            watch_comments_box,
            time_label,
            seek_scale,
            play_pause_btn,
            watch_paned,
        });

        let app_inst = Self { inner };
        app_inst.build_ui();
        app_inst.setup_shortcuts();
        app_inst.setup_polling_timer();
        app_inst.load_home_feed();

        Ok(app_inst)
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
        header_bar.append(&self.inner.search_entry);
        header_bar.append(&self.inner.status_label);

        root_box.append(&header_bar);

        // 2. Feed Page Container
        self.inner.main_stack.add_named(&self.inner.feed_scrolled, Some("feed"));

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
        video_container.append(&self.inner.video_area.widget);

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

        controls_bar.append(&self.inner.play_pause_btn);
        controls_bar.append(&seek_back_btn);
        controls_bar.append(&seek_fwd_btn);
        controls_bar.append(&self.inner.seek_scale);
        controls_bar.append(&self.inner.time_label);
        controls_bar.append(&theater_btn);
        controls_bar.append(&fs_btn);

        video_container.append(&controls_bar);

        // Metadata box
        let meta_box = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(4)
            .margin_top(6)
            .build();
        meta_box.append(&self.inner.watch_title);
        meta_box.append(&self.inner.watch_channel);
        meta_box.append(&self.inner.watch_meta);

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
            .child(&self.inner.watch_desc_box)
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
            .child(&self.inner.watch_comments_box)
            .build();
        right_details_box.append(&comments_scroll);

        self.inner.watch_paned.set_start_child(Some(&left_video_pane));
        self.inner.watch_paned.set_end_child(Some(&right_details_box));
        self.inner.watch_paned.set_position(820);

        watch_box.append(&self.inner.watch_paned);
        self.inner.main_stack.add_named(&watch_box, Some("watch"));

        root_box.append(&self.inner.main_stack);
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

        let app_search = self.clone();
        self.inner.search_entry.connect_activate(move |entry| {
            app_search.search_query(&entry.text());
        });

        // Connect Row Activated in Feed List
        let app_row = self.clone();
        self.inner.feed_list_box.connect_row_activated(move |_, row| {
            let idx = row.index() as usize;
            if let Some(item) = app_row.inner.current_cards.borrow().get(idx).cloned() {
                match item {
                    Card::Video(v) => {
                        app_row.open_watch_video(v);
                    }
                    Card::Playlist(p) => {
                        app_row.load_playlist(&p.id);
                    }
                    _ => {}
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

        let paned_t = self.inner.watch_paned.clone();
        let state_t = Rc::clone(&self.inner.state);
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
        let list_ref = self.inner.feed_list_box.clone();
        let window_ref = self.inner.window.clone();
        let paned_ref = self.inner.watch_paned.clone();
        let app_shortcuts = self.clone();

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
                    list_ref.grab_focus();
                    return glib::Propagation::Stop;
                }
                return glib::Propagation::Proceed;
            }

            let is_watch_view = state_ref.borrow().view == CurrentView::Watch;

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
                        list_ref.grab_focus();
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
                Key::Up if is_watch_view => {
                    let cur_vol = player_ref.status.lock().unwrap().volume;
                    let _ = player_ref.set_volume((cur_vol + 5.0).min(100.0));
                    glib::Propagation::Stop
                }
                Key::Down if is_watch_view => {
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
                        paned_ref.set_position(560);
                    } else {
                        paned_ref.set_orientation(Orientation::Horizontal);
                        paned_ref.set_position(820);
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
                // When in feed list: let Up, Down, PageUp, PageDown, Enter pass to ListBox!
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

    fn populate_feed(
        feed_list: &ListBox,
        cards_store: &Rc<RefCell<Vec<Card>>>,
        items: Vec<Card>,
    ) {
        while let Some(child) = feed_list.first_child() {
            feed_list.remove(&child);
        }

        let mut cards = Vec::new();

        for item in items {
            let row = ListBoxRow::builder()
                .activatable(true)
                .selectable(true)
                .can_focus(true)
                .focusable(true)
                .build();

            let card_box = GtkBox::builder()
                .orientation(Orientation::Vertical)
                .spacing(3)
                .margin_top(8)
                .margin_bottom(8)
                .margin_start(16)
                .margin_end(16)
                .build();

            match &item {
                Card::Video(v) => {
                    let mut prefix = String::new();
                    if v.badges.iter().any(|b| b.eq_ignore_ascii_case("shorts")) {
                        prefix.push_str("[SHORTS] ");
                    }
                    if v.live == LiveState::Live {
                        prefix.push_str("[LIVE] ");
                    }

                    let title_lbl = Label::builder()
                        .label(&format!("{prefix}{}", v.title))
                        .wrap(true)
                        .halign(Align::Start)
                        .css_classes(["title-4"])
                        .build();

                    let mut meta = Vec::new();
                    if !v.channel.is_empty() {
                        meta.push(v.channel.clone());
                    }
                    if let Some(ref dur) = v.duration {
                        if !dur.is_empty() {
                            meta.push(dur.clone());
                        }
                    }
                    if let Some(ref views) = v.views {
                        if !views.is_empty() {
                            meta.push(views.clone());
                        }
                    }
                    if let Some(ref pub_time) = v.published {
                        if !pub_time.is_empty() {
                            meta.push(pub_time.clone());
                        }
                    }

                    let meta_lbl = Label::builder()
                        .label(&meta.join(" • "))
                        .halign(Align::Start)
                        .css_classes(["dim-label"])
                        .build();

                    card_box.append(&title_lbl);
                    card_box.append(&meta_lbl);
                }
                Card::Playlist(p) => {
                    let title_lbl = Label::builder()
                        .label(&format!("[PLAYLIST] {}", p.title))
                        .wrap(true)
                        .halign(Align::Start)
                        .css_classes(["title-4"])
                        .build();

                    let mut meta = Vec::new();
                    if !p.channel.is_empty() {
                        meta.push(p.channel.clone());
                    }
                    if let Some(ref count) = p.video_count {
                        meta.push(format!("{count} videos"));
                    }

                    let meta_lbl = Label::builder()
                        .label(&meta.join(" • "))
                        .halign(Align::Start)
                        .css_classes(["dim-label"])
                        .build();

                    card_box.append(&title_lbl);
                    card_box.append(&meta_lbl);
                }
                Card::Channel(c) => {
                    let title_lbl = Label::builder()
                        .label(&format!("[CHANNEL] {}", c.title))
                        .wrap(true)
                        .halign(Align::Start)
                        .css_classes(["title-4"])
                        .build();

                    let meta_lbl = Label::builder()
                        .label(c.subscribers.as_deref().unwrap_or("Channel"))
                        .halign(Align::Start)
                        .css_classes(["dim-label"])
                        .build();

                    card_box.append(&title_lbl);
                    card_box.append(&meta_lbl);
                }
            }

            row.set_child(Some(&card_box));
            feed_list.append(&row);
            cards.push(item);
        }

        *cards_store.borrow_mut() = cards;

        if let Some(first_row) = feed_list.row_at_index(0) {
            feed_list.select_row(Some(&first_row));
        }
    }

    pub fn load_home_feed(&self) {
        self.inner.state.borrow_mut().view = CurrentView::Home;
        self.inner.main_stack.set_visible_child_name("feed");
        self.inner.status_label.set_label("Loading Home Feed...");
        self.inner.state.borrow_mut().is_loading = true;

        let backend = Arc::clone(&self.inner.backend);
        let list_box = self.inner.feed_list_box.clone();
        let cards_store = Rc::clone(&self.inner.current_cards);
        let status = self.inner.status_label.clone();
        let state = Rc::clone(&self.inner.state);

        spawn_task(
            async move { backend.home(None).await },
            move |result| {
                state.borrow_mut().is_loading = false;
                match result {
                    Ok(page) => {
                        let count = page.items.len();
                        tracing::info!("Loaded {count} items for home feed");
                        Self::populate_feed(&list_box, &cards_store, page.items);
                        status.set_label(&format!("Home ({count} videos)"));
                        list_box.grab_focus();
                    }
                    Err(err) => {
                        tracing::error!("Failed to load home feed: {err:#}");
                        status.set_label(&format!("Home error: {err}"));
                    }
                }
            },
        );
    }

    pub fn load_subscriptions(&self) {
        self.inner.state.borrow_mut().view = CurrentView::Subscriptions;
        self.inner.main_stack.set_visible_child_name("feed");
        self.inner.status_label.set_label("Loading Subscriptions...");
        self.inner.state.borrow_mut().is_loading = true;

        let backend = Arc::clone(&self.inner.backend);
        let list_box = self.inner.feed_list_box.clone();
        let cards_store = Rc::clone(&self.inner.current_cards);
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
                        Self::populate_feed(&list_box, &cards_store, page.items);
                        status.set_label(&format!("Subscriptions ({count} videos)"));
                        list_box.grab_focus();
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
        let list_box = self.inner.feed_list_box.clone();
        let cards_store = Rc::clone(&self.inner.current_cards);
        let status = self.inner.status_label.clone();
        let state = Rc::clone(&self.inner.state);

        spawn_task(
            async move { backend.history().await },
            move |result| {
                state.borrow_mut().is_loading = false;
                match result {
                    Ok(page) => {
                        let count = page.items.len();
                        tracing::info!("Loaded {count} items for history");
                        Self::populate_feed(&list_box, &cards_store, page.items);
                        status.set_label(&format!("History ({count} videos)"));
                        list_box.grab_focus();
                    }
                    Err(err) => {
                        tracing::error!("Failed to load history: {err:#}");
                        status.set_label(&format!("History error: {err}"));
                    }
                }
            },
        );
    }

    pub fn load_playlist(&self, playlist_id: &str) {
        self.inner.state.borrow_mut().view = CurrentView::Home;
        self.inner.main_stack.set_visible_child_name("feed");
        self.inner.status_label.set_label("Loading Playlist...");
        self.inner.state.borrow_mut().is_loading = true;

        let backend = Arc::clone(&self.inner.backend);
        let list_box = self.inner.feed_list_box.clone();
        let cards_store = Rc::clone(&self.inner.current_cards);
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
                        Self::populate_feed(&list_box, &cards_store, page.items);
                        status.set_label(&format!("Playlist ({count} videos)"));
                        list_box.grab_focus();
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
        self.inner.status_label.set_label(&format!("Searching '{q_trimmed}'..."));
        self.inner.state.borrow_mut().is_loading = true;

        let backend = Arc::clone(&self.inner.backend);
        let list_box = self.inner.feed_list_box.clone();
        let cards_store = Rc::clone(&self.inner.current_cards);
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
                        Self::populate_feed(&list_box, &cards_store, page.items);
                        status.set_label(&format!("Results for '{query}' ({count})"));
                        list_box.grab_focus();
                    }
                    Err(err) => {
                        tracing::error!("Failed search for '{q_trimmed}': {err:#}");
                        status.set_label(&format!("Search error: {err}"));
                    }
                }
            },
        );
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

        // Clear previous description and comments
        while let Some(c) = self.inner.watch_desc_box.first_child() {
            self.inner.watch_desc_box.remove(&c);
        }
        while let Some(c) = self.inner.watch_comments_box.first_child() {
            self.inner.watch_comments_box.remove(&c);
        }

        let _ = self.inner.player.load_video(v.clone(), None);

        let b = Arc::clone(&self.inner.backend);
        let p = Arc::clone(&self.inner.player);
        let vid = v.id.clone();
        let v_dur = v.duration.clone();
        let v_vw = v.views.clone();
        let wm = self.inner.watch_meta.clone();
        let wdesc = self.inner.watch_desc_box.clone();
        let wcomm = self.inner.watch_comments_box.clone();

        spawn_task(
            async move { b.watch(&vid).await },
            move |result| {
                if let Ok(watch_data) = result {
                    let likes = watch_data.likes.as_deref().unwrap_or("-");
                    let dislikes = watch_data.dislikes.as_deref().unwrap_or("-");
                    wm.set_label(&format!(
                        "{} • {}  |  Likes: {}  Dislikes: {}",
                        v_dur.as_deref().unwrap_or(""),
                        v_vw.as_deref().unwrap_or(""),
                        likes,
                        dislikes
                    ));

                    // Render Description with clickable timestamps
                    while let Some(c) = wdesc.first_child() {
                        wdesc.remove(&c);
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

                    // Render Comments
                    while let Some(c) = wcomm.first_child() {
                        wcomm.remove(&c);
                    }
                    for comment in &watch_data.comments {
                        let c_lbl = Label::builder()
                            .label(&format!("{}: {}", comment.author, comment.text))
                            .wrap(true)
                            .selectable(true)
                            .halign(Align::Start)
                            .build();
                        wcomm.append(&c_lbl);
                    }
                }
            },
        );
    }

    pub fn present(&self) {
        self.inner.window.present();
    }
}
