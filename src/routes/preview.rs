use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, StatusCode},
    response::Response,
};
use serde::Deserialize;

use crate::{error::AppError, AppState};

#[derive(Deserialize)]
pub struct PreviewQuery {
    pub url: String,
}

pub async fn handler(
    State(state): State<AppState>,
    Query(params): Query<PreviewQuery>,
) -> Result<Response, AppError> {
    if !params.url.starts_with("https://video.twimg.com/") {
        return Err(AppError::InvalidUrl);
    }

    let upstream = state
        .client
        .get(&params.url)
        .header("Referer", "https://twitter.com/")
        .send()
        .await?;

    if !upstream.status().is_success() {
        return Err(AppError::TwitterApi(format!(
            "CDN returned {}",
            upstream.status()
        )));
    }

    let content_type = upstream
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("video/mp4")
        .to_string();

    let content_length = upstream.headers().get(header::CONTENT_LENGTH).cloned();

    let bytes = upstream.bytes().await?;

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "public, max-age=3600");

    if let Some(cl) = content_length {
        builder = builder.header(header::CONTENT_LENGTH, cl);
    }

    let response = builder
        .body(Body::from(bytes))
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(response)
}