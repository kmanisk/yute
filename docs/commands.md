# Command mode

Press `:` to edit the bottom command row. Enter executes, Esc cancels, Tab completes
names and arguments. Up/Down, Shift-Tab and Ctrl-n/p cycle ranked suggestions;
Enter accepts the highlighted match (including prefixes such as :acc). Ctrl-r/s
recall session history; Up/Down also recall history when there are no suggestions.
Esc cancels, and Backspace on an empty line exits. All keys are captured while
editing. Matches and bounded descriptions appear in a framed popup above the input. History holds at most 100 successfully
parsed commands in memory; it is not persisted. Commands never evaluate a shell.
`?` is keyboard help; `:help` is command help, and `:help search` explains one command.

## Supported commands

| Group | Syntax | Behavior |
| --- | --- | --- |
| Navigation | `home`, `subscriptions` (`subs`), `history`, `watch-later` (`wl`), `liked`, `playlists`, `queue`, `local`, `settings`, `account` (`accounts`, `acc`, `a`) | Open the corresponding route. |
| Search | `search <query>` | YouTube search; preserves the previous route. Continuation pages require direct query evidence in title/creator metadata and are locally re-ranked, preventing drift into unrelated personalized recommendations. |
| Entities | `channel`, `playlist` | Open the selected item's channel ID or selected playlist. Missing identity is reported. |
| Watch dashboard | `details` (`now`, `watch`, `video-info`), `comments`, `description` (`desc`), `suggestions` (`up-next`) | Open/focus the currently playing video dashboard without restarting MPV. |
| Comments | `comment new`, `comment reply`, `comment delete`, `comment like`, `comment dislike` | Create/reply, confirmed own-comment deletion, or react to the selected comment. Writes are applied locally only after YouTube acknowledges them. |
| Playback | `play`, `pause`, `toggle`, `stop`, `next`, `prev`, `mute` | External yute-managed MPV only. `next` consumes the next queued video; `prev` uses bounded session playback history. |
| Seek | `seek +30`, `seek -10`, `seek 1:20`, `seek 1:02:30` | Relative seconds or absolute time, never percentages for colon timestamps. |
| Volume | `volume 50`, `volume +5`, `volume -5` (`vol`) | Absolute 0–100 or bounded relative change. |
| Local queue | `queue add`, `queue next`, `queue remove`, `queue clear` | Append/prepend a selected playable video, remove the selected queue instance, or clear the local queue. |
| Session | `account` (`acc`, `a`), `account <name/index/prefix>`, `account add` (`login`, `signin`), `account remove <name>` (`logout`), `account anon` (`anonymous`), `account whoami` | Quick picker, direct switching, TV login, confirmed local removal, Anonymous, or session details. |
| Subscription | `subscribe`, `unsubscribe` | Real authenticated requests for the selected channel; reported state changes only after successful results. |
| Rating | `like`, `unlike` | Authenticated selected-video/current-Watch rating requests. Unconfirmed results are errors. |
| Browser | `open`, `open channel` | Open the typed entity URL or selected item's channel through `xdg-open`. No embedded browser. |
| Clipboard | `copy url`, `copy title`, `copy channel` | Native `wl-copy` on Wayland or `xclip` on X11, invoked only by the copy action. |
| Refresh | `refresh`, `refresh hard` | Network refresh of the current route; hard is an alias, not a disk-cache purge. Normal `r` can retry continuation failures. |
| View | `view [toggle|grid|list]`, `grid`, `list` | Switch the active feed layout immediately without opening Settings. |
| Filtering | `filter all/videos/shorts/live/playlists/channels`, `filter <query>`, `filter` | Named types select a tab; other text filters loaded titles. Omit the query to clear the title filter. |
| Shorts | `shorts on`, `shorts off`, `shorts toggle` | Re-filter and save the visibility preference. Settings also offers Hidden outside Shorts, which also hides saved Shorts; explicitly opening Shorts always works. |
| Route names | `goto <route>` | Navigate to a canonical route; Tab lists available routes. |
| Theme | `theme terminal|auto|<name>`, `theme reload`, `theme list`, `theme check` | Apply/reload semantic themes live; invalid reloads keep the previous theme. |
| Graphics | `graphics auto|kitty|iterm2|sixel|halfblocks|off`, `set graphics <mode>` | Apply graphics mode live. Auto promotes only after a terminal protocol reply and otherwise stays on halfblocks. |
| Settings | `set <key> <value>` | Persist major UI/playback options without opening Settings. Tab completes keys/values for icons, columns/card width, cache, codec/hwdec, languages, autoplay/history sync, thumbnail cache/prefetch, action/sidebar behavior and key bindings. |
| Media formats | `play-select`, `audio` | Show actual available video/audio formats for one playback. |
| Download | `download [playlist] [video|audio] [best|2160p|1440p|1080p|720p|480p|360p|opus|aac|mp3|flac|m4a]` | Start a safe yt-dlp download. Playlist video qualities are per-item ceilings; conversion choices require ffmpeg. |
| Sidebar | `sidebar`, `sidebar show`, `sidebar hide`, `sidebar toggle` | Toggle or explicitly set visibility; hidden reclaims all width. |
| Help | `help`, `help <command>`, `keys` | Concise command help; Esc, q or Enter closes it. |
| Quit | `quit` (`q`), `quit!` (`q!`) | `:quit` confirms quit. Normal-mode `q` navigates back outside Home; on Home it opens a yes/no quit prompt. `:quit!` exits immediately. |

Prefix commands with `:` in the application. Empty, malformed, unknown and
non-finite numeric arguments produce status errors rather than launching a process.

## Account and backend limits

The built-in Anonymous account is always present. The picker and `ga` list saved
accounts, including returned brand identities. j/k + Enter or 1-9 switches; a adds
and d requests confirmed removal. Removal deletes local credentials and does not
revoke Google's token. Start as in Settings chooses last used or Anonymous.
Accounts own auth/visitor state and cache namespaces; switching cancels obsolete
requests and renders a saved Home page immediately before network revalidation.
All accounts share connection pools and thumbnail caches. Profile discovery uses
MediaServiceCore's V2 accounts_list read mask and branded X-Goog-Pageid header.

Subscription/rating commands use existing InnerTube endpoints; they are not locally
simulated, and live authenticated mutations were not performed during development.

Watch Later add/remove, like/dislike, subscription and supported history removal use authenticated backend mutations and update local state only after confirmation. Command history is bounded to the current Yute session and is intentionally not persisted across launches. Runtime theme and graphics preferences apply live where the active terminal supports them; terminal-protocol changes still depend on what that terminal actually confirms.

## Browsing preferences

```toml
[content]
shorts = "never" # "never", "hide" (default), or "show"
```

This supersedes the legacy `grid.show_shorts` preference. Home, Subscriptions,
Search and ordinary channel tabs hide Shorts by default, without deleting cached
items. Explicit Shorts filters/tabs show them. History, playlists, Watch Later,
Liked, Queue and Local preserve explicit content. `never` also hides saved Shorts and suggestions; explicit Shorts filters and channel Shorts tabs still work. `:shorts` explicitly changes and saves this preference without fetching again.

Local is intentionally offline-first: it always scans the configured `[downloads].directory` recursively (plus any optional `[local].dirs`) and does not require network access. This makes yt-dlp downloads immediately browsable when the laptop is offline.

Search and command rows support Left/Right, Home/End, Ctrl+A/E, Ctrl+U/W and
Unicode-safe editing. Command Up/Down recalls session history. `g` shows a small
navigation hint after 400 ms; it closes on a second key, Esc or a four-second timeout.
`g,` opens Settings (`go` remains an alias). `gn` returns to Watch without restarting playback.

Shift+J / Shift+K move forward/backward through the content-type tabs in browsing
views. Lowercase j/k continue moving through cards. Shifted characters remain text
inside command/search input; Watch uses contextual content navigation, with playback controls left to MPV.

Enter on a Mix loads its current returned video batch, plays the first in external
MPV, and queues the remainder before any existing queued items. Ordinary playlist
Enter remains internal navigation. Use `:playlist` on a Mix to browse it without
starting playback. Mix playback currently uses the returned batch; it does not
claim endless server-side radio continuation.


## Watch and settings update

Normal `q` goes back. On Home it opens a yes/no quit prompt; Ctrl+q and `:quit!` remain force-exit paths. Esc or Shift+H goes back and Shift+L goes forward. `gq` still opens Queue. `Space e` or Alt+E shows and focuses the sidebar, or hides it when it already has focus. `yy` copies the selected/current video URL, `zz` centers the current row, and `v` toggles grid/list view live. These bindings are configurable through `[keys]`, Settings, `:set`, and `YUTE_KEY_*` launch overrides. The header has no permanent sidebar hint, and the footer is limited to two functional rows.

Watch: d/c/w focus Description/Comments/Suggestions. Tab or h/l move between sections; j/k scroll or select. Enter opens a selected suggestion or a comment reply thread. +/- rate the video or focused comment. Comment reactions first resolve the focused comment detail continuation and then use YouTube's active toggle endpoint. `n` creates a top-level comment, `r` replies to the selected thread, and `x` offers confirmed deletion only when Yute can identify the selected comment as yours. `:comment new|reply|delete|like|dislike` exposes the same actions. Create/reply/delete use the TVHTML5 write protocol and update local state only after YouTube acknowledges the mutation. Older saved OAuth tokens may need one `:account add` re-login for the current YouTube write scope. Text input consumes normal keys; Enter submits and Esc cancels.
Description links are underlined; [/] selects a link and Shift+O opens it. Lowercase o opens the video in the browser.

`:quality [480p|720p|1080p|1440p|2160p|best]` reports or changes quality. Numeric
aliases such as `:quality 720` and `:quality 4k` work. With active MPV this reloads
once at the current position, preserving pause. `yute --quality 720p` applies a
session limit at startup. Quality completion offers actual supported presets.

Settings uses compact selectors and Enter-driven choice/text editors; paths, language tags, and configurable key sequences can be typed directly. Large YouTube seeks use a bounded seekable demux cache, short network timeouts and bounded FFmpeg reconnects. A real MPV 0.41 audit reproduced the long stall specifically from the stock OSC sending a burst of seek requests while the seekbar is dragged: single 40% seeks recovered in about two seconds, while drag bursts remained stuck past 30 seconds. With `MPV seekbar` off (the safe default), Yute launches MPV's upstream `slimbottombar` OSC layout, which keeps transport controls but has no draggable seekbar. Turning the setting on restores the stock full seekbar. If a network seek still wedges, Yute performs one fresh stream reload at the selected position; it never loops reloads and never applies this recovery to Local files. Dedicated YouTube playback enforces a 20-second minimum readahead. Yute does not pre-download media before playback.


## Item menu

Shift+Enter opens typed actions for the current video, channel or playlist.
Enter alone retains its original typed activation. o opens the entity in the
browser; g, opens Settings. Within the menu, j/k/arrows select and Esc/q cancels. The popup sizes to its content on normal terminals and scrolls on short ones, grouping playback, library, share and account actions while keeping the selected row visible.

Save requires authentication, loads real available destinations from
playlist/get_add_to_playlist, then submits browse/edit_playlist only after a
destination is chosen. Downloads run yt-dlp asynchronously and never interpolate entity text through a shell. Single videos use actual resolved formats; playlist video choices are per-item quality ceilings, and audio conversion choices appear only when ffmpeg is available. Playlist jobs use resumable archives and safe ordered templates under `~/Videos/Yute`. One job is active at a time, with explicit cancellation and child cleanup. Automated tests use fake/local processes rather than real media downloads.

## TV-code sign-in

`:account add`, `:login`, `:signin`, and `yute login` share the core TV-code flow.
The TUI modal shows the server-provided activation address and user code, an
optional half-block QR code when it fits, and the remaining lifetime. Esc cancels
without replacing the active session. CLI Ctrl+C cancels. Polling honors the
server interval, increases it after slow_down, and ends on denial or expiry.
OAuth refresh uses a 60-second skew, a single-flight lock per identity, and one
retry on HTTP 401. Revoked tokens mark that account as needing re-login.

The V2 request shape follows the repository's pinned MediaServiceCore source;
polling behavior also follows Google's limited-input-device documentation:
https://developers.google.com/youtube/data/guides/auth/devices

Anonymous is the default on a fresh installation. Subscriptions, Liked, Watch
Later, library playlists and account History show `Sign in with :account add`
without issuing authenticated requests. Search, public playlists, Local, Queue
and external MPV playback remain available.

## Available media formats

`:play-select` opens Play as (Video / Audio only / Best automatically) for the
selected YouTube video. Video and audio lists contain actual yt-dlp JSON formats;
`:audio` opens the audio list directly. Shift-Enter also exposes these actions.
Use j/k or arrows, Enter to choose, Esc/q to cancel. `:quality <height>` continues
to set the saved automatic ceiling. Manual format choices override it for one
playback only. There is no transcoding or download before playback.

`yute doctor` is a shell command, not a colon command. It checks tools, network,
InnerTube/session acceptance, encrypted storage, directories and terminal hints.
Failures return a nonzero exit status; optional missing tools produce warnings.
No credential values or account identifiers are displayed.


## Session and library states

`:account add` and `:login` use TV OAuth only; there is no selectable Cookie
session. Use `:account anon` for public browsing. Legacy or revoked accounts show
Needs re-login and require a new TV-code login. Library requests and mutations
preserve OAuth errors; yt-dlp never supplies a fake empty account library.
Both list and grid distinguish sign-in, re-login, loading, backend errors and
successful empty responses. Press r to retry a failed view.

Settings offers Auto or fixed 1–8 grid columns, and auto/nerd/unicode/ascii/none
icons. The sidebar and item menu share semantic glyphs with width-aware alignment.

## Theme, graphics and text

`youtube-dark` is the built-in red/black YouTube-inspired theme. `:theme terminal` follows the terminal palette using Reset/ANSI semantic roles. `:theme auto` selects `theme_dark` or `theme_light` after a bounded background query; config/theme files under `~/.config/yute/` reload from filesystem events. `NO_COLOR` removes decorative colors while retaining useful modifiers. `:theme check` reports known explicit-color contrast problems.

`:graphics auto` starts safely with halfblocks and promotes only after terminal replies confirm Kitty, Sixel or supported iTerm2 behavior. `YUTE_GRAPHICS` overrides the configured mode for one process. Multiplexed sessions conservatively fall back to halfblocks when passthrough has not been established.

`[text] emoji = "auto|keep|strip"` controls complex emoji handling. Titles and input use grapheme-safe, display-width-aware truncation, including extra safety for complex Indic text.


Explicit `:download video 1080p` checks the available streams before downloading.
Unknown qualities and audio modes are errors. `:download playlist` opens the
playlist download menu; `:download playlist video 1080p` selects the best available
stream up to that ceiling per item. Single-video menus list only actual formats.
See [configuration.md](configuration.md) for original versus converted audio,
archives, output directories and cancellation behavior.
