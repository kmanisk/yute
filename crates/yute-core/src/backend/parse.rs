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
            .filter_map(|r| {
                r.get("text")
                    .and_then(Value::as_str)
                    .or_else(|| {
                        r.pointer("/emoji/emojiId")
                            .and_then(Value::as_str)
                            .filter(|s| !s.is_ascii())
                    })
                    .or_else(|| r.pointer("/emoji/shortcuts/0").and_then(Value::as_str))
            })
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
        let texts: Vec<String> = parts
            .iter()
            .map(extract_text)
            .filter(|s| !s.is_empty())
            .collect();
        let res = texts.join(" • ").trim().to_string();
        if !res.is_empty() {
            return res;
        }
    }
    String::new()
}

fn thumbnail_source(sources: &[Value]) -> Option<&Value> {
    let valid = |value: &&Value| {
        value
            .get("url")
            .and_then(Value::as_str)
            .is_some_and(|url| !url.trim().is_empty())
    };
    sources
        .iter()
        .filter(valid)
        .filter(|v| {
            v.get("width")
                .and_then(Value::as_u64)
                .is_some_and(|w| w >= 240)
                && v.get("height")
                    .and_then(Value::as_u64)
                    .is_some_and(|h| h > 0)
                && v["width"].as_u64() >= v["height"].as_u64()
        })
        .min_by(|a, b| {
            let score = |v: &Value| {
                (v["width"].as_f64().unwrap_or(1.0) / v["height"].as_f64().unwrap_or(1.0)
                    - 16.0 / 9.0)
                    .abs()
            };
            score(a)
                .total_cmp(&score(b))
                .then_with(|| b["width"].as_u64().cmp(&a["width"].as_u64()))
        })
        .or_else(|| sources.iter().rev().find(valid))
}

pub fn extract_best_thumbnail(val: &Value) -> String {
    if let Some(thumbs) = val.get("thumbnails").and_then(|v| v.as_array()) {
        if let Some(last) = thumbnail_source(thumbs) {
            if let Some(url) = last.get("url").and_then(|u| u.as_str()) {
                return crate::media::thumbs::normalize_thumbnail_url(url).into_owned();
            }
        }
    }
    if let Some(sources) = val.get("sources").and_then(|v| v.as_array()) {
        if let Some(last) = thumbnail_source(sources) {
            if let Some(url) = last.get("url").and_then(|u| u.as_str()) {
                return crate::media::thumbs::normalize_thumbnail_url(url).into_owned();
            }
        }
    }
    if let Some(image) = val.pointer("/contentImage/avatarViewModel/image") {
        let url = extract_best_thumbnail(image);
        if !url.is_empty() {
            return url;
        }
    }
    if let Some(image) = val.get("image") {
        let u = extract_best_thumbnail(image);
        if !u.is_empty() {
            return u;
        }
    }
    if let Some(primary) = val.pointer(
        "/contentImage/collectionThumbnailViewModel/primaryThumbnail/thumbnailViewModel/image",
    ) {
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
    for key in [
        "avatar",
        "avatarViewModel",
        "thumbnail",
        "thumbnailViewModel",
    ] {
        if let Some(nested) = val.get(key) {
            let url = extract_best_thumbnail(nested);
            if !url.is_empty() {
                return url;
            }
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
    if let Some(byline) = v
        .get("ownerText")
        .or_else(|| v.get("shortBylineText"))
        .or_else(|| v.get("longBylineText"))
    {
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
            let style = b
                .get("metadataBadgeRenderer")
                .and_then(|r| r.get("style"))
                .and_then(|s| s.as_str())
                .unwrap_or("");
            if style.contains("VERIFIED") {
                verified = true;
                break;
            }
        }
    }

    let duration = v
        .get("lengthText")
        .map(extract_text)
        .filter(|s| !s.is_empty());
    let views = v
        .get("shortViewCountText")
        .or_else(|| v.get("viewCountText"))
        .map(extract_text)
        .filter(|s| !s.is_empty());
    let published = v
        .get("publishedTimeText")
        .map(extract_text)
        .filter(|s| !s.is_empty());

    let mut thumb_url = v
        .get("thumbnail")
        .map(extract_best_thumbnail)
        .unwrap_or_default();
    if thumb_url.is_empty() {
        thumb_url = format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg");
    }

    let mut progress_pct = None;
    if let Some(overlays) = v.get("thumbnailOverlays").and_then(|o| o.as_array()) {
        for ov in overlays {
            if let Some(resume) = ov.get("thumbnailOverlayResumePlaybackRenderer") {
                if let Some(pct) = resume
                    .get("percentDurationWatched")
                    .and_then(|p| p.as_u64())
                {
                    progress_pct = Some(pct.min(100) as u8);
                }
            }
        }
    }

    let mut live = LiveState::None;
    let mut badges = Vec::new();
    let is_short = v.pointer("/navigationEndpoint/reelWatchEndpoint").is_some()
        || v.pointer("/thumbnailOverlays/0/thumbnailOverlayTimeStatusRenderer/icon/iconType")
            .and_then(|s| s.as_str())
            .map(|s| s.contains("SHORTS"))
            .unwrap_or(false);
    if is_short {
        badges.push("SHORTS".to_string());
    }

    if let Some(b_list) = v.get("badges").and_then(|b| b.as_array()) {
        for b in b_list {
            let label = b
                .get("metadataBadgeRenderer")
                .and_then(|r| r.get("label").or_else(|| r.get("style")))
                .map(extract_text)
                .unwrap_or_default();
            if label.to_uppercase().contains("LIVE") {
                live = LiveState::Live;
            } else if label.to_uppercase().contains("UPCOMING") {
                live = LiveState::Upcoming;
            } else if !label.is_empty() && !badges.contains(&label) {
                badges.push(label);
            }
        }
    }

    let resume_time_sec = v
        .pointer("/navigationEndpoint/watchEndpoint/startTimeSeconds")
        .or_else(|| v.pointer("/onTap/innertubeCommand/watchEndpoint/startTimeSeconds"))
        .and_then(|s| s.as_u64());

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
        resume_time_sec,
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
    let command = v
        .pointer("/rendererContext/commandContext/onTap/innertubeCommand")
        .or_else(|| v.pointer("/onTap/innertubeCommand"));
    let browse_id = command
        .and_then(|c| c.pointer("/browseEndpoint/browseId"))
        .and_then(Value::as_str);
    if content_type.contains("CHANNEL") || browse_id.is_some_and(|id| id.starts_with("UC")) {
        return Some(Card::Channel(ChannelCard {
            id: browse_id.unwrap_or(&id).to_string(),
            title,
            handle: stats
                .iter()
                .chain(std::iter::once(&channel))
                .find(|s| s.starts_with('@'))
                .cloned(),
            subscribers: stats
                .iter()
                .chain(std::iter::once(&channel))
                .find(|s| s.contains("subscriber"))
                .cloned(),
            avatar_url: Some(extract_best_thumbnail(v)).filter(|s| !s.is_empty()),
            verified: false,
        }));
    }

    let playlist_type = content_type.contains("PLAYLIST") || content_type.contains("MIX");
    let legacy_collection =
        content_type.is_empty() && id.len() > 11 && (id.starts_with("PL") || id.starts_with("RD"));
    let is_mix = (playlist_type || legacy_collection) && id.starts_with("RD");
    if playlist_type || legacy_collection {
        let channel_name =
            if is_mix && (channel.is_empty() || channel.eq_ignore_ascii_case("playlist")) {
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
        let first_video_id = v.pointer("/rendererContext/commandContext/onTap/innertubeCommand/watchEndpoint/videoId")
            .or_else(|| v.pointer("/rendererContext/commandContext/onTap/innertubeCommand/watchPlaylistEndpoint/videoId"))
            .or_else(|| v.pointer("/onTap/innertubeCommand/watchEndpoint/videoId"))
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());
        Some(Card::Playlist(PlaylistCard {
            id,
            title,
            channel: channel_name,
            video_count,
            thumb_url,
            is_mix,
            first_video_id,
            updated: None,
        }))
    } else {
        if !content_type.is_empty()
            && !content_type.contains("VIDEO")
            && !content_type.contains("SHORT")
            && !content_type.contains("LIVE")
            && !content_type.contains("UPCOMING")
        {
            return None;
        }
        let mut badges = Vec::new();
        let is_short = content_type.contains("SHORT")
            || v.pointer("/contentImage/thumbnailViewModel/overlays/0/thumbnailBottomOverlayViewModel/badges/0/thumbnailBadgeViewModel/icon/sources/0/clientResource/imageName")
                .and_then(|s| s.as_str())
                .map(|s| s.contains("SHORTS"))
                .unwrap_or(false)
            || v.pointer("/rendererContext/commandContext/onTap/innertubeCommand/reelWatchEndpoint").is_some();
        if is_short {
            badges.push("SHORTS".to_string());
        }

        let mut progress_pct = None;
        if let Some(overlays) = v
            .pointer("/contentImage/thumbnailViewModel/overlays")
            .and_then(|o| o.as_array())
        {
            for ov in overlays {
                if let Some(resume) = ov.get("thumbnailOverlayResumePlaybackRenderer") {
                    if let Some(pct) = resume
                        .get("percentDurationWatched")
                        .and_then(|p| p.as_u64())
                    {
                        progress_pct = Some(pct.min(100) as u8);
                    }
                }
            }
        }

        let resume_time_sec = v.pointer("/rendererContext/commandContext/onTap/innertubeCommand/watchEndpoint/startTimeSeconds")
            .or_else(|| v.pointer("/rendererContext/commandContext/onLongPress/innertubeCommand/showMenuCommand/menu/menuRenderer/items/0/menuNavigationItemRenderer/navigationEndpoint/watchEndpoint/startTimeSeconds"))
            .or_else(|| v.pointer("/onTap/innertubeCommand/watchEndpoint/startTimeSeconds"))
            .and_then(|s| s.as_u64());

        Some(Card::Video(VideoCard {
            id,
            title,
            channel,
            channel_id: find_channel_browse_id(meta).map(str::to_string),
            verified: false,
            duration,
            views: stats.first().cloned(),
            published: if stats.len() > 1 {
                Some(stats[1].clone())
            } else {
                None
            },
            thumb_url,
            progress_pct,
            live: typed_live_state(v),
            badges,
            resume_time_sec,
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

    let mut thumb_url = v
        .get("thumbnail")
        .map(extract_best_thumbnail)
        .unwrap_or_default();
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
        resume_time_sec: None,
    }))
}

pub fn parse_reel_item_renderer(v: &Value) -> Option<Card> {
    let id = v
        .get("videoId")
        .and_then(|s| s.as_str())
        .or_else(|| {
            v.pointer("/navigationEndpoint/reelWatchEndpoint/videoId")
                .and_then(|s| s.as_str())
        })?
        .to_string();

    let title = v
        .get("headline")
        .map(extract_text)
        .unwrap_or_else(|| "Short".to_string());
    let views = v
        .get("viewCountText")
        .map(extract_text)
        .filter(|s| !s.is_empty());
    let channel = v
        .get("shortBylineText")
        .or_else(|| v.get("longBylineText"))
        .map(extract_text)
        .filter(|s| s != "YouTube Shorts")
        .unwrap_or_default();

    let mut thumb_url = v
        .get("thumbnail")
        .map(extract_best_thumbnail)
        .unwrap_or_default();
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
        resume_time_sec: None,
    }))
}

pub fn parse_playlist_renderer(v: &Value) -> Option<PlaylistCard> {
    let id = v.get("playlistId").and_then(|s| s.as_str())?.to_string();
    let title = extract_text(v.get("title")?);
    let channel = v
        .get("shortBylineText")
        .or_else(|| v.get("longBylineText"))
        .map(extract_text)
        .unwrap_or_default();
    let video_count = v
        .get("videoCountText")
        .or_else(|| v.get("videoCount"))
        .map(extract_text);
    let thumb_url = v
        .get("thumbnails")
        .or_else(|| v.get("thumbnail"))
        .map(extract_best_thumbnail)
        .unwrap_or_default();
    let is_mix = id.starts_with("RD");

    let first_video_id = v
        .pointer("/navigationEndpoint/watchEndpoint/videoId")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());

    Some(PlaylistCard {
        id,
        title,
        channel,
        video_count,
        thumb_url,
        is_mix,
        first_video_id,
        updated: v
            .get("updatedTimeText")
            .or_else(|| v.get("publishedTimeText"))
            .map(extract_text)
            .filter(|s| !s.trim().is_empty()),
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

    let channel_id = v
        .pointer("/onSelectCommand/browseEndpoint/browseId")
        .and_then(|s| s.as_str());

    let content_type = v.get("contentType").and_then(|s| s.as_str()).unwrap_or("");
    let is_shorts = v
        .get("style")
        .and_then(|s| s.as_str())
        .map(|s| s.contains("SHORTS"))
        .unwrap_or(false);

    let title = v
        .pointer("/metadata/tileMetadataRenderer/title")
        .or_else(|| v.pointer("/header/tileHeaderRenderer/title"))
        .or_else(|| v.pointer("/header/trackTileHeaderRenderer/title"))
        .map(extract_text)
        .unwrap_or_default();
    if title.is_empty() {
        return None;
    }

    let thumb_url = v
        .pointer("/header/tileHeaderRenderer/thumbnail")
        .or_else(|| v.pointer("/header/trackTileHeaderRenderer/thumbnail"))
        .map(extract_best_thumbnail)
        .unwrap_or_default();

    if content_type == "TILE_CONTENT_TYPE_PLAYLIST" || (playlist_id.is_some() && video_id.is_none())
    {
        let p_id = playlist_id.unwrap_or("").to_string();
        let channel = v.pointer("/metadata/tileMetadataRenderer/lines/0/lineRenderer/items/0/lineItemRenderer/badge/metadataBadgeRenderer/label")
            .or_else(|| v.pointer("/metadata/tileMetadataRenderer/userName"))
            .map(extract_text)
            .unwrap_or_default();
        let is_mix = p_id.starts_with("RD");
        return Some(Card::Playlist(PlaylistCard {
            id: p_id,
            title,
            channel,
            video_count: None,
            thumb_url,
            is_mix,
            first_video_id: video_id.map(|s| s.to_string()),
            updated: None,
        }));
    }

    if content_type == "TILE_CONTENT_TYPE_CHANNEL"
        || (channel_id.is_some() && video_id.is_none() && playlist_id.is_none())
    {
        let c_id = channel_id.unwrap_or("").to_string();
        let subscribers = v.pointer("/metadata/tileMetadataRenderer/lines/0/lineRenderer/items/0/lineItemRenderer/badge/metadataBadgeRenderer/label")
            .map(extract_text);
        return Some(Card::Channel(ChannelCard {
            id: c_id,
            title,
            handle: None,
            subscribers,
            avatar_url: if thumb_url.is_empty() {
                None
            } else {
                Some(thumb_url)
            },
            verified: false,
        }));
    }

    if let Some(vid) = video_id {
        let mut meta_items = Vec::new();
        if let Some(lines) = v
            .pointer("/metadata/tileMetadataRenderer/lines")
            .and_then(|l| l.as_array())
        {
            for line in lines {
                if let Some(items) = line
                    .pointer("/lineRenderer/items")
                    .and_then(|i| i.as_array())
                {
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

        let channel = v
            .pointer("/metadata/tileMetadataRenderer/userName")
            .map(extract_text)
            .filter(|s| !s.is_empty())
            .or_else(|| meta_items.first().cloned())
            .unwrap_or_default();

        let mut views = None;
        let mut published = None;
        let start_idx = if v
            .pointer("/metadata/tileMetadataRenderer/userName")
            .is_some()
        {
            0
        } else {
            1
        };
        for item in meta_items
            .iter()
            .skip(start_idx)
            .filter(|s| !s.chars().all(|c| matches!(c, '•' | '·' | ' ' | '|')))
        {
            if (item.contains("view")
                || item.contains("watching")
                || item.ends_with('M')
                || item.ends_with('K')
                || item.ends_with('B'))
                && views.is_none()
            {
                views = Some(item.clone());
                continue;
            }
            if (item.contains("ago")
                || item.contains("Streamed")
                || item.contains("Premier")
                || item.contains("year")
                || item.contains("month")
                || item.contains("day")
                || item.contains("hour")
                || item.contains("minute"))
                && published.is_none()
            {
                published = Some(item.clone());
                continue;
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

        let mut progress_pct = None;
        if let Some(overlays) = v
            .pointer("/header/tileHeaderRenderer/thumbnailOverlays")
            .and_then(|o| o.as_array())
        {
            for ov in overlays {
                if let Some(resume) = ov.get("thumbnailOverlayResumePlaybackRenderer") {
                    if let Some(pct) = resume
                        .get("percentDurationWatched")
                        .and_then(|p| p.as_u64())
                    {
                        progress_pct = Some(pct.min(100) as u8);
                    }
                }
            }
        }

        let resume_time_sec = v
            .pointer("/onTap/innertubeCommand/watchEndpoint/startTimeSeconds")
            .and_then(|s| s.as_u64());

        let mut badges = Vec::new();
        if is_shorts {
            badges.push("SHORTS".to_string());
        }
        return Some(Card::Video(VideoCard {
            id: vid.to_string(),
            title,
            channel,
            channel_id: find_channel_browse_id(v.get("metadata").unwrap_or(&Value::Null))
                .map(str::to_string),
            verified: false,
            duration,
            views,
            published,
            thumb_url,
            progress_pct,
            live: typed_live_state(v),
            badges,
            resume_time_sec,
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
    if let Some(video) = val.get("playlistPanelVideoRenderer") {
        return parse_video_renderer(video).map(Card::Video);
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
    if let Some(cr) = val
        .get("channelRenderer")
        .or_else(|| val.get("gridChannelRenderer"))
        .or_else(|| val.get("pivotChannelRenderer"))
        .or_else(|| val.get("compactChannelRenderer"))
    {
        let id = cr.get("channelId").and_then(|s| s.as_str())?.to_string();
        let title = extract_text(cr.get("title")?);
        let subscribers = cr.get("subscriberCountText").map(extract_text);
        let avatar = extract_best_thumbnail(cr);
        let avatar_url = (!avatar.is_empty()).then_some(avatar);
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
                    let active = chip_rnd
                        .get("isSelected")
                        .and_then(|b| b.as_bool())
                        .unwrap_or(false);
                    let token = chip_rnd
                        .pointer(
                            "/navigationEndpoint/continuationEndpoint/continuationCommand/token",
                        )
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string());
                    chips.push(Chip {
                        title,
                        token,
                        active,
                    });
                }
            }
        }
    }
    chips
}

pub fn parse_continuation(val: &Value) -> Option<String> {
    if let Some(token) = val
        .pointer("/nextContinuationData/continuation")
        .or_else(|| val.pointer("/reloadContinuationData/continuation"))
        .or_else(|| val.pointer("/continuationEndpoint/continuationCommand/token"))
        .or_else(|| {
            val.pointer("/continuationItemRenderer/continuationEndpoint/continuationCommand/token")
        })
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

/// Mix items arrive in watch-next, not saved-playlist browse. TV responses also
/// contain unrelated recommendation shelves; only accept this playlist's endpoints.
pub fn parse_reel_card(value: &Value, fallback_id: Option<&str>) -> Option<Card> {
    let header=value.pointer("/overlay/reelPlayerOverlayRenderer/reelPlayerHeaderSupportedRenderers/reelPlayerHeaderRenderer")?;
    let endpoint = value.pointer("/replacementEndpoint/reelWatchEndpoint");
    let id = endpoint
        .and_then(|e| e.get("videoId"))
        .and_then(Value::as_str)
        .or(fallback_id)?;
    let title = header
        .get("reelTitleText")
        .map(extract_text)
        .filter(|s| !s.is_empty())?;
    let thumb = endpoint
        .and_then(|e| e.get("thumbnail"))
        .map(extract_best_thumbnail)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"));
    Some(Card::Video(VideoCard {
        id: id.into(),
        title,
        channel: header
            .get("channelTitleText")
            .map(extract_text)
            .unwrap_or_default(),
        channel_id: header
            .pointer("/channelNavigationEndpoint/browseEndpoint/browseId")
            .and_then(Value::as_str)
            .map(str::to_string),
        verified: false,
        duration: None,
        views: header.get("viewCountText").map(extract_text),
        published: header.get("timestampText").map(extract_text),
        thumb_url: thumb,
        progress_pct: None,
        live: LiveState::None,
        badges: vec!["SHORTS".into()],
        resume_time_sec: None,
    }))
}

pub fn parse_mix_page(value: &Value, playlist_id: &str) -> FeedPage {
    fn collect(value: &Value, playlist_id: &str, items: &mut Vec<Card>) {
        if let Some(tile) = value.get("tileRenderer") {
            if tile
                .pointer("/onSelectCommand/watchEndpoint/playlistId")
                .and_then(Value::as_str)
                == Some(playlist_id)
            {
                if let Some(Card::Video(video)) = parse_tile_renderer(tile) {
                    items.push(Card::Video(video));
                }
            }
            return;
        }
        if let Some(panel) = value.get("playlistPanelRenderer") {
            if panel.get("playlistId").and_then(Value::as_str) == Some(playlist_id) {
                walk_and_collect_cards(panel.get("contents").unwrap_or(&Value::Null), items);
            }
            return;
        }
        if let Some(object) = value.as_object() {
            for value in object.values() {
                collect(value, playlist_id, items);
            }
        }
        if let Some(array) = value.as_array() {
            for value in array {
                collect(value, playlist_id, items);
            }
        }
    }
    let mut items = Vec::new();
    collect(value, playlist_id, &mut items);
    let mut ids = std::collections::HashSet::new();
    items.retain(|card| ids.insert(card.id().to_string()));
    FeedPage {
        items,
        chips: Vec::new(),
        continuation: None,
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
    if let Some(primary) = val.pointer(
        "/contents/twoColumnWatchNextResults/results/results/contents/0/videoPrimaryInfoRenderer",
    ) {
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
            resume_time_sec: None,
        });
    }

    if let Some(secondary) = val.pointer(
        "/contents/twoColumnWatchNextResults/results/results/contents/1/videoSecondaryInfoRenderer",
    ) {
        if let Some(ref mut v) = video {
            let channel = extract_text(secondary.get("owner").unwrap_or(&Value::Null));
            if !channel.is_empty() {
                v.channel = channel;
            }
        }
        let desc = secondary
            .get("description")
            .or_else(|| secondary.get("attributedDescription"))
            .map(extract_text)
            .unwrap_or_default();
        if !desc.is_empty() {
            description = desc;
        }
    }

    // If videoDetails is directly present (from /next response)
    if let Some(details) = val.get("videoDetails") {
        let id = details
            .get("videoId")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let title = details
            .get("title")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let channel = details
            .get("author")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        let channel_id = details
            .get("channelId")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());
        let desc = details
            .get("shortDescription")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_string();
        if !desc.is_empty() {
            description = desc;
        }
        let length_seconds: u32 = details
            .get("lengthSeconds")
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
        let views = details
            .get("viewCount")
            .and_then(|s| s.as_str())
            .map(|s| format!("{s} views"));
        let thumb_url = details
            .get("thumbnail")
            .map(extract_best_thumbnail)
            .unwrap_or_else(|| format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"));
        let is_live = details
            .get("isLiveContent")
            .and_then(|b| b.as_bool())
            .unwrap_or(false);

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
            live: if is_live {
                LiveState::Live
            } else {
                LiveState::None
            },
            badges: Vec::new(),
            resume_time_sec: None,
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
                resume_time_sec: None,
            });
        }
    }

    // Look for description in engagementPanels (TVHTML5 structuredDescriptionContentRenderer)
    if description.is_empty() {
        if let Some(panels) = val.get("engagementPanels").and_then(|p| p.as_array()) {
            for p in panels {
                if let Some(items) = p.pointer("/engagementPanelSectionListRenderer/content/structuredDescriptionContentRenderer/items").and_then(|i| i.as_array()) {
                    for it in items {
                        if let Some(body) = it.pointer("/expandableVideoDescriptionBodyRenderer/descriptionBodyText") {
                            let d = extract_text(body);
                            if !d.is_empty() {
                                description = d;
                                break;
                            }
                        }
                    }
                }
                if !description.is_empty() {
                    break;
                }
            }
        }
    }

    if description.is_empty() {
        if let Some(vm) = val.pointer("/contents/singleColumnWatchNextResults/results/results/contents/0/itemSectionRenderer/contents/0/videoMetadataRenderer") {
            if let Some(d) = vm.get("description").or_else(|| vm.get("attributedDescription")).map(extract_text) {
                if !d.is_empty() {
                    description = d;
                }
            }
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
                if p_rend.get("panelIdentifier").and_then(|s| s.as_str())
                    == Some("comment-item-section")
                {
                    if let Some(conts) = p_rend
                        .pointer("/content/sectionListRenderer/contents")
                        .and_then(|c| c.as_array())
                    {
                        for c in conts {
                            if let Some(conts_arr) = c
                                .pointer("/itemSectionRenderer/continuations")
                                .and_then(|a| a.as_array())
                            {
                                for cd in conts_arr {
                                    if let Some(tok) = cd
                                        .pointer("/reloadContinuationData/continuation")
                                        .or_else(|| {
                                            cd.pointer("/nextContinuationData/continuation")
                                        })
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
        if let Some(panels) = val.get("engagementPanels").and_then(Value::as_array) {
            for panel in panels {
                let renderer = panel
                    .get("engagementPanelSectionListRenderer")
                    .unwrap_or(panel);
                if renderer
                    .get("panelIdentifier")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id.contains("comment"))
                {
                    comments_token = parse_continuation(renderer).or_else(|| {
                        find_json_key(renderer, "reloadContinuationData")
                            .and_then(|v| v.get("continuation"))
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    });
                    if comments_token.is_some() {
                        break;
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
    if let Some(mutations) = val
        .pointer("/frameworkUpdates/entityBatchUpdate/mutations")
        .and_then(|m| m.as_array())
    {
        for mut_item in mutations {
            if let Some(status) = mut_item
                .pointer("/payload/likeStatusEntity/likeStatus")
                .and_then(|s| s.as_str())
            {
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
        resume_time_sec: None,
    };

    let current_id = video.as_ref().map(|v| v.id.as_str()).unwrap_or("");
    items.retain(|card| card.id() != current_id);
    let mut description_links = Vec::new();
    fn links(value: &Value, out: &mut Vec<(String, String)>) {
        if let Some(object) = value.as_object() {
            if let Some(url) = value
                .pointer("/navigationEndpoint/urlEndpoint/url")
                .and_then(Value::as_str)
                .filter(|url| url.starts_with("https://") || url.starts_with("http://"))
            {
                let label = value
                    .get("text")
                    .map(extract_text)
                    .unwrap_or_else(|| url.to_string());
                if !out.iter().any(|(_, u)| u == url) {
                    out.push((label, url.into()));
                }
            }
            for value in object.values() {
                links(value, out);
            }
        }
        if let Some(array) = value.as_array() {
            for value in array {
                links(value, out);
            }
        }
    }
    if let Some(body) = find_json_key(val, "videoDescriptionBodyRenderer") {
        links(body, &mut description_links);
    }
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
        comment_create_params: find_json_key(val, "createCommentParams")
            .and_then(Value::as_str)
            .map(str::to_string),
        description_links,
    })
}

pub fn find_json_key<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    if let Some(object) = value.as_object() {
        if let Some(value) = object.get(key) {
            return Some(value);
        }
        for value in object.values() {
            if let Some(found) = find_json_key(value, key) {
                return Some(found);
            }
        }
    }
    if let Some(array) = value.as_array() {
        for value in array {
            if let Some(found) = find_json_key(value, key) {
                return Some(found);
            }
        }
    }
    None
}

pub fn parse_comments(val: &Value) -> CommentsBatch {
    use crate::backend::models::{CommentMedia, CommentMediaKind};

    fn action(value: &Value, key: &str) -> Option<String> {
        find_json_key(value, key)
            .and_then(Value::as_str)
            .map(str::to_string)
    }
    fn continuation(value: &Value) -> Option<String> {
        find_json_key(value, "continuationCommand")
            .and_then(|v| v.get("token"))
            .and_then(Value::as_str)
            .or_else(|| {
                find_json_key(value, "reloadContinuationData")
                    .and_then(|v| v.get("continuation"))
                    .and_then(Value::as_str)
            })
            .map(str::to_string)
    }
    fn button_action(button: Option<&Value>, toggled: bool) -> Option<String> {
        let endpoint = if toggled {
            "toggledServiceEndpoint"
        } else {
            "defaultServiceEndpoint"
        };
        button
            .and_then(|value| find_json_key(value, endpoint))
            .and_then(|value| find_json_key(value, "performCommentActionEndpoint"))
            .and_then(|value| value.get("action"))
            .and_then(Value::as_str)
            .map(str::to_string)
    }
    fn media(value: &Value) -> Vec<CommentMedia> {
        fn add(
            out: &mut Vec<CommentMedia>,
            url: &str,
            kind: CommentMediaKind,
            width: Option<u32>,
            height: Option<u32>,
        ) {
            if url.is_empty() || out.iter().any(|m| m.url == url) {
                return;
            }
            out.push(CommentMedia {
                url: crate::media::thumbs::normalize_thumbnail_url(url).into_owned(),
                kind,
                width,
                height,
            });
        }
        fn walk(value: &Value, path: &str, out: &mut Vec<CommentMedia>) {
            match value {
                Value::Object(map) => {
                    if let Some(url) = map.get("imageUrl").and_then(Value::as_str) {
                        let kind = if path.to_ascii_lowercase().contains("gif")
                            || path.to_ascii_lowercase().contains("animated")
                            || url.to_ascii_lowercase().contains(".gif")
                        {
                            CommentMediaKind::Gif
                        } else {
                            CommentMediaKind::Image
                        };
                        add(out, url, kind, None, None);
                    }
                    for (key, child) in map {
                        // Author avatars are rendered separately. Only attachment/image
                        // renderers belong to the body of a comment.
                        let lower = key.to_ascii_lowercase();
                        let child_path = if path.is_empty() {
                            key.clone()
                        } else {
                            format!("{path}.{key}")
                        };
                        if (lower.contains("gif")
                            || lower.contains("animatedimage")
                            || lower.contains("commentimage")
                            || lower.contains("attachment"))
                            && key != "authorThumbnail"
                        {
                            let url = extract_best_thumbnail(child);
                            if !url.is_empty() {
                                let thumb = find_json_key(child, "thumbnails")
                                    .and_then(Value::as_array)
                                    .and_then(|a| a.last());
                                add(
                                    out,
                                    &url,
                                    if lower.contains("gif") || lower.contains("animated") {
                                        CommentMediaKind::Gif
                                    } else {
                                        CommentMediaKind::Image
                                    },
                                    thumb
                                        .and_then(|v| v.get("width"))
                                        .and_then(Value::as_u64)
                                        .map(|v| v as u32),
                                    thumb
                                        .and_then(|v| v.get("height"))
                                        .and_then(Value::as_u64)
                                        .map(|v| v as u32),
                                );
                            }
                        }
                        if key != "authorThumbnail" {
                            walk(child, &child_path, out);
                        }
                    }
                }
                Value::Array(items) => {
                    for child in items {
                        walk(child, path, out);
                    }
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        walk(value, "", &mut out);
        out
    }
    fn walk(value: &Value, comments: &mut Vec<CommentItem>) {
        if let Some(comment) = value.get("commentRenderer") {
            let author = comment
                .get("authorText")
                .map(extract_text)
                .unwrap_or_default();
            let text = comment
                .get("contentText")
                .map(extract_text)
                .unwrap_or_default();
            let media = media(comment);
            if !author.is_empty() && (!text.is_empty() || !media.is_empty()) {
                let buttons = comment.pointer("/actionButtons/commentActionButtonsRenderer");
                let like_button = buttons.and_then(|b| b.get("likeButton"));
                let dislike_button = buttons.and_then(|b| b.get("dislikeButton"));
                let detail_token = comment
                    .get("detailViewEndpoint")
                    .and_then(continuation)
                    .or_else(|| continuation(comment));
                let reply_count = comment
                    .get("replyCount")
                    .and_then(Value::as_u64)
                    .or_else(|| {
                        comment
                            .get("replyCount")
                            .map(extract_text)
                            .and_then(|text| {
                                text.split_whitespace()
                                    .next()?
                                    .replace(',', "")
                                    .parse::<u64>()
                                    .ok()
                            })
                    })
                    .map(|n| n as u32);
                comments.push(CommentItem {
                    id: comment
                        .get("commentId")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .into(),
                    author,
                    text,
                    author_thumb: comment
                        .get("authorThumbnail")
                        .map(extract_best_thumbnail)
                        .filter(|url| !url.is_empty()),
                    published: comment.get("publishedTimeText").map(extract_text),
                    likes: comment.get("voteCount").map(extract_text),
                    reply_count,
                    media,
                    is_liked: comment
                        .get("isLiked")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    is_disliked: dislike_button
                        .and_then(|b| find_json_key(b, "isToggled"))
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    detail_token: detail_token.clone(),
                    like_action: button_action(like_button, false),
                    unlike_action: button_action(like_button, true),
                    dislike_action: button_action(dislike_button, false),
                    undislike_action: button_action(dislike_button, true),
                    reply_params: action(comment, "createReplyParams"),
                    // Current TV list rows use detailViewEndpoint for both the
                    // focused comment and its reply thread, matching SmartTube.
                    replies_token: reply_count.filter(|n| *n > 0).and(detail_token),
                });
            }
            return;
        }
        if let Some(entity) = value.get("commentEntityPayload") {
            let properties = entity.get("properties").unwrap_or(&Value::Null);
            let text = properties
                .get("content")
                .map(extract_text)
                .unwrap_or_default();
            let author = entity
                .pointer("/author/displayName")
                .map(extract_text)
                .unwrap_or_default();
            let media = media(entity);
            if !author.is_empty() && (!text.is_empty() || !media.is_empty()) {
                comments.push(CommentItem {
                    id: properties
                        .get("commentId")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .into(),
                    author,
                    text,
                    author_thumb: entity
                        .pointer("/avatar/image")
                        .map(extract_best_thumbnail)
                        .filter(|url| !url.is_empty()),
                    published: properties.get("publishedTime").map(extract_text),
                    likes: entity
                        .pointer("/toolbar/likeCountNotliked")
                        .map(extract_text),
                    reply_count: entity
                        .pointer("/toolbar/replyCount")
                        .and_then(Value::as_u64)
                        .map(|n| n as u32),
                    media,
                    is_liked: entity
                        .pointer("/toolbar/likeState")
                        .and_then(Value::as_str)
                        .is_some_and(|v| v.eq_ignore_ascii_case("liked")),
                    is_disliked: false,
                    detail_token: continuation(entity),
                    like_action: None,
                    unlike_action: None,
                    dislike_action: None,
                    undislike_action: None,
                    reply_params: action(entity, "createReplyParams"),
                    replies_token: continuation(entity),
                });
            }
            return;
        }
        if let Some(object) = value.as_object() {
            for value in object.values() {
                walk(value, comments);
            }
        }
        if let Some(array) = value.as_array() {
            for value in array {
                walk(value, comments);
            }
        }
    }
    let mut comments = Vec::new();
    walk(val, &mut comments);
    let mut ids = std::collections::HashSet::new();
    comments.retain(|c| ids.insert(c.id.clone()));
    CommentsBatch {
        comments,
        continuation: parse_continuation(val).or_else(|| {
            find_json_key(val, "nextContinuationData")
                .and_then(|v| v.get("continuation"))
                .and_then(Value::as_str)
                .map(str::to_string)
        }),
        total_count: find_json_key(val, "commentsHeaderRenderer")
            .and_then(|v| v.get("countText"))
            .map(extract_text),
        create_params: action(val, "createCommentParams"),
    }
}

fn find_channel_browse_id(value: &Value) -> Option<&str> {
    if let Some(id) = value
        .pointer("/browseEndpoint/browseId")
        .and_then(Value::as_str)
        .filter(|id| id.starts_with("UC"))
    {
        return Some(id);
    }
    match value {
        Value::Object(map) => map.values().find_map(find_channel_browse_id),
        Value::Array(items) => items.iter().find_map(find_channel_browse_id),
        _ => None,
    }
}

pub fn parse_channel_page(value: &Value, id: &str) -> crate::backend::models::ChannelPage {
    use crate::backend::models::{ChannelLink, ChannelPage};
    fn find<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
        match value {
            Value::Object(map) => map
                .get(key)
                .or_else(|| map.values().find_map(|v| find(v, key))),
            Value::Array(items) => items.iter().find_map(|v| find(v, key)),
            _ => None,
        }
    }
    fn tabs(value: &Value, id: &str, out: &mut Vec<ChannelLink>) {
        if let Some(tab) = value
            .get("tabRenderer")
            .or_else(|| value.get("expandableTabRenderer"))
        {
            if let Some(endpoint) = tab.pointer("/endpoint/browseEndpoint") {
                if endpoint.get("browseId").and_then(Value::as_str) == Some(id) {
                    let title = tab.get("title").map(extract_text).unwrap_or_default();
                    let params = endpoint
                        .get("params")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    if !title.is_empty() && !out.iter().any(|t| t.params == params) {
                        out.push(ChannelLink {
                            title,
                            params,
                            selected: tab
                                .get("selected")
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        });
                    }
                }
            }
            return;
        }
        match value {
            Value::Object(map) => {
                for child in map.values() {
                    tabs(child, id, out);
                }
            }
            Value::Array(items) => {
                for child in items {
                    tabs(child, id, out);
                }
            }
            _ => {}
        }
    }
    let metadata = find(value, "channelMetadataRenderer").unwrap_or(&Value::Null);
    let header = find(value, "c4TabbedHeaderRenderer")
        .or_else(|| find(value, "channelHeaderRenderer"))
        .unwrap_or(&Value::Null);
    let subtitle = header.get("subtitle").map(extract_text).unwrap_or_default();
    let mut channel = ChannelCard {
        id: id.to_string(),
        title: metadata
            .get("title")
            .or_else(|| header.get("title"))
            .map(extract_text)
            .unwrap_or_default(),
        handle: header
            .get("channelHandleText")
            .map(extract_text)
            .or_else(|| {
                metadata
                    .get("vanityChannelUrl")
                    .and_then(Value::as_str)
                    .and_then(|u| u.rsplit('/').next())
                    .filter(|s| s.starts_with('@'))
                    .map(str::to_string)
            }),
        subscribers: header
            .get("subscriberCountText")
            .map(extract_text)
            .or_else(|| {
                subtitle
                    .split(['•', '·'])
                    .map(str::trim)
                    .find(|s| s.contains("subscriber"))
                    .map(str::to_string)
            }),
        avatar_url: metadata
            .get("avatar")
            .or_else(|| header.get("avatar"))
            .or_else(|| header.get("thumbnail"))
            .or_else(|| find(value, "avatarViewModel").and_then(|v| v.get("image")))
            .map(extract_best_thumbnail)
            .filter(|s| !s.is_empty()),
        verified: false,
    };
    if channel.handle.is_none() {
        channel.handle = subtitle
            .split_whitespace()
            .find(|part| part.starts_with('@'))
            .map(|s| s.trim_end_matches(['•', '·']).to_string());
    }
    let mut links = Vec::new();
    tabs(value, id, &mut links);
    let subscribed = find(header, "subscribeButtonRenderer")
        .and_then(|v| v.get("subscribed"))
        .and_then(Value::as_bool);
    // Only the selected tab is the active feed; inactive tab bodies are not concatenated.
    fn selected_content(value: &Value) -> Option<&Value> {
        if let Some(tab) = value
            .get("tabRenderer")
            .or_else(|| value.get("expandableTabRenderer"))
        {
            return tab
                .get("selected")
                .and_then(Value::as_bool)
                .filter(|s| *s)
                .and_then(|_| tab.get("content"));
        }
        match value {
            Value::Object(map) => map.values().find_map(selected_content),
            Value::Array(items) => items.iter().find_map(selected_content),
            _ => None,
        }
    }
    ChannelPage {
        channel,
        tabs: links,
        subscribed,
        feed: parse_feed_page(selected_content(value).unwrap_or(value)),
    }
}

fn typed_live_state(value: &Value) -> LiveState {
    fn contains(value: &Value, predicate: fn(&str, &Value) -> bool) -> bool {
        match value {
            Value::Object(map) => map
                .iter()
                .any(|(key, v)| predicate(key, v) || contains(v, predicate)),
            Value::Array(items) => items.iter().any(|v| contains(v, predicate)),
            _ => false,
        }
    }
    if contains(value, |key, v| {
        key == "upcomingEventData"
            || key == "isUpcoming" && v.as_bool() == Some(true)
            || key == "contentType" && v.as_str().is_some_and(|s| s.contains("UPCOMING"))
    }) {
        return LiveState::Upcoming;
    }
    if contains(value, |key, v| {
        (key == "isLive" || key == "isLiveNow") && v.as_bool() == Some(true)
            || key == "style"
                && v.as_str()
                    .is_some_and(|s| s == "LIVE" || s.contains("LIVE_NOW"))
            || key == "contentType" && v.as_str().is_some_and(|s| s.contains("LIVE"))
    }) {
        return LiveState::Live;
    }
    LiveState::None
}

/// The add-to-playlist endpoint returns actual destinations for this account/video.
pub fn parse_playlist_destinations(value: &Value) -> FeedPage {
    fn walk(value: &Value, out: &mut Vec<Card>) {
        if let Some(p) = value.get("playlistAddToOptionRenderer") {
            if p.get("containsSelectedVideos").and_then(Value::as_str) == Some("ALL")
                || p.get("isEditable").and_then(Value::as_bool) == Some(false)
            {
                return;
            }
            if let (Some(id), Some(title)) =
                (p.get("playlistId").and_then(Value::as_str), p.get("title"))
            {
                let title = extract_text(title);
                if !id.is_empty() && !title.is_empty() && !out.iter().any(|c| c.id() == id) {
                    out.push(Card::Playlist(crate::backend::models::PlaylistCard {
                        id: id.into(),
                        title,
                        channel: String::new(),
                        video_count: None,
                        thumb_url: String::new(),
                        is_mix: false,
                        first_video_id: None,
                        updated: None,
                    }));
                }
            }
            return;
        }
        match value {
            Value::Object(map) => {
                for v in map.values() {
                    walk(v, out)
                }
            }
            Value::Array(items) => {
                for v in items {
                    walk(v, out)
                }
            }
            _ => {}
        }
    }
    let mut items = Vec::new();
    walk(value, &mut items);
    FeedPage {
        items,
        chips: vec![],
        continuation: None,
    }
}
