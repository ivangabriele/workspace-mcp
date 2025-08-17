#![forbid(unsafe_code)]

use rmcp::transport::{
    StreamableHttpService, streamable_http_server::session::local::LocalSessionManager,
};

use super::*;
use crate::workspace_manager;

const INDEX_HTML: &str = include_str!("../../public/index.html");

async fn index() -> axum::response::Html<&'static str> {
    axum::response::Html(INDEX_HTML)
}

pub async fn router(
    addr: std::net::SocketAddr,
    _auth_token: String,
    workspace_path_as_string: String,
) -> anyhow::Result<axum::Router> {
    let oauth_store = std::sync::Arc::new(oauth::OauthStore::new());
    let app_state = constants::AppState {
        local_fqdn: addr.to_string(),
        public_fqdn: std::env::var("CLOUDFLARED_TUNNEL_DOMAIN")
            .unwrap_or_else(|_| addr.to_string()),
        oauth_store: oauth_store.clone(),
    };

    // let token_store =
    //     std::sync::Arc::new(simple_oauth::SimpleOauthTokenStore::new(vec![auth_token]));

    let oauth_router = oauth::oauth_router(app_state.clone());

    let mcp_service = StreamableHttpService::new(
        move || {
            Ok(workspace_manager::WorkspaceManager::new(
                workspace_path_as_string.clone(),
            ))
        },
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let api_router = axum::Router::new().route("/health", axum::routing::get(|| async { "ok" }));

    let mcp_router = axum::Router::new().nest_service("/mcp", mcp_service);
    let protected_mcp_router = mcp_router.layer(axum::middleware::from_fn_with_state(
        app_state.clone(),
        oauth::oauth_middleware,
    ));
    // let protected_mcp_router = mcp_router.layer(axum::middleware::from_fn_with_state(
    //     token_store.clone(),
    //     simple_oauth::simple_oauth_middleware,
    // ));

    Ok(axum::Router::new()
        .route("/", axum::routing::get(index))
        .nest("/api", api_router)
        .merge(oauth_router)
        .merge(protected_mcp_router)
        .with_state(()))
}
