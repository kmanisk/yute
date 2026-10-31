//! Playback-owned finite-range transport. No credentials enter this module.
use crate::oauth::CancellationToken;
use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use futures_util::{stream::FuturesOrdered, StreamExt};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Mutex as AsyncMutex, Semaphore},
    task::{JoinHandle, JoinSet},
};
const CHUNK: u64 = 1024 * 1024;
#[derive(Clone)]
pub struct Stream {
    url: String,
    pub length: u64,
    mime: String,
    headers: reqwest::header::HeaderMap,
}
impl Stream {
    pub fn new(url: String, length: u64, mime: String) -> Result<Self> {
        let parsed = reqwest::Url::parse(&url).map_err(|_| anyhow!("Invalid media source"))?;
        if !matches!(parsed.scheme(), "http" | "https") || length == 0 {
            bail!("Proxy needs a finite HTTP media stream");
        }
        Ok(Self {
            url,
            length,
            mime,
            headers: Default::default(),
        })
    }
}
#[derive(Clone)]
pub struct Streams {
    pub video: Stream,
    pub audio: Option<Stream>,
}
#[async_trait]
pub trait Source: Send + Sync {
    async fn resolve(&self) -> Result<Streams>;
}
pub struct Youtube {
    id: String,
    selector: AsyncMutex<String>,
}
impl Youtube {
    pub fn new(id: String, selector: String) -> Self {
        Self {
            id,
            selector: AsyncMutex::new(selector),
        }
    }
}
fn stream(value: &serde_json::Value, root: &serde_json::Value) -> Result<Stream> {
    let url = value["url"]
        .as_str()
        .ok_or_else(|| anyhow!("Resolver did not return an HTTP stream"))?;
    let parsed = reqwest::Url::parse(url).map_err(|_| anyhow!("Invalid resolver stream URL"))?;
    let length = value["filesize"]
        .as_u64()
        .or_else(|| {
            parsed
                .query_pairs()
                .find(|(k, _)| k == "clen")
                .and_then(|(_, v)| v.parse().ok())
        })
        .ok_or_else(|| anyhow!("Proxy does not support live/manifest-only streams; use mpv"))?;
    let mime = match value["ext"].as_str() {
        Some("webm") => {
            if value["vcodec"] == "none" {
                "audio/webm"
            } else {
                "video/webm"
            }
        }
        Some("m4a") => "audio/mp4",
        _ => "video/mp4",
    };
    let mut out = Stream::new(url.to_string(), length, mime.into())?;
    if let Some(h) = value
        .get("http_headers")
        .or_else(|| root.get("http_headers"))
        .and_then(|v| v.as_object())
    {
        for (k, v) in h {
            if !matches!(
                k.to_ascii_lowercase().as_str(),
                "user-agent" | "referer" | "origin" | "accept"
            ) {
                continue;
            }
            if let (Ok(name), Some(value)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                v.as_str(),
            ) {
                if let Ok(value) = reqwest::header::HeaderValue::from_str(value) {
                    out.headers.insert(name, value);
                }
            }
        }
    }
    Ok(out)
}
#[async_trait]
impl Source for Youtube {
    async fn resolve(&self) -> Result<Streams> {
        let selector = self.selector.lock().await.clone();
        let mut cmd = tokio::process::Command::new("yt-dlp");
        cmd.args([
            "--ignore-config",
            "--js-runtimes",
            "node",
            "--no-playlist",
            "--skip-download",
            "--dump-single-json",
            "--no-warnings",
            "--socket-timeout",
            "10",
            "--retries",
            "1",
            "--format",
            &selector,
            "--",
        ])
        .arg(format!("https://www.youtube.com/watch?v={}", self.id))
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
        let output = tokio::time::timeout(Duration::from_secs(45), cmd.output())
            .await
            .map_err(|_| anyhow!("Proxy resolution timed out"))?
            .map_err(|_| anyhow!("Cannot launch anonymous resolver"))?;
        if !output.status.success() {
            bail!("Anonymous stream resolution failed");
        }
        let v: serde_json::Value =
            serde_json::from_slice(&output.stdout).map_err(|_| anyhow!("Invalid resolver JSON"))?;
        let selected = v.get("requested_formats").and_then(|v| v.as_array());
        let (video, audio, ids) = if let Some(f) = selected {
            let first = f
                .iter()
                .find(|f| f["vcodec"] != "none")
                .or_else(|| f.first())
                .ok_or_else(|| anyhow!("No playable formats"))?;
            let second = f.iter().find(|f| {
                !std::ptr::eq(*f, first) && f["vcodec"] == "none" && f["acodec"] != "none"
            });
            let mut ids = vec![first["format_id"].as_str().unwrap_or_default()];
            if let Some(a) = second {
                ids.push(a["format_id"].as_str().unwrap_or_default());
            }
            (
                stream(first, &v)?,
                second.map(|a| stream(a, &v)).transpose()?,
                ids.join("+"),
            )
        } else {
            (
                stream(&v, &v)?,
                None,
                v["format_id"].as_str().unwrap_or_default().to_string(),
            )
        };
        if !ids.is_empty()
            && ids
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '_' | '-'))
        {
            *self.selector.lock().await = ids;
        }
        Ok(Streams { video, audio })
    }
}
struct Current {
    streams: Option<Arc<Streams>>,
    epoch: u64,
    last_refresh: Option<Instant>,
}
type Cached = VecDeque<((u8, u64), Arc<Vec<u8>>)>;
struct State {
    source: Arc<dyn Source>,
    current: AsyncMutex<Current>,
    cache: Mutex<Cached>,
    chunks: Semaphore,
    cancel: CancellationToken,
    client: reqwest::Client,
}
impl State {
    async fn streams(&self) -> Result<(Arc<Streams>, u64)> {
        let mut c = self.current.lock().await;
        if c.streams.is_none() {
            let s = tokio::select! {_=self.cancel.cancelled()=>bail!("Playback closed"),r=self.source.resolve()=>r?};
            c.streams = Some(Arc::new(s));
        }
        Ok((Arc::clone(c.streams.as_ref().unwrap()), c.epoch))
    }
    async fn refresh(&self, epoch: u64) -> Result<()> {
        let mut c = self.current.lock().await;
        if c.epoch != epoch {
            return Ok(());
        }
        if c.last_refresh
            .is_some_and(|t| t.elapsed() < Duration::from_secs(30))
        {
            bail!("Repeated upstream expiry; restart playback");
        }
        c.last_refresh = Some(Instant::now());
        let s = tokio::select! {_=self.cancel.cancelled()=>bail!("Playback closed"),r=self.source.resolve()=>r?};
        c.streams = Some(Arc::new(s));
        c.epoch = c.epoch.wrapping_add(1);
        self.cache.lock().unwrap().clear();
        Ok(())
    }
    async fn chunk(&self, role: u8, start: u64) -> Result<Arc<Vec<u8>>> {
        if let Some(data) = self
            .cache
            .lock()
            .unwrap()
            .iter()
            .find(|(key, _)| *key == (role, start))
            .map(|(_, v)| Arc::clone(v))
        {
            return Ok(data);
        }
        let _permit = self
            .chunks
            .acquire()
            .await
            .map_err(|_| anyhow!("Playback closed"))?;
        let mut refreshed = false;
        for attempt in 0..3 {
            let (streams, epoch) = self.streams().await?;
            let stream = if role == 0 {
                &streams.video
            } else {
                streams
                    .audio
                    .as_ref()
                    .ok_or_else(|| anyhow!("Audio stream absent"))?
            };
            let end = start
                .saturating_add(CHUNK - 1)
                .min(stream.length.saturating_sub(1));
            if start > end {
                bail!("Invalid byte range");
            }
            let response = self
                .client
                .get(&stream.url)
                .headers(stream.headers.clone())
                .header(reqwest::header::RANGE, format!("bytes={start}-{end}"))
                .send()
                .await;
            if let Ok(response) = response {
                if matches!(response.status().as_u16(), 401 | 403) && !refreshed {
                    self.refresh(epoch).await?;
                    refreshed = true;
                    continue;
                }
                let expected = end - start + 1;
                let range_ok = response.status() == reqwest::StatusCode::PARTIAL_CONTENT
                    && response
                        .headers()
                        .get(reqwest::header::CONTENT_RANGE)
                        .and_then(|s| s.to_str().ok())
                        .is_some_and(|s| s.starts_with(&format!("bytes {start}-")));
                if range_ok {
                    let mut body = response;
                    let mut bytes = Vec::with_capacity(expected as usize);
                    let mut failed = false;
                    loop {
                        let part = match body.chunk().await {
                            Ok(Some(p)) => Ok(p),
                            Ok(None) => break,
                            Err(e) => Err(e),
                        };
                        match part {
                            Ok(p) if bytes.len() + p.len() <= expected as usize => {
                                bytes.extend_from_slice(&p)
                            }
                            _ => {
                                failed = true;
                                break;
                            }
                        }
                    }
                    if !failed && bytes.len() == expected as usize {
                        let data = Arc::new(bytes);
                        let mut cache = self.cache.lock().unwrap();
                        if cache.len() >= 6 {
                            cache.pop_front();
                        }
                        cache.push_back(((role, start), Arc::clone(&data)));
                        return Ok(data);
                    }
                }
            }
            if attempt < 2 {
                tokio::time::sleep(Duration::from_millis(100 * (1 << attempt))).await;
            }
        }
        bail!("Upstream finite-range request failed after bounded retries")
    }
}
pub struct Session {
    address: std::net::SocketAddr,
    token: String,
    task: JoinHandle<()>,
    cancel: CancellationToken,
}
impl Session {
    pub fn youtube(id: String, selector: String) -> Result<Self> {
        Self::start(Arc::new(Youtube::new(id, selector)))
    }
    pub fn start(source: Arc<dyn Source>) -> Result<Self> {
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let listener = TcpListener::from_std(listener)?;
        let mut bytes = [0; 24];
        getrandom::fill(&mut bytes).map_err(|_| anyhow!("Cannot create playback capability"))?;
        let token = hex::encode(bytes);
        let cancel = CancellationToken::new();
        let state = Arc::new(State {
            source,
            current: AsyncMutex::new(Current {
                streams: None,
                epoch: 0,
                last_refresh: None,
            }),
            cache: Default::default(),
            chunks: Semaphore::new(3),
            cancel: cancel.clone(),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .connect_timeout(Duration::from_secs(3))
                .redirect(reqwest::redirect::Policy::limited(3))
                .build()?,
        });
        let path = token.clone();
        let stop = cancel.clone();
        let task = tokio::spawn(async move {
            let mut jobs = JoinSet::new();
            let permits = Arc::new(Semaphore::new(6));
            let warm = Arc::clone(&state);
            jobs.spawn(async move {
                let _ = warm.streams().await;
            });
            loop {
                tokio::select! {
                 _=stop.cancelled()=>break,
                 accepted=listener.accept()=>{let Ok((mut socket,peer))=accepted else{break;};if !peer.ip().is_loopback(){continue;}let Ok(permit)=Arc::clone(&permits).try_acquire_owned()else{continue;};let state=Arc::clone(&state);let token=path.clone();jobs.spawn(async move{let _permit=permit;let _=serve(&mut socket,state,&token).await;});},
                 _=jobs.join_next(),if !jobs.is_empty()=>{},
                }
            }
            jobs.abort_all();
            while jobs.join_next().await.is_some() {}
        });
        Ok(Self {
            address,
            token,
            task,
            cancel,
        })
    }
    pub fn url(&self, role: u8) -> String {
        format!(
            "http://{}/{}/{}",
            self.address,
            self.token,
            if role == 0 { "video" } else { "audio" }
        )
    }
    pub fn manifest(&self) -> String {
        format!("http://{}/{}/manifest", self.address, self.token)
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.task.abort();
    }
}
fn requested_range(raw: Option<&str>, length: u64) -> Result<(u64, u64)> {
    let Some(raw) = raw else {
        return Ok((0, length - 1));
    };
    let body = raw
        .strip_prefix("bytes=")
        .ok_or_else(|| anyhow!("Invalid range"))?;
    let (first, last) = body
        .split_once('-')
        .ok_or_else(|| anyhow!("Invalid range"))?;
    let start = first.parse::<u64>().map_err(|_| anyhow!("Invalid range"))?;
    let end = if last.is_empty() {
        length - 1
    } else {
        last.parse::<u64>()
            .map_err(|_| anyhow!("Invalid range"))?
            .min(length - 1)
    };
    if start > end || start >= length {
        bail!("Range outside stream");
    }
    Ok((start, end))
}
async fn reject(socket: &mut TcpStream, status: &str) -> Result<()> {
    socket
        .write_all(
            format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await?;
    Ok(())
}
async fn serve(socket: &mut TcpStream, state: Arc<State>, token: &str) -> Result<()> {
    let mut bytes = vec![];
    let read = async {
        loop {
            let b = socket.read_u8().await?;
            bytes.push(b);
            if bytes.ends_with(b"\r\n\r\n") {
                return Ok::<(), std::io::Error>(());
            }
            if bytes.len() > 8192 {
                return Err(std::io::Error::other("Headers too large"));
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(3), read)
        .await
        .map_err(|_| anyhow!("Header timeout"))??;
    let text = std::str::from_utf8(&bytes).map_err(|_| anyhow!("Invalid request"))?;
    let mut lines = text.split("\r\n");
    let mut first = lines.next().unwrap_or("").split_whitespace();
    let method = first.next().unwrap_or("");
    let path = first.next().unwrap_or("");
    if path == format!("/{token}/manifest") {
        if !matches!(method, "GET" | "HEAD") {
            return reject(socket, "405 Method Not Allowed").await;
        }
        let (streams, _) = match state.streams().await {
            Ok(s) => s,
            Err(_) => return reject(socket, "502 Bad Gateway").await,
        };
        let address = socket.local_addr()?;
        let mut edl = String::from("# mpv EDL v0\n");
        for role in 0..if streams.audio.is_some() { 2 } else { 1 } {
            let url = format!(
                "http://{address}/{token}/{}",
                if role == 0 { "video" } else { "audio" }
            );
            edl.push_str(&format!(
                "!new_stream;!no_clip;!no_chapters;%{}%{url};",
                url.len()
            ));
        }
        socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",edl.len()).as_bytes()).await?;
        if method == "GET" {
            socket.write_all(edl.as_bytes()).await?;
        }
        return Ok(());
    }
    let role = if path == format!("/{token}/video") {
        0
    } else if path == format!("/{token}/audio") {
        1
    } else {
        return reject(socket, "403 Forbidden").await;
    };
    if !matches!(method, "GET" | "HEAD") {
        return reject(socket, "405 Method Not Allowed").await;
    }
    let range = lines
        .filter_map(|s| s.split_once(':'))
        .find(|(k, _)| k.eq_ignore_ascii_case("range"))
        .map(|(_, v)| v.trim());
    let (streams, _) = match state.streams().await {
        Ok(s) => s,
        Err(_) => return reject(socket, "502 Bad Gateway").await,
    };
    let source = if role == 0 {
        &streams.video
    } else if let Some(a) = &streams.audio {
        a
    } else {
        return reject(socket, "404 Not Found").await;
    };
    let (start, end) = match requested_range(range, source.length) {
        Ok(r) => r,
        Err(_) => return reject(socket, "416 Range Not Satisfiable").await,
    };
    let status = if range.is_some() {
        "206 Partial Content"
    } else {
        "200 OK"
    };
    let content_range = if range.is_some() {
        format!("Content-Range: bytes {start}-{end}/{}\r\n", source.length)
    } else {
        String::new()
    };
    socket.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: {}\r\nAccept-Ranges: bytes\r\n{content_range}Connection: close\r\n\r\n",end-start+1,source.mime).as_bytes()).await?;
    if method == "HEAD" {
        return Ok(());
    }
    let (mut reader, mut writer) = socket.split();
    let transfer = async {
        let mut offset = start / CHUNK * CHUNK;
        let mut pending = FuturesOrdered::new();
        for _ in 0..2 {
            if offset > end {
                break;
            }
            pending.push_back(fetch(Arc::clone(&state), role, offset));
            offset = offset.saturating_add(CHUNK);
        }
        while let Some(result) = pending.next().await {
            let (at, data) = result?;
            let from = start.saturating_sub(at) as usize;
            let until = (end - at + 1).min(data.len() as u64) as usize;
            writer.write_all(&data[from..until]).await?;
            if offset <= end {
                pending.push_back(fetch(Arc::clone(&state), role, offset));
                offset = offset.saturating_add(CHUNK);
            }
        }
        Ok::<(), anyhow::Error>(())
    };
    let mut disconnect = [0; 1];
    tokio::select! {_=state.cancel.cancelled()=>{},_=reader.read(&mut disconnect)=>{},_result=transfer=>{}}
    Ok(())
}
async fn fetch(state: Arc<State>, role: u8, at: u64) -> Result<(u64, Arc<Vec<u8>>)> {
    let data = state.chunk(role, at).await?;
    Ok((at, data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Origin {
        url: String,
        task: JoinHandle<()>,
        ranges: Arc<AtomicUsize>,
        active: Arc<AtomicUsize>,
    }
    impl Drop for Origin {
        fn drop(&mut self) {
            self.task.abort();
        }
    }
    async fn origin() -> Origin {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let ranges = Arc::new(AtomicUsize::new(0));
        let active = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&ranges);
        let work = Arc::clone(&active);
        let task = tokio::spawn(async move {
            let mut jobs = JoinSet::new();
            loop {
                tokio::select! {accepted=listener.accept()=>{let Ok((mut s,_))=accepted else{break};let count=Arc::clone(&count);let work=Arc::clone(&work);jobs.spawn(async move{
                 let mut bytes=vec![];while !bytes.ends_with(b"\r\n\r\n"){let Ok(b)=s.read_u8().await else{return};bytes.push(b);}
                 let text=String::from_utf8_lossy(&bytes);let path=text.split_whitespace().nth(1).unwrap_or("");
                 if path=="/expired"{let _=reject(&mut s,"403 Forbidden").await;return;}
                 let range=text.lines().filter_map(|l|l.split_once(':')).find(|(k,_)|k.eq_ignore_ascii_case("range")).map(|(_,v)|v.trim());
                 if range.is_none(){tokio::time::sleep(Duration::from_secs(2)).await;return;}
                 let attempt=count.fetch_add(1,Ordering::SeqCst);let (start,end)=requested_range(range,CHUNK*4).unwrap();
                 if path=="/reset"&&attempt==0{let _=s.write_all(b"HTTP/1.1 206 Partial Content\r\nContent-Length: 1048576\r\nContent-Range: bytes 0-1048575/4194304\r\n\r\nx").await;return;}
                 struct Busy(Arc<AtomicUsize>);impl Drop for Busy{fn drop(&mut self){self.0.fetch_sub(1,Ordering::SeqCst);}}
                 work.fetch_add(1,Ordering::SeqCst);let _busy=Busy(work);
                 if path=="/slow"{let _=s.write_all(format!("HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\n\r\n",end-start+1,CHUNK*4).as_bytes()).await;let mut b=[0;1];let _=s.read(&mut b).await;return;}
                 let _=s.write_all(format!("HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {start}-{end}/{}\r\n\r\n",end-start+1,CHUNK*4).as_bytes()).await;
                 let data=vec![b'v';(end-start+1)as usize];let _=s.write_all(&data).await;
                });},_=jobs.join_next(),if !jobs.is_empty()=>{}}
            }
        });
        Origin {
            url,
            task,
            ranges,
            active,
        }
    }
    struct Mock {
        url: String,
        calls: AtomicUsize,
        expiry: bool,
    }
    #[async_trait]
    impl Source for Mock {
        async fn resolve(&self) -> Result<Streams> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            let path = if self.expiry && n == 0 {
                "expired"
            } else {
                "video"
            };
            Ok(Streams {
                video: Stream::new(
                    format!("{}/{path}", self.url),
                    CHUNK * 4,
                    "video/webm".into(),
                )?,
                audio: None,
            })
        }
    }
    fn source(url: &str, expiry: bool) -> Arc<Mock> {
        Arc::new(Mock {
            url: url.into(),
            calls: AtomicUsize::new(0),
            expiry,
        })
    }
    #[tokio::test]
    async fn finite_ranges_recover_synthetic_open_connection_stall_and_reject_unknown_paths() {
        let upstream = origin().await;
        let c = reqwest::Client::new();
        assert!(tokio::time::timeout(
            Duration::from_millis(100),
            c.get(format!("{}/video", upstream.url)).send()
        )
        .await
        .is_err());
        let session = Session::start(source(&upstream.url, false)).unwrap();
        let r = c
            .get(session.url(0))
            .header("Range", "bytes=100-199")
            .send()
            .await
            .unwrap();
        assert_eq!(r.status(), 206);
        assert_eq!(r.bytes().await.unwrap().as_ref(), &[b'v'; 100]);
        assert!(upstream.ranges.load(Ordering::SeqCst) > 0);
        let bad = format!("http://{}/wrong/video", session.address);
        assert_eq!(c.get(bad).send().await.unwrap().status(), 403);
        let manifest = c
            .get(session.manifest())
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert_eq!(manifest.matches("!new_stream").count(), 1);
        assert!(!manifest.contains("/audio"));
        let address = session.address;
        drop(session);
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert!(TcpStream::connect(address).await.is_err());
    }
    #[tokio::test]
    async fn expiry_resolves_again_and_connection_reset_is_retried() {
        let o = origin().await;
        let source = source(&o.url, true);
        let session = Session::start(source.clone()).unwrap();
        let c = reqwest::Client::new();
        let r = c
            .get(session.url(0))
            .header("Range", "bytes=0-99")
            .send()
            .await
            .unwrap();
        assert_eq!(r.bytes().await.unwrap().len(), 100);
        assert_eq!(source.calls.load(Ordering::SeqCst), 2);
        let o2 = origin().await;
        struct Reset(String);
        #[async_trait]
        impl Source for Reset {
            async fn resolve(&self) -> Result<Streams> {
                Ok(Streams {
                    video: Stream::new(
                        format!("{}/reset", self.0),
                        CHUNK * 4,
                        "video/webm".into(),
                    )?,
                    audio: None,
                })
            }
        }
        let s = Session::start(Arc::new(Reset(o2.url.clone()))).unwrap();
        assert_eq!(
            c.get(s.url(0))
                .header("Range", "bytes=0-99")
                .send()
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap()
                .len(),
            100
        );
        assert_eq!(o2.ranges.load(Ordering::SeqCst), 2);
    }
    #[tokio::test]
    async fn disconnected_seek_cancels_old_upstream_requests() {
        let o = origin().await;
        struct Slow(String);
        #[async_trait]
        impl Source for Slow {
            async fn resolve(&self) -> Result<Streams> {
                Ok(Streams {
                    video: Stream::new(format!("{}/slow", self.0), CHUNK * 4, "video/webm".into())?,
                    audio: None,
                })
            }
        }
        let s = Session::start(Arc::new(Slow(o.url.clone()))).unwrap();
        let mut socket = TcpStream::connect(s.address).await.unwrap();
        socket
            .write_all(
                format!(
                    "GET /{}/video HTTP/1.1\r\nHost: localhost\r\nRange: bytes=0-4194303\r\n\r\n",
                    s.token
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        for _ in 0..50 {
            if o.active.load(Ordering::SeqCst) > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(o.active.load(Ordering::SeqCst) > 0);
        assert!(o.active.load(Ordering::SeqCst) <= 3);
        drop(socket);
        for _ in 0..50 {
            if o.active.load(Ordering::SeqCst) == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(o.active.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn range_validation() {
        assert_eq!(requested_range(Some("bytes=10-"), 20).unwrap(), (10, 19));
        assert!(requested_range(Some("bytes=20-"), 20).is_err());
        assert!(requested_range(Some("bytes=0-1,3-4"), 20).is_err());
    }
}
