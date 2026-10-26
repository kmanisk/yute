# yute

```text
 __   __      _       
 \ \ / /   _| |_ ___ 
  \ V / | | | __/ _ \
   | || |_| | ||  __/
   |_| \__,_|\__\___|
```

> A fast, keyboard-first YouTube client for the terminal.

[![Rust](https://img.shields.io/badge/Rust-2021-b7410e?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-green)](LICENSE)
[![Stars](https://img.shields.io/github/stars/kmanisk/yute?style=flat)](https://github.com/kmanisk/yute/stargazers)
[![Last commit](https://img.shields.io/github/last-commit/kmanisk/yute)](https://github.com/kmanisk/yute/commits/main)
[![Platform](https://img.shields.io/badge/platform-Linux%20%C2%B7%20Unix-lightgrey)](#installation)

<p align="center">
  <a href="#why-yute">Why Yute?</a> •
  <a href="#features">Features</a> •
  <a href="#installation">Installation</a> •
  <a href="#keybindings">Keybindings</a> •
  <a href="#commands">Commands</a> •
  <a href="#configuration">Configuration</a> •
  <a href="#internals">Internals</a>
</p>

---

![Yute subscriptions view](assets/screenshots/subscriptions.png)

> **Status:** Yute is usable today for terminal-first YouTube browsing: personalized feeds, search, channels, playlists, history, account-backed views, terminal thumbnails and external MPV playback. Some account mutations and cross-client history parity are still incomplete; see [Current limitations](#current-limitations).

---

## Why `yute`?

YouTube's normal clients are built around a browser, pointer input, and a large amount of surrounding UI.

If most of your day is already spent in a terminal, Neovim, tmux, yazi or other keyboard-driven tools, opening a browser just to find and start a video breaks that workflow.

`yute` keeps discovery in the terminal and hands playback to MPV.

It is designed around:

- Vim-style keyboard navigation
- command-driven workflows
- personalized YouTube feeds
- terminal-native thumbnails
- fast route changes without page reloads
- external MPV instead of an embedded browser/player
- no daemon and no background service

The goal is not to redraw youtube.com in a terminal. It is to make YouTube feel like a terminal application.

---

## Features

- **Personalized browsing** — Home recommendations, Subscriptions, Search, History, Watch Later, Liked videos and playlists where the authenticated session exposes them.
- **Typed navigation** — videos, Shorts and live streams play in MPV; channels and playlists open as native Yute views instead of being treated as videos.
- **Terminal thumbnails** — Sixel, Kitty graphics protocol and Unicode halfblock fallback, decoded asynchronously and cached.
- **External MPV playback** — MPV stays independent from the TUI and is controlled through Unix-domain JSON IPC. Browsing can continue while playback remains active.
- **Keyboard-first UI** — `hjkl`, route history, `g` prefixes, command mode, contextual help, search and a collapsible sidebar.
- **Command line** — Vim-like `:` mode with completion and history for navigation, playback, filters, quality, queue actions and settings.
- **Configurable playback** — resolution limits, hardware decoding, stream buffer, autoplay and optional MPV user profiles.
- **Low overhead** — Rust + Ratatui + Tokio, no Electron, no webview, no embedded Chromium and no permanent helper process.

---

## Interface

<table>
  <tr>
    <td><img src="assets/screenshots/search.png" alt="Yute search results"></td>
    <td><img src="assets/screenshots/watch.png" alt="Yute watch view"></td>
  </tr>
  <tr>
    <td align="center"><sub>Search and typed entity navigation</sub></td>
    <td align="center"><sub>Watch details while MPV owns playback</sub></td>
  </tr>
  <tr>
    <td><img src="assets/screenshots/command.png" alt="Yute command mode"></td>
    <td><img src="assets/screenshots/settings.png" alt="Yute settings"></td>
  </tr>
  <tr>
    <td align="center"><sub>Command mode with completion</sub></td>
    <td align="center"><sub>Keyboard-driven settings</sub></td>
  </tr>
</table>

Active playback stays visible without turning the terminal into a video renderer:

![Yute active playback](assets/screenshots/active-playback.png)

---

## Installation

### Requirements

- Rust / Cargo
- [`mpv`](https://mpv.io/)
- [`yt-dlp`](https://github.com/yt-dlp/yt-dlp)
- optional: `ffmpeg` / `ffprobe` for local-media metadata
- optional: `wl-copy` or `xclip` for clipboard integration

### Install directly from GitHub

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

> `cargo install yute` will become the shortest install path once the first crates.io release is published.

---

## First launch

Start Yute:

```sh
yute
```

For personalized feeds, link a YouTube account with TV/device-code authentication:

```sh
yute login
```

Optional MPV helper setup:

```sh
yute setup-mpv
```

Configuration is stored under:

```text
~/.config/yute/
```

Yute can read the legacy encrypted `ytui` token/config format and migrates legacy files without overwriting an existing Yute configuration.

---

## Keybindings

Yute starts in Normal mode. Search and command input temporarily take over the bottom line, so typed characters do not leak into navigation commands.

| Key | Action |
| --- | --- |
| `h` / `j` / `k` / `l` | Move through cards and lists |
| `Enter` | Open the selected entity |
| `gg` / `G` | First / last item |
| `Ctrl-d` / `Ctrl-u` | Half-page down / up |
| `Shift-J` / `Shift-K` | Next / previous content filter |
| `Shift-H` / `Shift-L` | Back / forward through route history |
| `/` | Search |
| `:` | Command mode |
| `g` | Navigation prefix; delayed hints show destinations |
| `a` | Add selected video to queue |
| `y` | Copy selected URL |
| `b` | Toggle sidebar |
| `?` | Contextual help |
| `Esc` / `q` | Back / quit at root |

`Enter` is entity-aware:

```text
video / Short / live  -> MPV
channel               -> Channel view
playlist              -> Playlist view
```

Navigating away from Watch does not stop playback.

---

## Commands

Press `:` from Normal mode.

```text
:search rust programming
:filter videos
:shorts off
:quality 1080p
:quality best
:queue add
:sidebar toggle
:copy url
:open
:settings
:help
:q
```

Commands support completion and history.

See [`docs/commands.md`](docs/commands.md) for the complete command reference.

---

## Configuration

Yute reads:

```text
~/.config/yute/config.toml
```

Example:

```toml
[content]
shorts = "never"          # never | hide | show

[grid]
columns = 3               # 0 = automatic
min_card_width = 26
sidebar_expanded = false

[playback]
max_resolution = "1080p" # 480p | 720p | 1080p | 1440p | 2160p | best
hwdec = "auto-safe"
stream_cache_seconds = 20
use_user_mpv_config = false
osd_bar = false
autoplay = true
```

Shorts behavior:

- `never` — hide Shorts everywhere
- `hide` — hide them from ordinary feeds but allow explicit Shorts/saved content
- `show` — show normally

The underlying data remains cached, so toggling the preference does not require destructive filtering.

---

## Internals

```text
crates/
├── yute-core   headless YouTube / playback / data engine
├── yute        Ratatui terminal client
└── yute-gui    optional GTK frontend
```

The TUI is kept separate from the headless core. MPV is an external process controlled over JSON IPC rather than being embedded into the terminal renderer.

Core stack:

```text
Rust
├── Ratatui       UI
├── Crossterm     terminal input/output
├── Tokio         async runtime
├── InnerTube     YouTube data/session layer
├── yt-dlp        media resolver/fallback
└── MPV           external playback
```

Useful docs:

- [Architecture](docs/architecture.md)
- [Commands](docs/commands.md)
- [MPV tuning](docs/mpv-tuning.md)
- [SmartTube protocol notes](docs/smarttube-parity.md)
- [UI verification](docs/ui-polish-review.md)

---

## Development

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
cargo build --release
```

---

## Current limitations

Yute is still early software.

- account switching is not complete
- full cross-client history/resume parity is not complete
- endless Mix-radio continuation is not complete
- Mix playback currently uses the returned batch
- authenticated mutations only appear when YouTube returns the required action parameters

Yute does not fake successful account writes when the server does not expose the capability.

---

## License

MIT — see [LICENSE](LICENSE).
