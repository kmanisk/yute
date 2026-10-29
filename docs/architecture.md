# Architecture Guide

## Workspace Design

The codebase is organized as a Cargo workspace:

```
yute/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── yute-core/              # Engine library (zero terminal dependencies)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── backend/        # TVHTML5 client, OAuth, parser, models
│   │       ├── config.rs       # Configuration parser and paths
│   │       ├── local.rs        # Local video filesystem scanner
│   │       ├── media/          # Thumbnail loader and disk cache
│   │       └── player/         # MPV controller, IPC, watch tracking, queue
│   └── yute/                   # Ratatui TUI application
│       ├── Cargo.toml
│       └── src/
│           ├── app.rs          # UI state machine and event router
│           ├── events.rs       # Async event types
│           ├── keymap.rs       # Vim keybinding dispatch
│           ├── main.rs         # Entry point and CLI commands
│           ├── media/          # Ratatui Picker / StatefulProtocol adapter
│           ├── theme.rs        # Terminal color themes
│           └── ui/             # Layouts: grid, list, watch, sidebar, header
├── docs/                       # Technical documentation
└── PKGBUILD                    # Arch Linux package recipe
```

## Core Engine (`yute-core`)

`yute-core` provides headless services for consuming YouTube feeds and controlling playback:

1. **Authentication and Tokens:**
   - TV OAuth device-code authorization (`yute login`).
   - Tokens stored on disk with strict `0600` permissions.
   - Encrypted at rest via AES-256-GCM.
   - Fresh OS-random nonce and authenticated version/key-source header per write.
   - OS keyring master key; private keyfile fallback or Argon2id passphrase-file key.
   - Secrets use zeroizing owned strings with redacted Debug implementations.
   - Credential I/O runs on blocking workers after the first frame.
   - See [Credential storage](credentials.md) for paths and migration.

2. **InnerTube TVHTML5 Client:**
   - Emulates the stable TV client identity (`TVHTML5`).
   - Parses YouTube JSON renderers: `lockupViewModel`, `tileRenderer`, `shelfRenderer`, `reelItemRenderer`, and `compactVideoRenderer`.
   - Handles pagination via continuation tokens.

3. **History and Watchtime Tracking:**
   - Pings `https://www.youtube.com/api/stats/playback` when playback starts using a session client playback nonce (`cpn`).
   - Periodically queries MPV `time-pos` via IPC and pings `https://www.youtube.com/api/stats/watchtime` (every 25–30 seconds).
   - Transmits a final watchtime ping with actual elapsed position when playback is stopped.
   - Emits playback/watchtime tracking from real MPV positions when enabled; exact cross-client resume parity remains subject to live account verification.

4. **Player Controller:**
   - Launches external `mpv` processes communicating over dedicated Unix domain sockets.
   - Observes MPV property changes (`time-pos`, `duration`, `pause`, `volume`, `speed`).
   - Dispatches playback commands (`seek`, `cycle pause`, `set volume`, `stats toggle`).

## Terminal Frontend (`yute`)

`yute` handles layout and user interaction:

1. **Geometry and Responsive Grid:**
   - Calculates thumbnail rows based on actual font cell pixel dimensions (`font_size`).
   - Adapts to terminal heights from 80x24 up to ultrawide 240x60 displays.
   - Allocates card heights dynamically (top border + thumbnail + title + meta + bottom border) to eliminate border clipping.
   - Renders scrollbar indicators on the right margin with exact row offsets.

2. **Decoupled Graphics Adapter:**
   - Consumes raw `image::DynamicImage` instances from `yute-core`.
   - Adapts images to `ratatui_image::protocol::StatefulProtocol` for Kitty, Sixel, or Unicode half-block rendering.

## Typed activation and command mode

The terminal frontend uses `actions::activation(Card)` to return Play, OpenChannel,
OpenPlaylist or Unavailable. InnerTube renderer types and typed endpoints determine
Card variants. Titles and row positions do not classify entities. Main's
`dispatch_open` owns transitions; `start_playback` handles default playback and
the media overlay supplies a one-playback configuration for explicit formats.
Mixes remain playlists and upcoming streams are not launched.

`commands::REGISTRY` shares canonical names, aliases, syntax, descriptions and parse
functions with completion and help. Parsed Commands dispatch through application
controllers, reusing the same navigation/activation/player functions as keybindings.
History is bounded to 100 entries in memory and records only successfully parsed
commands. No shell evaluation or persistent command-history file is used.

ChannelPage carries actual header metadata, opaque returned tab endpoints, observed
subscription state and a feed. Channel route state caches each visited tab's items,
continuation, selection and scroll. NavState includes the detail route, filters,
query and view modes. Search draft input is separate from the current route query.
Full requests and continuations have generations; stale results cannot replace the
current route. Login/account callbacks additionally have a session generation.

Cover preparation uses cached decoded images and `spawn_blocking` for center crop,
resize and graphics encoding. A bounded set of exact-size protocols is reused by
Sixel, Kitty and halfblocks. Graphics overlays retain the first-cell payload and
restore normal text diffing for badge cells. The request channel is asynchronous,
so waiting for thumbnails cannot occupy a Tokio worker. There is no thumbnail
helper process. Settings mutation is separate from persistence in the key handler,
so renderer tests do not save live user settings.

## Diagnostics and structured formats

`yute-core::doctor` checks bounded network/process operations and existing
credentials without login, refresh or migration. Terminal information is added by
the CLI without entering raw mode. Graphics output describes environment hints.

`media::formats::FormatResolver` owns a single current-video metadata cache with a
five-minute TTL and serialized async probes. Cancellation drops a kill-on-drop
subprocess. Structured JSON is mapped into typed media kinds/codecs/dimensions,
bit rates, HDR, languages and optional sizes; signed URLs are not cached. Unknown
fields are ignored. Unsafe format IDs, DRM and non-media entries are excluded.

The action overlay routes through Play as and the available video/audio list.
Request IDs reject late results, and dropping the overlay cancels its probe.
Selectors identify an exact stream and pair video-only streams with original
audio. External MPV resolves those selectors through yt-dlp; the TUI never embeds
MPV. Normal playback resets the format selector on reused MPV IPC sessions, so
an explicit audio action does not persist. GTK retains its libmpv player. Both
frontends use the same OAuth account/configuration/backend models; neither
player receives cookie-based YouTube authentication.


## Authoritative account feeds and visible state

InnerTube is the authoritative backend for account libraries, account playlist
continuations, identity and mutations. yt-dlp remains an anonymous public fallback
and media resolver. Account errors propagate from the first backend; an empty
fallback cannot hide them. Account continuations retain an internal provenance
prefix so they cannot be dispatched to yt-dlp. Recognized successful empty
renderers are distinct from unknown response shapes and logged-out responses.

The TUI stores feed errors separately from transient footer messages. Grid and
list use the same empty/loading/sign-in/re-login/error decision. Cached content
can stay visible after a failed refresh, with a persistent error in the footer.
Account cancellation and feed generations prevent late responses crossing views.

Visible synthetic channel cards asynchronously request public channel metadata
with two concurrency slots. Feed/account navigation cancels superseded hydration;
generation checks discard late results. Avatars reuse the shared priority-aware
image pipeline, cache and decoding workers. Google avatar sizing rewrites only
recognized s/w/h numeric suffixes on supported hosts, preserving opaque IDs,
crop/privacy flags and queries. Unknown forms are unchanged.

Automatic grid geometry uses the content pane after sidebar allocation, 30-cell
minimum slots, two-cell gutters and 1–8 columns. Artwork height follows terminal
cell pixel dimensions. Typed cards keep video/channel/playlist activation separate.
Semantic icons are centralized in `ui/icons.rs`; display width governs padding.
Input editing and truncation use grapheme boundaries, including legacy cursor
positions inside multibyte text.

## Theme, terminal capability and live-config path

The TUI centralizes semantic styles in `theme.rs`. The default `terminal` theme uses terminal defaults and ANSI colours rather than hardcoded RGB. Custom TOML themes are resolved after the first frame, and `live_config.rs` watches config/theme paths using filesystem events; a failed parse keeps the previous state.

Background and graphics reports share the single `/dev/tty` reader in `terminal_input.rs`, so capability replies cannot race normal key input. Graphics auto mode begins with halfblocks. Post-first-frame queries can confirm Kitty, Sixel, iTerm2 cell reports, cell pixel dimensions and background/light state. Structural overlay changes invalidate image placements and clear/redraw the affected screen to prevent persistent graphics ghosts. Multiplexed sessions conservatively use halfblocks when passthrough has not been established.

## Downloads and subscribed channels

`yute-core::downloads` builds yt-dlp commands as argument arrays only. Single-item downloads can use exact resolved selectors; playlist video resolutions are ceilings per item. Audio conversion requires ffmpeg, playlist jobs use local archive files, and one owned asynchronous child is active at a time.

Home's Channels tab is not synthesized from Home recommendations. It uses the authenticated subscribed-channel browse path and an account-scoped stale cache; signed-out state does not reveal cached account channel names. The optional tall-sidebar channel list reuses the same typed channel cards.


### Terminal protocol sources

The query/parser contract follows primary terminal specifications:

- [Kitty graphics protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/):
  the non-storing `a=q` image probe followed by primary device attributes, and
  lowercase placement deletion. Switching away from Kitty also deletes previous
  Kitty placements before repainting.
- [XTerm control sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html):
  OSC 11 background queries, device attributes, and character-cell pixel reports.
- [Contour light/dark notifications](https://contour-terminal.org/vt-extensions/color-palette-update-notifications/):
  query CSI ? 996 n; response CSI ? 997 ; 1/2 n; notification mode 2031.
  Focus reporting is enabled while Yute runs; notification/focus modes are
  disabled on normal exit and panic cleanup.
- [iTerm2 escape codes](https://iterm2.com/documentation-escape-codes.html):
  ReportCellSize height/width and optional physical-pixel scale. A cell-size
  response is combined with the terminal identity before auto-selecting iTerm2.

No desktop portal dependency is enabled. Unsupported queries fall back without
holding first paint. Image visibility checks cover comment media and account
avatars as well as grid/list images; off and none are equivalent disabled modes.


Account continuations keep a bounded, backend-local record of issued page tokens
and identities. Complete page replays raise RepeatedAccountPage even if the
server changes its token. The TUI preserves content/selection, terminates loading
and disables automatic retry until the user requests it. This guard also applies
to authenticated playlist contents and does not rewrite OAuth/session ownership.
