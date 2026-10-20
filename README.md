# ytui

Fast, keyboard-driven YouTube terminal client in Rust.
Designed for performance, responsive Vim keybindings, Sixel and Kitty terminal graphics, personalized recommendations via InnerTube, and playback integration via MPV.

---

## Highlights

- Modular workspace architecture: Headless `ytui-core` engine decoupled from the `ytui` Ratatui presentation layer.
- YouTube-exact UI layout: Top bar, feed filter chips, responsive thumbnail card grid, collapsible guide sidebar, and Up Next column.
- Dynamic terminal graphics: Automatically negotiates terminal capabilities using `ratatui-image`. Supports DEC Sixel graphics on `foot`, Kitty graphics protocol on `kitty` and `ghostty`, and Unicode half-blocks on standard terminals.
- TVHTML5 InnerTube client: Zero official API quota limits. Emulates the TV client identity with continuation pagination, visitorData reuse, channel tabs, and comments.
- TV OAuth device-code login: Sign in securely with `ytui login` (enter code at `google.com/device`). Tokens stored encrypted at rest with AES-256-GCM and strict `0600` permissions.
- Real-time watch history sync: Periodic watchtime pings query real MPV `time-pos` every 30 seconds and on playback finish, synchronizing exact resume positions across SmartTube, mobile, and web.
- Configurable GPU decoding: Hardware acceleration configurable (`hwdec = "auto-safe"`, `"no"`, `"vaapi"`, `"nvdec-copy"`), saving battery in hybrid mode and delivering zero CPU load in direct MUX mode.
- Isolated MPV player: Communicates over Unix domain sockets with JSON IPC. Supports external window playback, inline terminal handoff, auto-loaded subtitles, and live stream chat without linking `libmpv`.
- Script integration: Dedicated profile support for `sponsorblock_minimal`, `reload`, `quality-menu`, and `youtube-chat`. Run `ytui setup-mpv` to install verified scripts.
- Zero daemon overhead: Pure single-process event loop driven by Tokio and Crossterm, running at near-zero CPU while idle.

---

## Documentation

Detailed technical references are located in the `docs/` directory:

- [Architecture Guide](docs/architecture.md): Workspace crates, data flow, parser mechanics, and player IPC.
- [MPV Tuning and Hardware Decoding](docs/mpv-tuning.md): GPU decoding guidelines, buffer sizing, and rate-limiting prevention for subtitles.

---

## Requirements

### Runtime
- `mpv`: Video playback engine with IPC socket support.
- `yt-dlp`: Media stream resolver (configured with external JS engine such as `node` for YouTube extraction).
- Optional: `ffmpeg` / `ffprobe` (for local video metadata), `wl-copy` or `xclip` (clipboard yanking).

### Build
- `rustc` >= 1.86.0 and `cargo`

---

## Quick Start

### Build and Install
```bash
git clone https://github.com/manisk/ytui.git
cd ytui
cargo build --release
install -Dm755 target/release/ytui ~/.local/bin/ytui
```

### Sign In
To link your YouTube account and synchronize subscriptions and history:
```bash
ytui login
```
Visit `google.com/device` or `youtube.com/activate` and enter the code shown in the terminal.

### Install MPV Plugins
To populate the dedicated MPV profile with SponsorBlock, live chat, and reload plugins:
```bash
ytui setup-mpv
```

### Run
```bash
ytui
```

---

## Configuration

ytui loads settings from `~/.config/ytui/config.toml`. If omitted, default values are applied automatically:

```toml
[general]
graphics = 'auto'          # auto | sixel | kitty | iterm2 | halfblocks | none
icons = 'nerd'             # nerd | unicode | ascii
theme = 'youtube-dark'     # youtube-dark | youtube-light
mouse = false
stop_mpv_on_quit = true

[backend]
order = ['innertube', 'ytdlp']
auth = 'tv_oauth'          # tv_oauth | cookies | guest
cookies_file = '~/.config/ytui/cookies.txt'
client_version = ''
hl = 'en'
gl = 'IN'

[grid]
columns = 3                # 3 for 3x3 layout | 0 for auto responsive
min_card_width = 38
show_shorts = true
sidebar_expanded = false

[playback]
default_mode = 'window'    # window | inline | auto
use_user_mpv_config = false
hwdec = 'auto-safe'        # auto-safe | no | vaapi | nvdec-copy
max_resolution = '1080p'   # 1080p | 720p | 480p | 1440p | 2160p
osd_bar = false
mark_watched = true
autoplay = true

[inline]
max_height = 480
extra_args = []

[cache]
max_mb = 200
home_ttl_min = 15

[local]
dirs = ['~/Videos']
```

---

## Keybindings

Normal mode is active by default. Insert mode is used exclusively within input fields.

### Global Navigation
| Key | Action |
| --- | --- |
| `h` / `j` / `k` / `l` | Move cursor left / down / up / right |
| `gg` / `G` | Jump to first / last item |
| `Ctrl-d` / `Ctrl-u` | Half page down / up |
| `Ctrl-f` / `Ctrl-b` | Full page down / up |
| `{n}` prefix | Count prefix modifier (e.g. `5j`) |
| `Tab` / `Shift-Tab` | Cycle focus between sidebar, chips, and content |
| `b` | Toggle sidebar expansion |
| `[` / `]` | Previous / next filter chip |
| `/` | Focus search bar (Insert mode) |
| `f` | Local live filter of loaded items |
| `r` / `R` | Refresh / hard refresh feed |
| `gh` / `gs` / `gy` | Navigate to Home / Subscriptions / History |
| `gw` / `gp` / `gl` | Navigate to Watch Later / Playlists / Liked |
| `gv` / `gq` | Navigate to Local Videos / Play Queue |
| `q` / `Esc` | Back to previous view / quit at root |
| `Ctrl-c` / `Q` | Force quit immediately |
| `:` | Open command line |
| `?` | Toggle keybinding help overlay |

### Cards and Lists
| Key | Action |
| --- | --- |
| `Enter` | Play video in default mode |
| `e` | Play inline inside the current terminal |
| `w` | Play in external MPV window |
| `o` | Open URL in default web browser |
| `a` / `A` | Append to queue / play next |
| `v` / `V` | Toggle item selection / select all |
| `y` / `Y` | Yank video URL / yank title and URL |
| `m` | Mark as watched |

### Watch Screen Controls
| Key | Action |
| --- | --- |
| `Space` | Toggle pause / resume |
| `h` / `l` | Seek -5s / +5s |
| `H` / `L` | Seek -30s / +30s |
| `+` / `-` | Volume up / down |
| `M` | Toggle audio mute |
| `<` / `>` | Speed step down / up (0.25x steps) |
| `d` | Toggle description visibility |
| `f` | Toggle MPV fullscreen |
| `S` | Stop playback |
| `q` | Return to browsing view (playback continues in background) |

---

## Terminal Graphics Pipeline

1. Capabilities Query: On startup, ytui probes standard input/output for terminal protocol support and font cell dimensions (`cell_width`, `cell_height`).
2. Protocol Negotiation:
   - `foot`: DEC Sixel graphics protocol.
   - `kitty`, `ghostty`, `wezterm`: Kitty graphics protocol.
   - Others: Unicode half-block fallback.
3. Card Dimension Calculation: Thumbnail rows are derived from cell aspect ratio:
   `rows = round(card_cols * cell_w / cell_h * 9 / 16)`
4. Async Decode and Cache: Thumbnails are fetched asynchronously via Tokio worker pools, decoded via the `image` crate, stored in a 200 MB disk LRU cache (`$XDG_CACHE_HOME/ytui/thumbs`), and maintained in an in-memory protocol cache (up to 80 entries).

---

## Arch Linux PKGBUILD

A `PKGBUILD` is provided in the repository root for packaging on Arch Linux / CachyOS:

```bash
makepkg -si
```

---

## License

MIT License. See [LICENSE](LICENSE) for details.
