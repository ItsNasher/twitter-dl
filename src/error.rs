use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Twitter API error: {0}")]
    TwitterApi(String),

    #[error("Tweet not found or is private")]
    TweetNotFound,

    #[error("No video found in this tweet")]
    NoVideo,

    #[error("No captions found for this tweet")]
    NoCaptions,

    #[error("Invalid Twitter URL")]
    InvalidUrl,

    #[error("ffmpeg error: {0}")]
    Ffmpeg(String),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::TweetNotFound  => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::NoVideo        => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::NoCaptions     => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::InvalidUrl     => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::TwitterApi(_)  => (StatusCode::BAD_GATEWAY, self.to_string()),
            AppError::Ffmpeg(_)      => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::Network(_)     => (StatusCode::BAD_GATEWAY, self.to_string()),
            AppError::Internal(_)    => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        tracing::error!("{}", message);

        (status, Json(json!({ "error": message }))).into_response()
    }
}