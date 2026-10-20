# Architecture Guide

## Workspace Design

The codebase is organized as a Cargo workspace:

```
ytui/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── ytui-core/              # Engine library (zero terminal dependencies)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── backend/        # TVHTML5 client, OAuth, parser, models
│   │       ├── config.rs       # Configuration parser and paths
│   │       ├── local.rs        # Local video filesystem scanner
│   │       ├── media/          # Thumbnail loader and disk cache
│   │       └── player/         # MPV controller, IPC, watch tracking, queue
│   └── ytui/                   # Ratatui TUI application
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

## Core Engine (`ytui-core`)

`ytui-core` provides headless services for consuming YouTube feeds and controlling playback:

1. **Authentication and Tokens:**
   - TV OAuth device-code authorization (`ytui login`).
   - Tokens stored on disk with strict `0600` permissions.
   - Encrypted at rest via AES-256-GCM.
   - Machine-bound encryption key derived from system hardware identifiers.
   - Token files (`~/.config/ytui/auth.json`) must remain excluded from dotfile managers such as chezmoi.

2. **InnerTube TVHTML5 Client:**
   - Emulates the stable TV client identity (`TVHTML5`).
   - Parses YouTube JSON renderers: `lockupViewModel`, `tileRenderer`, `shelfRenderer`, `reelItemRenderer`, and `compactVideoRenderer`.
   - Handles pagination via continuation tokens.

3. **History and Watchtime Tracking:**
   - Pings `https://www.youtube.com/api/stats/playback` when playback starts using a session client playback nonce (`cpn`).
   - Periodically queries MPV `time-pos` via IPC and pings `https://www.youtube.com/api/stats/watchtime` (every 25–30 seconds).
   - Transmits a final watchtime ping with actual elapsed position when playback is stopped.
   - Synchronizes exact resume positions and watch history across all devices.

4. **Player Controller:**
   - Launches external `mpv` processes communicating over dedicated Unix domain sockets.
   - Observes MPV property changes (`time-pos`, `duration`, `pause`, `volume`, `speed`).
   - Dispatches playback commands (`seek`, `cycle pause`, `set volume`, `stats toggle`).

## Terminal Frontend (`ytui`)

`ytui` handles layout and user interaction:

1. **Geometry and Responsive Grid:**
   - Calculates thumbnail rows based on actual font cell pixel dimensions (`font_size`).
   - Adapts to terminal heights from 80x24 up to ultrawide 240x60 displays.
   - Allocates card heights dynamically (top border + thumbnail + title + meta + bottom border) to eliminate border clipping.
   - Renders scrollbar indicators on the right margin with exact row offsets.

2. **Decoupled Graphics Adapter:**
   - Consumes raw `image::DynamicImage` instances from `ytui-core`.
   - Adapts images to `ratatui_image::protocol::StatefulProtocol` for Kitty, Sixel, or Unicode half-block rendering.
