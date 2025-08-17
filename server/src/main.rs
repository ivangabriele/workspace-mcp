#![forbid(unsafe_code)]

use clap::Parser;
use rmcp::transport::{
    StreamableHttpService, streamable_http_server::session::local::LocalSessionManager,
};
use tracing_subscriber::{
    layer::SubscriberExt,
    util::SubscriberInitExt,
    {self},
};

mod workspace_manager;

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

const INDEX_HTML: &str = include_str!("../public/index.html");

struct TokenStore {
    valid_tokens: Vec<String>,
}
impl TokenStore {
    fn new(valid_tokens: Vec<String>) -> Self {
        Self { valid_tokens }
    }

    fn is_valid(&self, token: &str) -> bool {
        self.valid_tokens.contains(&token.to_string())
    }
}

// Extract authorization token
fn extract_token(headers: &axum::http::HeaderMap) -> Option<String> {
    headers
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|auth_header| {
            auth_header
                .strip_prefix("Bearer ")
                .map(|stripped| stripped.to_string())
        })
}

// Authorization middleware
async fn auth_middleware(
    axum::extract::State(token_store): axum::extract::State<std::sync::Arc<TokenStore>>,
    headers: axum::http::HeaderMap,
    request: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, axum::http::StatusCode> {
    match extract_token(&headers) {
        Some(token) if token_store.is_valid(&token) => {
            // Token is valid, proceed with the request
            Ok(next.run(request).await)
        }
        _ => {
            // Token is invalid, return 401 error
            Err(axum::http::StatusCode::UNAUTHORIZED)
        }
    }
}

// Root path handler
async fn index() -> axum::response::Html<&'static str> {
    axum::response::Html(INDEX_HTML)
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

    let token_store = std::sync::Arc::new(TokenStore::new(vec![args.auth_token]));

    let mcp_service = StreamableHttpService::new(
        move || {
            Ok(workspace_manager::WorkspaceManager::new(
                args.workspace_path_as_string.clone(),
            ))
        },
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let api_router = axum::Router::new().route("/health", axum::routing::get(|| async { "ok" }));

    let mcp_router = axum::Router::new().nest_service("/mcp", mcp_service);
    let protected_mcp_router = mcp_router.layer(axum::middleware::from_fn_with_state(
        token_store.clone(),
        auth_middleware,
    ));

    let app = axum::Router::new()
        .route("/", axum::routing::get(index))
        .nest("/api", api_router)
        .merge(protected_mcp_router)
        .with_state(());

    let addr: std::net::SocketAddr = core::net::SocketAddr::from(([0, 0, 0, 0], args.port));
    tracing::info!("Server listening on http://{}", addr);
    let tcp_listener = tokio::net::TcpListener::bind(addr).await?;
    let _ = axum::serve(tcp_listener, app)
        .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.unwrap() })
        .await;

    Ok(())
}
