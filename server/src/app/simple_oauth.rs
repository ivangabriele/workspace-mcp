#![forbid(unsafe_code)]

pub struct SimpleOauthTokenStore {
    valid_tokens: Vec<String>,
}
impl SimpleOauthTokenStore {
    pub fn new(valid_tokens: Vec<String>) -> Self {
        Self { valid_tokens }
    }

    fn is_valid(&self, token: &str) -> bool {
        self.valid_tokens.contains(&token.to_string())
    }
}

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

pub async fn simple_oauth_middleware(
    axum::extract::State(token_store): axum::extract::State<std::sync::Arc<SimpleOauthTokenStore>>,
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
