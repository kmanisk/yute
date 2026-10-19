use crate::backend::models::{Card, ChannelCard, Chip, FeedPage, LiveState, PlaylistCard, VideoCard, WatchData};
use serde_json::Value;

pub fn extract_text(val: &Value) -> String {
    if let Some(s) = val.as_str() {
        return s.trim().to_string();
    }
    if let Some(s) = val.get("simpleText").and_then(|v| v.as_str()) {
        return s.trim().to_string();
    }
    if let Some(runs) = val.get("runs").and_then(|v| v.as_array()) {
        let texts: Vec<&str> = runs
            .iter()
            .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
            .collect();
        return texts.join("").trim().to_string();
    }
    if let Some(content) = val.get("content").and_then(|v| v.as_str()) {
        return content.trim().to_string();
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
    if let Some(rows) = meta.get("metadataRows").and_then(|r| r.as_array()) {
        if let Some(first_row) = rows.first() {
            channel = extract_text(first_row);
        }
        if rows.len() > 1 {
            for row in &rows[1..] {
                let text = extract_text(row);
                if !text.is_empty() {
                    stats.push(text);
                }
            }
        }
    }

    let thumb_url = v.get("image")
        .and_then(|img| img.get("imageViewModel"))
        .map(extract_best_thumbnail)
        .unwrap_or_else(|| format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"));

    Some(Card::Video(VideoCard {
        id,
        title,
        channel,
        channel_id: None,
        verified: false,
        duration: None,
        views: stats.first().cloned(),
        published: if stats.len() > 1 { Some(stats[1].clone()) } else { None },
        thumb_url,
        progress_pct: None,
        live: LiveState::None,
        badges: Vec::new(),
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

pub fn parse_card(val: &Value) -> Option<Card> {
    if let Some(rich) = val.get("richItemRenderer") {
        return parse_card(rich.get("content")?);
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
    if let Some(lvm) = val.get("lockupViewModel") {
        return parse_lockup_view_model(lvm);
    }
    if let Some(pr) = val.get("playlistRenderer") {
        return parse_playlist_renderer(pr).map(Card::Playlist);
    }
    if let Some(gpr) = val.get("gridPlaylistRenderer") {
        return parse_playlist_renderer(gpr).map(Card::Playlist);
    }
    if let Some(rr) = val.get("radioRenderer") {
        return parse_playlist_renderer(rr).map(Card::Playlist);
    }
    if let Some(cr) = val.get("channelRenderer") {
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
        for (k, v) in obj {
            // Skip richSectionRenderer shelves like Shorts
            if k == "richSectionRenderer" {
                continue;
            }
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

    // Look for chat token
    if let Some(tok) = val.pointer("/contents/twoColumnWatchNextResults/conversationBar/liveChatRenderer/continuations/0/reloadContinuationData/continuation")
        .and_then(|s| s.as_str())
    {
        chat_token = Some(tok.to_string());
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
    })
}
