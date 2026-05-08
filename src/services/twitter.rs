use anyhow::Context;
use reqwest::Client;
use regex::Regex;

use crate::error::AppError;
use crate::models::*;

const SYNDICATION_URL: &str = "https://cdn.syndication.twimg.com/tweet-result";

pub fn extract_tweet_id(url: &str) -> Result<String, AppError> {
    let re = Regex::new(r"(?:twitter\.com|x\.com)/[^/]+/status/(\d+)")
        .expect("regex is valid");
    re.captures(url)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or(AppError::InvalidUrl)
}

fn compute_token(tweet_id: &str) -> String {
    let id: f64 = tweet_id.parse::<u64>().unwrap_or(0) as f64;
    let val = (id / 1e15) * std::f64::consts::PI;
    let int_part = val as u64;
    let mut result = to_base36(int_part);
    result.retain(|c| c != '.');
    let result = result.trim_start_matches('0').to_string();
    if result.is_empty() { "0".to_string() } else { result }
}

fn to_base36(mut n: u64) -> String {
    if n == 0 { return "0".to_string(); }
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut buf = Vec::new();
    while n > 0 {
        buf.push(DIGITS[(n % 36) as usize] as char);
        n /= 36;
    }
    buf.iter().rev().collect()
}

pub async fn fetch_tweet(client: &Client, tweet_id: &str) -> Result<SyndicationTweet, AppError> {
    let token = compute_token(tweet_id);
    let url = format!(
        "{}?id={}&lang=en&token={}&features=tfw_timeline_list%3A%3Btfw_follower_count_sunset%3Atrue",
        SYNDICATION_URL, tweet_id, token
    );

    tracing::debug!("Fetching tweet {} with token={}", tweet_id, token);

    let resp = client
        .get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .header("Accept", "application/json")
        .header("Referer", "https://platform.twitter.com/")
        .send()
        .await?;

    if resp.status() == 404 {
        return Err(AppError::TweetNotFound);
    }
    if !resp.status().is_success() {
        return Err(AppError::TwitterApi(format!(
            "syndication API returned {}", resp.status()
        )));
    }

    let text = resp.text().await?;
    tracing::debug!("Syndication full response: {}", &text);

    if text.trim() == "{}" || text.trim().is_empty() {
        return Err(AppError::TweetNotFound);
    }

    let tweet: SyndicationTweet = serde_json::from_str(&text)
        .context("failed to parse syndication response")?;

    tracing::debug!(
        "Parsed tweet: extended_entities={} media_details={}",
        tweet.extended_entities.is_some(),
        tweet.media_details.as_ref().map(|v| v.len()).unwrap_or(0)
    );

    Ok(tweet)
}

pub async fn fetch_reply_parent(
    client: &Client,
    tweet: &SyndicationTweet,
) -> Option<TweetRef> {
    let id = tweet.in_reply_to_status_id_str.as_deref()?;
    let parent = fetch_tweet(client, id).await.ok()?;
    Some(TweetRef {
        author: parent.user.screen_name,
        text: parent.full_text,
    })
}

pub fn parse_variants(tweet: &SyndicationTweet) -> Result<Vec<VideoVariant>, AppError> {
    let media = tweet.video_media().ok_or(AppError::NoVideo)?;

    let video_item = media
        .iter()
        .find(|m| {
            let t = m.media_type.to_lowercase();
            t.contains("video") || t == "animated_gif"
        })
        .ok_or(AppError::NoVideo)?;

    let video_info = video_item.video_info.as_ref().ok_or(AppError::NoVideo)?;

    tracing::debug!("Found {} variants", video_info.variants.len());
    for v in &video_info.variants {
        tracing::debug!("  variant: content_type={:?} bitrate={:?} url={}", v.content_type, v.bitrate, &v.url[..v.url.len().min(80)]);
    }

    let mut variants: Vec<VideoVariant> = video_info
        .variants
        .iter()
        .filter(|v| v.content_type != "application/x-mpegURL" && !v.url.is_empty())
        .map(|v| {
            let bitrate = v.bitrate.unwrap_or(0);
            VideoVariant {
                label: bitrate_to_label(bitrate),
                url: v.url.clone(),
                bitrate,
            }
        })
        .collect();

    variants.sort_by(|a, b| b.bitrate.cmp(&a.bitrate));
    variants.dedup_by(|a, b| a.label == b.label);

    if variants.is_empty() {
        return Err(AppError::NoVideo);
    }

    Ok(variants)
}

pub fn find_subtitle_url(tweet: &SyndicationTweet) -> Option<String> {
    let media = tweet.video_media()?;
    let video_item = media.iter()
        .find(|m| m.media_type.to_lowercase().contains("video"))?;
    let video_info = video_item.video_info.as_ref()?;
    let subtitles = video_info.subtitles.as_ref()?;

    subtitles
        .iter()
        .find(|s| s.language.starts_with("en"))
        .or_else(|| subtitles.first())
        .map(|s| s.url.clone())
}

pub async fn build_tweet_info(
    client: &Client,
    tweet: &SyndicationTweet,
) -> Result<TweetInfo, AppError> {
    let variants = parse_variants(tweet)?;

    let quoted_tweet = tweet.quoted_tweet.as_ref().map(|q| TweetRef {
        author: q.user.screen_name.clone(),
        text: q.full_text.clone(),
    });

    let in_reply_to = if tweet.in_reply_to_status_id_str.is_some() {
        fetch_reply_parent(client, tweet).await
    } else {
        None
    };

    Ok(TweetInfo {
        author: tweet.user.screen_name.clone(),
        created_at: format_date(&tweet.created_at),
        text: tweet.full_text.clone(),
        quoted_tweet,
        in_reply_to,
        variants,
    })
}

fn bitrate_to_label(bitrate: u64) -> String {
    match bitrate {
        b if b >= 5_000_000 => "1080p".to_string(),
        b if b >= 1_500_000 => "720p".to_string(),
        b if b >= 600_000  => "480p".to_string(),
        b if b > 0         => "360p".to_string(),
        _                    => "low".to_string(),
    }
}

fn format_date(raw: &str) -> String {
    use chrono::DateTime;
    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return dt.format("%b %-d, %Y").to_string();
    }
    if let Ok(dt) = DateTime::parse_from_str(raw, "%a %b %d %H:%M:%S %z %Y") {
        return dt.format("%b %-d, %Y").to_string();
    }
    raw.to_string()
}