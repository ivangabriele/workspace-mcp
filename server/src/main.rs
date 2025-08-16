#![forbid(unsafe_code)]

use clap::Parser;
use rmcp::transport::{SseServer, sse_server::SseServerConfig};
use tracing_subscriber::{
    layer::SubscriberExt,
    util::SubscriberInitExt,
    {self},
};

mod file_manager;

/// App configuration from CLI.
#[derive(Debug, Parser, Clone)]
struct Args {
    /// Bearer token required in Authorization header
    #[arg(long = "auth-token", env = "WORKSPACE_MCP_AUTH_TOKEN")]
    auth_token: String,

    /// Workspace path
    #[arg(long = "workspace-path", value_parser)]
    workspace_path_as_string: String,

    /// MCP Server local port
    #[arg(long, default_value = "9876")]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();
    let addr: std::net::SocketAddr = core::net::SocketAddr::from(([127, 0, 0, 1], args.port));
    let config = SseServerConfig {
        bind: addr,
        sse_path: "/sse".to_string(),
        post_path: "/message".to_string(),
        ct: tokio_util::sync::CancellationToken::new(),
        sse_keep_alive: None,
    };

    let (sse_server, router) = SseServer::new(config);

    // Do something with the router, e.g., add routes or middleware

    let listener = tokio::net::TcpListener::bind(sse_server.config.bind).await?;
    let cancellation_token = sse_server.config.ct.child_token();
    let server = axum::serve(listener, router).with_graceful_shutdown(async move {
        cancellation_token.cancelled().await;
        tracing::info!("sse server cancelled");
    });

    tokio::spawn(async move {
        if let Err(e) = server.await {
            tracing::error!(error = %e, "sse server shutdown with error");
        }
    });

    let cancellation_token = sse_server.with_service(move || {
        file_manager::FileManager::new(args.workspace_path_as_string.clone())
    });
    tokio::signal::ctrl_c().await?;
    cancellation_token.cancel();

    Ok(())
}
