use reqwest::Client;
use std::{sync::OnceLock, time::Duration};

/// Clones share connections; identities are supplied per request, never as defaults.
pub fn client() -> Client {
    static HTTP: OnceLock<Client> = OnceLock::new();
    HTTP.get_or_init(build_client).clone()
}

fn build_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .tcp_nodelay(true)
        .tcp_keepalive(Duration::from_secs(60))
        .pool_idle_timeout(Duration::from_secs(90))
        .pool_max_idle_per_host(10)
        .http2_keep_alive_interval(Duration::from_secs(15))
        .http2_keep_alive_timeout(Duration::from_secs(5))
        .http2_keep_alive_while_idle(true)
        .brotli(true)
        .gzip(true)
        .build()
        .unwrap_or_default()
}
