mod error;
mod models;
mod routes;
mod services;

use std::net::SocketAddr;

use axum::{
    routing::{get, post},
    Router,
};
use dotenvy::dotenv;
use reqwest::Client;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct AppState {
    pub client: Client,
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

    let state = AppState { client };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/info",     post(routes::info::handler))
        .route("/api/download", post(routes::download::handler))
        .route("/api/captions", post(routes::captions::handler))
        .route("/api/audio",    post(routes::audio::handler))
        .route("/api/preview",  get(routes::preview::handler))  // <-- new
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

    tracing::info!("twdl backend listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");

    axum::serve(listener, app)
        .await
        .expect("server failed");
}