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
    services::download::{
        merge_srt_captions, original_reply_captions, promoted_captions, quoted_captions,
    },
    services::twitter::{extract_tweet_id, fetch_tweet_cached},
    AppState,
};

pub async fn handler(
    State(state): State<AppState>,
    Json(body): Json<DownloadRequest>,
) -> Result<Response, AppError> {
    let tweet_id = extract_tweet_id(&body.url)?;
    let tweet = fetch_tweet_cached(&state.client, &state.tweet_cache, &tweet_id).await?;

    let mut captions = Vec::new();

    match promoted_captions(&state.client, &tweet).await {
        Ok(s) => captions.push(s),
        Err(AppError::NoCaptions) => {}
        Err(e) => return Err(e),
    }

    if body.include_quote {
        if let Some(s) = quoted_captions(&state.client, &tweet).await? {
            captions.push(s);
        }
    }

    if body.include_reply {
        if let Some(s) = original_reply_captions(&state.client, &tweet).await? {
            captions.push(s);
        }
    }

    if captions.is_empty() {
        return Err(AppError::NoCaptions);
    }

    let srt = merge_srt_captions(captions);

    let filename = format!("{}_{}.srt", tweet.user.screen_name, tweet_id);

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .header(header::CONTENT_LENGTH, srt.len())
        .body(Body::from(srt.into_bytes()))
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(response)
}
