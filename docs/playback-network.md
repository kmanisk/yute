# Playback network and seek recovery

Yute keeps playback in external MPV. The network policy exists to work around seek/buffering failures in fragmented YouTube streams without increasing the default cache to 60 seconds or introducing a permanent background service.

## What was measured

The development audit used MPV 0.41, yt-dlp 2026.08.19, Node as the yt-dlp JavaScript runtime, a 1080p AV1/Opus YouTube stream, and a 20-second cache.

The original stock-MPV OSC issue is separate: a single large seek normally recovered in roughly two seconds, while a rapid draggable-seekbar burst could leave MPV stuck in `seeking=true` for more than 30 seconds. The safe default therefore uses MPV's `slimbottombar`, which keeps transport controls without the stock draggable seekbar.

The later network audit also reproduced a direct-transport stall at 20% with the safe seekbar and a 20-second cache. Anonymous yt-dlp extraction returned 182 formats with no n-signature warning. A finite 1 MiB byte-range request completed far faster in the controlled range probe than reading the same amount from an open-ended response, which justified testing a bounded range transport.

## Network engines

```toml
[playback]
network_engine = "auto" # auto | mpv | proxy
stream_cache_seconds = 20
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

Separate video/audio streams are exposed to MPV through a local EDL manifest. Live/manifest-only media falls back rather than pretending the finite-range transport is valid.

The proxy uses the project's existing Tokio, reqwest/rustls, futures, getrandom, hex and async-trait stack. No HTTP/3 dependency was added.

### `auto`

Auto starts on direct MPV. If a network stream remains in a seek/cache stall for about four seconds, Yute performs one fresh anonymous media resolution as part of switching the session to the finite-range proxy at the requested seek target. It does not loop between engines. Healthy playback for two seconds resets the bounded recovery guard for a later independent stall.

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

With the final Auto policy, all ten seeks completed. The seek that triggered the first engine escalation resumed after about 18.1 seconds and was confirmed advancing after about 20.1 seconds; after the switch, later seeks were typically about 0.25-2.8 seconds to first healthy playback.

The first Auto escalation is still slower than an already-running proxy because it includes fresh yt-dlp resolution and proxy startup. It is intentionally bounded and is preferable to the previous indefinite stall. Future optimization should target that first escalation latency rather than increasing global cache size.

## Local test server

Proxy tests use a loopback fake origin that covers:

- open-ended response stalling while finite ranges succeed;
- expired source/403 re-resolution;
- connection reset retry;
- client disconnect/re-seek cancellation;
- invalid session path rejection;
- session shutdown.

These tests do not prove every CDN/ISP path. Run the live seek harness on additional AV1, VP9 and H.264 videos when diagnosing a machine/network-specific issue.
