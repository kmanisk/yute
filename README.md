# Yute

<img src="assets/yute.png" alt="Yute terminal icon" width="160">

A keyboard-first YouTube terminal client built with Rust, Ratatui and Tokio.
Browse in the terminal; watch in an external MPV window.

## Features

- Personal Home recommendations, subscriptions, search, history and saved playlists.
- Typed navigation: channels and playlists open internally; videos and live streams play in MPV. Mix cards load a real video batch and queue it for playback.
- Compact cards with two-line titles, view counts, upload age, duration overlays and aspect-preserving thumbnails.
- Sixel, Kitty and halfblock thumbnails, decoded asynchronously and cached.
- Watch details with formatted description links, comments and thumbnail suggestions.
- Command completion, command history, route history and a collapsible sidebar.
- Adjustable quality, hardware decoding and MPV stream buffer. No daemon or embedded browser.

## Install from source

Requires Rust/Cargo, `mpv` and `yt-dlp`. Optional: `ffmpeg` for local media and
`wl-copy` or `xclip` for copying URLs.

```sh
git clone https://github.com/kmanisk/yute.git
cd yute
cargo build --release -p yute
install -Dm755 target/release/yute ~/.local/bin/yute
yute
```

The repository is private; cloning requires repository access. The optional GTK
frontend remains a separate workspace member and is not needed for the terminal
client. Build the entire workspace only if its GTK/libadwaita/libmpv development
libraries are available.

```sh
yute login               # YouTube device-code login
yute --quality 720p      # session quality limit
yute setup-mpv           # optional MPV scripts
```

## Keyboard

| Key | Action |
| --- | --- |
| `h/j/k/l` | Move between cards; h/l stay within the row |
| `Enter` | Open the selected entity |
| `Shift-Enter` | Item actions: play, browser, copy, queue, save, download |
| `o` | Open the selected video/channel/playlist in the browser |
| `g,` | Settings (`go` remains a compatibility shortcut) |
| `Shift-J/K` | Next/previous content filter |
| `Shift-H/L` | Back/forward through route history |
| `Alt-e` | Show/focus sidebar; press again to hide |
| `b` | Toggle sidebar |
| `/` / `:` | Search / commands in the bottom line |
| `g` | Navigation prefix; delayed hints show the destinations |
| `gg/G`, `Ctrl-d/u` | First/last, half-page navigation |
| `a` / `y` | Queue selected video / copy URL |
| `?` | Contextual help |
| `Esc` / `q` | Previous route / quit |

Watch starts with focus in the main content. `d`, `c`, `w` focus Description,
Comments and Suggestions; `Tab` or h/l move between these sections. j/k scroll or
select within the focused section. Enter opens a selected suggestion or link.
`+`/`-` rate the video or focused comment, `n` starts a comment, and `r` replies.
Posting/reactions require authentication **and real server-provided action
parameters**. Some YouTube clients do not return those capabilities; Yute reports
this and offers `:open` instead of pretending an action succeeded. Submitted drafts
are only sent on Enter; Esc cancels.

MPV keeps its own playback keys. Navigating away from Watch preserves playback.
The small playback line is passive feedback; Watch has no duplicate playback
control panel. Terminal font and window size remain controlled by your terminal.

## Commands

```text
:search rust async
:quality 720p
:quality best
:settings
:sidebar toggle
:filter videos
:shorts off
:queue add
:copy url
:open
```

See [all commands](docs/commands.md).

## Preferences

Settings are stored in `~/.config/yute/config.toml`. On first use, Yute copies
legacy `~/.config/ytui/` settings and login files without overwriting existing Yute
files. The legacy encrypted token format remains readable. Legacy files are kept.

```toml
[content]
shorts = "never"          # never | hide | show

[grid]
columns = 3              # 0 selects automatic columns
min_card_width = 26
sidebar_expanded = false

[playback]
max_resolution = "1080p" # 480p | 720p | 1080p | 1440p | 2160p | best
hwdec = "auto-safe"
stream_cache_seconds = 20 # Settings steps: 5, 10, 20, 40, 60
use_user_mpv_config = false
osd_bar = false
autoplay = true
```

`never` hides Shorts in ordinary feeds, saved feeds and suggestions. Explicitly
opening a Shorts filter or channel Shorts tab always shows Shorts without changing
the preference. `hide` also permits saved Shorts. The underlying data stays cached. Settings changes persist;
`:shorts on/off` explicitly changes the preference to show/hide.

The stream-cache slider controls MPV readahead and a bounded 32–256 MiB packet
budget. It updates an active session immediately. Initial playback does not wait
for the full readahead horizon; after an underrun MPV uses a one-second recovery
buffer. User MPV profile mode retains that profile's startup settings. `:quality`
reloads the current stream at its existing position and preserves pause state.
Network extraction and server latency still determine actual startup time.

## Development

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
cargo build --release
```

[Architecture](docs/architecture.md) · [MPV configuration](docs/mpv-tuning.md) ·
[SmartTube protocol notes](docs/smarttube-parity.md) ·
[UI verification](docs/ui-polish-review.md)

Account switching, full cross-client history/resume parity and endless Mix radio
continuation remain incomplete. Mix playback uses the current returned batch.
No account writes are performed by the tests.

MIT. See [LICENSE](LICENSE).


## Item actions and focus

The route label and result filters share the top row. Navigation and content have
separate frames: the keyboard-focused pane uses the accent border, while the
active sidebar route keeps its marker. Tab cycles pane focus; Alt+E shows/focuses
or hides navigation. Card gutters are two terminal columns, with no large empty
column slots when navigation is hidden.

Shift+Enter opens an item menu. j/k or arrows select, Enter runs the action, and
Esc/q closes the menu. Menu shortcuts are p play/open, v browse playlist, o browser,
y copy link, t copy title, a queue, s save video to playlist, d download, and x
cancel a running download. Options depend on entity type; channels never offer
playback. Ordinary Enter still opens normal playlists internally.

Saving requests the account's actual available playlist destinations, excluding
already-saved entries, and waits for YouTube to confirm the edit. A valid YouTube
session is required; no success is simulated when the endpoint is unavailable.

Downloads use an external yt-dlp process at the current quality limit and write
to `~/Videos/Yute`, which Local also scans. Node support is enabled alongside
any default yt-dlp runtime. Only one job runs at a time; cancel it from the menu.
Exiting Yute terminates an unfinished download; yt-dlp partial files can resume
on a later attempt. Playlist/Mix downloads use their collection URL. No shell is
used to execute entity URLs or filenames. Account cookies are only supplied for
non-guest sessions with an existing cookie file.
