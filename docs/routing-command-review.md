# Routing, cards and command pass review

Implemented on the working tree from the previous UI pass. No commit, global
installation, dependency addition, service, daemon or watcher was created. The
existing optional GTK application is unchanged. Terminal video playback remains
removed; external MPV and thumbnail graphics remain the playback/rendering split.

## Exact routing cause and fix

`parse_lockup_view_model` classified playlists, then treated every other lockup as
Video. `LOCKUP_CONTENT_TYPE_CHANNEL` therefore became `Card::Video`, and the TUI
correctly followed that incorrect type into MPV. The typed Channel branch was not
the source of playback, but it fetched into the current feed without a channel
route/history entry. A separate playlist mix branch explicitly played a seed video.

The same audit found title-based "mix" classification and ambiguous ID prefixes.
These are removed as behavioral criteria. Explicit renderer/extractor entity types
win over prefixes; valid video IDs starting PL, RD or UC remain videos. yt-dlp
YoutubeTab channel results are Channels rather than generic Playlists. Legacy
malformed cached video identities cannot reach MPV, and versioned cache keys avoid
reusing old mistyped entries.

`actions::activation` maps typed Cards to Play, OpenChannel, OpenPlaylist or
Unavailable. Shorts/live/local/queue videos use Play; upcoming streams report
unavailable. Channels and playlists never become a seed Video. Main's
`dispatch_open` handles those actions, and `start_playback` is the only TUI call
site for `Player::play_video`. Renderers do not route or launch playback. Sidebar,
settings, account, search and tab input contexts consume Enter appropriately; Enter
on filter/tab navigation returns focus to content rather than playing a card.

## Channel, playlist and navigation capabilities

- Channel routes display real returned title, handle/subscriber metadata where
  available, and observed subscription state. Both Web/c4 and TV surface channel
  headers are parsed. A cheap avatar is retained in the model; no banner is added.
- Tabs are built only from returned browse endpoints/opaque params. Brackets switch
  them; visited tab feeds, continuation, selection and scroll are cached separately.
  A response without advertised tabs shows its actual feed, without invented tabs.
- Playlist routes retain ID/title and load playlist items, including mixes without
  autoplay. Enter on an actual video then plays it. Refresh retains the route ID.
- NavState saves detail routes, filters, query, selection, scroll, view modes and
  loaded continuations. Search draft input is separate from the results query.
  Back restores prior content without unnecessary refetches. Channel/playlist
  refresh uses the current typed detail route.
- Up Next selects actual typed recommendation entries. Queue mutations operate on
  local VideoCard entries and remove the selected duplicate instance correctly.
  Sidebar routing uses its Screen model rather than hard-coded menu positions.

## Cards and thumbnail rendering

Video thumbnail dimensions use terminal cell geometry and a 16:9 target. Exact-size
cover preparation uses aspect-preserving center crop/resize, then graphics encoding
in `spawn_blocking`. Decoded images/disk cache are retained; URL+cell-size cover
protocols have an 80-entry LRU and at most 16 pending preparations. A useful returned
landscape thumbnail is preferred when provided. Tiny source images are not expanded
unnecessarily while populating the decode cache.

Shorts use the same intentional center-cropped thumbnail region as other video
cards, without stretching or a portrait floating in a black landscape canvas.
The red SHORTS marker remains; available duration overlays the bottom-right.
LIVE/UPCOMING/type markers overlay artwork. Channel/playlist cards have type
metadata rather than fake durations. Titles are strongest, channels readable,
views/age secondary, with fixed text rows and graceful truncation.

Graphics mark covered buffer cells Skip. The shared badge helper restores text
diffing and retains an image payload when an overlay touches its first cell;
otherwise Sixel duration text can disappear or a badge can delete the whole image.
A regression test covers both. The existing loader's blocking standard-channel
receive is replaced with an async receive, tested in a single-thread runtime.

Synthetic fixture captures use the real Ratatui renderer and halfblock pipeline,
with loopback-served landscape/portrait gradient images:

- [Browsing](../target/pass2-previews/browse.png)
- [Command completion](../target/pass2-previews/command.png)
- [Search input](../target/pass2-previews/search.png)
- [Collapsed sidebar text capture](../target/pass2-previews/collapsed.txt)

These are renderer captures, not real Sixel/Kitty emulator screenshots.

## Header, sidebar, footer and commands

The header contains `yute` only. Search opens a temporary bottom row; the result
pane carries the query. Account/YouTube/play-icon badges and repeated route labels
are removed from the header. The sidebar is narrow, grouped by spacing, separated
by one quiet edge, scrolls its focused row into view, distinguishes route/cursor,
and reclaims all width when hidden. Shortcuts are dim and require no Nerd Font.

The footer shows mode, route and position with `? help`, a notification, or relevant
playback state. Input uses SEARCH/COMMAND rows. Notifications expire through the
existing event tick; no animation or new timer is added. Completion is a small list
above the command row. Images remain visible outside an overlapping popup, and
selection stays visible while entering commands.

`commands::REGISTRY` shares names, aliases, syntax, descriptions and parsing with
completion/help. Typed commands dispatch through common application actions.
Session history is bounded to 100 successfully parsed commands in memory. Unknown,
malformed and non-finite numeric arguments produce useful errors, never shell
execution. Absolute seeks now send MPV seconds instead of absolute-percent.

Implemented commands and aliases are listed in [commands.md](commands.md): navigation,
search, channel/playlist, play/pause/toggle/stop/next/prev, seek/volume/mute, local
queue operations, login/logout/account, authenticated subscription/rating requests,
open/copy, refresh, local filtering, graphics preferences, sidebar, help and quit.
`:q` confirms quit; normal-mode q remains Back. Graphics switching saves a preference
and reports restart required. `refresh hard` is a refresh alias, not a cache purge.

Account switching explicitly reports unavailable; hard-coded profiles and fake
success messages are removed. Logout confirms local sign-out, saves guest mode,
cancels obsolete login/account results and clears private route history while
retaining encrypted saved credentials. Guest cache namespaces and per-launch MPV
cookie handling prevent yute-owned credentials crossing into guest operation.
Both InnerTube and yt-dlp fallback remain available. No token/cookie values appear
in messages or command history. Existing encryption/permissions are retained.

## Verification

- `cargo test --workspace`: 82 passed; two native MPV tests ignored by default.
  Both ignored tests were run explicitly and passed with generated local media,
  null outputs, fake resolver/loopback fixtures and socket cleanup checks.
- `cargo clippy --workspace --all-targets --all-features`: completed successfully.
  Pre-existing warnings remain; no warning suppression was added.
- `cargo build --release`: passed, including the unchanged optional GTK crate.
  TUI binary: `target/release/yute`; it was not installed globally.
- `cargo fmt --check`: fails only on pre-existing formatting in five untouched
  GTK files. Every changed Rust file passes independent rustfmt checks.
- `git diff --check`: passed.
- Read-only live `bbs` search returned mixed videos, one channel and playlists.
  The channel returned its real title and 86 items; that TV response advertised
  no tabs, so no tab labels were invented. Returned-tab parsing and cached tab
  navigation are covered by fixtures.
- Actual pseudo-terminal run: minimal header, Tab completion and command history
  passed. Search → Channel opened 86 items without an MPV child, and Back restored
  the search filter. Exit code 0. Five idle seconds produced zero output bytes
  and 0.01 process CPU seconds at /proc tick resolution; this is a short sample.
- Settings tests no longer save live settings: persistence is in the actual input
  handler, outside the setting-cycle function used by renderer tests.

## Intentionally deferred and remaining limits

No broad authentication, recommendations or history rewrite was made. Request-time
token refresh, actual multi-account switching, endpoint-aware/nested shelf
continuations, and verified cross-client history/resume remain backend work. Local
logout does not revoke Google credentials or override independently configured
user MPV/yt-dlp authentication.

Authenticated subscription/rating commands call existing real endpoints, update
visible state only after successful results, and report failures. Live account
mutations were not performed. Watch Later add/remove is explicitly unavailable;
local queue is separate. Manual m reporting is retired rather than fabricating
watched time or claiming synchronization. Sorting without reliable numeric
metadata, persistent command history and full runtime graphics protocol switching
are deferred.

Real-emulator Sixel/Kitty visual verification and live network-video playback were
not manually performed. Cover pixels, halfblock rendering, Sixel payload/overlay
behavior, typed playback dispatch and native MPV IPC were tested separately.

## Files changed in this pass

This list is relative to the preserved start-of-pass working tree, rather than
counting the previous UI pass as new work:

- `README.md`
- `crates/yute-core/src/backend/innertube.rs`
- `crates/yute-core/src/backend/mod.rs`
- `crates/yute-core/src/backend/models.rs`
- `crates/yute-core/src/backend/parse.rs`
- `crates/yute-core/src/backend/ytdlp.rs`
- `crates/yute-core/src/media/thumbs.rs`
- `crates/yute-core/src/player/ipc.rs`
- `crates/yute-core/src/player/mod.rs`
- `crates/yute-core/src/player/mpvconf.rs`
- `crates/yute-core/src/player/tests.rs`
- `crates/yute/src/actions.rs`
- `crates/yute/src/app.rs`
- `crates/yute/src/commands.rs`
- `crates/yute/src/events.rs`
- `crates/yute/src/keymap.rs`
- `crates/yute/src/lib.rs`
- `crates/yute/src/main.rs`
- `crates/yute/src/media/thumbs.rs`
- `crates/yute/src/ui/accounts.rs`
- `crates/yute/src/ui/channel.rs`
- `crates/yute/src/ui/cmdline.rs`
- `crates/yute/src/ui/grid.rs`
- `crates/yute/src/ui/header.rs`
- `crates/yute/src/ui/help.rs`
- `crates/yute/src/ui/list.rs`
- `crates/yute/src/ui/mod.rs`
- `crates/yute/src/ui/settings.rs`
- `crates/yute/src/ui/sidebar.rs`
- `crates/yute/src/ui/statusbar.rs`
- `crates/yute/src/ui/thumbnail.rs`
- `crates/yute/src/ui/watch.rs`
- `crates/yute/tests/command_tests.rs`
- `crates/yute/tests/routing_tests.rs`
- `crates/yute/tests/ui_tests.rs`
- `docs/README.md`
- `docs/architecture.md`
- `docs/commands.md`
- `docs/routing-command-review.md`
- `docs/smarttube-parity.md`
