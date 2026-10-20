mod app;
pub mod async_runtime;
mod gl_loader;
mod player_embedded;
mod render_context;
mod settings_view;
mod thumbnail_loader;
mod video_area;
mod video_card;

use app::GuiApp;
use gtk4::gdk::Display;
use gtk4::prelude::*;
use gtk4::{Application, CssProvider, STYLE_PROVIDER_PRIORITY_APPLICATION};
use std::cell::RefCell;
use std::rc::Rc;
use ytui_core::config::Config;

const APP_ID: &str = "io.github.manisk.ytui_gui";

const CSS_THEME: &str = r#"
window {
    background-color: #0f0f0f;
    color: #f1f1f1;
}

.yt-sidebar {
    background-color: #0f0f0f;
    border-right: 1px solid #272727;
}

.yt-nav-btn {
    padding: 10px 14px;
    font-size: 14px;
    font-weight: 500;
    border-radius: 10px;
    background: transparent;
    border: none;
    color: #f1f1f1;
}

.yt-nav-btn:hover {
    background-color: #272727;
}

.yt-logo {
    font-size: 19px;
    font-weight: 800;
    letter-spacing: -0.5px;
    color: #ffffff;
}

.yt-logo-badge {
    background-color: #ff0000;
    color: #ffffff;
    font-weight: bold;
    font-size: 12px;
    padding: 2px 6px;
    border-radius: 4px;
}

.yt-card {
    background-color: transparent;
    border-radius: 12px;
    padding: 6px;
}

.yt-card:hover {
    background-color: #222222;
}

.yt-card:focus, .yt-card:focus-visible, .yt-card:selected {
    outline: 2px solid #3ea6ff;
    outline-offset: 2px;
    background-color: #222222;
}

.yt-card-compact {
    background-color: transparent;
    border-radius: 8px;
    padding: 4px;
}

.yt-card-compact:hover {
    background-color: #222222;
}

.yt-card-compact:focus, .yt-card-compact:focus-visible, .yt-card-compact:selected {
    outline: 2px solid #3ea6ff;
    outline-offset: 2px;
    background-color: #222222;
}

.yt-thumb-container {
    border-radius: 12px;
    background-color: #1a1a1a;
}

.yt-thumb-container-compact {
    border-radius: 8px;
    background-color: #1a1a1a;
}

.yt-avatar-circle {
    background: linear-gradient(135deg, #cc0000, #ff4444);
    color: #ffffff;
    border-radius: 18px;
    font-size: 14px;
    font-weight: bold;
}

.yt-duration-badge {
    background-color: rgba(0, 0, 0, 0.85);
    color: #ffffff;
    font-size: 11px;
    font-weight: 600;
    padding: 2px 6px;
    border-radius: 4px;
}

.yt-live-badge {
    background-color: #cc0000;
    color: #ffffff;
    font-size: 11px;
    font-weight: bold;
    padding: 2px 6px;
    border-radius: 4px;
}

.yt-title {
    font-size: 14px;
    font-weight: 600;
    line-height: 1.3;
    color: #f1f1f1;
}

.yt-title-compact {
    font-size: 13px;
    font-weight: 500;
    line-height: 1.25;
    color: #f1f1f1;
}

.yt-channel-name {
    font-size: 12px;
    color: #aaaaaa;
    font-weight: 500;
}

.yt-meta {
    font-size: 12px;
    color: #aaaaaa;
}

.yt-video-wrapper {
    background-color: #000000;
    border-radius: 12px;
}

.yt-desc-box {
    background-color: #222222;
    border-radius: 12px;
    padding: 14px;
}

.yt-sub-btn {
    background-color: #cc0000;
    color: #ffffff;
    font-weight: bold;
    border-radius: 18px;
    padding: 6px 16px;
}

.yt-action-pill {
    background-color: #272727;
    color: #f1f1f1;
    font-size: 13px;
    font-weight: 500;
    border-radius: 18px;
    padding: 6px 14px;
}
"#;

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

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,ytui_gui=debug,ytui_core=debug"));

    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(log_file_path)
    {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(file)
            .with_ansi(false)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .init();
    }
}

fn load_theme() {
    let provider = CssProvider::new();
    provider.load_from_data(CSS_THEME);
    if let Some(display) = Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn main() {
    unsafe {
        libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr());
    }

    init_logging();
    let rt = async_runtime::runtime();
    let _rt_guard = rt.enter();

    let cfg = Config::load();

    let app = Application::builder().application_id(APP_ID).build();

    let gui_holder: Rc<RefCell<Option<GuiApp>>> = Rc::new(RefCell::new(None));

    let holder = Rc::clone(&gui_holder);
    let cfg_clone = cfg.clone();
    app.connect_activate(move |application| {
        unsafe {
            libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr());
        }
        load_theme();

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
