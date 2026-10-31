# Playback network and seek recovery

Yute keeps playback in external MPV. The network policy works around seek/buffering failures in fragmented YouTube streams without a permanent background service. Fresh installs now use a 60-second target; 90/120-second tiers are available for users who want more read-ahead, but transport throughput is treated separately from cache size.

## What was measured

The development audit used MPV 0.41, yt-dlp 2026.08.19, Node as the yt-dlp JavaScript runtime, a 1080p AV1/Opus YouTube stream, and a 20-second cache.

The original stock-MPV OSC issue is separate: a single large seek normally recovered in roughly two seconds, while a rapid draggable-seekbar burst could leave MPV stuck in `seeking=true` for more than 30 seconds. The safe default therefore uses MPV's `slimbottombar`, which keeps transport controls without the stock draggable seekbar.

The later network audit also reproduced a direct-transport stall at 20% with the safe seekbar and a 20-second cache. Anonymous yt-dlp extraction returned 182 formats with no n-signature warning. A finite 1 MiB byte-range request completed far faster in the controlled range probe than reading the same amount from an open-ended response, which justified testing a bounded range transport.

## Network engines

```toml
[playback]
network_engine = "auto" # auto | mpv | proxy
stream_cache_seconds = 60
```

The same setting is available through:

```text
:set network-engine auto
:set network-engine mpv
:set network-engine proxy
```

### `mpv`

MPV receives the YouTube URL directly. Yute enables a bounded seekable cache, short network timeout and bounded FFmpeg reconnect options. It does not switch transport automatically in forced `mpv` mode.

### `proxy`

Yute starts an in-process loopback-only HTTP server for the current playback session. It:

- binds only to `127.0.0.1` on an ephemeral port;
- uses an unguessable per-session path token;
- resolves media anonymously with yt-dlp and never forwards Yute OAuth/cookies;
- serves finite byte ranges in 1 MiB chunks;
- allows at most three upstream chunk requests concurrently;
- prefetches a bounded next chunk;
- retries bounded connection failures;
- re-resolves once on an expired/403 source;
- cancels obsolete work when the client disconnects/re-seeks;
- terminates with the playback session;
- rejects unknown session paths.

Separate video/audio streams are exposed to MPV through a local EDL manifest. Live media stays direct. For VODs where yt-dlp selects a manifest-only video representation but also reports finite direct HTTP formats, Yute picks the highest finite format at the same requested resolution, preferring the selected codec family, and keeps a finite audio stream. If no usable finite representation can be loaded, Auto retries the original media once through direct MPV; forced `proxy` reports the range error instead of silently changing engines.

The proxy uses the project's existing Tokio, reqwest/rustls, futures, getrandom, hex and async-trait stack. No HTTP/3 dependency was added.

### `auto`

Auto prefers the finite-range transport immediately for ordinary YouTube VODs. Live/upcoming videos and local files stay on direct MPV because the proxy requires finite byte ranges. Manifest-only preferred VOD representations are translated to finite HTTP representations when yt-dlp exposes them. If the proxy load itself fails, Auto bypasses it for that media and retries direct MPV once; the recovery watchdog then treats that session as direct so it cannot loop back into the same failing proxy. Direct MPV remains available with `network_engine = "mpv"`; forced `proxy` remains available for debugging.

This mirrors SmartTube's idea of selectable/fallback player network stacks, not its implementation. The inspected SmartTube reference exposes Default, Cronet and OkHttp sources and can rotate sources after network failures. Yute independently implements only the behavior needed for its MPV architecture.

## Seek diagnostics

Run:

```text
yute doctor --seek-test <video-id-or-youtube-url>
```

Useful development overrides are:

```text
--seek-rounds 2
--seek-engine auto|mpv|proxy
--seek-cache 20
--seek-threshold 20
```

The harness uses the real Yute Player/IPC path with null audio/video output. Each round tests start, 60%, 20%, 90% and 10%. Every ~250 ms it samples:

- `time-pos`
- `paused-for-cache`
- `cache-speed`
- `demuxer-cache-time`
- `demuxer-cache-duration`
- `cache-buffering-state`
- `seeking`
- `core-idle`
- recovery phase

The JSON report records both the first healthy sample (`resume_ms`) and the point where playback has advanced at least two seconds (`confirmed_ms`). Signed stream URLs are never printed. Diagnostics expose only host names and query parameter names when available.

## Measured result on the problem video

Problem video: `g1qNCcxFWRo`.

With direct MPV and a 20-second cache, the first round's 20% seek exceeded the diagnostic threshold and was reported as a stall. Other direct seeks ranged from sub-second to several seconds.

With forced proxy and the same 20-second cache, all ten seeks completed. First healthy samples were about 0.25-1.76 seconds in round one and about 0.25 seconds in round two.

A later 60-second-cache comparison reproduced the same direct failure: the 20% seek exceeded the 20-second threshold. Across the sampled direct run, median reported `cache-speed` was about **0.12 MB/s**. The existing 1 MiB finite-range proxy completed start/60%/20%/90%/10% with median `cache-speed` around **2.3 MB/s**.

Auto now starts ordinary finite VODs on that proxy immediately instead of spending four seconds on the known-slower direct path first. In the final one-round Auto test at a 60-second target, all five positions completed; first healthy progress was roughly 0.5–0.75 seconds after seeks and confirmation of +2 seconds of playback was roughly 2.5–2.8 seconds. Live/upcoming and Local media remain direct.

An experimental 2 MiB range size and wider prefetch window did not improve measured throughput and slightly worsened first-progress latency, so Yute keeps the simpler 1 MiB finite-range implementation.

A second compatibility case used `BCZGUDZgT1I`. yt-dlp preferred `616+251`: 1080p VP9 video `616` was `m3u8_native` without a finite content length, while Opus audio `251` was finite HTTPS. Yute now maps that video side to finite 1080p VP9 format `248` and retains finite Opus audio. With a 60-second target, both forced `proxy` and `auto` completed start/60%/20%/90%/10%; first healthy progress was about 0.5–0.75 seconds after seeks and +2-second confirmation about 2.5–2.8 seconds. The sampled proxy tail reported roughly 3.7 MB/s cache speed.

## Local test server

Proxy tests use a loopback fake origin that covers:

- open-ended response stalling while finite ranges succeed;
- expired source/403 re-resolution;
- connection reset retry;
- client disconnect/re-seek cancellation;
- invalid session path rejection;
- session shutdown.

These tests do not prove every CDN/ISP path. Run the live seek harness on additional AV1, VP9 and H.264 videos when diagnosing a machine/network-specific issue.
