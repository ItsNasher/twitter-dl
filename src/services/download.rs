use bytes::Bytes;
use reqwest::Client;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::error::AppError;
use crate::models::SyndicationTweet;
use crate::services::captions::fetch_and_convert_captions;
use crate::services::twitter::{
    fetch_quoted_tweet, fetch_tweet, find_subtitle_url, parse_variants,
};
use crate::services::video::{download_mp4, pick_variant};

/// Fast path: resolve just the download URL (handles reply promotion).
/// Avoids downloading the full video — used for streaming responses.
pub async fn promoted_video_url(
    client: &Client,
    tweet: &SyndicationTweet,
    quality: Option<&str>,
) -> Result<String, AppError> {
    if parse_variants(tweet).is_err() {
        if let Some(parent_id) = &tweet.in_reply_to_status_id_str {
            if let Ok(parent) = fetch_tweet(client, parent_id).await {
                if let Ok(variants) = parse_variants(&parent) {
                    let variant = pick_variant(&variants, quality);
                    return Ok(variant.url.clone());
                }
            }
        }
    }
    let variants = parse_variants(tweet)?;
    let variant = pick_variant(&variants, quality);
    Ok(variant.url.clone())
}

pub async fn main_video(
    client: &Client,
    tweet: &SyndicationTweet,
    quality: Option<&str>,
) -> Result<Bytes, AppError> {
    let variants = parse_variants(tweet)?;
    let variant = pick_variant(&variants, quality);
    download_mp4(client, variant).await
}

/// Gets the video to serve as the main download.
/// If the given tweet is a reply (has in_reply_to_status_id_str),
/// fetches the parent tweet's video instead.
pub async fn promoted_video(
    client: &Client,
    tweet: &SyndicationTweet,
    quality: Option<&str>,
) -> Result<Bytes, AppError> {
    if parse_variants(tweet).is_err() {
        if let Some(parent_id) = &tweet.in_reply_to_status_id_str {
            if let Ok(parent) = fetch_tweet(client, parent_id).await {
                if let Ok(variants) = parse_variants(&parent) {
                    let variant = pick_variant(&variants, quality);
                    return download_mp4(client, variant).await;
                }
            }
        }
    }
    main_video(client, tweet, quality).await
}

/// If the tweet is a reply, tries to get the original reply's video.
/// Returns None if the reply has no video or if this isn't a reply.
pub async fn original_reply_video(
    client: &Client,
    tweet: &SyndicationTweet,
    quality: Option<&str>,
) -> Result<Option<Bytes>, AppError> {
    if tweet.in_reply_to_status_id_str.is_none() {
        return Ok(None);
    }
    match main_video(client, tweet, quality).await {
        Ok(v) => Ok(Some(v)),
        Err(AppError::NoVideo) => Ok(None),
        Err(e) => Err(e),
    }
}

pub async fn quoted_video(
    client: &Client,
    tweet: &SyndicationTweet,
    quality: Option<&str>,
) -> Result<Option<(Bytes, String)>, AppError> {
    let quoted = fetch_quoted_tweet(client, tweet).await;
    match quoted {
        Some(ref t) if !t.variants.is_empty() => {
            let v = pick_variant(&t.variants, quality);
            let bytes = download_mp4(client, v).await?;
            Ok(Some((bytes, t.author.clone())))
        }
        _ => Ok(None),
    }
}

pub async fn promoted_captions(client: &Client, tweet: &SyndicationTweet) -> Result<String, AppError> {
    if let Some(parent_id) = &tweet.in_reply_to_status_id_str {
        if let Ok(parent) = fetch_tweet(client, parent_id).await {
            if let Ok(s) = main_captions(client, &parent).await {
                return Ok(s);
            }
        }
    }
    main_captions(client, tweet).await
}

pub async fn original_reply_captions(client: &Client, tweet: &SyndicationTweet) -> Result<Option<String>, AppError> {
    if tweet.in_reply_to_status_id_str.is_none() {
        return Ok(None);
    }
    match main_captions(client, tweet).await {
        Ok(s) => Ok(Some(s)),
        Err(AppError::NoCaptions) => Ok(None),
        Err(e) => Err(e),
    }
}

pub async fn main_captions(client: &Client, tweet: &SyndicationTweet) -> Result<String, AppError> {
    let vtt_url = find_subtitle_url(tweet).ok_or(AppError::NoCaptions)?;
    let srt = fetch_and_convert_captions(client, &vtt_url).await?;
    Ok(String::from_utf8_lossy(&srt).to_string())
}

pub async fn quoted_captions(client: &Client, tweet: &SyndicationTweet) -> Result<Option<String>, AppError> {
    let quoted = match &tweet.quoted_tweet {
        Some(q) => q,
        None => return Ok(None),
    };
    let quoted_id = match &quoted.id_str {
        Some(id) => id,
        None => return Ok(None),
    };
    let quoted_tweet = fetch_tweet(client, quoted_id).await?;
    let vtt_url = match find_subtitle_url(&quoted_tweet) {
        Some(u) => u,
        None => return Ok(None),
    };
    let srt = fetch_and_convert_captions(client, &vtt_url).await?;
    Ok(Some(String::from_utf8_lossy(&srt).to_string()))
}

pub async fn merge_mp4s(videos: Vec<Bytes>) -> Result<Bytes, AppError> {
    match videos.len() {
        0 => Err(AppError::NoVideo),
        1 => Ok(videos.into_iter().next().unwrap()),
        _ => {
            let dir = std::env::temp_dir().join(format!(
                "twdl_{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&dir)
                .map_err(|e| AppError::Internal(e.into()))?;

            let mut files = Vec::new();
            for (i, v) in videos.iter().enumerate() {
                let path = dir.join(format!("{}.mp4", i));
                std::fs::write(&path, v)
                    .map_err(|e| AppError::Internal(e.into()))?;
                files.push(path);
            }

            let list_path = dir.join("list.txt");
            let list: String = files
                .iter()
                .map(|p| {
                    let s = p.display().to_string().replace('\'', "'\\''");
                    format!("file '{}'", s)
                })
                .collect::<Vec<_>>()
                .join("\n");
            std::fs::write(&list_path, &list)
                .map_err(|e| AppError::Internal(e.into()))?;

            let output = Command::new("ffmpeg")
                .args([
                    "-f", "concat",
                    "-safe", "0",
                    "-i", list_path.to_str().unwrap(),
                    "-c", "copy",
                    "-movflags", "faststart",
                    "-f", "mp4",
                    "pipe:1",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
                .await
                .map_err(|e| AppError::Ffmpeg(format!("ffmpeg merge failed: {}", e)))?;

            let _ = std::fs::remove_dir_all(&dir);

            if !output.status.success() {
                return Err(AppError::Ffmpeg("ffmpeg concat failed".into()));
            }

            Ok(Bytes::from(output.stdout))
        }
    }
}

pub fn merge_srt_captions(captions: Vec<String>) -> String {
    let mut result = String::new();
    let mut index = 1u32;

    for cap in captions {
        for block in cap.split("\n\n") {
            let block = block.trim();
            if block.is_empty() {
                continue;
            }
            let lines: Vec<&str> = block.lines().collect();
            if lines.len() < 2 {
                continue;
            }
            if lines[0].parse::<u32>().is_err() {
                continue;
            }
            result.push_str(&format!("{}\n", index));
            for line in &lines[1..] {
                result.push_str(line);
                result.push('\n');
            }
            result.push('\n');
            index += 1;
        }
    }

    result.trim().to_string()
}

pub async fn extract_audio(mp4_bytes: Bytes) -> Result<Bytes, AppError> {
    let mut child = Command::new("ffmpeg")
        .args([
            "-i", "pipe:0",
            "-vn",
            "-acodec", "copy",
            "-f", "mp4",
            "pipe:1",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| AppError::Ffmpeg(format!("failed to spawn ffmpeg: {}", e)))?;

    let mut stdin = tokio::io::BufWriter::new(child.stdin.take().expect("stdin piped"));
    stdin
        .write_all(&mp4_bytes)
        .await
        .map_err(|e| AppError::Ffmpeg(format!("stdin write failed: {}", e)))?;
    stdin.flush().await.ok();
    drop(stdin);

    let output = child
        .wait_with_output()
        .await
        .map_err(|e| AppError::Ffmpeg(format!("ffmpeg wait failed: {}", e)))?;

    if !output.status.success() {
        return Err(AppError::Ffmpeg(format!(
            "ffmpeg exited with code {}",
            output.status.code().unwrap_or(-1)
        )));
    }

    Ok(Bytes::from(output.stdout))
}
