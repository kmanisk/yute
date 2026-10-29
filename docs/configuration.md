# Configuration notes

Configuration is read from `$XDG_CONFIG_HOME/yute/config.toml` (default
`~/.config/yute/config.toml`); legacy `~/.config/ytui/config.toml` is a fallback.
Cache uses `$XDG_CACHE_HOME/yute`; encrypted data uses `$XDG_DATA_HOME/yute`.

```toml
[general]
icons = "auto" # auto, nerd, unicode, ascii, none

[grid]
columns = 0 # Auto; or a fixed value from 1 through 8
min_card_width = 30

[playback]
max_resolution = "1080p"
```

Supported saved limits are 480p, 720p, 1080p, 1440p, 2160p and best. Automatic
bounded selectors exclude unknown heights and have no unrestricted fallback.
The media overlay permits a manual format override without changing saved config.

Playback preferences include `default_quality`, `max_resolution`,
`preferred_codec = "auto|av1|vp9|h264"`, and
`preferred_audio_codec = "auto|opus|aac"`. Automatic resolution respects the lower
of the default and maximum quality, then the preferred codec with a compatible
fallback. `audio_language` and `subtitle_language` accept MPV language lists or
`"auto"`. Changes to an already running MPV process require restarting playback.
`:play-select` and `:audio` select actual streams for one playback; an explicit
format selection does not permanently enable audio-only mode.

```toml
[downloads]
directory = "~/Videos/Yute"
video_quality = "1080p"
audio_mode = "best" # best, opus, aac, mp3, flac, m4a
playlist_subdirectories = true
write_metadata = false
write_thumbnail = false
write_subtitles = false
keep_partial = true
```

Downloads run in one owned asynchronous yt-dlp process. Playlist video quality
is a ceiling per item, with best available audio; it does not demand the same
resolution from every video. Playlist subdirectories use a sanitized title and
ordered filenames. A per-job local video-ID archive permits retrying without
redownloading completed items. Cancelling terminates the owned process; partial
files remain for yt-dlp resume when enabled. Archive skips and unavailable-item
categories are not separately counted in the current summary.

Original audio choices do not transcode. MP3, FLAC and M4A are explicitly labelled
conversions and require ffmpeg; ffmpeg may also be needed to merge separate
video/audio streams. Extraction is anonymous and does not receive OAuth tokens
or legacy cookies, so private or restricted downloads can fail explicitly.

`:download` opens the selection menu. Examples:

```text
:download video 1080p
:download audio best
:download playlist video 720p
:download playlist audio opus
```

An explicitly requested single-video resolution is checked against available
formats. Invalid quality text is rejected instead of silently selecting best.


Auto uses portable Unicode unless `YUTE_NERD_FONT=1` opts into Nerd Fonts. Set
`icons = "nerd"` for a terminal configured with Iosevka Nerd Font. `[ui].icons`,
when present, overrides `[general].icons`. No font query blocks the render loop.
Grid Auto uses the pane width after sidebar allocation; resizing does not refetch.
Existing configs keep other settings. The local installation was backed up before
selecting Auto columns and Nerd icons; the legacy ytui config was left untouched.
`yute doctor` checks TOML/type validity and reports malformed configs without values.


## Themes and live reload

```toml
[general]
theme = "terminal"
theme_dark = "gruvbox-dark"
theme_light = "gruvbox-light"
graphics = "auto" # auto, kitty, iterm2, sixel, halfblocks, off

[ui]
action_bar = "auto" # auto or off; shown on terminals at least 30 rows tall
sidebar_channels = false # subscribed names in the tall sidebar

[text]
emoji = "auto" # auto, keep, strip
```

The terminal theme uses Reset, ANSI colours and modifiers. Custom themes reside
in `~/.config/yute/themes/<name>.toml`; packaged examples are in
`crates/yute/themes/`. For example:

```toml
[roles.text]
fg = "#ebdbb2"
bg = "#282828"

[roles.selection]
modifiers = ["bold", "reversed"]
```

Available roles: `text`, `text_dim`, `border`, `border_focused`, `accent`,
`selection`, `chip_active`, `chip_idle`, `badge`, `badge_live`, `badge_mix`,
`progress`, `skeleton`, `error`, `warning`, `scrollbar`, `popup`,
`popup_selected`, `status`, `action_hint`. Omitted roles inherit the terminal
styles. Colours accept `reset`, ANSI names, quoted indexes 0–255, or `#rrggbb`.
Modifiers accept `bold`, `dim`, `reversed`, and `underlined`.

`:theme terminal|auto|<name>`, `:theme list`, `:theme reload`, and `:theme check`
apply or inspect themes. Native filesystem notifications debounce config/theme
changes for 150 ms; parsing runs off the render thread and a failed reload keeps
the previous configuration and theme. Backend or active-player changes need a
restart, as indicated in the status message. `NO_COLOR` removes decorative
colours; unsupported RGB/indexed colours downgrade to the advertised palette.
Contrast warnings cover known explicit colour pairs; Reset and terminal-defined
ANSI palette entries cannot be measured without their actual RGB values. Identical
explicit foreground/background entries are still detected as zero contrast.

Auto light/dark selection uses terminal replies after first paint, with
`COLORFGBG` as a fallback. Focus events trigger re-querying; there is no idle
polling or desktop dependency. Without a reply or fallback it chooses the dark
configured theme. The TV-code QR remains scanner-oriented black on white.

## Graphics

`:graphics auto|kitty|iterm2|sixel|halfblocks|off` applies live.
`YUTE_GRAPHICS` overrides selection for one process. Auto starts with halfblocks
and requires a confirming terminal response before emitting an image protocol.
Explicitly selecting a protocol assumes the user knows it is supported. Under
tmux or zellij, Yute conservatively uses halfblocks; passthrough negotiation is
not implemented. `off` disables images. `yute doctor` reports capabilities and
configuration without exposing account secrets.
