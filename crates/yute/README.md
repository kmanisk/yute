# Yute

A fast, keyboard-first YouTube terminal client built with Rust, Ratatui and Tokio.
Browse in the terminal and play video in external MPV.

[![Rust](https://img.shields.io/badge/Rust-2021-b7410e?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)](https://github.com/kmanisk/yute/blob/main/LICENSE)

## Install

```sh
cargo install --git https://github.com/kmanisk/yute yute
```

Runtime requirements:

- `mpv`
- `yt-dlp`
- `node` for yt-dlp's YouTube JavaScript runtime
- optional: `ffmpeg` / `ffprobe` for metadata and explicit audio conversion
- optional: `wl-copy` or `xclip` for clipboard integration

Yute currently targets Linux/Unix-like systems.

## What it does

- Personalized Home recommendations, subscriptions, search, history and saved feeds.
- Typed navigation: videos and live streams play in MPV; channels and playlists open inside Yute.
- Confirmed-protocol Kitty, iTerm2, Sixel and Unicode-halfblock thumbnails with asynchronous decoding and caching.
- Semantic terminal themes, light/dark auto mode, custom theme files and live reload.
- Vim-style navigation, route history, command completion, command history and contextual help.
- Configurable quality, hardware decoding, stream buffering and MPV profiles.
- No daemon, embedded browser or webview.

## Quick start

```sh
yute
yute login       # optional YouTube device-code login
yute setup-mpv   # optional MPV profile/scripts
```

Press `/` to search, `:` for command mode and `?` for contextual help.

Example commands:

```text
:search rust async
:filter videos
:quality 1080p
:play-select
:audio
:theme auto
:graphics auto
:shorts off
:queue add
:settings
```

Configuration lives at `~/.config/yute/config.toml`.

## Project

Source, screenshots, command reference and architecture documentation:

https://github.com/kmanisk/yute

Yute is early software. Full cross-client history/resume parity, endless Mix-radio continuation and some terminal-protocol combinations still require live verification.

MIT licensed.

Shift+Enter opens typed item actions for playback/quality, audio, account writes, queueing, copying and downloads. `o` opens the selected entity in the browser. `g,` opens Settings. Downloads use yt-dlp, expose actual single-video formats and playlist quality ceilings, support original audio plus ffmpeg-backed conversions, and write to `~/Videos/Yute`.
