# Yute UI and release review

This review supersedes the earlier UI pass notes. The existing project is now
Yute: crates `yute` and `yute-core`, binary `yute`, configuration in
`~/.config/yute/`. The separate existing GTK frontend is `yute-gui` and is not
published. The local checkout stays in its original directory.

## Routing and feeds

The original Channel-to-MPV bug came from channel lockups falling through
`parse_lockup_view_model` into `Card::Video`. Entity activation now goes through
`actions::activation`: videos/Shorts/playable Live use MPV, Channel opens its
internal route, and ordinary Playlist opens internally. Upcoming streams report
unavailability. Renderers never start playback. Explicit Mix activation resolves
an actual playlist batch before playing its first video and queuing the rest.

The earlier single-result Home problem was separate from geometry. TV Home was
using an unsupported browse ID; its search fallback returned mostly Shorts, which
were hidden. The supported TV Home request now lets the existing backend chain
fall through to actual personal recommendations when necessary. View counts
missing from yt-dlp extraction are enriched from read-only metadata, only for
visible cards with at most two concurrent requests. Counts use K/M/B and upload
age is retained when the source provides it.

Home's Shorts and Live filters previously only filtered an already loaded batch
that contained neither type. They now request actual reel sequences and the Live
feed, cache each category separately, and restore positions when switching back.
Channels derives real owning channels from loaded video channel IDs; it is not a
fabricated global channel directory. Search filters continue to select from its
mixed typed results. Live read-only probes returned 10 actual Shorts, and 15 Live
feed entries including 10 tagged playable Live streams.

Channel routes expose returned tabs and data, with avatars or a monogram fallback.
Playlist routes show their actual returned videos. Mixes use watch-next playlist
requests, not invalid VLRD browse requests. Earlier live probes resolved 20 videos
per Mix; endless radio continuation beyond that returned batch is not implemented.

## Geometry and shell

The old image-derived card height admitted only one whole row at some viewport
sizes. One GridGeometry now determines drawing, selection scrolling and fetch
thresholds from content width and terminal cell dimensions. Cards are capped at
30 columns, have two title rows, and use physical 16:9 thumbnail rectangles.
Frames never share cells. At the screenshot viewport, multiple rows fit with and
without the sidebar; selected tint stays inside the frame and gutters stay clear.
Channel and playlist cards keep their typed labels and have no video duration.

A shared shell puts filter tabs at content x and starts the sidebar at the same y
on every route. There is no Home subtitle, permanent sidebar hint or NORMAL mode
chip. Sidebar grouping and dim shortcut hints remain. Alt+E reveals/focuses the
sidebar, then hides it on a second press; b remains a compatible toggle. h/l stay
inside the card row, Shift+H/L traverse route history, Shift+J/K change content
filters, Esc goes back, and standalone q quits. gq still opens Queue. Text modes
consume ordinary keys before normal dispatch.

## Watch and settings

Watch is a content view rather than a second MPV control panel. It has a passive
timeline, description with underlined/selectable links, comments below, and a
right suggestion pane with asynchronous cover thumbnails. On narrow terminals,
Suggestions can occupy the main pane. d/c/w or Tab focus Description, Comments and
Suggestions; j/k scroll or select. External MPV remains independent and playing
when leaving Watch. Its passive mini-player appears on other screens only while
an owned MPV session is alive.

Video +/- ratings accept YouTube's valid responseContext acknowledgement when the
TV endpoint does not echo an entity status. HttpOnly cookie rows are retained.
The UI displays authenticated mutation failures rather than generic feed errors;
like/dislike markers are mutually exclusive. No live account writes were tested.

Comments preserve author names, Unicode and emoji. Reaction, create and reply
commands use only real server-returned action parameters. Missing parameters
produce a useful unavailable message and :open fallback. A live read-only request
returned 20 comments but no reaction/post/reply tokens for the configured TV
client; posting and reactions on that client remain a backend capability limit.
Drafts require Enter to send and Esc cancels; successful posting requires a
confirmed returned comment action, not optimistic local content.

Settings uses compact step selectors, toggles, a bounded stream-cache slider and
selected-option explanations. Its Shorts selector offers Never show, Hidden in
feeds, and Show. Never overrides all saved-route/explicit-tab exceptions and
suggestions without deleting the underlying cache.

The stream-cache slider changes actual MPV readahead/cache-secs and a 32–256 MiB
packet budget. Dedicated launches use the same values. Initial playback does not
wait to fill that whole horizon; cache-pause-wait remains one second. User MPV
profile mode retains its startup preferences. Network resolution latency remains
outside the slider's control; no unsupported faster-startup measurement is claimed.

`:quality [480p|720p|1080p|1440p|2160p|best]` reports or changes quality with
completion. Numeric aliases and 4k work. Active streams reload once at the current
position while preserving pause; `yute --quality 720p` sets a session limit.
Quality, stream cache and Shorts preferences persist. Other existing navigation,
queue, search, volume, playback and account commands remain documented in
[commands.md](commands.md). Account switching and full cross-client history/resume
synchronization remain incomplete. No daemon or new frequent timer was added.

## Verification

- `cargo fmt --check`: pass across the renamed workspace, including GTK files.
- `cargo clippy --workspace --all-targets --all-features`: pass with existing style warnings.
- `cargo test --workspace`: 107 pass, 3 native tests ignored by default.
- All three native MPV tests were separately run and passed with local generated
  media/null outputs. Runtime quality, position, pause and actual cache properties
  were checked over native MPV JSON IPC. No YouTube playback/account writes were
  used for those native tests.
- `cargo build --release`: pass for the complete workspace.
- `cargo package -p yute-core -p yute --allow-dirty`: both packages build from their
  packaged sources using Cargo's temporary local registry. This does not publish.
- Layout/input coverage includes 80x24, 120x30, 135x31, 160x40, 200x50, typed
  entities, selection tint bounds, resizing, completion, q/gq/text consumption,
  category position restoration and legacy config/token compatibility.
- Release PTY check: Settings, Alt+E focus/hide, quality completion and q exit work.
  Five idle seconds produced 0 redraw bytes and approximately 0.2% CPU on this run.
  Thumbnail preparation remains asynchronous and bounded as before.
- Actual Sixel display was not visually verified. The following previews are
  TestBackend frames with synthetic fixture content and real halfblock thumbnail
  rendering, not live YouTube screenshots. Rasterization approximates terminal
  typography and uses the current theme.

[Subscriptions](../assets/screenshots/subscriptions.png) ·
[Search](../assets/screenshots/search.png) ·
[Active playback fixture](../assets/screenshots/active-playback.png) ·
[Command mode](../assets/screenshots/command.png) ·
[Watch](../assets/screenshots/watch.png) ·
[Settings](../assets/screenshots/settings.png)

The supplied icon is in `assets/yute.png` and the README. GitHub was created
private before pushing. Public crates.io publication is a separate decision:
its package source becomes public even while the GitHub repository stays private.
