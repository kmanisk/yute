# UI pass review

No commit, installation, package change or service was made. Existing uncommitted
MPV IPC/lifecycle edits and the player tests were preserved. The optional GTK
client has no changes from this pass.

## Resulting terminal UI

The existing header/search, filter tabs, sidebar and card grid remain. The selected
card alone has a plain accent border and selected background; inactive cards have
no visible box. Titles are bold, channels use readable normal foreground, and
views/date use a lighter secondary foreground. Narrow metadata drops the age
before clipping a combined date string. Active tabs have a solid contrasting fill;
inactive tabs have quiet text, and the active tab stays visible on narrow screens.

Shorts keep the red SHORTS label. Their portrait artwork uses the full available
card interior height with text beside it; rows keep equal height. Artwork fits its
original proportions. Other cached portrait images get the same treatment. No
black landscape canvas is painted around portrait artwork. Very narrow cards use
the ordinary fitted thumbnail layout.

`b` actually hides/shows the sidebar and releases its full width. Labels stay
readable, hints are dim, route markers and cursor backgrounds are distinct. Fixed
columns remain explicit; automatic columns use min_card_width. The renderer,
keyboard movement and footer share the actual column count. Resizing retains
selection and decoded image caches, without feed requests.

The footer uses one row, for example:

```text
YUTE · Home · 122 items · 1/122 · row 1/41       Enter Play · a Queue · y Copy · ? Help
```

It prioritizes state/loading/errors and falls back to `? Help` when short on width.
Help scrolls with j/k. Search and sidebar actions are contextual. Loading indicators
are static. Refresh retains cards and selected identity; continuation requests
append unique items without moving selection. Request generations discard stale
results, including responses from a superseded request on the same route.

Text-only renderer captures, with synthetic data and placeholder thumbnails:

- [80x24, sidebar hidden](../target/ui-review/80x24.txt)
- [120x40, sidebar expanded](../target/ui-review/120x40.txt)
- [160x40, sidebar hidden](../target/ui-review/160x40.txt)

These captures do not establish visual Sixel/Kitty correctness in a real emulator.
Metadata fitting was subsequently tightened to drop the age at narrow widths.

## Playback and architectural changes

- Removed the terminal video handoff and blocking terminal MPV path, the playback
  mode enum/override state, PlayInline action, special Shift+Enter action,
  InlineConfig and playback.default_mode fields, and obsolete documentation/hints.
  Enter and modified Enter use external MPV. There was no dedicated `e` binding
  in the inspected dispatcher; its obsolete README entry was removed.
- No inline-only dependency existed: the removed implementation used std process
  and terminal facilities already needed elsewhere. Manifests are unchanged.
- Kept external MPV process management and Unix JSON IPC in core, outside Ratatui.
  MPV remains active when leaving Watch; feed selection, filter, scroll and loaded
  continuation are restored. The footer can show title, playing/paused and times.
- Replaced unconditional 100 ms redraws with dirty rendering and a 250 ms update
  check. The existing asynchronous thumbnail pipeline and caches are retained.
- Added safe feed-error classifications for unauthorized/rejected/rate-limited/
  timed-out requests. Raw request diagnostics are not displayed. Typed backend
  diagnostics/details, account state and reporting correctness remain next-pass work.

## Checks and limits

- `cargo test --workspace --all-features`: 57 passed, two native MPV tests ignored
  by default. Both ignored native MPV tests were also run explicitly and passed
  with local generated media, fake resolver/loopback fixtures and null outputs.
- `cargo clippy --workspace --all-targets --all-features`: completed successfully;
  existing warnings remain. No global warning suppression was added.
- `cargo build --release -p yute`: passed. Binary: `target/release/yute`.
- `cargo fmt --check`: fails on pre-existing formatting in untouched GTK files.
  Formatting the baseline GTK app source also fails; unrelated formatting was
  reverted. `rustfmt --check` on every changed Rust file passes.
- `git diff --check`: passed.
- Pseudo-terminal launch at 120x40 with capability replies: successful, Ctrl-C exit
  code 0. Over five idle seconds: 0 output bytes and 0.00 process CPU seconds at
  /proc tick resolution. This is a short idle observation, not a broad benchmark.
- Live read-only InnerTube probe: Home 121 items, Subscriptions zero items, History
  HTTP 401. Available session: cookies, no OAuth token. The auth/request/parser
  implementation was not rewritten. Authenticated subscriptions/history/resume
  parity could not be verified with this session.
- Real Sixel/Kitty visual inspection and interactive network-video playback remain
  unverified; thumbnail code/protocol support is preserved, and IPC tests passed.

## SmartTube findings and postponed backend work

[Protocol mapping](smarttube-parity.md) records concrete modules, responsibilities,
request behavior, yute gaps and Rust recommendations for auth, refresh, accounts,
visitor/client context, Home, subscriptions, history/reporting, resume, libraries,
channels, continuations and retry/client-rejection behavior. Root licenses are MIT;
no Android/Java implementation or dependency was imported.

Key gaps: token refresh currently has no callers; visible account switching does
not switch AuthContext credentials. Atomic account/visitor switching and logout,
typed endpoint/client continuations and nested shelves, safe structured diagnostics,
and server-derived tracking fields also remain. Existing generic stats pings do not prove
cross-client history synchronization; the README's claim was corrected. These are
concrete follow-up work, not implemented parity. Also deferred: cancellation of
thumbnail work already in flight, off-thread graphics protocol encoding, and
refreshing channel/playlist detail routes with their complete route parameters.

## Files present in the working diff

- `README.md`
- `crates/yute-core/src/config.rs`
- `crates/yute-core/src/player/ipc.rs`
- `crates/yute-core/src/player/mod.rs`
- `crates/yute/src/app.rs`
- `crates/yute/src/events.rs`
- `crates/yute/src/keymap.rs`
- `crates/yute/src/main.rs`
- `crates/yute/src/media/thumbs.rs`
- `crates/yute/src/theme.rs`
- `crates/yute/src/ui/chips.rs`
- `crates/yute/src/ui/grid.rs`
- `crates/yute/src/ui/header.rs`
- `crates/yute/src/ui/help.rs`
- `crates/yute/src/ui/mod.rs`
- `crates/yute/src/ui/sidebar.rs`
- `crates/yute/src/ui/statusbar.rs`
- `crates/yute/tests/ui_tests.rs`
- `docs/README.md`
- `crates/yute-core/src/player/tests.rs` (pre-existing untracked tests; updated for the window-only API)
- `docs/smarttube-parity.md` (new)
- `docs/ui-pass-review.md` (new)
- `/home/onyx/docs/projects/yute/README.md` (removed the obsolete terminal-video shortcut row)

`player/ipc.rs` was already modified on entry; only formatting was applied to that
file in this pass. `player/mod.rs`, `app.rs` and `main.rs` also carried prior edits
which were retained. No commit was created.
