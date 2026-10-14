use crate::types::{ProviderError, ProviderResult, ProviderTrack};
use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const QUOTA_COOLDOWN: Duration = Duration::from_secs(24 * 3600);
const RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(3600);
const SCRAPE_MAX_FAILURES: u32 = 5;

fn quota_reset_iso() -> String {
    use chrono::TimeZone;
    let la = chrono_tz::America::Los_Angeles;
    let tomorrow = (chrono::Utc::now() + chrono::Duration::days(1))
        .with_timezone(&la)
        .date_naive();
    la.from_local_datetime(&tomorrow.and_hms_opt(0, 0, 0).unwrap())
        .single()
        .map(|d| d.to_rfc3339())
        .unwrap_or_default()
}

fn quota_error() -> ProviderError {
    let mut e = ProviderError::new(
        "youtube",
        "La recherche YouTube est temporairement indisponible - Le quota quotidien est dépassé",
        429,
    );
    e.quota_exceeded = true;
    e.reset_time = Some(quota_reset_iso());
    e
}

fn rate_limit_error() -> ProviderError {
    let mut e = ProviderError::new(
        "youtube",
        "YouTube search is temporarily unavailable - Rate limit reached",
        429,
    );
    e.rate_limited = true;
    e.reset_time = Some(quota_reset_iso());
    e
}

pub struct YouTubeApiClient {
    http: reqwest::Client,
    api_key: String,
    app_url: String,
    quota_blocked_until: Mutex<Option<Instant>>,
}

impl YouTubeApiClient {
    pub fn new(api_key: &str, app_url: &str) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            api_key: api_key.to_string(),
            app_url: app_url.to_string(),
            quota_blocked_until: Mutex::new(None),
        }
    }

    fn is_quota_exceeded(&self) -> bool {
        self.quota_blocked_until
            .lock()
            .unwrap()
            .map(|until| Instant::now() < until)
            .unwrap_or(false)
    }

    fn mark_quota_exceeded(&self) {
        *self.quota_blocked_until.lock().unwrap() = Some(Instant::now() + QUOTA_COOLDOWN);
    }

    pub async fn search_track(&self, term: &str) -> ProviderResult {
        if term.trim().is_empty() {
            return Ok(vec![]);
        }
        if self.is_quota_exceeded() {
            return Err(quota_error());
        }
        let resp = self
            .http
            .get("https://www.googleapis.com/youtube/v3/search")
            .header("Referer", &self.app_url)
            .query(&[
                ("key", self.api_key.as_str()),
                ("part", "snippet"),
                ("q", format!("{} music", term.trim()).as_str()),
                ("type", "video"),
                ("videoCategoryId", "10"),
                ("maxResults", "5"),
                ("videoEmbeddable", "true"),
                (
                    "fields",
                    "items(id/videoId,snippet(title,channelTitle,publishedAt,thumbnails/default))",
                ),
            ])
            .send()
            .await
            .map_err(|e| {
                tracing::error!(term, error = %e, "youtube api search failed");
                ProviderError::new("youtube", "YouTube service is temporarily unavailable", 500)
            })?;

        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        if status == 403 || status == 429 {
            if body.to_lowercase().contains("quota") || status == 429 {
                self.mark_quota_exceeded();
                return Err(quota_error());
            }
            return Err(ProviderError::new("youtube", "API access denied", status));
        }
        if !(200..300).contains(&status) {
            return Err(ProviderError::new("youtube", "Service unavailable", status));
        }

        let j = serde_json::from_str::<Value>(&body).unwrap_or_default();
        let tracks = j["items"]
            .as_array()
            .map(|a| a.iter().map(format_track).collect())
            .unwrap_or_default();
        Ok(tracks)
    }
}

const USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (iPhone; CPU iPhone OS 17_2 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/120.0.6099.119 Mobile/15E148 Safari/604.1",
];

pub struct YouTubeScrapeClient {
    http: reqwest::Client,
    rate_limited_until: Mutex<Option<Instant>>,
    failed_attempts: Mutex<u32>,
}

impl Default for YouTubeScrapeClient {
    fn default() -> Self {
        Self::new()
    }
}

impl YouTubeScrapeClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            rate_limited_until: Mutex::new(None),
            failed_attempts: Mutex::new(0),
        }
    }

    fn is_rate_limited(&self) -> bool {
        self.rate_limited_until
            .lock()
            .unwrap()
            .map(|until| Instant::now() < until)
            .unwrap_or(false)
    }

    fn mark_rate_limited(&self) {
        *self.rate_limited_until.lock().unwrap() =
            Some(Instant::now() + RATE_LIMIT_COOLDOWN);
    }

    fn increment_failures(&self) {
        let mut n = self.failed_attempts.lock().unwrap();
        *n += 1;
        if *n >= SCRAPE_MAX_FAILURES {
            self.mark_rate_limited();
        }
    }

    pub async fn search_track(&self, term: &str) -> ProviderResult {
        if term.trim().is_empty() {
            return Ok(vec![]);
        }
        if self.is_rate_limited() {
            return Err(rate_limit_error());
        }

        use rand::prelude::IndexedRandom;
        let ua = USER_AGENTS
            .choose(&mut rand::rng())
            .copied()
            .unwrap_or(USER_AGENTS[0]);
        let query = term.trim().replace(' ', "+");
        let resp = self
            .http
            .get("https://www.youtube.com/results")
            .header("User-Agent", ua)
            .header("Accept-Language", "en-US,en;q=0.9")
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/webp,*/*;q=0.8",
            )
            .header("Referer", "https://www.youtube.com")
            .header("DNT", "1")
            .query(&[("search_query", query.as_str())])
            .send()
            .await;

        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                self.increment_failures();
                tracing::error!(term, error = %e, "youtube scrape failed");
                return Err(ProviderError::new(
                    "youtube",
                    "YouTube service is temporarily unavailable",
                    500,
                ));
            }
        };

        let status = resp.status().as_u16();
        let html = resp.text().await.unwrap_or_default();
        if !(200..300).contains(&status) {
            self.increment_failures();
            if status == 403 || status == 429 || html.contains("denied") {
                self.mark_rate_limited();
                return Err(rate_limit_error());
            }
            return Err(ProviderError::new(
                "youtube",
                "YouTube service is temporarily unavailable",
                status,
            ));
        }

        let results = parse_search_results(&html);
        if results.is_empty() {
            self.increment_failures();
            return Err(ProviderError::new("youtube", "No results found", 404));
        }
        *self.failed_attempts.lock().unwrap() = 0;
        Ok(results)
    }
}

fn parse_search_results(html: &str) -> Vec<ProviderTrack> {
    let marker = "var ytInitialData = ";
    let Some(start) = html.find(marker).map(|i| i + marker.len()) else {
        return vec![];
    };
    let bytes = html.as_bytes();
    let mut depth = 0i32;
    let mut end = None;
    let mut in_str = false;
    let mut esc = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if esc {
                esc = false;
            } else if b == b'\\' {
                esc = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i + 1);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(end) = end else { return vec![] };
    let data: Value = serde_json::from_str(&html[start..end]).unwrap_or_default();

    let contents = data["contents"]["twoColumnSearchResultsRenderer"]["primaryContents"]
        ["sectionListRenderer"]["contents"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    'outer: for section in contents {
        for item in section["itemSectionRenderer"]["contents"]
            .as_array()
            .cloned()
            .unwrap_or_default()
        {
            let v = &item["videoRenderer"];
            let video_id = v["videoId"].as_str().unwrap_or_default();
            let title = v["title"]["runs"][0]["text"].as_str().unwrap_or_default();
            if video_id.is_empty() || title.is_empty() {
                continue;
            }
            let pseudo_track = serde_json::json!({
                "id": {"videoId": video_id},
                "snippet": {
                    "title": title,
                    "channelTitle": v["ownerText"]["runs"][0]["text"].as_str().unwrap_or_default(),
                    "publishedAt": v["publishedTimeText"]["simpleText"].as_str().unwrap_or_default(),
                    "thumbnails": {"default": {"url": v["thumbnail"]["thumbnails"][0]["url"]}},
                },
            });
            out.push(format_track(&pseudo_track));
            if out.len() >= 5 {
                break 'outer;
            }
        }
    }
    out
}

fn format_track(t: &Value) -> ProviderTrack {
    let title = t["snippet"]["title"].as_str().unwrap_or_default();
    let (artist, name) = match title.split_once(" - ") {
        Some((a, n)) => (a.trim().to_string(), n.trim().to_string()),
        None => (
            t["snippet"]["channelTitle"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            title.to_string(),
        ),
    };
    let video_id = t["id"]["videoId"]
        .as_str()
        .or_else(|| t["snippet"]["resourceId"]["videoId"].as_str())
        .unwrap_or_default();
    ProviderTrack {
        provider: "youtube".to_string(),
        provider_id: video_id.to_string(),
        provider_url: Some(format!("https://www.youtube.com/watch?v={video_id}")),
        provider_popularity: 0,
        artist_name: Some(artist),
        track_name: name,
        album_name: None,
        preview_url: Some(video_id.to_string()),
        release_date: t["snippet"]["publishedAt"]
            .as_str()
            .map(|d| d.chars().take(10).collect()),
        artwork_url: t["snippet"]["thumbnails"]["default"]["url"]
            .as_str()
            .map(str::to_string),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html_with_initial_data(json: &str) -> String {
        format!("<html><script>var ytInitialData = {json};</script></html>")
    }

    #[test]
    fn parses_video_renderers() {
        let html = html_with_initial_data(
            r#"{"contents":{"twoColumnSearchResultsRenderer":{"primaryContents":{"sectionListRenderer":{"contents":[{"itemSectionRenderer":{"contents":[
              {"videoRenderer":{"videoId":"abc123","title":{"runs":[{"text":"Artist - Song"}]},"ownerText":{"runs":[{"text":"Chan"}]},"publishedTimeText":{"simpleText":"2023-01-01"},"thumbnail":{"thumbnails":[{"url":"https://i.ytimg.com/t.jpg"}]}}},
              {"videoRenderer":{"videoId":"def456","title":{"runs":[{"text":"NoSeparator"}]},"ownerText":{"runs":[{"text":"ChanB"}]},"thumbnail":{"thumbnails":[]}}}
            ]}}]}}}}}"#,
        );
        let tracks = parse_search_results(&html);
        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].provider_id, "abc123");
        assert_eq!(tracks[0].artist_name.as_deref(), Some("Artist"));
        assert_eq!(tracks[0].track_name, "Song");
        assert_eq!(tracks[0].artwork_url.as_deref(), Some("https://i.ytimg.com/t.jpg"));
        assert_eq!(tracks[1].artist_name.as_deref(), Some("ChanB"));
        assert_eq!(tracks[1].track_name, "NoSeparator");
    }

    #[test]
    fn braces_inside_strings_do_not_break_parsing() {
        let html = html_with_initial_data(
            r#"{"contents":{"twoColumnSearchResultsRenderer":{"primaryContents":{"sectionListRenderer":{"contents":[{"itemSectionRenderer":{"contents":[
              {"videoRenderer":{"videoId":"v1","title":{"runs":[{"text":"A } { weird"}]},"thumbnail":{"thumbnails":[]}}}
            ]}}]}}}}}"#,
        );
        let tracks = parse_search_results(&html);
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].provider_id, "v1");
    }

    #[test]
    fn caps_at_five_results() {
        let items: Vec<String> = (0..8)
            .map(|i| format!(
                r#"{{"videoRenderer":{{"videoId":"v{i}","title":{{"runs":[{{"text":"T{i}"}}]}},"thumbnail":{{"thumbnails":[]}}}}}}"#
            ))
            .collect();
        let html = html_with_initial_data(&format!(
            r#"{{"contents":{{"twoColumnSearchResultsRenderer":{{"primaryContents":{{"sectionListRenderer":{{"contents":[{{"itemSectionRenderer":{{"contents":[{}]}}}}]}}}}}}}}}}"#,
            items.join(",")
        ));
        assert_eq!(parse_search_results(&html).len(), 5);
    }

    #[test]
    fn missing_marker_returns_empty() {
        assert!(parse_search_results("<html></html>").is_empty());
    }
}
