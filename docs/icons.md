# Semantic terminal icons

`crates/yute/src/ui/icons.rs` is the sole semantic glyph mapping. Sidebar,
action menu and Watch ratings use it. Labels measure terminal display width.
Modes: auto, nerd, unicode, ascii and none. Auto conservatively selects Unicode;
`YUTE_NERD_FONT=1` opts into Nerd. Explicit Nerd mode does not require detection.

On 2026-10-29, fontTools read the cmap of the installed
`~/.local/share/fonts/ttf/IosevkaNerdFont-Regular.ttf`. All mappings below matched
these glyph names. No font binary or new font dependency is bundled.

| Semantic icon | Codepoint | Font glyph name |
| --- | --- | --- |
| Home | U+F02DC | md-home |
| Subscriptions | U+F0D40 | md-youtube_subscription |
| History | U+F02DA | md-history |
| Watch Later | U+F0150 | md-clock_outline |
| Like | U+F0513 | md-thumb_up |
| Dislike | U+F0511 | md-thumb_down |
| Playlists | U+F0411 | md-playlist_play |
| Queue | U+F0412 | md-playlist_plus |
| Local | U+F024B | md-folder |
| Account | U+F0004 | md-account |
| Settings | U+F0493 | md-cog |
| Play | U+F040A | md-play |
| Audio | U+F02CB | md-headphones |
| Download | U+F01DA | md-download |
| Save | U+F0193 | md-content_save |
| Search | U+F0349 | md-magnify |
| Channel | U+F0006 | md-account_box |
| Browser | U+F03CC | md-open_in_new |
| Copy | U+F018F | md-content_copy |

Nerd availability cannot be proven from TERM alone. Use Unicode/ASCII if the
terminal's selected font lacks the glyphs. `none` retains readable text labels.
