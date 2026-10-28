# SmartTube protocol reference and next backend pass

Inspected 2026-10-26. This is static source evidence, not a claim that YouTube
currently accepts every request or synchronizes history successfully.

## Sources and licensing

- Local SmartTube: `/home/onyx/Documents/repo/smarttube`, revision
  `63d5fe43f8e1157cf7b792a23f33c82859f3f3d4`.
- Its `settings.gradle` delegates `youtubeapi` to MediaServiceCore. That submodule
  was uninitialized locally. The exact pinned revision
  `05feeb7e7889ab0243e9d44cb06bc8a7d3f066b6` was fetched from
  <https://github.com/yuliskov/MediaServiceCore> into ignored
  `target/smarttube-reference` for inspection. SmartTube was not modified.
- Both inspected repositories have MIT root licenses, copyright yuliskov.
  yute is MIT. No implementation was copied; dependencies may have their own
  licenses. Preserve attribution if copying substantial implementation later.

Paths below use these roots:

- `ST`: `common/src/main/java/com/liskovsoft/smartyoutubetv2/common/`
  in SmartTube.
- `YT`: `youtubeapi/src/main/java/com/liskovsoft/youtubeapi/`
  in MediaServiceCore.
- `GO`: `youtubeapi/src/main/java/com/liskovsoft/googleapi/`
  in MediaServiceCore.
- yute paths are relative to `crates/yute-core/src/`, except `TUI:` paths,
  relative to `crates/yute/src/`.

## Behavior mapping

| Behavior | SmartTube module/class and responsibility | Request/client behavior in inspected source | yute today | Gap | Recommended Rust implementation |
| --- | --- | --- | --- | --- | --- |
| Device/TV login | `ST app/presenters/YTSignInPresenter.java`; `YT service/internal/YouTubeAccountManager.java`; `YT auth/V2/AuthService.java` | Presenter displays code; manager requests user code then waits for device authorization, retaining refresh token. V2 is the active YouTube path; `GO oauth2/OAuth2Service.java` is a separate Google OAuth implementation. | `backend/auth.rs` device request/poll; `TUI:main.rs` CLI and interactive loops. | Login is a task/status string rather than explicit session state; pending/slow-down/cancellation behavior needs validation. | Explicit pending state with code expiry, server poll interval, cancellation, bounded slow-down, and typed errors. Do not copy OAuth client secrets or diagnostic formatting. |
| Token refresh and reauthentication | `YT service/YouTubeSignInService.java::checkAuth/updateAuthHeadersIfNeeded/createAuthorizationHeader` | Refreshes cached authorization after a one-hour interval, from the selected account's refresh token. | `AuthContext::refresh_access_token` exists but has no call sites in the inspected workspace. `post_endpoint` builds headers without checking expiry. | Stored and active sessions can retain expired authorization. | Async single-flight refresh before expiry; one retry after 401 for read requests; invalid grant becomes reauthentication required. Avoid holding std mutex over network awaits. |
| Account selection/state | `YT service/internal/YouTubeAccountManager.java::selectAccount/onAccountChanged`; `ST misc/MediaServiceManager.java` | Selected account controls authorization; change invalidates sign-in and app caches, regenerates visitor data, notifies consumers. | Auth file has accounts/active account; `TUI:app.rs::switch_account` changes visible profile, persists active_account.json and clears cache, without switching AuthContext credentials. | Visible account selection is disconnected from active credentials; visitor state and cached routes are not switched atomically. | Central session owner with account ID and generation; switch credentials, visitor data, caches and request cancellation together. |
| Logout/removal | `YT service/YouTubeSignInService.java::removeAccount`; `YT service/internal/YouTubeAccountManager.java::removeAccount` | Removes stored account and triggers account-change invalidation when applicable. | No complete TUI logout flow. | Guest and expired state can look like a selected profile. | Explicit logout and account removal; clear in-memory authorization and account-specific continuations, keep credentials encrypted and mode 0600. Define whether remote revocation is available separately. |
| Context/client identity | `YT common/helpers/AppClient.kt`; `YT common/helpers/PostDataHelper` | Client-specific name/version, numeric identity, UA, locale, visitor data, and TV app-quality fields; multiple TV/Web variants. | `AuthContext::build_headers/build_client_context`, configured TV name/version, hl/gl. | One mostly fixed identity; changing only a version can mismatch the rest of context. | Typed client profiles; headers/body generated together. Keep overrides explicit; validate with sanitized fixtures and live read-only probes. |
| Visitor data | `YT app/AppServiceCore.java::getVisitorData/invalidateVisitorData`; `YT common/helpers/AppClient.kt` | App-derived visitor data joins request context; account changes invalidate app cache. | `InnerTubeBackend::post_endpoint` captures response visitorData, saves it in encrypted auth file; context reuses it. | Visitor data is global in AuthContext rather than clearly scoped to session/account. | Session-scoped visitor state, reset or reacquire on account/client change; persist only when useful. |
| Personalized Home | `YT service/YouTubeContentService.java::getHome/getHomeObserve`; `YT browse/v2/BrowseService2.kt::getHome`; `BrowseApiHelper.kt` | Uses TV browse `default`, grouped shelves/chips and root continuation. | `home` uses `FEwhat_to_watch`; guest empty results fall back to trending search; chain can use yt-dlp. | Request/context and renderer differences; generic fallback is not proof of personalization. | Validate TV Home variants against actual responses; preserve shelf continuations and backend provenance; label guest/fallback content truthfully. |
| Subscriptions/live/upcoming | `YT browse/v2/BrowseService2.kt::getSubscriptions`; `BrowseService2Wrapper.kt` | TV subscriptions by default; wrapper can fall back to RSS of known subscribed channel IDs. | `FEsubscriptions`; parser has Live/Upcoming model. | Generic backend fallback cannot establish account subscription parity; filter currently covers Live only. | Authenticated subscription endpoint and continuation provenance; keep upcoming/live distinct; label RSS only as fallback. |
| Account history | `YT browse/v2/BrowseService2.kt::getHistory`; `ST app/presenters/BrowsePresenter.java::appendLocalHistory` | `FEhistory`; SmartTube can merge local state when remote history is broken. | `FEhistory` plus parsed resume/progress; generic backend chain. | No proof server accepted reports; local state must not masquerade as synchronized account history. | Account-bound history with explicit remote/local provenance and integration checks across clients. |
| Playback reporting | `YT service/YouTubeMediaItemService.java::updateHistoryPosition`; `YT track/TrackingService.java`; `TrackingApi.java` | Loads format data, syncs auth-related info, requires event ID, visitor monitoring data and `of`; reports old/current positions, length and playback nonce; completion threshold changes record behavior. | `player/tracking.rs` tracks actual MPV positions and sends periodic pings; `backend/innertube.rs` constructs generic stats URLs, ignores HTTP failures. | Missing server tracking context; success return does not demonstrate synchronization. Seek spans and final reports also need review. | Parse server playbackTracking/format metadata and validate allowed HTTPS endpoints; keep per-playback nonce and actual watched intervals; check HTTP results. No fabricated synchronization. |
| Pauses/seeks/completion | `ST app/models/playback/controllers/VideoStateController.java::saveState/updateHistory`; `YT track/TrackingService.java` | Controller schedules state saves at 10 seconds, guards disabled/remote history and persists position; tracking service handles old position and near-end threshold. | Pings while unpaused with 25-second position or 30-second time threshold; final ping when MPV exits. | A forward seek can be interpreted as watched span; backtracking/stop/switch/final completion need explicit handling. | Consume IPC lifecycle/pause/seek events; separate elapsed watch intervals from resume position; flush when switching/stopping, without high-frequency polling. |
| Resume position | `ST app/models/playback/service/VideoStateService.java`; `VideoStateController.java` | Local video state plus metadata; remote history failure is tracked separately. | `VideoCard::resolved_resume_sec`, parsed percentage/seconds, MPV `loadfile` start option. | Local and remote resume provenance/account scoping not explicit. | Account-keyed local resume fallback with timestamps; honor remote resume when verified; avoid restarting almost-complete items midway. |
| Watch Later/Liked/playlists | `YT browse/v2/BrowseApiHelper.kt`; `BrowseService2.kt::getMyPlaylists`; `BrowseService2Wrapper.kt` | Special IDs include `VLWL`, `VLLL`; library/aggregation routes and returned reload keys; local pinned-playlist overlay in wrapper. | `watch_later` uses VLWL, Liked calls playlist LL, playlists uses FElibrary. | A fixed library ID does not cover all client variants; returned endpoints can carry params/reload keys. | Retain server endpoint params, correct client and playlist identity; test authenticated library layouts before adding mutations. |
| Continuations/pagination | `YT service/YouTubeContentService.java::continueGroupChecked/continueGroupInt`; `YT browse/v2/BrowseService2.kt::continueGroup` | Dispatch by group kind to search, watch-next, TV or Web; reject repeated items AND token; bounded attempts through empty groups. | FeedPage has one opaque string; `continue_feed` posts browse; search has its own continuation argument. This UI pass wires append requests, preserves selection and rejects stale/repeated responses. | Mixed shelves, next endpoints and client provenance cannot fit a bare token. | Typed continuation containing endpoint/client/session/generation and shelf scope; bounded empty-page traversal; cursor retry without clearing cards. |
| Channel browsing | `YT browse/v2/BrowseService2.kt::getChannel/getChannelAsGrid`; `BrowseApiHelper.kt` | Canonical IDs, tab-specific params, sorting and reload continuations; combines video/live where appropriate. | `channel(id, ChannelTab)` selects fixed params. | Missing returned endpoint/params and full tab state. | Model channel route and server-provided tab endpoints; refresh that route directly; test live/upcoming layouts. |
| Rejected/changing clients | `YT common/helpers/AppClient.kt`; `YT app/AppServiceCore.java::refreshCacheIfNeeded`; `YT app/playerdata/WebPlayerDataExtractor.kt` | Multiple client variants and refreshed player/app extraction; comments document incompatible player variants and Web 403 cases. | Static defaults and yt-dlp resolution fallback. | No structured client rejection policy or extraction refresh. | Classify auth/client/playability failures separately; bounded intentional fallback, not arbitrary client rotation. Surface source and reauth requirements. |
| Retries/backoff | `YT auth/V2/AuthService.java::getAccessTokenWait`; `YT service/YouTubeContentService.java::continueGroupInt` | Bounded device polling with fixed interval; up to three empty-group continuations. | Request timeout, fall-through backend chain; device polling with fixed interval. | No general exponential retry policy was established in inspected source. Empty-page traversal is not HTTP backoff. | Respect Retry-After, bounded jitter for transient read failures; no silent retry of mutations. Keep unsupported behavior unknown. |

## Backend implementation sequence

1. Add typed session states: guest, device-code pending, authenticated, refreshing,
   expired/reauthentication required. Centralize identity and active credentials;
   preserve encrypted storage and restrictive permissions. Existing encryption
   derives its key from machine/user identifiers; this is not an OS keyring and
   should receive a separate security review, not a cosmetic rewrite here.
2. Add account/session generations across backend/cache work, atomic switching,
   logout, proactive refresh and one bounded unauthorized retry. Retain underlying
   typed diagnostics; redact URLs, bodies, headers, cookies and tokens by default.
3. Add request/client provenance and endpoint-aware continuations, parser fixtures
   for actual TV Home, subscriptions, library, history and channel responses.
   Distinguish guest search/RSS/resolver fallbacks from authenticated feeds.
4. Obtain server tracking fields before enabling claims of synchronized history.
   Use actual MPV events/positions, including pause/seek/end and playback replacement.
   Verify resume/history on a second YouTube client with an explicit test account.
5. Only then add account-sensitive mutations/retries. Confirm live behavior rather
   than treating the inspected unofficial protocol as stable public API.

## Scope of the UI pass

No authentication, recommendation endpoint, watchtime or encryption rewrite.
External MPV remains isolated in core with Unix JSON IPC. Existing uncommitted IPC
lifecycle changes and tests were preserved; inline playback code and its mode API
were removed. Renderer, navigation and footer share actual grid columns. Full feed
and continuation responses carry UI generations. Refresh preserves existing cards
and selected identity; continuations append and deduplicate. Playback survives
leaving Watch, with navigation state restored. The optional existing GTK crate was
not redesigned or extended.

Deferred: typed backend continuations, nested shelf pagination, authenticated live
parity verification, correct server-derived playback reporting, account/logout
state machine, typed error details, off-thread protocol encoding and cancellation
of thumbnail requests already in flight. Visible cards still drive thumbnail work;
resizing reuses decoded image/protocol caches rather than refetching the feed.

## Routing/command follow-up, 2026-10-26

The next UI pass adds typed entity activation, actual channel routes/headers/tabs,
and command mode; see [the review](routing-command-review.md). Channel browse
parameter corrections were checked against pinned MediaServiceCore
`browse/v2/BrowseApiHelper.kt`; TV surface/header/tab structure was checked against
`browse/v2/gen/BrowseHelper.kt` and live read-only responses. The observed TV
channel returned 86 items and a `channelHeaderRenderer`, with no advertised tabs.

Local logout/session callback guards and truthful unavailable account switching are
now exposed. Token refresh, multi-account credentials and server-derived history
reporting remain deferred. The old manual watched-time UI action no longer claims
synchronization. Existing authenticated mutation methods are used by commands; no
Android/Java implementation or dependency was copied.

## Home and channel image corrections

A live TV `FEwhat_to_watch` request returned a feed nudge, then the old yute code
substituted an unfiltered `trending` search. A probe returned 115 Shorts and two
ordinary videos, explaining the one-card Home when Shorts are hidden.
SmartTube's `browse/v2/BrowseApiHelper.kt` uses `HOME_TV` browse ID `default`;
`BrowseService2.getHome()` sends that through the TV browse path. yute now uses
`default` for TV and lets the existing backend chain try actual yt-dlp `:ytrec`
recommendations when TV returns no feed. It does not relabel a search as Home.
This session's TV response still requires login; TV OAuth refresh is unchanged.

SmartTube's `common/models/gen/CommonHelper.kt`, `Thumbnail.getUrl()`, adds
`https:` to protocol-relative CDN URLs before returning them. A live yute channel
result had precisely this URL form. The equivalent small Rust normalization now
runs in thumbnail extraction and the loader, including cached older card URLs.
No Android/Java implementation was copied; the reference license remains MIT.

## Dynamic Mix collections

`next/v2/WatchNextApiHelper.kt` constructs Mix watch-next queries using playlistId,
playlistIndex and optionally videoId; it notes that videoId can be omitted when
loading by index. `gen/WatchNextHelper.kt` distinguishes playlist metadata from
suggestion shelves. In the live TV response, playlist metadata had no item list;
20 pivot tiles carried watch endpoints with the requested playlistId, while other
shelves were unrelated. yute now resolves RD collections through next and filters
those endpoint identities before exposing a playable Mix batch. Canonical and
opaque Mix IDs both returned 20 matching videos in read-only probes. This is a
small protocol fix, without changes to authentication or account state.

## Backend pass, 2026-10-28 (partial)

Current static reference: SmartTube revision
`4b3f333e540a1403429d1a9d86dec758e59288f9`, root LICENSE MIT.
The checked-out `MediaServiceCore/` submodule is empty and the sibling directory
is absent. The historical reference directory mentioned above was also absent.
For this pass, its currently pinned revision
`40fec843eb24bc264e9c8b429fcee25068e230e2` was fetched into ignored
`target/smarttube-reference`; its root LICENSE is MIT too. Neither source tree was
modified. No source, comments, strings or implementation structure were copied.

Inspected paths:

- SmartTube `settings.gradle`, `.gitmodules`, `LICENSE`.
- `ST app/presenters/YTSignInPresenter.java`: displays activation data and
  disposes the sign-in subscription when the view is destroyed.
- `YT browse/v2/BrowseApiHelper.kt`: TV Home uses `default`; subscriptions use
  `FEsubscriptions`; history uses `FEhistory`; special list IDs include `VLWL`
  and `VLLL`. Watch Later also has a client-specific `FEmy_youtube` route.
- `YT browse/v2/BrowseService2.kt`: client-aware browse dispatch.
- `YT auth/V2/AuthService.java` was located; no new OAuth protocol claims are
  made from that file in this pass.

### Implemented in this pass

- `yute doctor`: bounded network/tool/session checks; existing encrypted store
  authentication without migration, token refresh or account writes. Token and
  visitor values, headers, response bodies and account identifiers are omitted.
  A successful HTTP JSON response is request acceptance, not proof of personalized
  feed correctness. Terminal graphics are explicitly environment hints.
- yt-dlp feed subprocess deadline and kill-on-drop; raw resolver stderr is no
  longer propagated. Removed implicit Brave browser-cookie discovery. Explicit
  cookie configuration still exists and needs a separate removal pass.
- Shorts reel item/sequence errors propagate rather than becoming fake empty
  pages. This repairs an observed error-swallowing path; no live claim about the
  reported Shorts hang is made without a sanitized reproducing response.
- Typed yt-dlp JSON formats, current-video five-minute cache, cancellation,
  available video/audio overlays, exact selectors and automatic audio pairing.
  Manual selections affect one playback only. Resolution ceilings no longer have
  an unbounded `/best` fallback. Unknown heights are excluded under a ceiling.
- New encrypted stores require an OS-keyring master key. Legacy keyfile and
  passphrase stores remain readable with their original key; they are not yet
  migrated to keyring-only storage.

Format field units and selectors are independently based on yt-dlp's public
structured output contract:
<https://github.com/yt-dlp/yt-dlp#format-selection>.
No SmartTube media selection implementation was translated.

### Remaining requested work

This is not the completed 0–27 feature set. Existing TV OAuth/refresh/account
switching has not been replaced or live validated. Cookie-mode removal, a single
client-profile module, full OAuth mock lifecycle tests, all feed/write/history
parity, semantic glyphs, layout changes, expanded settings/integrations, download
and playlist quality menus/progress/counts/archive/conversions remain outstanding.
GTK still contains its pre-existing embedded player; only compilation was checked.
Format probing currently runs anonymously; it does not export cookies/tokens to
resolve private or age-restricted videos. New format menus do not remember codec
preferences. The quality ceiling helper supports codec preferences in core, but
those preferences have not yet been added to settings.


## OAuth/UI delivery, 2026-10-29

The follow-up retires selectable cookie sessions, cookie request headers and
cookie-bearing resolver/player/download options in both frontends. Legacy cookie
files remain untouched at startup; cookie-only stored identities require TV
re-login. OAuth profiles use TV context even when inherited from older Web config.
Missing expiry now triggers refresh instead of assuming validity indefinitely.
Request deadlines cover refresh/keyring waits as well as HTTP transport.

Account libraries, account continuations and mutations retain authoritative
InnerTube errors. Known successful empty renderers are accepted; unknown or
logged-out responses become explicit errors. Both presentation modes share
persistent failure state and sign-in/re-login messaging. Writes require OAuth
and backend acknowledgement; no fallback simulates success.

Channel avatar extraction handles classic/header/view-model shapes. Visible
synthetic channels hydrate through bounded cancellable public metadata requests;
avatars use the shared priority cache. A real anonymous public-channel/CDN decode
check and generated-media/null-output MPV tests were run successfully. These are
not personalized account validation. Subscriptions/history/list IDs remain based
on the inspected protocol reference; real TV OAuth/account responses still need
interactive authorization to validate for the user's identity.

Grid Auto uses 1–8 content-width columns; settings supports each fixed choice.
Semantic icon mappings were verified against the installed Iosevka Nerd cmap;
Unicode and ASCII remain portable fallbacks. See [icons](icons.md).
