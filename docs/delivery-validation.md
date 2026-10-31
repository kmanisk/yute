# Terminal polish delivery validation

Verified 2026-10-29 in `/home/onyx/Projects/ytui`. Work continues on `main`;
previous backend, TV OAuth, watch/comment and media commits are preserved.
The source revision validated here is `403d2cc0e80a8702eda6e73f7b8998e85e72f938`;
the following documentation commit does not change executable code.

## Commits in this continued pass

- `9876220` fix(core): load owned playlists and preserve TV browse identities
- `1bfbcab` fix(ui): honour graphics visibility and terminal theme capabilities
- `546dac5` fix(download): validate requests and resolve explicit video qualities
- `df28727` fix(download): terminate conversion processes on cancellation
- `9d63454` fix(theme): warn for identical palette foreground and background
- `403d2cc` fix(core): reject account continuations without forward progress

## Implemented behavior and evidence

| Area | Result |
| --- | --- |
| Theme architecture | Prebuilt semantic styles, terminal-native default, custom TOML themes, packaged dark/light examples, modifiers, NO_COLOR and palette downgrade. Indexed colours now also downgrade on 16-colour terminals. Contrast checks do not invent ANSI/Reset RGB values; equal explicit palette entries are detected. |
| Light/dark detection | OSC background and colour-scheme reports after first frame, cached per process; COLORFGBG fallback. Focus reporting is enabled and re-queries background. No portal dependency or idle polling. |
| Live reload | Native notify filesystem events with 150 ms quiet debounce and background parsing. Config/theme replacement is atomic; parse errors retain previous state. Backend/active-player changes are labelled restart required. PTY valid/error reload checks passed. |
| Graphics | Auto requires terminal responses before protocol promotion. Kitty, Sixel, iTerm2 and halfblocks paths remain available; off/none now disable every image path. Doctor reports detection method and dimensions when available. Multiplexers conservatively use halfblocks. |
| Overlay ghosts | Popup rectangles suppress intersecting images, including account avatars/comment media. Structural repaint clears cells; switching away from Kitty also deletes its previous placements. Physical terminal visual confirmation remains pending. |
| Thumbnail bars | Cover preparation crops to exact physical target dimensions. Conservative removal handles strongly detected symmetric bars in 4:3 sources; dark images are not blindly cropped. Synthetic crop tests pass. The original thin-strip screenshot's precise cause cannot be established from the available image/terminal evidence. |
| Shell bars | One route title with filter chips and account indicator at top. Bottom status shows position/state/messages/help. Contextual action row appears automatically on tall terminals, sharing footer space with the active mini-player. |
| Item menu | Content-derived geometry, two-line width-safe title, grouped contextual actions, download destination hint, bounded scroll and visible selection. Tests cover video/channel/playlist, active downloads and 15/24/40-row bounds; all applicable normal menus fit at 24 rows. |
| Unicode/text | Grapheme-aware wrapping retains long-word remainders; ellipsis is inside display width. Indic titles reserve a safety cell. Emoji auto/keep/strip remains configurable. Hindi, CJK, combining marks, flags and ZWJ fixtures pass. Account header sizing now uses display width. |
| Dates/progress | Separator-only metadata is ignored. Sanitized Home/Search/Live/Premiere/playlist/date fixtures pass. Nonempty metadata joins avoid trailing separators. Watch progress stays on thumbnails. Missing metadata enrichment remains asynchronous and visible-item bounded. |
| Responsive grid | Auto 1–8 columns uses content width after sidebar; complete rows avoid border stubs. One-row gaps, resize/sidebar selection consistency and fixed-column behavior remain covered. Next-row prefetch is now exactly one column-count row before lower-priority work. |
| Channels | OAuth subscribed-channel route, account cache, real avatars and optional tall-sidebar names list are preserved. Live read-only probe parsed 68 channels and opened a returned tab with 15 videos. |
| Playlists | Owned listing now uses FEplaylist_aggregation instead of the general FElibrary shelf. Browse-linked VL identities are preserved; missing identities are rejected. Playlist cache key is versioned. Live probe returned 41 playlist cards and 15 videos when opening one. Its continuation replayed those same 41 identities while rotating the token; the backend now returns an explicit error and the TUI retains existing content instead of looping. Existing grid/selection/thumbnail infrastructure is reused. |
| Icons | Central semantic enum; auto/nerd/unicode/ascii/none remain supported. All 19 Nerd mappings are present in the locally installed Iosevka Nerd Font cmap. No new glyph codepoints were introduced. |
| Quality/audio | Existing :play-select, :audio and external MPV paths are preserved. Menus use structured available formats, including codec/fps/HDR/language where present; current-video cache TTL is five minutes. Explicit selection affects one playback. |
| Downloads | Strict command grammar rejects invalid qualities/modes and channel downloads. Explicit single-video resolutions are probed before download; playlist resolutions remain per-item ceilings. Original audio and ffmpeg-gated conversion entries remain distinct. |
| Download lifecycle | One owned async yt-dlp job, direct arguments, sanitized templates, ordered playlist directories, partial files and per-job archives. Cancellation now kills the Unix process group, including conversion descendants. Fake-process cancellation tests passed. |
| Authentication/GUI | OAuth/account/token architecture is preserved. No cookie auth fallback was added. Shared APIs remain compatible; GTK check and workspace release build pass. GTK presentation was not redesigned or installed. |

## Quality gates

All commands ran sequentially with normal Cargo configuration; the global
six-job limit was not overridden and no independent Cargo builds overlapped.

| Command | Result |
| --- | --- |
| cargo fmt --check | Passed |
| cargo clippy --workspace --all-targets --all-features | Passed; the same 28 existing warning diagnostics as the starting revision, no new warning category/count |
| cargo test --workspace | 201 passed, 0 failed, 6 opt-in tests ignored across 18 suites |
| cargo build --release | Passed for the workspace, including GTK linking |
| cargo check -p yute-gui | Passed |
| git diff --check | Passed |

The six ignored tests comprise three native MPV tests, public-avatar network QA,
and two read-only OAuth probes. Ignored tests are not counted as passes. The
owned-playlist probe was explicitly run and passed for listing/opening and the
expected typed replay error. It did not verify an additional distinct page.
A broader read-only probe separately loaded Subscriptions (28 cards), History,
Watch Later and Liked (15 cards each), plus their next pages. No account writes
were sent. Repeated live probes also encountered intermittent ten-second YouTube
request timeouts; they were explicit errors, never substituted empty feeds. The
installed doctor subsequently exited 0 and confirmed request acceptance; this is
separate from personalized-feed evidence.

PTY checks passed for theme reload, malformed config retention, real RGB theme
output with NO_COLOR absent, focus query emission and mode cleanup. These are
pseudo-terminal tests, not physical image/font validation.

## Performance and installation

Historical before-polish YUTE_TRACE first-frame sample: 1.419 ms. Two final-source
samples: 1.322 ms and 1.328 ms. These are individual Rust-entry-to-first-frame
measurements, not process-launch timings or a statistically controlled speedup.
The 200x40 PTY used 10x20-pixel cells and isolated anonymous config/cache/state.
Both final package-build idle samples emitted 0 bytes and recorded 0 CPU ticks;
the additional settled two-second interval also recorded 0/0. CPU tick
granularity is 10 ms. An earlier workspace-build sample had one startup
follow-up draw with 19 style/cursor bytes and no text-cell updates.

Installed path: `/home/onyx/.local/bin/yute`. The prior executable was backed up
under `~/.local/state/yute/binary-backups/`. Installed and release SHA256:

```text
43fcc6fea67d92a8431c0db79450fef513ccbdb71e96f72068014d7486499a3d
```

Release size is 12,262,264 bytes (21,352 bytes above the executable found when
this continued pass began). Existing running instances require restarting.
The installed artifact is from cargo build --release -p yute. The workspace
release gate produces a different hash through Cargo feature unification
(including smallvec union and tracing-subscriber env-filter); the final package
build was measured and verified separately.
There is no existing PATH installation of yute-gui, so no new GUI packaging was
introduced. Installed --help and doctor both exited 0. Doctor ran through a pipe:
terminal protocol/cell/background detection was unavailable there and reported
as such rather than inferred from environment hints.

Active `~/.config/yute/config.toml` already uses terminal theme, Auto columns and
explicit Nerd icons. It is a regular user-owned file and not chezmoi-managed.
No migration or config write was needed in this pass; unrelated settings and the
legacy `~/.config/ytui/config.toml` were preserved.

## Dependencies and reference boundary

Theme/input work already on main uses notify 8.2.1 for native filesystem events,
TOML 0.8 for theme parsing, and libc 0.2 for the single terminal input owner. This
pass also declares libc directly in yute-core to signal owned Unix download
process groups; it was already a transitive dependency. No heavy theme library,
desktop portal, daemon or browser was added. Per-dependency binary impact was
not isolated from other source changes.

SmartTube `/home/onyx/Documents/repo/smarttube` and its pinned MediaServiceCore
reference in ignored `target/smarttube-reference` have MIT root licenses, as
inspected. Only behavior/request shapes were used; no implementation was copied.
Protocol paths/findings are recorded in [smarttube-parity.md](smarttube-parity.md)
and authoritative terminal specifications in [architecture.md](architecture.md).

## Remaining manual QA and limits

Use [qa.md](qa.md), especially physical Kitty/Sixel/iTerm2 overlays, terminal
light/dark changes, Hindi/emoji shaping, 16:9 images and resize/fullscreen. Kitty,
foot and tmux are available locally, but none of the physical terminal matrix
cells were visually verified. Multiplexer graphics passthrough is not negotiated.
No claim is made that the original thin black strip was visually reproduced.
The observed TV owned-playlist continuation replays its current page; results
beyond the 41 returned playlists could not be verified. This server behavior
now produces an explicit retry error, never duplicate growth or fake emptiness.

Interactive fresh-login/QR/revocation/account-switch flows, real account writes,
cross-client history/resume and live download/conversion/resume workflows remain
manual QA. Downloads stay anonymous; private/restricted extraction can fail.
Archive skips/unavailable-item categories are not separately counted in the
completion summary. TV channel tabs that supply only opaque navigation and no
canonical channel ID cannot expose canonical-ID subscription writes. This is not
full official YouTube/SmartTube parity.

The delivery workflow commits on main and pushes directly to origin/main;
no PR, tag or force push is part of this workflow. The pre-existing untracked
root AGENTS.md is intentionally left outside commits. Final SHAs and push outcome
are reported in the delivery response.


## Screenshot follow-up, 2026-10-31

Implementation commit: `99fe67bbe4e16c128c6f9087e7e14182e1d449f3` on main.
This follow-up preserves the backend, OAuth and GTK architecture.

- Automatic layout can use narrower cards to fit two complete rows with the
  sidebar shown at the reported window sizes. Cards stay inside the pane, with
  a dedicated scrollbar column. The generic Content title is removed.
- The focused card uses a double border, highlighted title and normal image
  brightness. Other cards have dimmed titles and asynchronously cached dimmed
  artwork. Watched progress is a thin bottom-border mark rather than a full
  row covering the thumbnail. Account text/avatar have right-edge padding.
- Scrolling is calculated before image invalidation. Raster anchors restore
  cursor/rendition state, and structural repaint clears and draws together in
  one synchronized terminal update instead of flushing a blank frame first.
- Inspected the supplied recording; a native Foot/Sixel anonymous fixture on
  Wayland then showed two rows at 115x31 with sidebar, clean rapid navigation,
  and clean action-menu open/close restoration. Other protocols and real
  personalized feeds still require manual visual QA; see [qa.md](qa.md).

| Final check | Result |
| --- | --- |
| cargo fmt --check | Passed |
| cargo clippy --workspace --all-targets --all-features | Passed; same 28 pre-existing warning diagnostics |
| cargo test --workspace | 206 passed, 0 failed, 6 opt-in ignored |
| cargo check --workspace | Passed |
| cargo build --release | Passed |
| cargo build --release -p yute | Passed; final installed artifact |
| cargo check -p yute-gui | Passed |
| Installed yute --help / doctor | Both exited 0 |

Doctor confirmed encrypted-store readability and server acceptance of the OAuth
JSON request. This does not prove personalized-feed contents. Its non-TTY run
reported terminal/cell dimensions unavailable, as expected.

YUTE_TRACE before/after package-release samples in the same isolated 200x40 PTY
(10x20-pixel cells): 1.480 ms / 1.399 ms to first frame. Settled two-second idle
intervals produced 0 bytes and 0 CPU ticks in both runs (10 ms tick resolution).
These are individual samples, not a statistical speedup claim. An additional
baseline sample taken during Cargo linking recorded 768.163 ms; the otherwise
idle baseline was measured again after that build finished.

No dependencies were added. The package release is 12,267,408 bytes, 5,144 bytes
larger than the previously installed executable. It was installed atomically to
`/home/onyx/.local/bin/yute`, after backing up the old binary under
`~/.local/state/yute/binary-backups/`. Installed/build SHA256:

```text
e986c42b839fc3140d24f5429f36cec235b3fbf4d6de8acea58045eb38d25b75
```

The active config already selects auto columns, Nerd icons and terminal theme;
no local configuration was changed. The legacy ytui config is retained. Existing
Yute processes need restarting to use the installed build. No GUI packaging or
installation method was changed. Cargo gates ran sequentially with the configured
six-job limit; no Cargo/global system settings were modified.
