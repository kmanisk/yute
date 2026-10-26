# Yute

A fast, keyboard-first YouTube terminal client built with Rust, Ratatui and Tokio.
Browse in the terminal and play video in external MPV.

[![Crates.io](https://img.shields.io/crates/v/yute.svg?style=flat-square&logo=rust)](https://crates.io/crates/yute)
[![docs.rs](https://img.shields.io/docsrs/yute?style=flat-square&logo=docs.rs)](https://docs.rs/yute)
[![License](https://img.shields.io/crates/l/yute?style=flat-square)](https://github.com/kmanisk/yute/blob/main/LICENSE)

## Install

```sh
cargo install yute
```

Runtime requirements:

- `mpv`
- `yt-dlp`
- optional: `ffmpeg` / `ffprobe` for local-media metadata
- optional: `wl-copy` or `xclip` for clipboard integration

Yute currently targets Linux/Unix-like systems.

## What it does

- Personalized Home recommendations, subscriptions, search, history and saved feeds.
- Typed navigation: videos and live streams play in MPV; channels and playlists open inside Yute.
- Sixel, Kitty and Unicode halfblock terminal thumbnails with asynchronous decoding and caching.
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
:shorts off
:queue add
:settings
```

Configuration lives at `~/.config/yute/config.toml`.

## Project

Source, screenshots, command reference and architecture documentation:

https://github.com/kmanisk/yute

Yute is early software. Account switching, full cross-client history/resume parity and endless Mix-radio continuation are not complete.

MIT licensed.

Shift+Enter opens typed item actions (play, copy, queue, save to a returned YouTube playlist, or download). `o` opens the selected entity in the browser. `g,` opens Settings. Downloads use yt-dlp at the selected quality and write to `~/Videos/Yute`.
