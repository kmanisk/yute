# Performance measurements

Opt in with `YUTE_TRACE=1 yute`. Static-name tracing spans and duration events go
to `$XDG_STATE_HOME/yute/trace-<pid>.log` (default `~/.local/state/yute/`). Files
are mode 0600. Only operation classes, durations and counts are logged; no URLs,
queries, identities, tokens, cookies, headers or response bodies are included.
Normal runs do not enable this subscriber.

## Before (step 0)

Release build, 113x28 pseudo-terminal, font-query responses supplied by the test
harness. One cold-image run and one subsequent warm-cache run, with an isolated
XDG cache directory. Existing feed metadata was copied for the experiment; user
caches were not removed. Times start at Rust main entry (excluding process loader
and Tokio runtime startup), and "visible" means placed in a flushed terminal
frame, not a physical-display timestamp. These are samples, not network guarantees.

| Measurement | Before |
| --- | --- |
| Main entry to first frame, cold/warm runs | 1.78 / 1.78 ms |
| First frame to Home, cold feed refresh | 3,480 ms |
| First frame to cached Home, warm | 0.02 ms |
| Cold thumbnail request to first placement | 772–1,021 ms |
| Warm disk thumbnail request to placement | 501 ms |
| Subscriptions first page | 2,571 ms |
| Subscriptions complete (this response had no continuation) | 2,571 ms |
| Account switch | Unsupported before this work; not measurable |

The warm-thumbnail delay spans two 250 ms polling ticks. Resize currently changes
only the dirty flag; font pixel dimensions remain cached from startup. Startup
also calls the blocking ratatui-image stdio capability query; the responsive
pseudo-terminal benchmark does not represent an unresponsive real terminal.

Raw trace samples and the measurement harness are ignored under
`target/perf-baseline/`. Later sections record after results under matching
conditions. No authenticated writes or media playback were used for these runs.

## Layout correction

Resize previously only marked the frame dirty: it retained the startup font size.
Resize and focus regain now re-read TIOCGWINSZ pixel dimensions on a blocking
worker with a 40 ms timeout, preserving the detected protocol. Terminals that
return zero pixel dimensions retain the last valid font size; no competing stdin
query is started alongside Crossterm. Cover cache generations reject old encodes.
The grid adapts thumbnail width to fit two rows where feasible. Clipped trailing
rows show a title and cropped Kitty/half-block cells; protocols that cannot crop
safely show a title and skeleton without emitting graphics outside the viewport.
Structural changes erase Kitty placements and the terminal before repainting.
Geometry tests cover 80x24 through 300x80 with both sidebar states. Real terminal
fullscreen and Sixel erase behavior still require interactive verification.

## Launch and feed pipeline

The TUI first paints from public account metadata and account-scoped stale feed
caches. It does not wait for keyring unlock, OAuth refresh, MPV profile creation,
or a stdin capability query. Account metadata contains only profile/display
fields, never tokens, cookies or visitor data; it is still private mode 0600.
Terminal hints choose Kitty, Foot/Sixel, WezTerm/iTerm2 or half-block fallback.
Cell pixels are queried off-thread with a 40 ms timeout and cached per terminal.
Unknown terminals can explicitly choose a protocol in Settings. This avoids the
image crate's detached stdin query thread competing with Crossterm input.

InnerTube and source thumbnails share one pooled rustls HTTP client. Unauthenticated
HEAD requests pre-warm YouTube and the image host concurrently. MPV profile setup
is deferred until playback. Subscriptions already used FEsubscriptions, so no
per-channel fetching was introduced: each first page renders immediately and
continuations stream one at a time until exhausted, failed or superseded. The
loaded count grows as pages arrive; no total is fabricated when YouTube omits it.

## Thumbnail pipeline

Visible cards use a bounded 128-request queue, ordered before prefetch work, with
12 in-flight jobs and a shared HTTP semaphore. Requests for cards outside the
visible/prefetch window are cancelled; failed requests can retry after a cooldown.
Feed image URLs identify the video; mqdefault/hqdefault variants match card size,
with alternate variants on HTTP/decode failure. Missing video artwork uses its
video ID. No maxres thumbnail is requested. Source decode uses image's zune-JPEG
backend on already downsized CDN variants, off the UI thread; SIMD resizing uses
fast_image_resize. It does not require a system turbojpeg install.

Source JPEGs are keyed by image identity and dimensions. Encoded cells are keyed
by the full URL, protocol, font pixels, target rectangle and tmux state. Both share
an access-based disk budget; decoded and encoded memory caches have bounded LRU
entry counts. Kitty transmission data is persisted separately from its Unicode
placements and is transmitted once in each process. Sixel retains its final
encoded payload. Cache/decode/encode completion wakes the UI with Notify instead
of waiting for two 250 ms timer polls. When no player, status expiry, or key prefix
needs a timer, the idle UI has no ticking branch enabled.

Settings and config expose `[thumbnails] cache_mb = 256` and `prefetch_pages = 1`.
The effective cache budget is bounded to 16–2048 MiB; prefetch is bounded to 0–3
pages. Source and encoded entries are pruned together after work completes.

Ratatui's public `Terminal::clear` also queries cursor position in this version.
Structural repaints therefore reset the fullscreen viewport with its known size;
this clears graphics/diff state without issuing a cursor query to stdin.

## After (steps 0–8)

Release samples on the same machine and 113x28 pseudo-terminal, using isolated
benchmark caches. A subsequent text-only run also exercised 90x24 and 160x44
resizes. Feed/profile requests were read-only; no likes, comments or media
playback were sent to YouTube. These remain single-run observations.

| Measurement | Before | After |
| --- | --- | --- |
| Rust entry to first frame, warm | 1.78 ms | 1.96 ms |
| Process spawn to first flushed frame, warm | Not recorded | 5.63 ms |
| First frame to cached Home | 0.02 ms | 0.02 ms |
| Cold thumbnail request to placement | 772–1,021 ms | 145–548 ms |
| Warm disk thumbnail request to placement | 501 ms | 0.89–1.73 ms |
| Uncached Subscriptions first page | 2,571 ms | 2,609 ms |
| Cached Subscriptions first page | Not recorded | 0.31–1.33 ms |
| Signed-in account switch to cached flushed frame | Unsupported | 2.61–3.67 ms |

The Subscriptions samples returned no continuation, so completion coincided with
the first page; multi-page live completion was not measured. The 1.5 s uncached
Subscriptions target is **not met**. A later fresh-cache run was delayed during
credential hydration, so it supplies no additional network timing. Unlock work
runs off-thread, has a waiting notice, and does not block the cached first frame.
Quitting also bounds runtime shutdown when an OS keyring call is still waiting.

Over a two-second idle interval the PTY received zero bytes and `/proc` recorded
zero additional process CPU ticks (10 ms tick granularity). Cached account
switches, command prefix acceptance, the account picker, and resize/header/footer
placement were exercised in the PTY. Physical Kitty/Sixel redraw after window
manager fullscreen changes, interactive Google approval, multiple Google/brand
identities, cross-platform keyrings, and the GTK runtime were not verified here.
The GTK frontend compiles with all features. Native MPV smoke tests use local
generated media and null outputs; they do not validate remote playback latency.


## Terminal polish continuation, 2026-10-29

Final source 403d2cc, using the existing YUTE_TRACE first_frame metric and an
isolated anonymous 200x40 PTY with 10x20-pixel cells. The historical initial
polish baseline was 1.419 ms; final-source samples were 2.244 ms and 1.851 ms.
These individual samples do not establish a statistically controlled improvement.
An early interval contained one 19-byte style/cursor-only follow-up draw. After
eight additional settling seconds, two idle seconds produced 0 bytes and 0 CPU
ticks at 10 ms tick resolution. There is no continuous idle redraw loop.
Current terminal queries are read by terminal_input.rs; older sections describing
Crossterm as the input owner or environment-only protocol selection are historical.
See [delivery-validation.md](delivery-validation.md) for quality gates and limits.
