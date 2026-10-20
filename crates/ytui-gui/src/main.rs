mod app;
pub mod async_runtime;
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
        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,ytui_gui=debug,ytui_core=debug"));

        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(file)
            .with_ansi(false)
            .init();
    }
}

fn main() {
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr());
    }

    init_logging();
    let _ = async_runtime::runtime();

    let cfg = Config::load();

    let app = Application::builder().application_id(APP_ID).build();

    let gui_holder: Rc<RefCell<Option<GuiApp>>> = Rc::new(RefCell::new(None));

    let holder = Rc::clone(&gui_holder);
    let cfg_clone = cfg.clone();
    app.connect_activate(move |application| {
        unsafe {
            libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr());
        }
        match GuiApp::new(application, cfg_clone.clone()) {
            Ok(gui) => {
                gui.present();
                *holder.borrow_mut() = Some(gui);
            }
            Err(err) => {
                eprintln!("Failed to initialize ytui-gui: {err:#}");
                tracing::error!("Failed to initialize ytui-gui: {err:#}");
            }
        }
    });

    app.run_with_args(&Vec::<String>::new());
}
