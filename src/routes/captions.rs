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
    services::{
        captions::fetch_and_convert_captions,
        twitter::{extract_tweet_id, fetch_tweet, find_subtitle_url},
    },
    AppState,
};

pub async fn handler(
    State(state): State<AppState>,
    Json(body): Json<DownloadRequest>,
) -> Result<Response, AppError> {
    let tweet_id = extract_tweet_id(&body.url)?;
    let tweet = fetch_tweet(&state.client, &tweet_id).await?;

    let vtt_url = find_subtitle_url(&tweet).ok_or(AppError::NoCaptions)?;

    tracing::info!("Fetching captions for tweet {}", tweet_id);

    let srt_bytes = fetch_and_convert_captions(&state.client, &vtt_url).await?;

    let filename = format!("{}_{}.srt", tweet.user.screen_name, tweet_id);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .header(header::CONTENT_LENGTH, srt_bytes.len())
        .body(Body::from(srt_bytes))
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(response)
}