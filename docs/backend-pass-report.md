Historical backend checkpoint; later work is integrated on main. Branch/push
instructions and incomplete-feature statements below describe that earlier pass.
See [delivery-validation.md](delivery-validation.md) for the current delivery.

# Historical backend pass: 2026-10-28

This report records the earlier partial pass. The OAuth/UI delivery on 2026-10-29
supersedes its cookie, avatar, icon and grid limitations; see
[the current architecture](architecture.md) and [QA](qa.md).

The complete requested 0–27 feature set is not finished. This report describes
only the implemented and checked subset. No push, PR, tag or SmartTube mutation.

1. Branch: `feat/smarttube-backend`.
2. Local implementation commits:
   - `57dabee`: bounded redacted core diagnostics and yt-dlp deadline.
   - `cfc9128`: require keyring keys for new encrypted stores.
   - `10094b6`: propagate Shorts sequence/item failures.
   - `0290916`: typed formats, per-playback selection, overlays and tests.
   - A final documentation commit records this report and QA.
3. Changed files are listed below. The pre-existing untracked `AGENTS.md` is
   preserved and excluded from commits.
4. SmartTube paths/revisions inspected are recorded in
   [protocol notes](smarttube-parity.md#backend-pass-2026-10-28-partial).
   MediaServiceCore was empty locally; its exact pinned revision was fetched
   into ignored `target/smarttube-reference` for read-only inspection.
5. Both reference root licenses inspected are MIT. Protocol documentation only;
   no source/comments/strings or nontrivial implementation structure copied.
6. Confirmed TV Home/subscription/history/list routing identifiers and the
   sign-in view lifecycle. This pass does not claim new live protocol parity.
7. TV OAuth already existed: cancellable device flow, pending/slow-down/expiry/
   denial, refresh and one 401 retry. It was audited, not reimplemented. New
   stores now require a keyring; full lifecycle mock coverage remains pending.
8. Existing account cancellation/generation/cache isolation is retained. No live
   primary/brand-account acceptance or account writes were performed.
9. Shorts swallowed-error paths fixed; no claim that every library feed is fixed.
10. New semantic glyph system: not implemented.
11. New responsive layout: not implemented; existing layout retained.
12. Existing grapheme-aware text handling retained; no new Unicode pass.
13. Available-format resolution uses typed structured yt-dlp JSON and a bounded,
    cancellable asynchronous subprocess. Single current-video cache, five-minute
    TTL, serialized probes. Signed URLs are not retained in the model/cache.
14. Video menu contains actual playable video/muxed formats. Explicit choices
    override defaults for one playback; existing default quality ceiling remains.
15. `:audio` and Audio only action show original audio formats with codec/rate/
    language metadata. Exact stream selection; next normal playback resets it.
16. Single-video downloads: existing generic downloader retained. Specialized
    menus/progress/options requested in the task are not implemented.
17. Playlist downloads: existing generic action retained. Ordered directories,
    quality/audio menus, per-item accounting and archives are not implemented.
18. Core selection helper chooses height within requested/configured ceilings,
    then codec preference, frame rate and bitrate. Missing preferred codec falls
    back to an available codec. Settings do not yet expose codec preferences.
    Bounded automatic yt-dlp selectors no longer end with unrestricted `/best`.
    Exact video-only selections pair with the best available original audio.
19. Audio conversion: not added. No MP3/FLAC format is falsely represented as an
    original stream. ffmpeg merging remains yt-dlp's existing responsibility.
20. Format resolution is cancelled on overlay close. Download cancellation uses
    the existing abort/kill-on-drop implementation; playlist resume/archive and
    completed/failed/skipped summaries remain pending.
21. Validation commands: see results below.
22. Exact aggregate results: see results below; ignored tests are not passes.
23. Unverified: real TV login, refresh/revocation, live feeds/writes/history,
    real media stream compatibility, terminal graphics, encrypted OS-keyring
    provisioning on the user's session, all requested download workflows.
24. Limitations: cookie mode and explicit cookie resolver options remain;
    client configuration is not centralized yet; GTK still embeds its existing
    player; format probing is anonymous; legacy encrypted keyfile/passphrase
    stores retain their original key source; full settings/integrations/glyph/
    layout/download work remains outstanding. Doctor HTTP acceptance is not
    proof of identity or personalized feed correctness.
25. Manual checklist: [docs/qa.md](qa.md), with all live checks pending.
26. Review/push commands, for the user to run only after reviewing the partial scope:

```bash
cd /home/onyx/Projects/ytui
git status --short
git log --oneline main..feat/smarttube-backend
git diff --stat main...feat/smarttube-backend
git diff main...feat/smarttube-backend
# After review, if you want to publish this partial branch:
git push -u origin feat/smarttube-backend
```

## Changed files

- `HANDOFF.md`
- `README.md`
- `crates/yute-core/src/backend/innertube.rs`
- `crates/yute-core/src/backend/ytdlp.rs`
- `crates/yute-core/src/config.rs`
- `crates/yute-core/src/credentials.rs`
- `crates/yute-core/src/doctor.rs`
- `crates/yute-core/src/http.rs`
- `crates/yute-core/src/lib.rs`
- `crates/yute-core/src/media/formats.rs`
- `crates/yute-core/src/media/mod.rs`
- `crates/yute-core/src/player/mod.rs`
- `crates/yute-core/tests/fixtures/formats/available.json`
- `crates/yute-core/tests/format_tests.rs`
- `crates/yute/src/actions.rs`
- `crates/yute/src/app.rs`
- `crates/yute/src/commands.rs`
- `crates/yute/src/events.rs`
- `crates/yute/src/main.rs`
- `crates/yute/src/ui/help.rs`
- `crates/yute/tests/media_menu_tests.rs`
- `crates/yute/tests/routing_tests.rs`
- `docs/README.md`
- `docs/architecture.md`
- `docs/backend-pass-report.md`
- `docs/commands.md`
- `docs/configuration.md`
- `docs/credentials.md`
- `docs/qa.md`
- `docs/smarttube-parity.md`

## Validation results

- `cargo fmt --check`: exit 0.
- `cargo check --workspace`: exit 0 during implementation, including GTK.
- `cargo clippy --workspace --all-targets --all-features`: exit 0, warnings
  remain (core, TUI, GTK and existing tests); this is not a warning-free claim.
- `cargo test --workspace`: exit 0; 146 passed, 0 failed, 3 ignored. The three
  ignored tests require explicit native MPV execution with generated local media.
- `cargo build --release`: exit 0, including the default GTK workspace member.
- `cargo build -p yute-gui`: exit 0.
- `git diff --check`: exit 0.
- Isolated anonymous `target/release/yute doctor`: exit 0. Network HTTP 204,
  YouTube HTTP 200, anonymous InnerTube JSON accepted; yt-dlp, MPV, ffmpeg and
  ffprobe version probes succeeded. Expected warnings: no credential store,
  anonymous session and unavailable terminal/cell dimensions under non-TTY
  execution. Graphics reported Halfblocks as a hint, not negotiated capability.
- Media TestBackend coverage: 80 overlay renders (five dimensions, four icon
  config values, four media stages), plus actual format rows and close/cancel.
  This does not establish Nerd Font availability or all-screen UI parity.

Build and test logs are ignored artifacts under `target/smarttube-*.log`.
No live account/login, mutation, media download or actual playback QA performed.
