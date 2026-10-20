use crate::backend::models::{
    Card, ChannelCard, Chip, CommentItem, CommentsBatch, FeedPage, LiveState, PlaylistCard,
    VideoCard, WatchData,
};
use serde_json::Value;

pub fn extract_text(val: &Value) -> String {
    if let Some(s) = val.as_str() {
        return s.trim().to_string();
    }
    if let Some(s) = val.get("simpleText").and_then(|v| v.as_str()) {
        return s.trim().to_string();
    }
    if let Some(content) = val.get("content").and_then(|v| v.as_str()) {
        return content.trim().to_string();
    }
    if let Some(runs) = val.get("runs").and_then(|v| v.as_array()) {
        let texts: Vec<&str> = runs
            .iter()
            .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
            .collect();
        let res = texts.join("").trim().to_string();
        if !res.is_empty() {
            return res;
        }
    }
    if let Some(text_obj) = val.get("text") {
        let s = extract_text(text_obj);
        if !s.is_empty() {
            return s;
        }
    }
    if let Some(parts) = val.get("metadataParts").and_then(|p| p.as_array()) {
        let texts: Vec<String> = parts.iter().map(extract_text).filter(|s| !s.is_empty()).collect();
        let res = texts.join(" • ").trim().to_string();
        if !res.is_empty() {
            return res;
        }
    }
    String::new()
}

pub fn extract_best_thumbnail(val: &Value) -> String {
    if let Some(thumbs) = val.get("thumbnails").and_then(|v| v.as_array()) {
        if let Some(last) = thumbs.last() {
            if let Some(url) = last.get("url").and_then(|u| u.as_str()) {
                return url.to_string();
            }
        }
    }
    if let Some(sources) = val.get("sources").and_then(|v| v.as_array()) {
        if let Some(last) = sources.last() {
            if let Some(url) = last.get("url").and_then(|u| u.as_str()) {
                return url.to_string();
            }
        }
    }
    if let Some(image) = val.get("image") {
        let u = extract_best_thumbnail(image);
        if !u.is_empty() {
            return u;
        }
    }
    if let Some(primary) = val.pointer("/contentImage/collectionThumbnailViewModel/primaryThumbnail/thumbnailViewModel/image") {
        let u = extract_best_thumbnail(primary);
        if !u.is_empty() {
            return u;
        }
    }
    if let Some(thumb_vm) = val.pointer("/contentImage/thumbnailViewModel/image") {
        let u = extract_best_thumbnail(thumb_vm);
        if !u.is_empty() {
            return u;
        }
    }
    String::new()
}

pub fn parse_video_renderer(v: &Value) -> Option<VideoCard> {
    let id = v.get("videoId").and_then(|s| s.as_str())?.to_string();
    let title = extract_text(v.get("title")?);
    if title.is_empty() {
        return None;
    }

    let mut channel = String::new();
    let mut channel_id = None;
    if let Some(byline) = v.get("ownerText").or_else(|| v.get("shortBylineText")).or_else(|| v.get("longBylineText")) {
        channel = extract_text(byline);
        if let Some(runs) = byline.get("runs").and_then(|r| r.as_array()) {
            if let Some(first) = runs.first() {
                channel_id = first
                    .get("navigationEndpoint")
                    .and_then(|ep| ep.get("browseEndpoint"))
                    .and_then(|be| be.get("browseId"))
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string());
            }
        }
    }

    let mut verified = false;
    if let Some(badges) = v.get("ownerBadges").and_then(|b| b.as_array()) {
        for b in badges {
            let style = b.get("metadataBadgeRenderer")
                .and_then(|r| r.get("style"))
                .and_then(|s| s.as_str())
                .unwrap_or("");
            if style.contains("VERIFIED") {
                verified = true;
                break;
            }
        }
    }

    let duration = v.get("lengthText").map(extract_text).filter(|s| !s.is_empty());
    let views = v.get("shortViewCountText")
        .or_else(|| v.get("viewCountText"))
        .map(extract_text)
        .filter(|s| !s.is_empty());
    let published = v.get("publishedTimeText").map(extract_text).filter(|s| !s.is_empty());

    let mut thumb_url = v.get("thumbnail").map(extract_best_thumbnail).unwrap_or_default();
    if thumb_url.is_empty() {
        thumb_url = format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg");
    }

    let mut progress_pct = None;
    if let Some(overlays) = v.get("thumbnailOverlays").and_then(|o| o.as_array()) {
        for ov in overlays {
            if let Some(resume) = ov.get("thumbnailOverlayResumePlaybackRenderer") {
                if let Some(pct) = resume.get("percentDurationWatched").and_then(|p| p.as_u64()) {
                    progress_pct = Some(pct.min(100) as u8);
                }
            }
        }
    }

    let mut live = LiveState::None;
    let mut badges = Vec::new();
    if let Some(b_list) = v.get("badges").and_then(|b| b.as_array()) {
        for b in b_list {
            let label = b.get("metadataBadgeRenderer")
                .and_then(|r| r.get("label").or_else(|| r.get("style")))
                .map(extract_text)
                .unwrap_or_default();
            if label.to_uppercase().contains("LIVE") {
                live = LiveState::Live;
            } else if label.to_uppercase().contains("UPCOMING") {
                live = LiveState::Upcoming;
            } else if !label.is_empty() {
                badges.push(label);
            }
        }
    }

    Some(VideoCard {
        id,
        title,
        channel,
        channel_id,
        verified,
        duration,
        views,
        published,
        thumb_url,
        progress_pct,
        live,
        badges,
    })
}

pub fn parse_lockup_view_model(v: &Value) -> Option<Card> {
    let id = v.get("contentId").and_then(|s| s.as_str())?.to_string();
    let meta = v.get("metadata")?.get("lockupMetadataViewModel")?;
    let title = meta.get("title").map(extract_text)?;
    if title.is_empty() {
        return None;
    }

    let mut channel = String::new();
    let mut stats = Vec::new();

    let rows_opt = meta
        .get("metadataRows")
        .or_else(|| meta.pointer("/metadata/contentMetadataViewModel/metadataRows"))
        .and_then(|r| r.as_array());

    if let Some(rows) = rows_opt {
        if let Some(first_row) = rows.first() {
            channel = extract_text(first_row);
        }
        if rows.len() > 1 {
            for row in &rows[1..] {
                if let Some(parts) = row.get("metadataParts").and_then(|p| p.as_array()) {
                    for part in parts {
                        let t = extract_text(part);
                        if !t.is_empty() {
                            stats.push(t);
                        }
                    }
                } else {
                    let text = extract_text(row);
                    if !text.is_empty() {
                        stats.push(text);
                    }
                }
            }
        }
    }

    if channel.is_empty() {
        if let Some(a11y) = meta
            .pointer("/image/decoratedAvatarViewModel/a11yLabel")
            .and_then(|s| s.as_str())
        {
            if let Some(stripped) = a11y.strip_prefix("Go to channel ") {
                channel = stripped.to_string();
            }
        }
    }

    let duration = v
        .pointer("/contentImage/thumbnailViewModel/overlays/0/thumbnailBottomOverlayViewModel/badges/0/thumbnailBadgeViewModel/text")
        .or_else(|| v.pointer("/contentImage/thumbnailViewModel/overlays/0/thumbnailOverlayTimeStatusRenderer/text"))
        .map(extract_text)
        .filter(|s| !s.is_empty());

    let mut thumb_url = extract_best_thumbnail(v);
    if thumb_url.is_empty() {
        thumb_url = format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg");
    }

    let content_type = v.get("contentType").and_then(|s| s.as_str()).unwrap_or("");
    let is_mix = id.starts_with("RD") || title.to_lowercase().contains("mix") || content_type.contains("MIX");
    if content_type.contains("PLAYLIST") || id.starts_with("PL") || id.starts_with("RD") || is_mix {
        let channel_name = if is_mix && (channel.is_empty() || channel.eq_ignore_ascii_case("playlist")) {
            "YouTube".to_string()
        } else if channel.is_empty() {
            "YouTube".to_string()
        } else {
            channel
        };
        let video_count = if is_mix {
            Some("Mix".to_string())
        } else {
            stats.first().cloned()
        };
        Some(Card::Playlist(PlaylistCard {
            id,
            title,
            channel: channel_name,
            video_count,
            thumb_url,
            is_mix,
        }))
    } else {
        Some(Card::Video(VideoCard {
            id,
            title,
            channel,
            channel_id: None,
            verified: false,
            duration,
            views: stats.first().cloned(),
            published: if stats.len() > 1 { Some(stats[1].clone()) } else { None },
            thumb_url,
            progress_pct: None,
            live: LiveState::None,
            badges: Vec::new(),
        }))
    }
}

pub fn parse_shorts_lockup_view_model(v: &Value) -> Option<Card> {
    let id = v
        .pointer("/onTap/innertubeCommand/reelWatchEndpoint/videoId")
        .and_then(|s| s.as_str())
        .or_else(|| {
            v.get("entityId")
                .and_then(|s| s.as_str())
                .map(|e| e.trim_start_matches("shorts-shelf-item-"))
        })?
        .to_string();

    let title = v
        .pointer("/overlayMetadata/primaryText/content")
        .map(extract_text)
        .or_else(|| {
            v.get("accessibilityText")
                .and_then(|s| s.as_str())
                .map(|t| t.split(',').next().unwrap_or(t).to_string())
        })
        .unwrap_or_else(|| "Short".to_string());

    let views = v
        .pointer("/overlayMetadata/secondaryText/content")
        .map(extract_text)
        .filter(|s| !s.is_empty());

    let mut thumb_url = v.get("thumbnail").map(extract_best_thumbnail).unwrap_or_default();
    if thumb_url.is_empty() {
        thumb_url = format!("https://i.ytimg.com/vi/{id}/frame0.jpg");
    }

    Some(Card::Video(VideoCard {
        id,
        title,
        channel: String::new(),
        channel_id: None,
        verified: false,
        duration: None,
        views,
        published: None,
        thumb_url,
        progress_pct: None,
        live: LiveState::None,
        badges: vec!["SHORTS".to_string()],
    }))
}

pub fn parse_reel_item_renderer(v: &Value) -> Option<Card> {
    let id = v
        .get("videoId")
        .and_then(|s| s.as_str())
        .or_else(|| v.pointer("/navigationEndpoint/reelWatchEndpoint/videoId").and_then(|s| s.as_str()))?
        .to_string();

    let title = v.get("headline").map(extract_text).unwrap_or_else(|| "Short".to_string());
    let views = v.get("viewCountText").map(extract_text).filter(|s| !s.is_empty());
    let channel = v
        .get("shortBylineText")
        .or_else(|| v.get("longBylineText"))
        .map(extract_text)
        .filter(|s| s != "YouTube Shorts")
        .unwrap_or_default();

    let mut thumb_url = v.get("thumbnail").map(extract_best_thumbnail).unwrap_or_default();
    if thumb_url.is_empty() {
        thumb_url = format!("https://i.ytimg.com/vi/{id}/frame0.jpg");
    }

    Some(Card::Video(VideoCard {
        id,
        title,
        channel,
        channel_id: None,
        verified: false,
        duration: None,
        views,
        published: None,
        thumb_url,
        progress_pct: None,
        live: LiveState::None,
        badges: vec!["SHORTS".to_string()],
    }))
}

pub fn parse_playlist_renderer(v: &Value) -> Option<PlaylistCard> {
    let id = v.get("playlistId").and_then(|s| s.as_str())?.to_string();
    let title = extract_text(v.get("title")?);
    let channel = v.get("shortBylineText")
        .or_else(|| v.get("longBylineText"))
        .map(extract_text)
        .unwrap_or_default();
    let video_count = v.get("videoCountText")
        .or_else(|| v.get("videoCount"))
        .map(extract_text);
    let thumb_url = v.get("thumbnails")
        .or_else(|| v.get("thumbnail"))
        .map(extract_best_thumbnail)
        .unwrap_or_default();
    let is_mix = id.starts_with("RD") || title.to_lowercase().contains("mix");

    Some(PlaylistCard {
        id,
        title,
        channel,
        video_count,
        thumb_url,
        is_mix,
    })
}

pub fn parse_tile_renderer(v: &Value) -> Option<Card> {
    let video_id = v.pointer("/onSelectCommand/watchEndpoint/videoId")
        .or_else(|| v.pointer("/onSelectCommand/reelWatchEndpoint/videoId"))
        .or_else(|| v.pointer("/onSelectCommand/showMenuCommand/contentId"))
        .or_else(|| v.pointer("/menu/menuRenderer/items/0/menuNavigationItemRenderer/navigationEndpoint/watchEndpoint/videoId"))
        .and_then(|s| s.as_str());

    let playlist_id = v.pointer("/onSelectCommand/watchEndpoint/playlistId")
        .or_else(|| v.pointer("/onSelectCommand/watchPlaylistEndpoint/playlistId"))
        .or_else(|| v.pointer("/menu/menuRenderer/items/0/menuNavigationItemRenderer/navigationEndpoint/watchEndpoint/playlistId"))
        .and_then(|s| s.as_str());

    let channel_id = v.pointer("/onSelectCommand/browseEndpoint/browseId")
        .and_then(|s| s.as_str());

    let content_type = v.get("contentType").and_then(|s| s.as_str()).unwrap_or("");
    let is_shorts = v.get("style").and_then(|s| s.as_str()).map(|s| s.contains("SHORTS")).unwrap_or(false);

    let title = v.pointer("/metadata/tileMetadataRenderer/title")
        .or_else(|| v.pointer("/header/tileHeaderRenderer/title"))
        .or_else(|| v.pointer("/header/trackTileHeaderRenderer/title"))
        .map(extract_text)
        .unwrap_or_default();
    if title.is_empty() {
        return None;
    }

    let thumb_url = v.pointer("/header/tileHeaderRenderer/thumbnail")
        .or_else(|| v.pointer("/header/trackTileHeaderRenderer/thumbnail"))
        .map(extract_best_thumbnail)
        .unwrap_or_default();

    if content_type == "TILE_CONTENT_TYPE_PLAYLIST" || (playlist_id.is_some() && video_id.is_none()) {
        let p_id = playlist_id.unwrap_or("").to_string();
        let channel = v.pointer("/metadata/tileMetadataRenderer/lines/0/lineRenderer/items/0/lineItemRenderer/badge/metadataBadgeRenderer/label")
            .or_else(|| v.pointer("/metadata/tileMetadataRenderer/userName"))
            .map(extract_text)
            .unwrap_or_default();
        let is_mix = p_id.starts_with("RD") || title.to_lowercase().contains("mix");
        return Some(Card::Playlist(PlaylistCard {
            id: p_id,
            title,
            channel,
            video_count: None,
            thumb_url,
            is_mix,
        }));
    }

    if content_type == "TILE_CONTENT_TYPE_CHANNEL" || (channel_id.is_some() && video_id.is_none() && playlist_id.is_none()) {
        let c_id = channel_id.unwrap_or("").to_string();
        let subscribers = v.pointer("/metadata/tileMetadataRenderer/lines/0/lineRenderer/items/0/lineItemRenderer/badge/metadataBadgeRenderer/label")
            .map(extract_text);
        return Some(Card::Channel(ChannelCard {
            id: c_id,
            title,
            handle: None,
            subscribers,
            avatar_url: if thumb_url.is_empty() { None } else { Some(thumb_url) },
            verified: false,
        }));
    }

    if let Some(vid) = video_id {
        let mut meta_items = Vec::new();
        if let Some(lines) = v.pointer("/metadata/tileMetadataRenderer/lines").and_then(|l| l.as_array()) {
            for line in lines {
                if let Some(items) = line.pointer("/lineRenderer/items").and_then(|i| i.as_array()) {
                    for item in items {
                        if let Some(text_val) = item.pointer("/lineItemRenderer/text") {
                            let t = extract_text(text_val);
                            if !t.is_empty() {
                                meta_items.push(t);
                            }
                        }
                    }
                }
            }
        }

        let channel = v.pointer("/metadata/tileMetadataRenderer/userName")
            .map(extract_text)
            .filter(|s| !s.is_empty())
            .or_else(|| meta_items.first().cloned())
            .unwrap_or_default();

        let mut views = None;
        let mut published = None;
        let start_idx = if v.pointer("/metadata/tileMetadataRenderer/userName").is_some() { 0 } else { 1 };
        for item in meta_items.iter().skip(start_idx) {
            if item.contains("view") || item.contains("watching") || item.ends_with('M') || item.ends_with('K') || item.ends_with('B') {
                if views.is_none() {
                    views = Some(item.clone());
                    continue;
                }
            }
            if item.contains("ago") || item.contains("Streamed") || item.contains("Premier") || item.contains("year") || item.contains("month") || item.contains("day") || item.contains("hour") || item.contains("minute") {
                if published.is_none() {
                    published = Some(item.clone());
                    continue;
                }
            }
            if views.is_none() {
                views = Some(item.clone());
            } else if published.is_none() {
                published = Some(item.clone());
            }
        }

        let duration = v.pointer("/header/tileHeaderRenderer/thumbnailOverlays/0/thumbnailOverlayTimeStatusRenderer/text")
            .or_else(|| v.pointer("/header/tileHeaderRenderer/badgeText"))
            .map(extract_text)
            .filter(|s| !s.is_empty());
        let mut badges = Vec::new();
        if is_shorts {
            badges.push("SHORTS".to_string());
        }
        return Some(Card::Video(VideoCard {
            id: vid.to_string(),
            title,
            channel,
            channel_id: None,
            verified: false,
            duration,
            views,
            published,
            thumb_url,
            progress_pct: None,
            live: LiveState::None,
            badges,
        }));
    }

    None
}

pub fn parse_card(val: &Value) -> Option<Card> {
    if let Some(rich) = val.get("richItemRenderer") {
        return parse_card(rich.get("content")?);
    }
    if let Some(tr) = val.get("tileRenderer") {
        return parse_tile_renderer(tr);
    }
    if let Some(vr) = val.get("videoRenderer") {
        return parse_video_renderer(vr).map(Card::Video);
    }
    if let Some(cvr) = val.get("compactVideoRenderer") {
        return parse_video_renderer(cvr).map(Card::Video);
    }
    if let Some(gvr) = val.get("gridVideoRenderer") {
        return parse_video_renderer(gvr).map(Card::Video);
    }
    if let Some(pvr) = val.get("pivotVideoRenderer") {
        return parse_video_renderer(pvr).map(Card::Video);
    }
    if let Some(tmvr) = val.get("tvMusicVideoRenderer") {
        return parse_video_renderer(tmvr).map(Card::Video);
    }
    if let Some(lvm) = val.get("lockupViewModel") {
        return parse_lockup_view_model(lvm);
    }
    if let Some(slvm) = val.get("shortsLockupViewModel") {
        return parse_shorts_lockup_view_model(slvm);
    }
    if let Some(rir) = val.get("reelItemRenderer") {
        return parse_reel_item_renderer(rir);
    }
    if let Some(pr) = val.get("playlistRenderer") {
        return parse_playlist_renderer(pr).map(Card::Playlist);
    }
    if let Some(gpr) = val.get("gridPlaylistRenderer") {
        return parse_playlist_renderer(gpr).map(Card::Playlist);
    }
    if let Some(ppr) = val.get("pivotPlaylistRenderer") {
        return parse_playlist_renderer(ppr).map(Card::Playlist);
    }
    if let Some(cpr) = val.get("compactPlaylistRenderer") {
        return parse_playlist_renderer(cpr).map(Card::Playlist);
    }
    if let Some(rr) = val.get("radioRenderer") {
        return parse_playlist_renderer(rr).map(Card::Playlist);
    }
    if let Some(grr) = val.get("gridRadioRenderer") {
        return parse_playlist_renderer(grr).map(Card::Playlist);
    }
    if let Some(prr) = val.get("pivotRadioRenderer") {
        return parse_playlist_renderer(prr).map(Card::Playlist);
    }
    if let Some(crr) = val.get("compactRadioRenderer") {
        return parse_playlist_renderer(crr).map(Card::Playlist);
    }
    if let Some(cr) = val.get("channelRenderer")
        .or_else(|| val.get("gridChannelRenderer"))
        .or_else(|| val.get("pivotChannelRenderer"))
        .or_else(|| val.get("compactChannelRenderer")) {
        let id = cr.get("channelId").and_then(|s| s.as_str())?.to_string();
        let title = extract_text(cr.get("title")?);
        let subscribers = cr.get("subscriberCountText").map(extract_text);
        let avatar_url = cr.get("thumbnail").map(extract_best_thumbnail);
        return Some(Card::Channel(ChannelCard {
            id,
            title,
            handle: None,
            subscribers,
            avatar_url,
            verified: false,
        }));
    }
    None
}

pub fn parse_chips(val: &Value) -> Vec<Chip> {
    let mut chips = Vec::new();
    let mut chip_bars = Vec::new();

    // Look for feedFilterChipBarRenderer in various paths
    if let Some(bar) = val.pointer("/header/feedFilterChipBarRenderer") {
        chip_bars.push(bar);
    }
    if let Some(bar) = val.pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/richGridRenderer/header/feedFilterChipBarRenderer") {
        chip_bars.push(bar);
    }

    for bar in chip_bars {
        if let Some(contents) = bar.get("contents").and_then(|c| c.as_array()) {
            for chip_obj in contents {
                if let Some(chip_rnd) = chip_obj.get("chipCloudChipRenderer") {
                    let title = extract_text(chip_rnd.get("text").unwrap_or(&Value::Null));
                    if title.is_empty() {
                        continue;
                    }
                    let active = chip_rnd.get("isSelected").and_then(|b| b.as_bool()).unwrap_or(false);
                    let token = chip_rnd.pointer("/navigationEndpoint/continuationEndpoint/continuationCommand/token")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string());
                    chips.push(Chip { title, token, active });
                }
            }
        }
    }
    chips
}

pub fn parse_continuation(val: &Value) -> Option<String> {
    if let Some(token) = val
        .pointer("/continuationEndpoint/continuationCommand/token")
        .or_else(|| val.pointer("/continuationItemRenderer/continuationEndpoint/continuationCommand/token"))
        .and_then(|t| t.as_str())
    {
        return Some(token.to_string());
    }

    if let Some(arr) = val.as_array() {
        for item in arr {
            if let Some(tok) = parse_continuation(item) {
                return Some(tok);
            }
        }
    } else if let Some(obj) = val.as_object() {
        for (_, v) in obj {
            if let Some(tok) = parse_continuation(v) {
                return Some(tok);
            }
        }
    }
    None
}

pub fn walk_and_collect_cards(val: &Value, items: &mut Vec<Card>) {
    if let Some(card) = parse_card(val) {
        items.push(card);
        return;
    }

    if let Some(arr) = val.as_array() {
        for elem in arr {
            walk_and_collect_cards(elem, items);
        }
    } else if let Some(obj) = val.as_object() {
        for (_k, v) in obj {
            walk_and_collect_cards(v, items);
        }
    }
}

pub fn parse_feed_page(val: &Value) -> FeedPage {
    let chips = parse_chips(val);
    let mut items = Vec::new();
    walk_and_collect_cards(val, &mut items);
    let continuation = parse_continuation(val);

    FeedPage {
        chips,
        items,
        continuation,
    }
}

pub fn parse_watch_data(val: &Value) -> anyhow::Result<WatchData> {
    let mut items = Vec::new();
    walk_and_collect_cards(val, &mut items);

    let mut video = None;
    let mut description = String::new();
    let chapters = Vec::new();
    let mut chat_token = None;

    // Look for current video info in videoDetails or primary info
    if let Some(primary) = val.pointer("/contents/twoColumnWatchNextResults/results/results/contents/0/videoPrimaryInfoRenderer") {
        let title = extract_text(primary.get("title").unwrap_or(&Value::Null));
        let views = primary.get("viewCount").map(extract_text);
        let published = primary.get("dateText").map(extract_text);
        video = Some(VideoCard {
            id: String::new(),
            title,
            channel: String::new(),
            channel_id: None,
            verified: false,
            duration: None,
            views,
            published,
            thumb_url: String::new(),
            progress_pct: None,
            live: LiveState::None,
            badges: Vec::new(),
        });
    }

    if let Some(secondary) = val.pointer("/contents/twoColumnWatchNextResults/results/results/contents/1/videoSecondaryInfoRenderer") {
        if let Some(ref mut v) = video {
            let channel = extract_text(secondary.get("owner").unwrap_or(&Value::Null));
            if !channel.is_empty() {
                v.channel = channel;
            }
        }
        let desc = secondary.get("description").or_else(|| secondary.get("attributedDescription")).map(extract_text).unwrap_or_default();
        if !desc.is_empty() {
            description = desc;
        }
    }

    // If videoDetails is directly present (from /next response)
    if let Some(details) = val.get("videoDetails") {
        let id = details.get("videoId").and_then(|s| s.as_str()).unwrap_or("").to_string();
        let title = details.get("title").and_then(|s| s.as_str()).unwrap_or("").to_string();
        let channel = details.get("author").and_then(|s| s.as_str()).unwrap_or("").to_string();
        let channel_id = details.get("channelId").and_then(|s| s.as_str()).map(|s| s.to_string());
        let desc = details.get("shortDescription").and_then(|s| s.as_str()).unwrap_or("").to_string();
        if !desc.is_empty() {
            description = desc;
        }
        let length_seconds: u32 = details.get("lengthSeconds")
            .and_then(|s| s.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let duration = if length_seconds > 0 {
            let mins = length_seconds / 60;
            let secs = length_seconds % 60;
            Some(format!("{mins}:{secs:02}"))
        } else {
            None
        };
        let views = details.get("viewCount").and_then(|s| s.as_str()).map(|s| format!("{s} views"));
        let thumb_url = details.get("thumbnail")
            .map(extract_best_thumbnail)
            .unwrap_or_else(|| format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"));
        let is_live = details.get("isLiveContent").and_then(|b| b.as_bool()).unwrap_or(false);

        video = Some(VideoCard {
            id,
            title,
            channel,
            channel_id,
            verified: false,
            duration,
            views,
            published: None,
            thumb_url,
            progress_pct: None,
            live: if is_live { LiveState::Live } else { LiveState::None },
            badges: Vec::new(),
        });
    }

    if video.is_none() {
        if let Some(vm) = val.pointer("/contents/singleColumnWatchNextResults/results/results/contents/0/itemSectionRenderer/contents/0/videoMetadataRenderer") {
            let id = vm.get("videoId").and_then(|s| s.as_str()).unwrap_or("").to_string();
            let title = extract_text(vm.get("title").unwrap_or(&Value::Null));
            let channel = vm.pointer("/owner/videoOwnerRenderer/title").map(extract_text).unwrap_or_default();
            let channel_id = vm.pointer("/owner/videoOwnerRenderer/subscribeButton/subscribeButtonRenderer/channelId").and_then(|s| s.as_str()).map(|s| s.to_string());
            let views = vm.pointer("/viewCount/videoViewCountRenderer/viewCount").map(extract_text);
            let published = vm.get("dateText").map(extract_text);
            let thumb_url = format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg");

            video = Some(VideoCard {
                id,
                title,
                channel,
                channel_id,
                verified: false,
                duration: None,
                views,
                published,
                thumb_url,
                progress_pct: None,
                live: LiveState::None,
                badges: Vec::new(),
            });
        }
    }

    // Look for chat token
    if let Some(tok) = val.pointer("/contents/twoColumnWatchNextResults/conversationBar/liveChatRenderer/continuations/0/reloadContinuationData/continuation")
        .and_then(|s| s.as_str())
    {
        chat_token = Some(tok.to_string());
    }

    // Look for comments continuation token
    let mut comments_token = None;
    if let Some(panels) = val.get("engagementPanels").and_then(|p| p.as_array()) {
        for p in panels {
            let panel = p.get("engagementPanelSectionListRenderer");
            if let Some(p_rend) = panel {
                if p_rend.get("panelIdentifier").and_then(|s| s.as_str()) == Some("comment-item-section") {
                    if let Some(conts) = p_rend.pointer("/content/sectionListRenderer/contents").and_then(|c| c.as_array()) {
                        for c in conts {
                            if let Some(conts_arr) = c.pointer("/itemSectionRenderer/continuations").and_then(|a| a.as_array()) {
                                for cd in conts_arr {
                                    if let Some(tok) = cd.pointer("/reloadContinuationData/continuation")
                                        .or_else(|| cd.pointer("/nextContinuationData/continuation"))
                                        .and_then(|s| s.as_str())
                                    {
                                        comments_token = Some(tok.to_string());
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if comments_token.is_none() {
        if let Some(tok) = val.pointer("/contents/twoColumnWatchNextResults/results/results/contents/2/itemSectionRenderer/continuations/0/reloadContinuationData/continuation")
            .or_else(|| val.pointer("/contents/twoColumnWatchNextResults/results/results/contents/2/itemSectionRenderer/continuations/0/nextContinuationData/continuation"))
            .and_then(|s| s.as_str())
        {
            comments_token = Some(tok.to_string());
        }
    }

    // Look for subscribed status
    let mut is_subscribed = None;
    if let Some(sub) = val.pointer("/contents/singleColumnWatchNextResults/results/results/contents/0/itemSectionRenderer/contents/0/videoMetadataRenderer/owner/videoOwnerRenderer/subscribeButton/subscribeButtonRenderer/subscribed")
        .or_else(|| val.pointer("/transportControls/transportControlsRenderer/buttons/1/button/subscribeButtonRenderer/subscribed"))
        .or_else(|| val.pointer("/contents/twoColumnWatchNextResults/results/results/contents/1/videoSecondaryInfoRenderer/subscribeButton/subscribeButtonRenderer/subscribed"))
        .and_then(|b| b.as_bool())
    {
        is_subscribed = Some(sub);
    }

    // Look for like status
    let mut is_liked = None;
    let mut is_disliked = None;
    if let Some(mutations) = val.pointer("/frameworkUpdates/entityBatchUpdate/mutations").and_then(|m| m.as_array()) {
        for mut_item in mutations {
            if let Some(status) = mut_item.pointer("/payload/likeStatusEntity/likeStatus").and_then(|s| s.as_str()) {
                match status {
                    "LIKE" => {
                        is_liked = Some(true);
                        is_disliked = Some(false);
                    }
                    "DISLIKE" => {
                        is_liked = Some(false);
                        is_disliked = Some(true);
                    }
                    "INDIFFERENT" => {
                        is_liked = Some(false);
                        is_disliked = Some(false);
                    }
                    _ => {}
                }
            }
        }
    }

    let default_video = VideoCard {
        id: String::new(),
        title: "Video".to_string(),
        channel: String::new(),
        channel_id: None,
        verified: false,
        duration: None,
        views: None,
        published: None,
        thumb_url: String::new(),
        progress_pct: None,
        live: LiveState::None,
        badges: Vec::new(),
    };

    Ok(WatchData {
        video: video.unwrap_or(default_video),
        description,
        chapters,
        up_next: items,
        chat_token,
        likes: None,
        dislikes: None,
        rating: None,
        is_liked,
        is_disliked,
        is_subscribed,
        comments_token,
        comments: Vec::new(),
    })
}

pub fn parse_comments(val: &Value) -> CommentsBatch {
    let mut comments = Vec::new();
    let mut continuation = None;
    let mut total_count = None;

    if let Some(cnt) = val.pointer("/header/commentsHeaderRenderer/countText").map(extract_text) {
        if !cnt.is_empty() {
            total_count = Some(cnt);
        }
    }

    let section = val.pointer("/continuationContents/itemSectionContinuation")
        .or_else(|| val.pointer("/contents/twoColumnWatchNextResults/results/results/contents/2/itemSectionRenderer"));

    if let Some(sec) = section {
        if let Some(contents) = sec.get("contents").and_then(|c| c.as_array()) {
            for item in contents {
                if let Some(ctr) = item.get("commentThreadRenderer") {
                    let cr = ctr.pointer("/comment/commentRenderer");
                    if let Some(cr) = cr {
                        let id = cr.get("commentId").and_then(|s| s.as_str()).unwrap_or("").to_string();
                        let author = cr.get("authorText").map(extract_text).unwrap_or_default();
                        let author_thumb = cr.get("authorThumbnail").map(extract_best_thumbnail);
                        let text = cr.get("contentText").map(extract_text).unwrap_or_default();
                        let published = cr.get("publishedTimeText").map(extract_text);
                        let likes = cr.get("voteCount").map(extract_text);
                        let reply_count = cr.get("replyCount").and_then(|c| c.as_u64()).map(|c| c as u32);
                        if !author.is_empty() && !text.is_empty() {
                            comments.push(CommentItem {
                                id,
                                author,
                                author_thumb,
                                text,
                                published,
                                likes,
                                reply_count,
                            });
                        }
                    }
                }
            }
        }

        if let Some(conts) = sec.get("continuations").and_then(|c| c.as_array()) {
            for c in conts {
                if let Some(tok) = c.pointer("/nextContinuationData/continuation").and_then(|s| s.as_str()) {
                    continuation = Some(tok.to_string());
                    break;
                }
            }
        }
    }

    CommentsBatch {
        comments,
        continuation,
        total_count,
    }
}
