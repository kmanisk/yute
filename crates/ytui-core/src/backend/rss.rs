use crate::backend::models::{Card, FeedPage, LiveState, VideoCard};
use anyhow::Result;
use reqwest::Client;
use std::time::Duration;

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct RssItem {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub channel_id: String,
    pub published: String,
    pub thumb_url: String,
}

#[allow(dead_code)]
pub struct RssBackend {
    client: Client,
    channels: Vec<String>,
}

impl RssBackend {
    pub fn new(channels: Vec<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self { client, channels }
    }

    pub async fn fetch_channel_feed(&self, channel_id: &str) -> Result<Vec<VideoCard>> {
        let url = format!("https://www.youtube.com/feeds/videos.xml?channel_id={channel_id}");
        let text = self.client.get(&url).send().await?.text().await?;

        // Simple XML tag parsing for YouTube Atom feeds
        let mut videos = Vec::new();
        let entries: Vec<&str> = text.split("<entry>").skip(1).collect();

        for entry in entries {
            let id = extract_xml_tag(entry, "yt:videoId")
                .or_else(|| extract_xml_tag(entry, "id"))
                .unwrap_or_default();
            let title = extract_xml_tag(entry, "title").unwrap_or_default();
            let channel = extract_xml_tag(entry, "name").unwrap_or_default();
            let published = extract_xml_tag(entry, "published").unwrap_or_default();
            let thumb_url = extract_xml_attr(entry, "media:thumbnail", "url")
                .unwrap_or_else(|| format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg"));

            if !id.is_empty() && !title.is_empty() {
                videos.push(VideoCard {
                    id,
                    title,
                    channel,
                    channel_id: Some(channel_id.to_string()),
                    verified: false,
                    duration: None,
                    views: None,
                    published: Some(published),
                    thumb_url,
                    progress_pct: None,
                    live: LiveState::None,
                    badges: Vec::new(),
                    resume_time_sec: None,
                });
            }
        }
        Ok(videos)
    }

    pub async fn fetch_all(&self) -> Result<FeedPage> {
        let mut cards = Vec::new();
        // Fetch channels concurrently (up to 8)
        for chunk in self.channels.chunks(8) {
            let mut tasks = Vec::new();
            for ch in chunk {
                let ch_clone = ch.clone();
                let client = self.client.clone();
                tasks.push(tokio::spawn(async move {
                    let backend = RssBackend { client, channels: Vec::new() };
                    backend.fetch_channel_feed(&ch_clone).await
                }));
            }
            for t in tasks {
                if let Ok(Ok(vids)) = t.await {
                    for v in vids {
                        cards.push(Card::Video(v));
                    }
                }
            }
        }

        // Sort by published descending if available
        cards.sort_by(|a, b| {
            let pub_a = match a {
                Card::Video(v) => v.published.as_deref().unwrap_or(""),
                _ => "",
            };
            let pub_b = match b {
                Card::Video(v) => v.published.as_deref().unwrap_or(""),
                _ => "",
            };
            pub_b.cmp(pub_a)
        });

        Ok(FeedPage {
            chips: Vec::new(),
            items: cards,
            continuation: None,
        })
    }
}

#[allow(dead_code)]
fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim().to_string())
}

#[allow(dead_code)]
fn extract_xml_attr(xml: &str, tag: &str, attr: &str) -> Option<String> {
    let tag_start = xml.find(&format!("<{tag}"))?;
    let tag_end = xml[tag_start..].find('>')? + tag_start;
    let tag_content = &xml[tag_start..tag_end];
    let attr_key = format!("{attr}=\"");
    let attr_start = tag_content.find(&attr_key)? + attr_key.len();
    let attr_end = tag_content[attr_start..].find('"')? + attr_start;
    Some(tag_content[attr_start..attr_end].trim().to_string())
}
