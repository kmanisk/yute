<p align="center">
  <img src="assets/yute.svg" alt="Yute terminal icon" width="132">
</p>

<h1 align="center">Yute</h1>

<p align="center">
  <strong>A keyboard-first YouTube client for the terminal.</strong><br>
  Browse in the TUI. Play in MPV.
</p>

<p align="center">
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-2021-b7410e?logo=rust&logoColor=white" alt="Rust 2021"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="MIT License"></a>
  <a href="https://github.com/kmanisk/yute/stargazers"><img src="https://img.shields.io/github/stars/kmanisk/yute?style=flat" alt="GitHub stars"></a>
  <a href="https://github.com/kmanisk/yute/commits/main"><img src="https://img.shields.io/github/last-commit/kmanisk/yute" alt="Last commit"></a>
  <img src="https://img.shields.io/badge/platform-Linux%20%C2%B7%20Unix-lightgrey" alt="Linux and Unix">
</p>

<p align="center">
  Rust + Ratatui + Tokio · InnerTube browsing · Sixel/Kitty thumbnails · external MPV
</p>

![Yute search results](assets/screenshots/search.png)

Yute keeps discovery, navigation, account views and queue management inside the terminal while delegating actual video playback to external MPV over Unix JSON IPC. It is designed for keyboard-heavy workflows rather than reproducing `youtube.com` inside a terminal.

## Highlights

- **Keyboard first** — Vim-style movement, `g` navigation, `/` search and a `:` command line.
- **Real YouTube browsing** — Home, Subscriptions, Search, History, Watch Later, Liked videos, playlists and channels where the active session exposes them.
- **Entity-aware routing** — videos, Shorts and playable live streams go to MPV; channels and playlists open inside Yute.
- **Terminal graphics** — confirmed-protocol Kitty, iTerm2 and Sixel thumbnails with Unicode halfblock fallback.
- **Terminal-native themes** — semantic styles, a palette-following `terminal` default, light/dark auto selection, and live config/theme reload.
- **External MPV** — playback stays in a real media player while Yute remains responsive and browsable.
- **Account-aware feeds** — Anonymous, saved TV OAuth identities and explicit re-login states are isolated cleanly.
- **Command mode** — completion and history expose power-user actions without filling the UI with permanent chrome.
- **No daemon, webview or Electron** — an event-driven terminal client with near-zero idle redraw overhead.

## Screenshots

### Search and content browsing

The content pane uses YouTube-style thumbnail hierarchy while keeping terminal-native navigation and filters.

![Yute search view](assets/screenshots/search.png)

### Sidebar navigation

The sidebar can be shown when needed and gives fast access to Home, Subscriptions, History, Watch Later, Liked videos, playlists, the queue, local media, accounts and settings.

![Yute navigation sidebar](assets/screenshots/navigation.png)

<table>
  <tr>
    <td width="50%"><img src="assets/screenshots/settings.png" alt="Yute settings"></td>
    <td width="50%"><img src="assets/screenshots/account.png" alt="Yute account picker"></td>
  </tr>
  <tr>
    <td align="center"><sub>Keyboard-driven playback, grid, Shorts and cache settings</sub></td>
    <td align="center"><sub>Anonymous and saved account/session selection</sub></td>
  </tr>
</table>

## Install

### Requirements

- Rust / Cargo
- [`mpv`](https://mpv.io/)
- [`yt-dlp`](https://github.com/yt-dlp/yt-dlp)
- `node` for yt-dlp's external YouTube JavaScript runtime
- optional: `ffmpeg` / `ffprobe` for local-media metadata and explicit audio conversions
- optional: `wl-copy` or `xclip` for clipboard integration

### From GitHub

```sh
cargo install --git https://github.com/kmanisk/yute yute
```

### Arch Linux / CachyOS

```sh
sudo pacman -S mpv yt-dlp
cargo install --git https://github.com/kmanisk/yute yute
```

### Build from source

```sh
git clone https://github.com/kmanisk/yute.git
cd yute
cargo build --release -p yute
install -Dm755 target/release/yute ~/.local/bin/yute
```

`cargo install yute` will become the shortest path after the first crates.io release.

## First run

```sh
yute
```

Guest browsing works without an account. For personalized account-backed views:

```sh
yute login
```

Yute supports YouTube TV/device-code authentication and account/session selection inside the TUI.

Optional MPV helper/profile setup:

```sh
yute setup-mpv
```

Configuration lives under:

```text
~/.config/yute/
```

Legacy `ytui` settings can be imported without overwriting Yute settings. OAuth tokens migrate into the encrypted XDG data store; legacy cookie data is never used as a signed-in account; see [credential storage](docs/credentials.md).

## Keyboard workflow

Normal mode is the default.

```text
          k
          ↑
      h ←   → l
          ↓
          j
```

| Key | Action |
| --- | --- |
| `h/j/k/l` | Move through cards and lists |
| `Enter` | Open the selected entity |
| `Shift-Enter` | Open item actions |
| `/` | Search |
| `:` | Command mode |
| `g` | Navigation prefix |
| `ga` | Accounts |
| `g,` | Settings (`go` remains compatible) |
| `Space e` / `Alt-e` / `b` | Show, focus or hide the sidebar |
| `Tab` | Cycle pane focus |
| `gg` / `G` | First / last item |
| `Ctrl-d` / `Ctrl-u` | Half-page down / up |
| `Shift-J/K` | Next / previous content filter |
| `Shift-H/L` | Back / forward through route history |
| `a` | Add selected video to queue |
| `S` | Subscribe / unsubscribe when the selected entity has a channel |
| `d` | Download the selected video or playlist |
| `yy` | Copy selected/current video URL |
| `o` | Open selected entity in the browser |
| `v` | Toggle grid/list view live |
| `zz` | Center the selected row |
| `?` | Contextual help |
| `Esc` / `Shift-H` | Previous route / route history back |
| `:q` / `:quit` / `Ctrl-c` | Quit; asks for confirmation while playback or a download is active |

`Enter` is driven by the selected entity type:

```text
video / Short / live  ──▶ MPV
channel               ──▶ Channel view
playlist              ──▶ Playlist view
```

## Watch dashboard

`:details` (aliases `:now`, `:watch`, `:video-info`) opens the currently playing video's dashboard without restarting MPV. `d`, `c`, and `w` focus Description, Comments, and Suggestions; `Tab` or `h/l` moves between sections. Comments render author avatars, reply counts, reaction state, and supported image/GIF attachments. Animated media is decoded with bounded size/frame limits and only schedules redraws while the media is visible.

Comment like/dislike follows YouTube's focused-comment detail continuation before using the active toggle action. `n` creates a top-level comment, `r` replies to the selected thread, and `x` offers confirmed deletion only for comments Yute can identify as yours. The same operations are available through `:comment new|reply|delete|like|dislike`. Create/reply/delete parameters are generated from the TVHTML5 write protocol and local state changes happen only after YouTube acknowledges the request. Older saved OAuth tokens may require one `:account add` re-login to receive the current YouTube write scope; reading and reactions continue to use the existing session.

Search keeps YouTube's returned ordering and continuation tokens instead of locally dropping or re-ranking later pages. On the Search screen, `f` or `:search-filters` opens verified server-side filters for type, duration, upload date, Live/4K/HDR features and relevance/popularity; the encoded params are sent on both the first request and continuations. Music-like queries can apply the configured automatic Videos plus metadata-duration bias without changing server ranking; `M` disables that bias for the current query.

Terminal text uses grapheme-safe width accounting plus a post-first-frame terminal probe for complex scripts. `yute text-probe` measures cursor advance without blocking startup; `[text].scripts = "auto"` keeps native text when the terminal agrees with Yute's width model and can fall back to approximate transliteration for measured failures. `T` toggles the selected title between original/transliterated forms where available. Font coverage remains a separate diagnostic, so missing Korean/CJK/emoji glyphs are reported instead of silently blamed on layout. See [`docs/text-rendering.md`](docs/text-rendering.md).

## Item actions

`Shift-Enter` opens the contextual item menu. Available actions depend on the selected entity.

Typical actions include:

```text
p   play / open
v   browse playlist
o   open in browser
y   copy URL
t   copy title
a   add to queue
s   save to playlist
d   download
x   cancel active download
```

Channels never expose playback actions. Playlist and account writes are only reported as successful when YouTube confirms them.

Downloads use external `yt-dlp`, respect the current quality limit and default to `~/Videos/Yute`. Local media scanning includes that directory.

## Command line

Press `:` from Normal mode.

```text
:search linux racing
:filter videos
:search-filters
:view toggle
:details
:comments
:comment new
:comment reply
:comment delete
:description
:suggestions
:shorts off
:quality 1080p
:quality best
:play-select
:audio
:download video 1080p
:theme auto
:graphics auto
:queue add
:sidebar toggle
:set scrolloff 3
:set columns auto
:set icons nerd
:set cache-seconds 40
:set codec av1
:set autoplay off
:set download-dir ~/Videos/Yute
:account
:account whoami
:copy url
:open
:settings
:help
:q
```

Commands support completion and history. See [`docs/commands.md`](docs/commands.md) for the full reference.
`:set` also exposes the main UI/playback preferences directly: icons, columns, card width, cache horizon, codec/hwdec, languages, autoplay/history sync, thumbnail cache/prefetch, sidebar/action-bar behavior and configurable keys.

Use `:play-select` or Shift-Enter → Play / quality to choose real available
video formats; `:audio` opens original audio streams. Playback stays in external
MPV, with no pre-download or conversion. Manual choices apply to one playback.

For large seek/scrub jumps, Yute uses keyframe-oriented IPC seeks and watches MPV's
`seeking`/cache state. A real MPV 0.41 audit reproduced two distinct failures: the
stock OSC can create a seek storm when its draggable seekbar is scrubbed, and direct
MPV transport can still wedge on an isolated DASH seek even with the safe seekbar.
With **MPV seekbar** off (the default), Yute uses MPV's upstream `slimbottombar`.
The playback network engine defaults to `auto`: it starts direct, then after a
persistent network stall performs one fresh anonymous resolution and switches once
to Yute's loopback finite-range transport at the requested seek target. Forced
`mpv` stays direct; forced `proxy` starts on the finite-range transport. On the
problem video with a 20-second cache, direct MPV stalled at the 20% seek, while
forced proxy and the final Auto policy completed all ten measured seeks. See
[`docs/playback-network.md`](docs/playback-network.md) for the sanitized measurements,
state machine and `yute doctor --seek-test` harness.

## Accounts

Open the account picker with `ga` or `:account`.

```text
:account
:account add
:account <name|index|prefix>
:account remove <name>
:account anon
:account whoami
```

TV OAuth is the only authenticated account mechanism. `:account add` (or `:login`)
shows an activation URL, code, QR and expiry countdown; Esc cancels. Legacy
cookie-only profiles require TV-code re-login. Status shows Anonymous,
OAuth, or Needs re-login. yt-dlp extraction and downloads do not receive cookies
or OAuth credentials. Account-library errors remain visible and are never
replaced with an empty yt-dlp fallback.

The picker can contain Anonymous plus saved account identities. Returned brand profiles use their page-id header when available. Feed caches are isolated per account; thumbnail caches are shared.

Set this if Yute should always start anonymously instead of restoring the last used account:

```toml
[general]
start_anonymous = true
```

## Configuration

```text
~/.config/yute/config.toml
```

Example:

```toml
[general]
graphics = "auto"        # auto | kitty | iterm2 | sixel | halfblocks | none
icons = "auto"           # auto | nerd | unicode | ascii | none
theme = "terminal"       # terminal | auto | youtube-dark | gruvbox-dark | gruvbox-light | custom name
theme_dark = "gruvbox-dark"
theme_light = "gruvbox-light"

[ui]
action_bar = "auto"      # auto | off
sidebar_channels = false
sidebar_at_launch = "remember" # show | hide | remember
scrolloff = 3             # Vim-style context rows

[keys]
sidebar = "space e"
sidebar_alt = "alt-e"
copy_url = "yy"
center = "zz"
quit = ""                # normal q is intentionally unbound by default

[text]
emoji = "auto"           # auto | keep | strip
scripts = "auto"         # auto | native | transliterate

[content]
shorts = "never"         # never | hide | show

[grid]
columns = 0               # automatic; fixed 1–8 also supported
min_card_width = 30
sidebar_expanded = false

[playback]
max_resolution = "1080p" # 480p | 720p | 1080p | 1440p | 2160p | best
hwdec = "auto-safe"
network_engine = "auto"  # auto | mpv | proxy
stream_cache_seconds = 20
use_user_mpv_config = false
osd_bar = false
autoplay = true

[thumbnails]
cache_mb = 256
prefetch_pages = 1

[downloads]
directory = "~/Videos/Yute"
video_quality = "1080p"
audio_mode = "best"
playlist_subdirectories = true
keep_partial = true
```

Shorts modes:

- `never` — hide Shorts outside explicit Shorts filters/tabs, including saved feeds and suggestions
- `hide` — hide them in ordinary feeds but allow explicit Shorts/saved content
- `show` — show normally

See [configuration reference](docs/configuration.md) for semantic theme roles, live reload, codec/language options and downloads. `sidebar_at_launch = "remember"` persists only explicit sidebar toggles; a very narrow terminal hides it temporarily without overwriting that preference.

Visible thumbnails take priority over prefetch work. The cache stores source images and encoded terminal payloads. `:theme` and `:graphics` apply supported visual changes live. Custom themes live in `~/.config/yute/themes/<name>.toml`; the packaged Gruvbox files are examples. `YUTE_GRAPHICS` can override graphics selection for one launch.

## Architecture

```text
YouTube / InnerTube
        │
        ▼
┌────────────────────────────┐
│            yute            │
│                            │
│  Home       Subscriptions  │
│  Search     History        │
│  Channels   Playlists      │
│  Queue      Local media    │
└─────────────┬──────────────┘
              │ Unix JSON IPC
              ▼
             MPV
```

Workspace layout:

```text
crates/
├── yute-core   headless YouTube, data and playback logic
├── yute        Ratatui terminal client
└── yute-gui    optional GTK frontend
```

Rendering does not decide what an item does. Content is routed through typed actions before reaching navigation or the player:

```text
ContentItem ──▶ Action ──▶ Router / Player
                    │
                    ├── OpenChannel
                    ├── OpenPlaylist
                    └── Play
```

### Documentation

- [Architecture](docs/architecture.md)
- [Commands](docs/commands.md)
- [Credential storage](docs/credentials.md)
- [MPV tuning](docs/mpv-tuning.md)
- [Playback network and seek recovery](docs/playback-network.md)
- [Text rendering and terminal probes](docs/text-rendering.md)
- [Performance measurements](docs/performance.md)
- [SmartTube protocol notes](docs/smarttube-parity.md)
- [Current delivery validation](docs/delivery-validation.md)
- [Historical UI verification](docs/ui-polish-review.md)
- [Manual QA matrix](docs/qa.md)

## Development

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
cargo build --release
```

Automatic grid mode fills the content width with 1–8 columns and two-cell gaps.
Auto adds columns where needed to fit two complete rows, including with the
sidebar shown, while preserving physical 16:9 artwork. Settings offers Auto and
fixed 1–8 columns. Resize preserves selected content. The focused card has a
double border, highlighted title and full-brightness artwork; other cards are
dimmed. Watched progress is a thin mark along the bottom card border.
Nerd glyphs use verified semantic mappings; auto conservatively uses Unicode
unless `YUTE_NERD_FONT=1` is set. Use `nerd` explicitly with a supported font.
See [configuration](docs/configuration.md), [icon verification](docs/icons.md),
and the [manual QA checklist](docs/qa.md).

## Current limitations

Yute is still early software. Known incomplete areas include:

- full cross-client history/resume parity still needs interactive account verification
- endless Mix-radio continuation
- some authenticated mutations when YouTube does not return usable action parameters
- Kitty/iTerm2/Sixel visuals and live terminal-theme switching still need manual verification on each real terminal/multiplexer combination

Mix playback currently uses the returned batch. If YouTube does not expose a working account action, Yute reports the limitation instead of pretending the write succeeded.

## License

MIT — see [LICENSE](LICENSE).

