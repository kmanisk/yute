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
- [ ] Esc/q during resolution cancels; late results cannot reopen a closed menu.
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
- [ ] `q` navigates back; Home `q` shows yes/no quit confirmation; `yy`, `zz`, `v`, `Space e`, and Alt-e work.
- [ ] Local works with networking disabled and recursively discovers media under `[downloads].directory`.
- [ ] With `MPV seekbar` off, MPV keeps slim OSC transport controls without the draggable seekbar; repeated large seeks must not produce a seek storm. With the full seekbar explicitly enabled, verify the one-shot stream-reload fallback recovers a stalled DASH seek. Test AV1/VP9/H.264 and inspect `~/.cache/yute/mpv.log` on failure.


## Continued delivery checks

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
