use super::*;

#[derive(Clone)]
pub struct AppState {
    /// Local FQDN for the MCP server.
    ///
    /// ## Example
    /// `0.0.0.0:9876`
    pub local_fqdn: String,

    pub oauth_store: std::sync::Arc<oauth::OauthStore>,

    /// Public FQDN for the MCP server (Cloudflare Tunnel).
    ///
    /// ## Example
    /// `workspace-mcp.example.org`
    pub public_fqdn: String,
}
impl axum::extract::FromRef<AppState> for std::sync::Arc<oauth::OauthStore> {
    fn from_ref(app: &AppState) -> Self {
        app.oauth_store.clone()
    }
}
