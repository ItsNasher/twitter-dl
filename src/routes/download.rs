use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::Response,
    Json,
};
use futures::StreamExt;

use crate::{
    error::AppError,
    models::DownloadRequest,
    services::download::{
        merge_mp4s, original_reply_video, promoted_video, promoted_video_url, quoted_video,
    },
    services::overlay,
    services::twitter::{extract_tweet_id, fetch_tweet_cached, tweet_ref_from},
    AppState,
};

pub async fn handler(
    State(state): State<AppState>,
    Json(body): Json<DownloadRequest>,
) -> Result<Response, AppError> {
    let tweet_id = extract_tweet_id(&body.url)?;
    let tweet = fetch_tweet_cached(&state.client, &state.tweet_cache, &tweet_id).await?;

    let filename = format!("{}_{}.mp4", tweet.user.screen_name, tweet_id);

    // ── Fast path: single video, no overlay → stream from Twitter CDN ─────
    if !body.render_card && !body.include_quote && !body.include_reply {
        let url = promoted_video_url(&state.client, &tweet, body.quality.as_deref()).await?;
        let resp = state.client.get(&url).send().await?;
        let stream = resp.bytes_stream().map(|r| r.map_err(|e| anyhow::anyhow!(e)));
        let body = Body::from_stream(stream);

        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "video/mp4")
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            )
            .body(body)
            .map_err(|e| AppError::Internal(e.into()));
    }

    // normal path
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

    let result = if body.render_card {
        let tref = tweet_ref_from(&tweet);
        overlay::apply_tweet_overlay(&state.client, merged, &tref, &tweet_id).await?
    } else {
        merged
    };

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .header(header::CONTENT_LENGTH, result.len())
        .body(Body::from(result))
        .map_err(|e| AppError::Internal(e.into()))?;

    Ok(response)
}
