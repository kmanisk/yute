mod app;
mod gl_loader;
mod player_embedded;
mod render_context;
mod video_area;

use app::GuiApp;
use gtk4::prelude::*;
use gtk4::Application;
use std::cell::RefCell;
use std::rc::Rc;
use ytui_core::config::Config;

const APP_ID: &str = "io.github.manisk.ytui_gui";

fn init_logging() {
    let cache_dir = if let Some(runtime_dir) = std::env::var_os("XDG_CACHE_HOME") {
        std::path::PathBuf::from(runtime_dir).join("ytui")
    } else if let Some(home) = std::env::var_os("HOME") {
        std::path::PathBuf::from(home).join(".cache/ytui")
    } else {
        std::path::PathBuf::from("/tmp/ytui")
    };

    let _ = std::fs::create_dir_all(&cache_dir);
    let log_file_path = cache_dir.join("gui.log");

    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file_path)
    {
        tracing_subscriber::fmt()
            .with_writer(file)
            .with_ansi(false)
            .init();
    }
}

fn main() {
    init_logging();

    let cfg = Config::load();

    let app = Application::builder().application_id(APP_ID).build();

    let gui_holder: Rc<RefCell<Option<GuiApp>>> = Rc::new(RefCell::new(None));

    let holder = Rc::clone(&gui_holder);
    let cfg_clone = cfg.clone();
    app.connect_activate(move |application| {
        let gui = GuiApp::new(application, cfg_clone.clone());
        gui.present();
        *holder.borrow_mut() = Some(gui);
    });

    app.run_with_args(&Vec::<String>::new());
}
