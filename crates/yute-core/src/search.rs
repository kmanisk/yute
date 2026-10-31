//! Search request fields observed in MediaServiceCore's search parameter fixtures.
//! This encoder is independent of that implementation; no renderer text ranking.
use base64::{engine::general_purpose::URL_SAFE, Engine};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Video,
    Channel,
    Playlist,
    Movie,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Duration {
    Short,
    Medium,
    Long,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Date {
    Today,
    Week,
    Month,
    Year,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Feature {
    Live,
    FourK,
    Hdr,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Options {
    pub kind: Option<Kind>,
    pub duration: Option<Duration>,
    pub date: Option<Date>,
    pub features: Vec<Feature>,
    pub popularity: bool,
}
fn varint(mut n: u64, out: &mut Vec<u8>) {
    while n >= 128 {
        out.push(n as u8 | 128);
        n >>= 7;
    }
    out.push(n as u8);
}
fn field(number: u64, value: u64, out: &mut Vec<u8>) {
    varint(number << 3, out);
    varint(value, out);
}
impl Options {
    pub fn params(&self) -> Option<String> {
        let mut filters = vec![];
        if let Some(d) = self.date {
            field(
                1,
                match d {
                    Date::Today => 2,
                    Date::Week => 3,
                    Date::Month => 4,
                    Date::Year => 5,
                },
                &mut filters,
            );
        }
        if let Some(k) = self.kind {
            field(
                2,
                match k {
                    Kind::Video => 1,
                    Kind::Channel => 2,
                    Kind::Playlist => 3,
                    Kind::Movie => 4,
                },
                &mut filters,
            );
        }
        if let Some(d) = self.duration {
            field(
                3,
                match d {
                    Duration::Short => 1,
                    Duration::Medium => 3,
                    Duration::Long => 2,
                },
                &mut filters,
            );
        }
        for (flag, f) in [(8, Feature::Live), (14, Feature::FourK), (25, Feature::Hdr)] {
            if self.features.contains(&f) {
                field(flag, 1, &mut filters);
            }
        }
        let mut outer = vec![];
        if self.popularity {
            field(1, 3, &mut outer);
        }
        if !filters.is_empty() {
            outer.push(18);
            varint(filters.len() as u64, &mut outer);
            outer.extend(filters);
        }
        (!outer.is_empty()).then(|| URL_SAFE.encode(outer))
    }
    pub fn from_legacy(f: &crate::backend::models::SearchFilters) -> Self {
        Self {
            kind: match f.filter_type.as_deref() {
                Some("video" | "videos") => Some(Kind::Video),
                Some("channel" | "channels") => Some(Kind::Channel),
                Some("playlist" | "playlists") => Some(Kind::Playlist),
                _ => None,
            },
            duration: match f.duration.as_deref() {
                Some("short") => Some(Duration::Short),
                Some("medium") => Some(Duration::Medium),
                Some("long") => Some(Duration::Long),
                _ => None,
            },
            ..Self::default()
        }
    }
    pub fn biased(
        &self,
        query: &str,
        config: &crate::config::SearchConfig,
        disabled: bool,
    ) -> (Self, bool) {
        let automatic = !disabled
            && config.music_bias == "auto"
            && self.kind.is_none()
            && self.duration.is_none()
            && music_intent(query, &config.music_words);
        let mut result = self.clone();
        if automatic {
            result.kind = Some(Kind::Video);
            // TV medium starts at four minutes. The requested music range is
            // applied from duration metadata, without changing server ranking.
        }
        (result, automatic)
    }
    pub fn labels(&self) -> Vec<String> {
        let mut v = vec![];
        if let Some(x) = self.kind {
            v.push(format!("{x:?}"));
        }
        if let Some(x) = self.duration {
            v.push(format!("duration:{x:?}"));
        }
        if let Some(x) = self.date {
            v.push(format!("date:{x:?}"));
        }
        for f in &self.features {
            v.push(format!("{f:?}"));
        }
        if self.popularity {
            v.push("Popularity".into());
        }
        v
    }
}
pub fn music_intent(query: &str, words: &[String]) -> bool {
    let q = format!(
        " {} ",
        query
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    );
    words
        .iter()
        .any(|s| q.contains(&format!(" {} ", s.to_lowercase())))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn observed_protocol_vectors() {
        for (o, expected) in [
            (
                Options {
                    kind: Some(Kind::Video),
                    ..Default::default()
                },
                "EgIQAQ==",
            ),
            (
                Options {
                    kind: Some(Kind::Playlist),
                    ..Default::default()
                },
                "EgIQAw==",
            ),
            (
                Options {
                    features: vec![Feature::Live, Feature::FourK, Feature::Hdr],
                    ..Default::default()
                },
                "EgdAAXAByAEB",
            ),
            (
                Options {
                    date: Some(Date::Today),
                    kind: Some(Kind::Video),
                    duration: Some(Duration::Medium),
                    ..Default::default()
                },
                "EgYIAhABGAM=",
            ),
        ] {
            assert_eq!(o.params().as_deref(), Some(expected));
        }
    }
    #[test]
    fn music_uses_words_not_local_ranking() {
        let c = crate::config::SearchConfig::default();
        for q in [
            "koi mil gya song",
            "16 shots songs",
            "MUSIC video",
            "movie ost",
            "video song",
        ] {
            assert!(music_intent(q, &c.music_words));
        }
        for q in ["songbird migration", "linux kernel", "mpv seeks"] {
            assert!(!music_intent(q, &c.music_words));
        }
        let o = Options::default();
        assert!(o.biased("koi mil gaya song", &c, false).1);
        assert!(!o.biased("song", &c, true).1);
        assert!(
            !Options {
                kind: Some(Kind::Channel),
                ..o
            }
            .biased("song", &c, false)
            .1
        );
    }
}

/// TV's primary paged result shelf precedes unpaged alternative shelves. Preserve
/// server order and continuation; do not interpret localized titles as semantics.
pub fn parse_page(value: &serde_json::Value) -> crate::backend::models::FeedPage {
    fn find<'a>(v: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
        if let Some(x) = v.get(key) {
            return Some(x);
        }
        match v {
            serde_json::Value::Object(o) => o.values().find_map(|v| find(v, key)),
            serde_json::Value::Array(a) => a.iter().find_map(|v| find(v, key)),
            _ => None,
        }
    }
    if let Some(c) = value.pointer("/continuationContents/horizontalListContinuation") {
        return crate::backend::parse::parse_feed_page(c);
    }
    if let Some(sections) = find(value, "sectionListRenderer")
        .and_then(|s| s.get("contents"))
        .and_then(serde_json::Value::as_array)
    {
        if let Some(primary) = sections.iter().find_map(|s| s.get("shelfRenderer")) {
            return crate::backend::parse::parse_feed_page(primary);
        }
    }
    crate::backend::parse::parse_feed_page(value)
}
#[cfg(test)]
mod page_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn only_primary_tv_shelf_not_alternative_recommendations() {
        let card = |id: &str, title: &str| json!({"videoRenderer":{"videoId":id,"title":{"simpleText":title}}});
        let value = json!({"contents":{"sectionListRenderer":{"contents":[{"shelfRenderer":{"content":{"horizontalListRenderer":{"items":[card("a","Koi Mil Gaya music video"),card("b","Full Movie upload")],"continuations":[{"nextContinuationData":{"continuation":"page2"}}]}}}},{"shelfRenderer":{"content":{"horizontalListRenderer":{"items":[card("rec","Unrelated alternative shelf")]}}}}]}}});
        let p = parse_page(&value);
        assert_eq!(p.items.len(), 2);
        assert_eq!(p.items[0].id(), "a");
        assert_eq!(p.continuation.as_deref(), Some("page2"));
    }
}

/// Metadata-only music range; unknown durations remain visible.
pub fn music_duration_matches(duration: Option<&str>) -> bool {
    let Some(s) = duration else {
        return true;
    };
    let mut seconds = 0u32;
    for part in s.split(':') {
        let Ok(n) = part.parse::<u32>() else {
            return true;
        };
        seconds = seconds.saturating_mul(60).saturating_add(n);
    }
    (180..=1200).contains(&seconds)
}
