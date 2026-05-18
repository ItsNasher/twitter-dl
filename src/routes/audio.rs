use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::Response,
    Json,
};

use crate::{
    error::AppError,
    models::DownloadRequest,
    services::download::{extract_audio, merge_mp4s, original_reply_video, promoted_video, quoted_video},
    services::twitter::{extract_tweet_id, fetch_tweet_cached},
    AppState,
};

pub async fn handler(
    State(state): State<AppState>,
    Json(body): Json<DownloadRequest>,
) -> Result<Response, AppError> {
    let tweet_id = extract_tweet_id(&body.url)?;
    let tweet = fetch_tweet_cached(&state.client, &state.tweet_cache, &tweet_id).await?;

    let mut videos = Vec::new();

    let main = promoted_video(&state.client, &tweet, body.quality.as_deref()).await?;
    videos.push(main);

    if body.include_quote {
        if let Some((v, _)) = quoted_video(&state.client, &tweet, body.quality.as_deref()).await? {
            videos.push(v);
        }
    }

    if body.include_reply {
        if let Some(v) = original_reply_video(&state.client, &tweet, body.quality.as_deref()).await? {
            videos.push(v);
        }
    }

    let merged = merge_mp4s(videos).await?;
    let m4a_bytes = extract_audio(merged).await?;

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
