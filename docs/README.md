# yute Documentation

This directory contains technical documentation for `yute` and `yute-core`.

## Documents

- [UI Polish Review](ui-polish-review.md): Shared shell/grid geometry, input changes, captures and validation.
- [Command Reference](commands.md): Parsing, completion, session history and supported operations.
- [Routing and Command Pass Review](routing-command-review.md): Root cause, implementation, captures, checks and remaining limits.

- [SmartTube Protocol Reference](smarttube-parity.md): Inspected account/feed/history behavior, current gaps, and the next backend implementation sequence.
- [Architecture Guide](architecture.md): Cargo workspace separation, engine design, InnerTube TVHTML5 client, and MPV IPC integration.
- [MPV Tuning and Hardware Decoding](mpv-tuning.md): GPU hardware decoding considerations for hybrid and direct MUX laptop architectures, buffer tuning, and subtitle handling.

## Overview

`yute` is split into two workspace crates:

1. `yute-core`: Pure Rust engine containing the InnerTube API client, TV OAuth device-code flow, feed parsers, playback tracking, queue manager, and MPV process controller. It contains zero terminal dependencies.
2. `yute`: Terminal user interface implemented with Ratatui and Crossterm, utilizing `ratatui-image` for hardware-negotiated Kitty and Sixel thumbnail rendering.

- [Manual QA](qa.md): Human verification checklist, including outstanding features.
- [Configuration notes](configuration.md): Quality ceilings, paths and current limitations.
