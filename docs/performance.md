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
The grid adapts thumbnail width to fit two rows where feasible and renders a
text preview for clipped trailing rows without emitting cropped graphics payloads.
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
