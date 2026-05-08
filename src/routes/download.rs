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

    tracing::info!(
        "Downloading video: {} quality={} url={}",
        tweet_id,
        variant.label,
        variant.url
    );

    let mp4_bytes = download_mp4(&state.client, variant).await?;

    let filename = format!(
        "{}_{}.mp4",
        tweet.user.screen_name,
        tweet_id
    );

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .header(header::CONTENT_LENGTH, mp4_bytes.len())
        .body(Body::from(mp4_bytes))
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(response)
}