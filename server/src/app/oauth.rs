use std::{collections::HashMap, sync::Arc};

use super::*;
use anyhow::Result;
use askama::Template;
use axum::{
    Json,
    body::Body,
    extract::{Form, Query, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::{Html, IntoResponse, Redirect, Response},
};
use rand::{Rng, distr::Alphanumeric};
use rmcp::transport::auth::{
    AuthorizationMetadata, ClientRegistrationRequest, ClientRegistrationResponse, OAuthClientConfig,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// A easy way to manage MCP OAuth Store for managing tokens and sessions
#[derive(Clone, Debug)]
pub struct OauthStore {
    clients: Arc<tokio::sync::RwLock<HashMap<String, OAuthClientConfig>>>,
    auth_sessions: Arc<tokio::sync::RwLock<HashMap<String, AuthSession>>>,
    access_tokens: Arc<tokio::sync::RwLock<HashMap<String, McpAccessToken>>>,
}
impl OauthStore {
    pub fn new() -> Self {
        let mut clients = HashMap::new();
        clients.insert(
            "mcp-client".to_string(),
            OAuthClientConfig {
                client_id: "mcp-client".to_string(),
                client_secret: Some("mcp-client-secret".to_string()),
                scopes: vec!["profile".to_string(), "email".to_string()],
                redirect_uri: "http://localhost:8080/callback".to_string(),
            },
        );

        Self {
            clients: Arc::new(tokio::sync::RwLock::new(clients)),
            auth_sessions: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            access_tokens: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        }
    }

    async fn validate_client(
        &self,
        client_id: &str,
        redirect_uri: &str,
    ) -> Option<OAuthClientConfig> {
        let clients = self.clients.read().await;
        if let Some(client) = clients.get(client_id) {
            if client.redirect_uri.contains(&redirect_uri.to_string()) {
                return Some(client.clone());
            }
        }
        None
    }

    async fn create_auth_session(
        &self,
        client_id: String,
        scope: Option<String>,
        state: Option<String>,
        session_id: String,
    ) -> String {
        let session = AuthSession {
            client_id,
            scope,
            _state: state,
            _created_at: chrono::Utc::now(),
            auth_token: None,
        };

        self.auth_sessions
            .write()
            .await
            .insert(session_id.clone(), session);
        session_id
    }

    async fn update_auth_session_token(
        &self,
        session_id: &str,
        token: AuthToken,
    ) -> Result<(), String> {
        let mut sessions = self.auth_sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.auth_token = Some(token);
            Ok(())
        } else {
            Err("Session not found".to_string())
        }
    }

    async fn create_mcp_token(&self, session_id: &str) -> Result<McpAccessToken, String> {
        let sessions = self.auth_sessions.read().await;
        if let Some(session) = sessions.get(session_id) {
            if let Some(auth_token) = &session.auth_token {
                let access_token = format!("mcp-token-{}", Uuid::new_v4());
                let token = McpAccessToken {
                    access_token: access_token.clone(),
                    token_type: "Bearer".to_string(),
                    expires_in: 3600,
                    refresh_token: format!("mcp-refresh-{}", Uuid::new_v4()),
                    scope: session.scope.clone(),
                    auth_token: auth_token.clone(),
                    client_id: session.client_id.clone(),
                };

                self.access_tokens
                    .write()
                    .await
                    .insert(access_token.clone(), token.clone());
                Ok(token)
            } else {
                Err("No third-party token available for session".to_string())
            }
        } else {
            Err("Session not found".to_string())
        }
    }

    async fn validate_token(&self, token: &str) -> Option<McpAccessToken> {
        self.access_tokens.read().await.get(token).cloned()
    }
}

// a simple session record for auth session
#[derive(Clone, Debug)]
struct AuthSession {
    client_id: String,
    scope: Option<String>,
    _state: Option<String>,
    _created_at: chrono::DateTime<chrono::Utc>,
    auth_token: Option<AuthToken>,
}

// a simple token record for auth token
// not used oauth2 token for avoid include oauth2 crate in this example
#[derive(Clone, Debug, Serialize, Deserialize)]
struct AuthToken {
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: String,
    scope: Option<String>,
}

// a simple token record for mcp token ,
// not used oauth2 token for avoid include oauth2 crate in this example
#[derive(Clone, Debug, Serialize)]
struct McpAccessToken {
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: String,
    scope: Option<String>,
    auth_token: AuthToken,
    client_id: String,
}

#[derive(Debug, Deserialize)]
pub struct AuthorizeQuery {
    #[allow(dead_code)]
    response_type: String,
    client_id: String,
    redirect_uri: String,
    scope: Option<String>,
    state: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct TokenRequest {
    grant_type: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    client_secret: String,
    #[serde(default)]
    redirect_uri: String,
    #[serde(default)]
    code_verifier: Option<String>,
    #[serde(default)]
    refresh_token: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct UserInfo {
    sub: String,
    name: String,
    email: String,
    username: String,
}

fn generate_random_string(length: usize) -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

#[derive(Template)]
#[template(path = "oauth.html")]
struct OAuthAuthorizeTemplate {
    client_id: String,
    redirect_uri: String,
    scope: String,
    state: String,
    scopes: String,
}

// Initial OAuth authorize endpoint
pub async fn handle_get_oauth_authorize(
    Query(params): Query<AuthorizeQuery>,
    State(oauth_store): axum::extract::State<std::sync::Arc<OauthStore>>,
) -> impl IntoResponse {
    debug!("doing oauth_authorize");
    if let Some(_client) = oauth_store
        .validate_client(&params.client_id, &params.redirect_uri)
        .await
    {
        let template = OAuthAuthorizeTemplate {
            client_id: params.client_id,
            redirect_uri: params.redirect_uri,
            scope: params.scope.clone().unwrap_or_default(),
            state: params.state.clone().unwrap_or_default(),
            scopes: params
                .scope
                .clone()
                .unwrap_or_else(|| "Basic scope".to_string()),
        };

        Html(template.render().unwrap()).into_response()
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "invalid_request",
                "error_description": "invalid client id or redirect uri"
            })),
        )
            .into_response()
    }
}

// handle approval of authorization
#[derive(Debug, Deserialize)]
pub struct ApprovalForm {
    client_id: String,
    redirect_uri: String,
    scope: String,
    state: String,
    approved: String,
}

pub async fn handle_post_oauth_approve(
    State(oauth_store): axum::extract::State<std::sync::Arc<OauthStore>>,
    Form(form): Form<ApprovalForm>,
) -> impl IntoResponse {
    if form.approved != "true" {
        // user rejected the authorization request
        let redirect_url = format!(
            "{}?error=access_denied&error_description={}{}",
            form.redirect_uri,
            "user rejected the authorization request",
            if form.state.is_empty() {
                "".to_string()
            } else {
                format!("&state={}", form.state)
            }
        );
        return Redirect::to(&redirect_url).into_response();
    }

    // user approved the authorization request, generate authorization code
    let session_id = Uuid::new_v4().to_string();
    let auth_code = format!("mcp-code-{}", session_id);

    // create new session record authorization information
    let session_id = oauth_store
        .create_auth_session(
            form.client_id.clone(),
            Some(form.scope.clone()),
            Some(form.state.clone()),
            session_id.clone(),
        )
        .await;

    // create token
    let created_token = AuthToken {
        access_token: format!("tp-token-{}", Uuid::new_v4()),
        token_type: "Bearer".to_string(),
        expires_in: 3600,
        refresh_token: format!("tp-refresh-{}", Uuid::new_v4()),
        scope: Some(form.scope),
    };

    // update session token
    if let Err(e) = oauth_store
        .update_auth_session_token(&session_id, created_token)
        .await
    {
        error!("Failed to update session token: {}", e);
    }

    // redirect back to client, with authorization code
    let redirect_url = format!(
        "{}?code={}{}",
        form.redirect_uri,
        auth_code,
        if form.state.is_empty() {
            "".to_string()
        } else {
            format!("&state={}", form.state)
        }
    );

    info!("authorization approved, redirecting to: {}", redirect_url);
    Redirect::to(&redirect_url).into_response()
}

// Handle token request from the MCP client
async fn handle_post_oauth_token(
    State(oauth_store): axum::extract::State<std::sync::Arc<OauthStore>>,
    request: axum::http::Request<Body>,
) -> impl IntoResponse {
    info!("Received token request");

    let bytes = match axum::body::to_bytes(request.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("can't read request body: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_request",
                    "error_description": "can't read request body"
                })),
            )
                .into_response();
        }
    };

    let body_str = String::from_utf8_lossy(&bytes);
    info!("request body: {}", body_str);

    let token_req = match serde_urlencoded::from_bytes::<TokenRequest>(&bytes) {
        Ok(form) => {
            info!("successfully parsed form data: {:?}", form);
            form
        }
        Err(e) => {
            error!("can't parse form data: {}", e);
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "error": "invalid_request",
                    "error_description": format!("can't parse form data: {}", e)
                })),
            )
                .into_response();
        }
    };
    if token_req.grant_type == "refresh_token" {
        warn!("this easy server only support authorization_code now");
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "unsupported_grant_type",
                "error_description": "only authorization_code is supported"
            })),
        )
            .into_response();
    }
    if token_req.grant_type != "authorization_code" {
        info!("unsupported grant type: {}", token_req.grant_type);
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "unsupported_grant_type",
                "error_description": "only authorization_code is supported"
            })),
        )
            .into_response();
    }

    // get session_id from code
    if !token_req.code.starts_with("mcp-code-") {
        info!("invalid authorization code: {}", token_req.code);
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "invalid_grant",
                "error_description": "invalid authorization code"
            })),
        )
            .into_response();
    }

    // handle empty client_id
    let client_id = if token_req.client_id.is_empty() {
        "mcp-client".to_string()
    } else {
        token_req.client_id.clone()
    };

    // validate client
    match oauth_store
        .validate_client(&client_id, &token_req.redirect_uri)
        .await
    {
        Some(_) => {
            let session_id = token_req.code.replace("mcp-code-", "");
            info!("got session id: {}", session_id);

            // create mcp access token
            match oauth_store.create_mcp_token(&session_id).await {
                Ok(token) => {
                    info!("successfully created access token");
                    (
                        StatusCode::OK,
                        Json(serde_json::json!({
                            "access_token": token.access_token,
                            "token_type": token.token_type,
                            "expires_in": token.expires_in,
                            "refresh_token": token.refresh_token,
                            "scope": token.scope,
                        })),
                    )
                        .into_response()
                }
                Err(e) => {
                    error!("failed to create access token: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({
                            "error": "server_error",
                            "error_description": format!("failed to create access token: {}", e)
                        })),
                    )
                        .into_response()
                }
            }
        }
        None => {
            info!(
                "invalid client id or redirect uri: {} / {}",
                client_id, token_req.redirect_uri
            );
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "invalid_client",
                    "error_description": "invalid client id or redirect uri"
                })),
            )
                .into_response()
        }
    }
}

pub async fn oauth_middleware(
    State(oauth_store): axum::extract::State<std::sync::Arc<OauthStore>>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    debug!("oauth_middleware");
    let auth_header = request.headers().get("Authorization");
    let token = match auth_header {
        Some(header) => {
            let header_str = header.to_str().unwrap_or("");
            if let Some(stripped) = header_str.strip_prefix("Bearer ") {
                stripped.to_string()
            } else {
                return StatusCode::UNAUTHORIZED.into_response();
            }
        }
        None => {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    };

    match oauth_store.validate_token(&token).await {
        Some(_) => next.run(request).await,
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

pub async fn handle_get_oauth_metadata(
    axum::extract::State(app_state): axum::extract::State<constants::AppState>,
) -> impl IntoResponse {
    let mut additional_fields = HashMap::new();
    additional_fields.insert(
        "response_types_supported".into(),
        Value::Array(vec![Value::String("code".into())]),
    );
    additional_fields.insert(
        "code_challenge_methods_supported".into(),
        Value::Array(vec![Value::String("S256".into())]),
    );
    let metadata = AuthorizationMetadata {
        authorization_endpoint: format!("https://{}/oauth/authorize", app_state.public_fqdn),
        token_endpoint: format!("https://{}/oauth/token", app_state.public_fqdn),
        scopes_supported: Some(vec!["profile".to_string(), "email".to_string()]),
        registration_endpoint: format!("https://{}/oauth/register", app_state.public_fqdn),
        issuer: Some(app_state.public_fqdn.to_string()),
        jwks_uri: Some(format!("https://{}/oauth/jwks", app_state.public_fqdn)),
        additional_fields,
    };
    debug!("metadata: {:?}", metadata);
    (StatusCode::OK, Json(metadata))
}

// handle client registration request
async fn handle_post_oauth_register(
    State(oauth_store): axum::extract::State<std::sync::Arc<OauthStore>>,
    Json(req): Json<ClientRegistrationRequest>,
) -> impl IntoResponse {
    debug!("register request: {:?}", req);
    if req.redirect_uris.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "invalid_request",
                "error_description": "at least one redirect uri is required"
            })),
        )
            .into_response();
    }

    let client_id = format!("client-{}", Uuid::new_v4());
    let client_secret = generate_random_string(32);

    let client = OAuthClientConfig {
        client_id: client_id.clone(),
        client_secret: Some(client_secret.clone()),
        redirect_uri: req.redirect_uris[0].clone(),
        scopes: vec![],
    };

    oauth_store
        .clients
        .write()
        .await
        .insert(client_id.clone(), client);

    // return client information
    let response = ClientRegistrationResponse {
        client_id,
        client_secret: Some(client_secret),
        client_name: req.client_name,
        redirect_uris: req.redirect_uris,
        additional_fields: HashMap::new(),
    };

    (StatusCode::CREATED, Json(response)).into_response()
}

pub fn oauth_router(app_state: constants::AppState) -> axum::Router {
    // let oauth_store = Arc::new(OauthStore::new());

    let cors_layer = tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    axum::Router::new()
        .route(
            "/oauth/authorize",
            axum::routing::get(handle_get_oauth_authorize),
        )
        .route(
            "/oauth/approve",
            axum::routing::post(handle_post_oauth_approve),
        )
        .route(
            "/oauth/token",
            axum::routing::post(handle_post_oauth_token).options(handle_post_oauth_token),
        )
        .route(
            "/oauth/register",
            axum::routing::post(handle_post_oauth_register).options(handle_post_oauth_register),
        )
        .route(
            "/.well-known/oauth-authorization-server",
            axum::routing::get(handle_get_oauth_metadata).options(handle_get_oauth_metadata),
        )
        .layer(cors_layer)
        .with_state(app_state)
}
