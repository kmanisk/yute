use gtk4::glib;
use std::future::Future;
use std::sync::OnceLock;
use tokio::runtime::Runtime;

static RUNTIME: OnceLock<Runtime> = OnceLock::new();

pub fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .thread_name("ytui-tokio")
            .build()
            .expect("Failed to initialize Tokio runtime")
    })
}

pub fn spawn_task<F, T>(future: F, on_done: impl FnOnce(T) + 'static)
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = tokio::sync::oneshot::channel::<T>();
    runtime().spawn(async move {
        let result = future.await;
        let _ = tx.send(result);
    });

    glib::MainContext::default().spawn_local(async move {
        if let Ok(res) = rx.await {
            on_done(res);
        }
    });
}
