<div align="center">
  <img src="assets/yute.png" alt="Yute terminal icon" width="150">

# Yute

**YouTube from the terminal.**

A fast, keyboard-first YouTube client built in Rust with Ratatui and Tokio.  
Browse in the terminal, keep the browser closed, and play video in external MPV.

[![Crates.io](https://img.shields.io/crates/v/yute.svg?style=flat-square&logo=rust)](https://crates.io/crates/yute)
[![docs.rs](https://img.shields.io/docsrs/yute?style=flat-square&logo=docs.rs)](https://docs.rs/yute)
[![License](https://img.shields.io/crates/l/yute?style=flat-square)](LICENSE)

</div>

<p align="center">
  <img src="assets/screenshots/subscriptions.png" alt="Yute subscriptions view" width="900">
</p>

## Why Yute?

Yute is for people who want YouTube discovery and account-backed browsing without living in a browser tab.

- **Keyboard first** — Vim-style navigation, route history, search, commands, contextual help and a collapsible sidebar.
- **Real YouTube browsing** — Home recommendations, subscriptions, search, history, playlists, liked videos and Watch Later where supported by the authenticated session.
- **Terminal-native thumbnails** — Sixel, Kitty graphics and Unicode halfblock fallbacks, loaded asynchronously and cached.
- **MPV playback** — Videos, Shorts and live streams open in external MPV over Unix JSON IPC; channels and playlists stay inside Yute.
- **Low overhead** — No daemon, no embedded browser, no webview and no libmpv UI dependency.
- **Configurable playback** — Resolution limits, hardware decoding, MPV stream buffering, autoplay and optional MPV user profiles.

## Install

### Cargo

```sh
cargo install yute
```

Runtime requirements:

- [`mpv`](https://mpv.io/)
- [`yt-dlp`](https://github.com/yt-dlp/yt-dlp)
- optional: `ffmpeg` / `ffprobe` for local-media metadata
- optional: `wl-copy` or `xclip` for clipboard integration

On Arch Linux / CachyOS:

```sh
sudo pacman -S mpv yt-dlp
cargo install yute
```

### From source

```sh
git clone https://github.com/kmanisk/yute.git
cd yute
cargo build --release -p yute
install -Dm755 target/release/yute ~/.local/bin/yute
```

Then run:

```sh
yute
```

Optional account and MPV setup:

```sh
yute login       # YouTube TV/device-code login
yute setup-mpv   # install the optional Yute MPV profile/scripts
```

## Interface

<table>
  <tr>
    <td><img src="assets/screenshots/search.png" alt="Search results"></td>
    <td><img src="assets/screenshots/watch.png" alt="Watch details"></td>
  </tr>
  <tr>
    <td align="center"><sub>Search and typed entity navigation</sub></td>
    <td align="center"><sub>Watch details while MPV owns playback</sub></td>
  </tr>
  <tr>
    <td><img src="assets/screenshots/command.png" alt="Command mode"></td>
    <td><img src="assets/screenshots/settings.png" alt="Settings"></td>
  </tr>
  <tr>
    <td align="center"><sub>Vim-like command line with completion</sub></td>
    <td align="center"><sub>Keyboard-driven settings</sub></td>
  </tr>
</table>

## Keyboard workflow

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

The selected entity determines what `Enter` does: playable media goes to MPV, channels open a channel view, and playlists open inside Yute.

Playback continues when you navigate away from the Watch view. MPV keeps its own playback controls; Yute shows passive session state without duplicating a full player UI.

## Command mode

Press `:` from normal mode.

```text
:search rust async
:filter videos
:shorts off
:quality 720p
:quality best
:queue add
:sidebar toggle
:copy url
:open
:settings
:help
:q
```

Commands support history and completion. See [`docs/commands.md`](docs/commands.md) for the full command reference.

## Configuration

Yute reads `~/.config/yute/config.toml`.

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

`never` hides Shorts everywhere. `hide` removes them from ordinary feeds while still allowing explicit Shorts and saved content. The underlying data remains cached, so changing the preference does not require destructive filtering.

Yute can read the legacy encrypted `ytui` token/config format and migrates legacy files without overwriting existing Yute configuration.

## Architecture

```text
crates/
├── yute-core   headless YouTube / playback / data engine
├── yute        Ratatui terminal client
└── yute-gui    optional GTK frontend
```

The TUI stays decoupled from the headless core. MPV runs as an external process and is controlled through Unix-domain JSON IPC rather than being embedded into the terminal renderer.

## Development

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features
cargo test --workspace
cargo build --release
```

Useful internals:

- [Architecture](docs/architecture.md)
- [Commands](docs/commands.md)
- [MPV tuning](docs/mpv-tuning.md)
- [SmartTube protocol notes](docs/smarttube-parity.md)
- [UI verification](docs/ui-polish-review.md)

## Current limitations

Yute is still early software. Account switching, full cross-client history/resume parity and endless Mix-radio continuation are not complete. Mix playback uses the current returned batch. Authenticated mutations are only exposed when the server returns the required action parameters; Yute does not pretend unsupported writes succeeded.

## License

MIT — see [LICENSE](LICENSE).
