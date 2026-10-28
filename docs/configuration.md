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

The requested `[downloads]` configuration and codec/language settings are not
implemented yet. The existing generic downloader writes to `~/Videos/Yute` and
uses the playback quality selector. It does not expose original-audio or
conversion menus, ordered playlist subdirectories, progress totals or archives.
ffmpeg is needed when yt-dlp must merge separate streams; future conversion
choices must explicitly state they convert rather than imply original formats.


Auto uses portable Unicode unless `YUTE_NERD_FONT=1` opts into Nerd Fonts. Set
`icons = "nerd"` for a terminal configured with Iosevka Nerd Font. `[ui].icons`,
when present, overrides `[general].icons`. No font query blocks the render loop.
Grid Auto uses the pane width after sidebar allocation; resizing does not refetch.
Existing configs keep other settings. The local installation was backed up before
selecting Auto columns and Nerd icons; the legacy ytui config was left untouched.
`yute doctor` checks TOML/type validity and reports malformed configs without values.
