//! DeepClaude - A high-performance LLM inference API and Chat UI that integrates DeepSeek R1's CoT reasoning traces with Anthropic Claude models..
//!
//! This application provides a REST API for chat interactions that:
//! - Processes messages through DeepSeek R1 for reasoning
//! - Uses Anthropic's Claude for final responses
//! - Supports both streaming and non-streaming responses
//! - Tracks token usage and costs
//! - Provides detailed usage statistics
//!
//! The API requires authentication tokens for both services and
//! supports custom configuration through a TOML config file.

mod clients;
mod config;
mod error;
mod handlers;
mod models;

use crate::{config::Config, handlers::AppState};
use axum::routing::{post, Router};
use std::{net::SocketAddr, sync::Arc};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::fmt::time::FormatTime;
use chrono::Utc;

/// Application entry point.
///
/// Sets up logging, loads configuration, and starts the HTTP server
/// with the configured routes and middleware.
///
/// # Returns
///
/// * `anyhow::Result<()>` - Ok if server starts successfully, Err otherwise
///
/// # Errors
///
/// Returns an error if:
/// - Logging setup fails
/// - Server address binding fails
/// - Server encounters a fatal error while running
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 自定义时间格式化器，使用北京时间
    struct BeijingTime;

    impl FormatTime for BeijingTime {
        fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
            // 北京时间是UTC+8
            let beijing_time = (Utc::now() + chrono::Duration::hours(8)).format("%Y-%m-%dT%H:%M:%S%.3f%:z");
            write!(w, "{}", beijing_time)
        }
    }

    // 设置日志格式，使用自定义时间格式化器
    let format = tracing_subscriber::fmt::format()
        .with_level(true)
        .with_target(true)
        .with_thread_ids(true)
        .with_timer(BeijingTime);

    // 明确设置日志级别，不依赖环境变量
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "deepclaude=debug,tower_http=debug".into());

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .event_format(format)
        .init();

    // Load configuration
    let config = Config::load().unwrap_or_else(|_| {
        tracing::warn!("Failed to load config.toml, using default configuration");
        Config::default()
    });

    // Create application state
    let state = Arc::new(AppState::new(config.clone()));

    // Set up CORS
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_headers(Any)
        .allow_origin(Any);

    // Build router
    let app = Router::new()
        .route("/v1/chat/completions", post(handlers::handle_chat))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state);

    // Get host and port from config
    let addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port)
        .parse()
        .expect("Invalid host/port configuration");

    tracing::info!("Starting server on {}", addr);

    // Start server
    axum::serve(
        tokio::net::TcpListener::bind(&addr).await?,
        app.into_make_service(),
    )
    .await?;

    Ok(())
}
