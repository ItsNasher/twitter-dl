use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::Response,
    Json,
};
use bytes::Bytes;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::{
    error::AppError,
    models::DownloadRequest,
    services::{
        twitter::{extract_tweet_id, fetch_tweet, parse_variants},
        video::{download_mp4, pick_variant},
    },
    AppState,
};

pub async fn handler(
    State(state): State<AppState>,
    Json(body): Json<DownloadRequest>,
) -> Result<Response, AppError> {
    let tweet_id = extract_tweet_id(&body.url)?;
    let tweet = fetch_tweet(&state.client, &tweet_id).await?;
    let variants = parse_variants(&tweet)?;
    let variant = pick_variant(&variants, body.quality.as_deref());

    let mp4_bytes = download_mp4(&state.client, variant).await?;
    let m4a_bytes = extract_audio(mp4_bytes).await?;

    let filename = format!("{}_{}.m4a", tweet.user.screen_name, tweet_id);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "audio/mp4")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .header(header::CONTENT_LENGTH, m4a_bytes.len())
        .body(Body::from(m4a_bytes))
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(response)
}

async fn extract_audio(mp4_bytes: Bytes) -> Result<Bytes, AppError> {
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

    let mut stdin = tokio::io::BufWriter::new(
        child.stdin.take().expect("stdin piped")
    );

    stdin.write_all(&mp4_bytes).await
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