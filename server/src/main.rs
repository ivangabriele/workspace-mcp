#![forbid(unsafe_code)]

use clap::Parser;

use tracing_subscriber::{
    layer::SubscriberExt,
    util::SubscriberInitExt,
    {self},
};

use mcp_server::app;

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
    let addr: std::net::SocketAddr = core::net::SocketAddr::from(([0, 0, 0, 0], args.port));
    let router = app::router(addr, args.auth_token, args.workspace_path_as_string).await?;

    tracing::info!("Server listening on http://{}", addr);
    let tcp_listener = tokio::net::TcpListener::bind(addr).await?;
    let _ = axum::serve(tcp_listener, router)
        .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.unwrap() })
        .await;

    Ok(())
}
