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
    models::{DownloadRequest, SyndicationTweet, TweetRef},
    services::download::{
        merge_mp4s, original_reply_video, promoted_video, promoted_video_url, quoted_video,
    },
    services::overlay,
    services::twitter::{
        extract_tweet_id, fetch_quoted_tweet, fetch_tweet, fetch_tweet_cached,
        parse_variants, tweet_ref_from,
    },
    AppState,
};

/// If the tweet is a reply with no video of its own, the parent is the
/// real content — use the parent's data for the overlay card.
async fn resolve_overlay_tref(
    client: &reqwest::Client,
    tweet: &SyndicationTweet,
) -> TweetRef {
    if parse_variants(tweet).is_err() {
        if let Some(parent_id) = &tweet.in_reply_to_status_id_str {
            if let Ok(parent) = fetch_tweet(client, parent_id).await {
                return tweet_ref_from(&parent);
            }
        }
    }
    tweet_ref_from(tweet)
}

pub async fn handler(
    State(state): State<AppState>,
    Json(body): Json<DownloadRequest>,
) -> Result<Response, AppError> {
    let tweet_id = extract_tweet_id(&body.url)?;
    let tweet = fetch_tweet_cached(&state.client, &state.tweet_cache, &tweet_id).await?;

    let filename = format!("{}_{}.mp4", tweet.user.screen_name, tweet_id);

    // ── No caption mode ──────────────────────────────────────────────────
    if !body.render_card {
        // When quote is requested without caption, download the quoted tweet's video raw
        if body.include_quote {
            let (video_bytes, author) = quoted_video(&state.client, &tweet, body.quality.as_deref()).await?
                .ok_or(AppError::NoVideo)?;
            let qt_id = tweet.quoted_tweet.as_ref()
                .and_then(|q| q.id_str.as_deref())
                .unwrap_or(&tweet_id);
            let filename = format!("{}_{}.mp4", author, qt_id);
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "video/mp4")
                .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename))
                .body(Body::from(video_bytes))
                .map_err(|e| AppError::Internal(e.into()));
        }

        // Fast path: stream single main/promoted video from CDN
        let url = promoted_video_url(&state.client, &tweet, body.quality.as_deref()).await?;
        let resp = state.client.get(&url).send().await?;
        let stream = resp.bytes_stream().map(|r| r.map_err(|e| anyhow::anyhow!(e)));

        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "video/mp4")
            .header(
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            )
            .body(Body::from_stream(stream))
            .map_err(|e| AppError::Internal(e.into()));
    }

    // Get the main video. For quote mode, fall back to the quoted tweet's
    // video if the outer tweet has none of its own.
    let main = match promoted_video(&state.client, &tweet, body.quality.as_deref()).await {
        Ok(v) => v,
        Err(AppError::NoVideo) if body.include_quote => {
            match quoted_video(&state.client, &tweet, body.quality.as_deref()).await? {
                Some((v, _)) => v,
                None => return Err(AppError::NoVideo),
            }
        }
        Err(e) => return Err(e),
    };

    let result = if body.render_card {
        let outer_tref = resolve_overlay_tref(&state.client, &tweet).await;

        if body.include_quote {
            // ── Quote tweet layout ────────────────────────────────────
            // Outer tweet card on top + quoted tweet's video + quote box + footer.
            let quoted_tref = fetch_quoted_tweet(&state.client, &tweet)
                .await
                .ok_or_else(|| AppError::Internal(
                    anyhow::anyhow!("no quoted tweet found")
                ))?;

            let display_video = match quoted_video(&state.client, &tweet, body.quality.as_deref()).await? {
                Some((v, _)) => v,
                None => main,
            };

            overlay::apply_quote_overlay(
                &state.client,
                &outer_tref,
                &quoted_tref,
                display_video,
                &tweet_id,
            ).await?

        } else if body.include_reply {
            // ── Reply thread layout ───────────────────────────────────
            // Main tweet card + video + reply card below.
            let reply_tref = tweet_ref_from(&tweet);
            overlay::apply_combined_overlays(
                &state.client,
                Some(&outer_tref),
                Some(&reply_tref),
                main,
                outer_tref.variants.first()
                    .map(|v| {
                        // use vid_w from variant label — overlay probes it anyway
                        let _ = v;
                        1280i32
                    })
                    .unwrap_or(1280),
                &tweet_id,
            ).await?

        } else {
            // ── Single card overlay ───────────────────────────────────
            overlay::apply_caption_card(&state.client, &outer_tref, main).await?
        }

    } else {
        // ── No overlay — merge whatever was requested ─────────────────
        let mut videos = vec![main];

        if body.include_quote {
            if let Some((v, _)) =
                quoted_video(&state.client, &tweet, body.quality.as_deref()).await?
            {
                videos.push(v);
            }
        }

        if body.include_reply {
            if let Some(v) =
                original_reply_video(&state.client, &tweet, body.quality.as_deref()).await?
            {
                videos.push(v);
            }
        }

        merge_mp4s(videos).await?
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .header(header::CONTENT_LENGTH, result.len())
        .body(Body::from(result))
        .map_err(|e| AppError::Internal(e.into()))
}