use anyhow::{Context};
use bytes::Bytes;
use futures::future;
use reqwest::Client;
use std::process::Stdio;
use tokio::process::Command;

use crate::error::AppError;
use crate::models::VideoVariant;

pub fn pick_variant<'a>(variants: &'a [VideoVariant], quality: Option<&str>) -> &'a VideoVariant {
    if let Some(q) = quality {
        if let Some(v) = variants.iter().find(|v| v.label == q) {
            return v;
        }
    }
    &variants[0]
}
pub async fn download_mp4(client: &Client, variant: &VideoVariant) -> Result<Bytes, AppError> {
    if variant.url.contains(".m3u8") || variant.url.contains("/pl/") {
        let ts_bytes = fetch_hls_segments(client, &variant.url).await?;
        remux_to_mp4(ts_bytes).await
    } else {
        let resp = client
            .get(&variant.url)
            .send()
            .await?
            .bytes()
            .await?;
        Ok(resp)
    }
}

async fn fetch_hls_segments(client: &Client, m3u8_url: &str) -> Result<Bytes, AppError> {
    let playlist = client
        .get(m3u8_url)
        .send()
        .await?
        .text()
        .await?;

    let base = base_url(m3u8_url);
    let segment_urls: Vec<String> = playlist
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            if l.starts_with("http") {
                l.to_string()
            } else {
                format!("{}/{}", base.trim_end_matches('/'), l.trim_start_matches('/'))
            }
        })
        .collect();

    if segment_urls.is_empty() {
        return Err(AppError::NoVideo);
    }

    tracing::debug!("Downloading {} .ts segments", segment_urls.len());

    let chunks = segment_urls.chunks(8);
    let mut all_bytes: Vec<u8> = Vec::new();

    for chunk in chunks {
        let fetches: Vec<_> = chunk
            .iter()
            .map(|url| {
                let client = client.clone();
                let url = url.clone();
                async move {
                    client
                        .get(&url)
                        .send()
                        .await?
                        .bytes()
                        .await
                }
            })
            .collect();

        let results = future::join_all(fetches).await;

        for result in results {
            let bytes = result.context("failed to fetch .ts segment")?;
            all_bytes.extend_from_slice(&bytes);
        }
    }

    Ok(Bytes::from(all_bytes))
}

async fn remux_to_mp4(ts_bytes: Bytes) -> Result<Bytes, AppError> {
    let mut child = Command::new("ffmpeg")
        .args([
            "-i", "pipe:0",
            "-c", "copy",  
            "-movflags", "faststart",
            "-f", "mp4",
            "pipe:1", 
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| AppError::Ffmpeg(format!("failed to spawn ffmpeg: {}", e)))?;

    let stdin = child.stdin.take().expect("stdin piped");

    // write input bytes
    use tokio::io::AsyncWriteExt;
    let mut stdin = tokio::io::BufWriter::new(stdin);
    stdin.write_all(&ts_bytes).await
        .map_err(|e| AppError::Ffmpeg(format!("failed to write to ffmpeg stdin: {}", e)))?;
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

    if output.stdout.is_empty() {
        return Err(AppError::Ffmpeg("ffmpeg produced no output".into()));
    }

    Ok(Bytes::from(output.stdout))
}

fn base_url(url: &str) -> String {
    url.rfind('/')
        .map(|i| url[..=i].to_string())
        .unwrap_or_else(|| url.to_string())
}