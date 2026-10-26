use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SponsorSegment {
    pub category: String,
    #[serde(rename = "actionType")]
    pub action_type: String,
    pub segment: (f64, f64),
    #[serde(rename = "UUID")]
    pub uuid: String,
}

#[derive(Clone)]
pub struct SponsorBlockClient {
    client: Client,
    cache: Arc<RwLock<HashMap<String, Vec<SponsorSegment>>>>,
    server: String,
    categories: Vec<String>,
}

impl Default for SponsorBlockClient {
    fn default() -> Self {
        Self::new(None, None)
    }
}

impl SponsorBlockClient {
    pub fn new(server: Option<&str>, categories: Option<Vec<String>>) -> Self {
        let server = server
            .unwrap_or("https://sponsor.ajay.app")
            .trim_end_matches('/')
            .to_string();

        let categories = categories.unwrap_or_else(|| {
            vec![
                "sponsor".to_string(),
                "selfpromo".to_string(),
                "interaction".to_string(),
                "intro".to_string(),
                "outro".to_string(),
                "preview".to_string(),
                "filler".to_string(),
            ]
        });

        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            server,
            categories,
        }
    }

    pub async fn fetch_segments(&self, video_id: &str) -> Result<Vec<SponsorSegment>> {
        if video_id.is_empty() {
            return Ok(Vec::new());
        }

        // Check in-memory cache
        {
            let guard = self.cache.read().await;
            if let Some(segments) = guard.get(video_id) {
                return Ok(segments.clone());
            }
        }

        let cat_json = serde_json::to_string(&self.categories).unwrap_or_else(|_| "[]".to_string());
        let url = format!("{}/api/skipSegments", self.server);

        let resp = match self
            .client
            .get(&url)
            .query(&[("videoID", video_id), ("categories", &cat_json)])
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!("SponsorBlock query failed for {video_id}: {e}");
                return Ok(Vec::new());
            }
        };

        if resp.status().as_u16() == 404 {
            let mut guard = self.cache.write().await;
            guard.insert(video_id.to_string(), Vec::new());
            return Ok(Vec::new());
        }

        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let segments: Vec<SponsorSegment> = resp
            .json()
            .await
            .context("Failed to parse SponsorBlock response")
            .unwrap_or_default();

        let mut guard = self.cache.write().await;
        guard.insert(video_id.to_string(), segments.clone());
        Ok(segments)
    }
}

pub struct SponsorBlockTracker {
    pub video_id: String,
    pub segments: Vec<SponsorSegment>,
    pub skipped_uuids: HashSet<String>,
    pub last_skip_instant: Option<Instant>,
    pub cooldown_seconds: f64,
}

impl SponsorBlockTracker {
    pub fn new(video_id: &str, segments: Vec<SponsorSegment>) -> Self {
        Self {
            video_id: video_id.to_string(),
            segments,
            skipped_uuids: HashSet::new(),
            last_skip_instant: None,
            cooldown_seconds: 1.25,
        }
    }

    pub fn check_position(&mut self, current_pos: f64) -> Option<(f64, String)> {
        // Enforce cooldown to prevent seek loop / underrun storms
        if let Some(last) = self.last_skip_instant {
            if last.elapsed().as_secs_f64() < self.cooldown_seconds {
                return None;
            }
        }

        for s in &self.segments {
            let (start, end) = s.segment;
            if current_pos >= start && current_pos < end {
                if self.skipped_uuids.contains(&s.uuid) {
                    continue;
                }

                self.skipped_uuids.insert(s.uuid.clone());
                self.last_skip_instant = Some(Instant::now());

                let target = end + 0.1;
                let duration = (end - start).max(0.0);
                let msg = format!("Skipped {} ({:.1}s)", s.category, duration);
                return Some((target, msg));
            }
        }

        None
    }

    pub fn on_manual_seek(&mut self, target_pos: f64) {
        // Reset segments that are ahead of our seek destination
        self.skipped_uuids.retain(|uuid| {
            if let Some(seg) = self.segments.iter().find(|s| s.uuid == *uuid) {
                target_pos >= seg.segment.0 - 2.0
            } else {
                false
            }
        });
        self.last_skip_instant = None;
    }
}
