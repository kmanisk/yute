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
| Search | `search <query>` | YouTube search; preserves the previous route. |
| Entities | `channel`, `playlist` | Open the selected item's channel ID or selected playlist. Missing identity is reported. |
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
| Filtering | `filter all/videos/shorts/live/playlists/channels`, `filter <query>`, `filter` | Named types select a tab; other text filters loaded titles. Omit the query to clear the title filter. |
| Shorts | `shorts on`, `shorts off`, `shorts toggle` | Re-filter and save the visibility preference. Settings also offers Hidden outside Shorts, which also hides saved Shorts; explicitly opening Shorts always works. |
| Route names | `goto <route>` | Navigate to a canonical route; Tab lists available routes. |
| Graphics | `set graphics sixel`, `kitty`, `halfblocks`, `none`, `auto` | Save the chosen graphics preference; restart required. |
| Sidebar | `sidebar`, `sidebar show`, `sidebar hide`, `sidebar toggle` | Toggle or explicitly set visibility; hidden reclaims all width. |
| Help | `help`, `help <command>`, `keys` | Concise command help; Esc, q or Enter closes it. |
| Quit | `quit` (`q`), `quit!` (`q!`) | Confirm application quit or quit immediately. Normal-mode q remains Back. |

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

Watch Later add/remove, sorting on unreliable text metadata, persistent command
history and runtime graphics switching are intentionally not implemented. Manual
`m` history reporting no longer claims synchronization or fabricates watched time.

## Browsing preferences

```toml
[content]
shorts = "never" # "never", "hide" (default), or "show"
```

This supersedes the legacy `grid.show_shorts` preference. Home, Subscriptions,
Search and ordinary channel tabs hide Shorts by default, without deleting cached
items. Explicit Shorts filters/tabs show them. History, playlists, Watch Later,
Liked, Queue and Local preserve explicit content. `never` also hides saved Shorts and suggestions; explicit Shorts filters and channel Shorts tabs still work. `:shorts` explicitly changes and saves this preference without fetching again.

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

Normal `q` quits; Esc or Shift+H goes back and Shift+L goes forward. `gq` still opens
Queue. Alt+E shows and focuses the sidebar, or hides it when it already has focus.
The header has no permanent sidebar hint, and the footer has no NORMAL label.

Watch: d/c/w focus Description/Comments/Suggestions. Tab or h/l move between
sections; j/k scroll or select. Enter opens the selected suggestion/link. +/- rate
the video or focused comment; n starts a comment and r replies. Text input consumes
normal keys. Real action parameters and authentication are required; absent
capabilities are reported rather than simulated. Enter posts a draft; Esc cancels.
Description links are underlined; [/] selects a link and Shift+O opens it. Lowercase o opens the video in the browser.

`:quality [480p|720p|1080p|1440p|2160p|best]` reports or changes quality. Numeric
aliases such as `:quality 720` and `:quality 4k` work. With active MPV this reloads
once at the current position, preserving pause. `yute --quality 720p` applies a
session limit at startup. Quality completion offers actual supported presets.

Settings uses compact step selectors/toggles and a stream buffer slider. Buffer
presets are 5/10/20/40/60 seconds with 32/64/128/192/256 MiB packet budgets. The
slider updates active MPV properties; dedicated-profile launches use the same
values. Initial playback does not wait to fill the entire cache horizon.


## Item menu

Shift+Enter opens typed actions for the current video, channel or playlist.
Enter alone retains its original typed activation. o opens the entity in the
browser; g, opens Settings. Within the menu, j/k/arrows select and Esc/q cancels.
Shortcuts p/v/o/y/t/a/s/d/x select play, browse, browser, copy URL, copy title,
queue, save to playlist, download, and cancel download when applicable.

Save requires authentication, loads real available destinations from
playlist/get_add_to_playlist, then submits browse/edit_playlist only after a
destination is chosen. Downloads run yt-dlp asynchronously at the chosen quality,
write to ~/Videos/Yute, and appear under Local. One download job is allowed at a
time, with explicit cancellation and process cleanup on exit. No account writes
or real online downloads are part of the automated tests.

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
