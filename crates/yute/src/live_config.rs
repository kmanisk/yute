//! Filesystem-event driven reloads; quiet debounce is armed only after a change.
use crate::{config::Config, events::AppEvent, theme::Theme};
use notify::{RecursiveMode, Watcher};
use tokio::sync::mpsc::Sender;
pub fn load(light: Option<bool>) -> anyhow::Result<(Config, Theme)> {
    let c = Config::load_checked()?;
    let t = Theme::resolve(&c, light)?;
    Ok((c, t))
}
pub fn watch(tx: Sender<AppEvent>) {
    tokio::spawn(async move {
        let path = crate::config::config_dir();
        if tokio::fs::create_dir_all(path.join("themes"))
            .await
            .is_err()
        {
            return;
        }
        let (events, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let Ok(mut watcher) =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if let Ok(event) = event {
                    if !matches!(event.kind, notify::EventKind::Access(_))
                        && event.paths.iter().any(|p| {
                            p.file_name().is_some_and(|s| s == "config.toml")
                                || p.components().any(|s| s.as_os_str() == "themes")
                        })
                    {
                        let _ = events.send(());
                    }
                }
            })
        else {
            return;
        };
        if watcher.watch(&path, RecursiveMode::Recursive).is_err() {
            return;
        }
        while rx.recv().await.is_some() {
            while tokio::time::timeout(std::time::Duration::from_millis(150), rx.recv())
                .await
                .is_ok()
            {}
            let result = tokio::task::spawn_blocking(|| load(crate::terminal::background_light()))
                .await
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Theme loader failed")));
            if tx
                .send(AppEvent::ConfigReloaded(Box::new(result)))
                .await
                .is_err()
            {
                break;
            }
        }
    });
}
pub fn reload(tx: Sender<AppEvent>, mut c: Config, name: Option<String>, persist: bool) {
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            if let Some(name) = name {
                c.general.theme = name;
            } else if !persist {
                return load(crate::terminal::background_light());
            }
            let theme = Theme::resolve(&c, crate::terminal::background_light())?;
            if persist {
                c.save()?;
            }
            Ok((c, theme))
        })
        .await
        .unwrap_or_else(|_| Err(anyhow::anyhow!("Theme loader failed")));
        let _ = tx.send(AppEvent::ConfigReloaded(Box::new(result))).await;
    });
}
