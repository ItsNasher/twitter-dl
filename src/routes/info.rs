use axum::{extract::State, Json};

use crate::{
    error::AppError,
    models::{InfoRequest, TweetInfo},
    services::twitter::{build_tweet_info, extract_tweet_id, fetch_tweet_cached},
    AppState,
};

pub async fn handler(
    State(state): State<AppState>,
    Json(body): Json<InfoRequest>,
) -> Result<Json<TweetInfo>, AppError> {
    let tweet_id = extract_tweet_id(&body.url)?;
    let tweet = fetch_tweet_cached(&state.client, &state.tweet_cache, &tweet_id).await?;
    let info = build_tweet_info(&state.client, &tweet).await?;

    Ok(Json(info))
}