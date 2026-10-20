# ytui Documentation

This directory contains technical documentation for `ytui` and `ytui-core`.

## Documents

- [Architecture Guide](architecture.md): Cargo workspace separation, engine design, InnerTube TVHTML5 client, and MPV IPC integration.
- [MPV Tuning and Hardware Decoding](mpv-tuning.md): GPU hardware decoding considerations for hybrid and direct MUX laptop architectures, buffer tuning, and subtitle handling.

## Overview

`ytui` is split into two workspace crates:

1. `ytui-core`: Pure Rust engine containing the InnerTube API client, TV OAuth device-code flow, feed parsers, playback tracking, queue manager, and MPV process controller. It contains zero terminal dependencies.
2. `ytui`: Terminal user interface implemented with Ratatui and Crossterm, utilizing `ratatui-image` for hardware-negotiated Kitty and Sixel thumbnail rendering.
