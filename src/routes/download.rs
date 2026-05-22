use std::process::Stdio;

use axum::{
    body::Body,
    extract::State,
    http::{header, StatusCode},
    response::Response,
    Json,
};
use bytes::Bytes;
use futures::StreamExt;
use tokio::process::Command;

use crate::{
    error::AppError,
    models::{DownloadRequest, SyndicationTweet},
    services::download::{
        merge_mp4s, original_reply_video, promoted_video, promoted_video_url, quoted_video,
    },
    services::overlay,
    services::twitter::{
        extract_tweet_id, fetch_tweet_cached, tweet_ref_from,
    },
    AppState,
};

/// Concatenates videos with re-encoding (concat filter), handling
/// mismatched codecs/streams that the stream-copy merge_mp4s cannot.
async fn concat_videos_reencode(videos: Vec<Bytes>) -> Result<Bytes, AppError> {
    let dir = std::env::temp_dir().join(format!(
        "twdl_concat_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).map_err(|e| AppError::Internal(e.into()))?;

    let mut paths = Vec::new();
    for (i, v) in videos.iter().enumerate() {
        let path = dir.join(format!("{}.mp4", i));
        std::fs::write(&path, v).map_err(|e| AppError::Internal(e.into()))?;
        paths.push(path);
    }

    let out_path = dir.join("out.mp4");

    let mut args: Vec<String> = vec!["-y".into()];
    for p in &paths {
        args.push("-i".into());
        args.push(p.to_str().unwrap().to_string());
    }

    let n = videos.len();
    let mut filter = String::new();
    for i in 0..n {
        filter.push_str(&format!("[{}:v:0][{}:a:0]", i, i));
    }
    filter.push_str(&format!("concat=n={}:v=1:a=1[v][a]", n));

    args.extend([
        "-filter_complex".into(), filter,
        "-map".into(), "[v]".into(),
        "-map".into(), "[a]".into(),
        "-c:v".into(), "libx264".into(),
        "-preset".into(), "fast".into(),
        "-crf".into(), "20".into(),
        "-c:a".into(), "aac".into(),
        "-movflags".into(), "+faststart".into(),
        out_path.to_str().unwrap().to_string(),
    ]);

    let output = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| AppError::Ffmpeg(format!("ffmpeg reencode concat failed: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_dir_all(&dir);
        return Err(AppError::Ffmpeg(format!("ffmpeg reencode concat failed:\n{}", stderr)));
    }

    let result = std::fs::read(&out_path).map_err(|e| AppError::Internal(e.into()))?;
    let _ = std::fs::remove_dir_all(&dir);
    Ok(Bytes::from(result))
}

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

    let main = promoted_video(&state.client, &tweet, body.quality.as_deref()).await?;

    // helper: probe raw video bytes for width
    let probe_width = |bytes: &Bytes| -> i32 {
        let dir = std::env::temp_dir().join(format!("twdl_pw_{}", tweet_id));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("v.mp4");
        let _ = std::fs::write(&p, bytes);
        let out = std::process::Command::new("ffprobe")
            .args(["-v", "error", "-select_streams", "v:0",
                   "-show_entries", "stream=width", "-of", "csv=p=0",
                   p.to_str().unwrap()])
            .output().ok();
        let _ = std::fs::remove_dir_all(&dir);
        out.and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            s.parse::<i32>().ok()
        }).unwrap_or(1280)
    };

    let result = if body.render_card || body.include_reply {
        // ── identify top (caption) and bottom (reply) tweets ────────
        let (caption_tweet, reply_tweet_opt) = if let Some(parent) = &tweet.parent {
            (parent.as_ref() as &SyndicationTweet, Some(&tweet))
        } else {
            (&tweet, None)
        };

        let vid_w = probe_width(&main);
        let mut final_video = main;

        // ── both checked + reply tweet → combined overlay ──────────
        if body.render_card && body.include_reply && reply_tweet_opt.is_some() {
            let top_tref = tweet_ref_from(caption_tweet);
            let bot_tref = tweet_ref_from(reply_tweet_opt.unwrap());
            final_video = overlay::apply_combined_overlays(
                &state.client, Some(&top_tref), Some(&bot_tref),
                final_video, vid_w, &tweet_id,
            ).await?;
        }
        // ── fallback for single-option or normal tweets ────────────
        else {
            if body.render_card {
                let ct = tweet_ref_from(caption_tweet);
                final_video = overlay::apply_caption_card(&state.client, &ct, final_video).await?;
            }
            if body.include_reply {
                if let Some(reply) = reply_tweet_opt {
                    let rt = tweet_ref_from(reply);
                    final_video = overlay::apply_combined_overlays(
                        &state.client, None, Some(&rt),
                        final_video, vid_w, &tweet_id,
                    ).await?;
                }
            }
        }

        let mut parts = vec![final_video];

        if body.include_quote {
            if let Some((v, _)) =
                quoted_video(&state.client, &tweet, body.quality.as_deref()).await?
            {
                parts.push(v);
            }
        }

        if parts.len() == 1 { parts.into_iter().next().unwrap() }
        else { concat_videos_reencode(parts).await? }
    } else {
        // No overlay, no reply — just merge whatever was requested
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

        if videos.len() == 1 { videos.into_iter().next().unwrap() }
        else { merge_mp4s(videos).await? }
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
