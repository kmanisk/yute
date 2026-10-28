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

## Downloads (requested extensions remain outstanding)

The existing generic item download/cancel action is available. The specialized
menus, conversions, playlist accounting and archive behaviour below are pending
implementation and must not be described as passing.

- [ ] Single video: original/default and chosen quality.
- [ ] Audio: original streams versus explicitly labelled conversion.
- [ ] Missing ffmpeg hides conversion choices; no unnecessary conversion.
- [ ] Cancel and resume partial download; quit terminates children.
- [ ] Playlist video, <=1080p ceiling and lower-quality per-item fallback.
- [ ] Playlist original audio, Opus/AAC preferences and conversions.
- [ ] One unavailable item and one failed item do not abort remaining items.
- [ ] Final completed/failed/skipped totals and useful sanitized details.
- [ ] Playlist retry/archive skips completed IDs; safe directory ordering.
- [ ] Filenames/URLs containing shell metacharacters remain literal arguments.

## Follow-up delivery evidence, 2026-10-29

The automated suite covers legacy-cookie re-login state, authoritative account
errors, recognized empty feeds, account continuation provenance, avatar variants
and sizing, priority/cancellation, automatic grid bounds, selection retention,
Unicode/grapheme/cursor safety and icon fallbacks. The live anonymous channel
avatar/CDN decode test and three native MPV tests (generated local media with
null audio/video outputs) passed explicitly.

Interactive TV OAuth and personalized Subscriptions/History/Watch Later/Liked/
Playlists/writes still require the user's authorization. No cookie session was
used to substitute for that validation. Kitty/Sixel visual review also remains
human QA; environment hints are not graphics-negotiation proof.
