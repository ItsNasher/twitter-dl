mod error;
mod models;
mod routes;
mod services;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::{
    http::header::HeaderValue,
    routing::{get, post},
    Router,
};
use dotenvy::dotenv;
use reqwest::Client;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::models::SyndicationTweet;
use crate::services::overlay;

#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn check(&self, key: &str, max_requests: usize, window_secs: u64) -> bool {
        let now = Instant::now();
        let window = std::time::Duration::from_secs(window_secs);
        let mut map = self.inner.lock().unwrap();
        let entries = map.entry(key.to_string()).or_default();
        entries.retain(|t| now.duration_since(*t) < window);
        if entries.len() >= max_requests {
            return false;
        }
        entries.push(now);
        true
    }
}

#[derive(Clone)]
pub struct AppState {
    pub client: Client,
    pub rate_limiter: RateLimiter,
    pub tweet_cache: Arc<Mutex<HashMap<String, (SyndicationTweet, Instant)>>>,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "twitter_dl=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .gzip(true)
        .build()
        .expect("failed to build reqwest client");

    let state = AppState {
        client,
        rate_limiter: RateLimiter::new(),
        tweet_cache: Arc::new(Mutex::new(HashMap::new())),
    };

    let cors = if let Ok(origin) = std::env::var("CORS_ORIGIN") {
        CorsLayer::new()
            .allow_origin(origin.parse::<HeaderValue>().expect("invalid CORS_ORIGIN"))
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    };

    let app = Router::new()
        .route("/api/info", post(routes::info::handler))
        .route("/api/download", post(routes::download::handler))
        .route("/api/captions", post(routes::captions::handler))
        .route("/api/audio", post(routes::audio::handler))
        .route("/api/preview", get(routes::preview::handler)) // <-- new
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(3000);

    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .expect("invalid address");

    // Eager encoder detection — runs synchronously before the server starts
    // so it never blocks a Tokio worker thread at request time.
    overlay::init_encoder();

    tracing::info!("twdl backend listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app).await.expect("server failed");
}
