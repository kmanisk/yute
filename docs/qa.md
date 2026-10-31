# Manual QA

All items below require human verification; unchecked means unverified, not passed.
Run `yute doctor` first. It uses bounded requests and never prints token values.
A doctor success does not establish account/feed correctness.

## Account and backend

- [ ] TV OAuth: activation URL, Unicode QR, pending, slow-down, expiry, denial,
  Esc cancellation, successful encrypted save; restart and refresh.
- [ ] Revoke a test session; verify needs re-login and a single refresh/retry.
- [ ] Switch primary/brand/anonymous accounts during slow requests; no stale
  cards, continuations or account state appear after switching.
- [ ] Home, Search, Subscriptions with avatars, date-grouped History, Watch Later,
  Liked, Playlists and playlist contents.
- [ ] Queue while signed out/offline; Local configured directories.
- [ ] Channel advertised tabs, metadata and avatars; Watch details and Up Next.
- [ ] Like/dislike/remove rating, subscribe/unsubscribe, save/remove playlist
  items and history removal; confirm server state independently.
- [ ] History sync on/off and cross-client resume after pause/seek/stop.
- [ ] Shorts success, genuinely empty response, HTTP/parse errors; no endless
  Loading/0-of-0 state. Mixes and live/upcoming content.

## Terminal

- [ ] Resize and fullscreen at 50x15, 80x24, 120x40, 200x60 and 320x90.
- [ ] Hindi, Japanese, accented Latin and grapheme sequences truncate safely.
- [ ] Sixel, Kitty and halfblocks; terminal cell dimensions and graphics hints.
- [ ] Icon modes Nerd, Unicode, ASCII and none; menu/sidebar alignment.

## Requested terminal UX matrix

These checks are intentionally terminal-specific. A blank box stays unchecked unless that exact terminal was exercised; capability/environment hints are not counted as visual proof.

| Terminal | Non-Latin/native toggle | Selection fill/no holes | Search Filters modal | Watch 80x24 / 120x40 / 200x60 / 320x90 | `doctor --seek-test` path |
| --- | --- | --- | --- | --- | --- |
| foot | [ ] | [x] | [x] | [ ] | [ ] |
| kitty | [ ] | [ ] | [ ] | [ ] | [ ] |
| Ghostty | [ ] | [ ] | [ ] | [ ] | [ ] |
| WezTerm | [ ] | [ ] | [ ] | [ ] | [ ] |

For non-Latin checks use at least Hindi plus Russian, Japanese, Korean, Chinese, Thai, Tamil/Bengali, Arabic/Hebrew and emoji. Verify `T` restores the original title where Auto chose transliteration. For selection fill, inspect the entire highlighted title rectangle for unpainted cells. In Search, open `f`, move across all five sections, toggle multiple Features, apply, then continue to another page and confirm order/token continuity. In Watch, inspect loading/content/error and with/without-comments states at each size. The seek harness is sanitized and can be run outside a graphical terminal; the table still records which terminal/session environment was used when the test was launched.

Foot/Sixel evidence from this pass used Iosevka Nerd Font Mono in a native Sway/Wayland window. Valid captures confirmed: Auto fallback on the measured Hindi/Tamil/Bengali width failures; `T` restoring the selected Hindi title to native Devanagari; a fully painted selected-title rectangle with no black holes; the five-column Search Filters modal; and a byte-for-byte identical screenshot after pressing normal `q` inside that modal. The same live instance hid the sidebar at 56 columns and restored it at 115 columns without changing the remembered state. The full non-Latin cell remains unchecked because Korean/Chinese glyph coverage is missing on this machine and the attempted second-page Arabic/Hebrew/emoji capture was obscured by another window. Kitty is installed but was not visually exercised in this pass. Ghostty and WezTerm were not found on PATH. Watch's terminal-specific size matrix and terminal-specific seek-launch cells remain manual.

## Available formats (implemented; live QA pending)

- [ ] Enter plays default video with the configured maximum.
- [ ] Shift-Enter → Play / quality → Video, or `:play-select`.
- [ ] Select available 1080p and 720p; absent qualities never appear.
- [ ] Choose AV1/VP9/H.264 where available; HDR and frame rate labels agree
  with MPV stream properties.
- [ ] `:audio` chooses original audio only; test multiple codecs/languages.
- [ ] Next normal playback returns to configured video defaults.
- [ ] Reopen a video menu within five minutes: cached probe; another video
  invalidates the single current-video cache.
- [ ] Esc during format resolution cancels; normal `q` remains unbound and late results cannot reopen a closed menu.
- [ ] Unavailable requested automatic quality falls back below the ceiling.
- [ ] Missing yt-dlp, network failure and malformed output produce an error.
- [ ] Private/age-restricted media reports a useful error. Authenticated format
  resolution is outstanding; do not put tokens in command arguments.

## Downloads (implemented; live network QA pending)

Automated tests cover argument-only subprocess construction, single-video selectors, playlist quality ceilings, conversion gating, progress parsing, archives, safe templates, continuation after a failed playlist item, and cancellation. Live media downloads remain manual QA.

- [ ] Single video: chosen available video quality.
- [ ] Audio: original best/Opus/AAC versus explicitly labelled MP3/FLAC/M4A conversion.
- [ ] Missing ffmpeg hides conversion choices; no unnecessary conversion.
- [ ] Cancel and resume partial download; quit terminates children.
- [ ] Playlist video <=1080p ceiling with lower-quality per-item fallback.
- [ ] Playlist original audio, Opus/AAC preferences and conversions.
- [ ] One unavailable/failed item does not abort the remaining playlist.
- [ ] Final completed/failed totals stay sanitized.
- [ ] Playlist retry/archive skips completed IDs; safe directory ordering.
- [ ] Filenames/URLs containing shell metacharacters remain literal arguments.

## Theme and terminal graphics matrix

For each available terminal, test dark and light palettes on Wayland and X11 where applicable. Unavailable combinations stay unchecked rather than being inferred.

| Terminal | Graphics expectation | Dark | Light | Overlay ghosts | Live theme/config reload | Resize/fullscreen |
| --- | --- | --- | --- | --- | --- | --- |
| kitty | Kitty | [ ] | [ ] | [ ] | [ ] | [ ] |
| Ghostty | Kitty when query confirms | [ ] | [ ] | [ ] | [ ] | [ ] |
| WezTerm | confirmed Kitty/iTerm2/Sixel path | [ ] | [ ] | [ ] | [ ] | [ ] |
| foot | Sixel | [ ] | [ ] | [ ] | [ ] | [ ] |
| Konsole | Sixel when reported | [ ] | [ ] | [ ] | [ ] | [ ] |
| xterm + Sixel | Sixel | [ ] | [ ] | [ ] | [ ] | [ ] |
| Alacritty | halfblocks | [ ] | [ ] | [ ] | [ ] | [ ] |
| tmux/zellij | halfblocks; passthrough not negotiated | [ ] | [ ] | [ ] | [ ] | [ ] |

Exact manual checks: 16:9 crop with no black strip, no image/text ghosts after every overlay, terminal background/color-scheme response handling, terminal palette switch while unfocused then FocusGained, config/theme hot reload, Hindi/CJK/emoji boundaries, subscribed-channel avatars/sidebar, playlist density and action-menu scrolling at short heights.

## Follow-up delivery evidence, 2026-10-29

The automated suite covers legacy-cookie re-login state, authoritative account
errors, recognized empty feeds, account continuation provenance, avatar variants
and sizing, priority/cancellation, automatic grid bounds, selection retention,
Unicode/grapheme/cursor safety and icon fallbacks. The live anonymous channel
avatar/CDN decode test and three native MPV tests (generated local media with
null audio/video outputs) passed explicitly.

TV OAuth sign-in is user-confirmed working. Read-only OAuth probes exercised
Subscriptions, History, Watch Later, Liked and their next pages, subscribed
channels, and owned-playlist listing/opening. Fresh login/QR behavior, account
mutations and cross-client history/resume still require interactive QA. No cookie
session substituted for account validation. Kitty/Sixel visual review remains
human QA; environment hints are not graphics-negotiation proof.

## Watch dashboard/comment media

- [ ] `:details` opens the active video without restarting MPV; d/c/w and Tab/h/l switch Description, Comments and Suggestions.
- [ ] Comment avatars, reply counts and nested thread opening match the returned YouTube data.
- [ ] Like/dislike on a focused comment resolves the detail continuation first and toggles the correct server action.
- [ ] Image attachments render through the selected graphics protocol; GIFs/WebP animate only while visible and stop driving redraws after leaving Comments.
- [ ] With a freshly authorized TV OAuth account, `n` posts a top-level comment and `r` posts a thread reply; local state changes only after YouTube acknowledges the write. Older tokens should show the one-time `:account add` write-scope re-login message instead of fake success.
- [ ] Normal `q` is unbound; Esc/Shift-H navigate back; `:q` and Ctrl+C quit immediately when idle but confirm while playback/download is active; `yy`, `zz`, `v`, `Space e`, and Alt-e work.
- [ ] Local works with networking disabled and recursively discovers media under `[downloads].directory`.
- [ ] With `MPV seekbar` off, MPV keeps slim OSC transport controls without the draggable seekbar; repeated large seeks must not produce a seek storm. Run `yute doctor --seek-test <id>` with `--seek-engine mpv|proxy|auto --seek-cache 20` on AV1/VP9/H.264 samples. Direct failures must remain bounded, proxy must serve only loopback finite ranges, and Auto may switch at most once per persistent stall while preserving the requested target. Inspect sanitized diagnostics and `~/.cache/yute/mpv.log` on failure.


## Playback network evidence, 2026-10-31

On problem video `g1qNCcxFWRo` with MPV 0.41 and a 20-second cache, forced direct MPV reproduced a first-round 20% seek stall. Forced proxy completed all ten measured seeks; the first healthy sample was about 0.25–1.76 seconds in round one and about 0.25 seconds in round two. After fixing Auto target/phase bookkeeping, Auto completed all ten seeks: the first escalation seek resumed in about 18.1 seconds because it included fresh yt-dlp resolution and proxy startup, while subsequent first-progress times were about 0.25–2.8 seconds. The loopback fake-origin tests also cover range validation, 403 re-resolution, connection-reset retry and obsolete request cancellation. These measurements prove the tested path, not every CDN/codec/network combination.

## Continued delivery checks

- [ ] `[ui].sidebar_at_launch = show|hide|remember` selects the expected startup state. In `remember`, explicit sidebar toggles survive restart; shrinking below 72 columns hides it temporarily and widening restores it without changing the saved preference.
- [ ] `config.toml` writes from sidebar/settings changes replace atomically and do not leave `.config.toml.tmp-*` files.
- [ ] `graphics = "off"` and `:graphics off` hide account/comment images as well as cards.
- [ ] Switching Kitty to halfblocks/off removes previous placements.
- [ ] Focus returns re-query background; exiting restores terminal notification modes.
- [ ] `:download video typo` rejects the value; unavailable explicit single-video
  resolution reports an error instead of silently selecting best.
- [ ] `:download playlist` opens the playlist menu; channel entities cannot start downloads.
- [ ] Owned playlist cards open their returned browse IDs; old mixed library cache
  entries do not appear in Playlists.

Automated PTY checks confirm native config notifications, valid light-theme
reload, malformed-file rejection retaining the previous state, focus query
emission and cleanup. They do not verify physical image placement or font
shaping. The current environment contains Kitty, foot and tmux executables;
Ghostty, WezTerm, Konsole, xterm, Alacritty and zellij were not found on PATH.
No physical terminal/Wayland/X11 cell in the matrix above is marked passed.


Observed TV owned-playlist limitation: the next-page response repeated the same
41 identities with a different token. Verify that this leaves the 41 cards and
selection intact, stops automatic pagination, and shows an explicit retry error.
Do not count replayed cards as new results or a proven additional page.


## Screenshot and scrolling follow-up

Inspected the supplied 1920x1200 recording (14.421 seconds) without audio playback.
It showed a blank intermediate scrolling frame and image/text placement remnants.
An isolated anonymous fixture was then tested in a real Foot terminal on Wayland,
using forced Sixel and Iosevka Nerd Font Mono, without live account writes or MPV.
At 115x31 cells with the sidebar shown, two complete card rows fit. Rapid j/k/h/l
navigation and opening/closing the action menu restored images without text
overprinting in the captured frames. The selected card, thin border progress,
account padding and removal of the generic Content label were also checked.
This is fixture evidence for Foot/Sixel only, not validation of every protocol.

- [ ] Repeat scrolling with real personalized feeds in the user's normal terminal.
- [ ] Check two complete rows at floating-window and fullscreen sizes, with sidebar
  shown/hidden. Fixed-column settings can intentionally limit density.
- [ ] Check the focused card stays aligned with the position counter after resize.
- [ ] Check partially/fully watched cards have a thin bottom-border progress mark,
  with duration badges intact and no full-cell progress block over the image.
- [ ] Repeat action/help/command overlays with Kitty, iTerm2 and halfblocks; verify
  closing them restores images and leaves no text or border remnants.
- [ ] Confirm the account avatar/name has space before the terminal's right edge.
