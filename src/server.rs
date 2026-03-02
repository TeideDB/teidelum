use std::sync::Arc;

use axum::http::StatusCode;
use axum::{
    extract::Request,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use tower_http::cors::CorsLayer;

use crate::api::TeidelumApi;
use crate::routes;

/// Build the axum router with all routes, CORS, and optional auth.
pub fn build_router(api: Arc<TeidelumApi>) -> Router {
    let mut app = Router::new()
        .merge(routes::api_routes())
        .with_state(api)
        .layer(CorsLayer::permissive());

    // If TEIDELUM_API_KEY is set, capture it once and wrap all routes with auth middleware
    if let Ok(key) = std::env::var("TEIDELUM_API_KEY") {
        if !key.is_empty() {
            app = app.layer(middleware::from_fn(move |req, next| {
                let key = key.clone();
                async move { auth_check(req, next, key).await }
            }));
        }
    }

    app
}

/// Auth check: requires `Authorization: Bearer <key>` matching the captured key.
async fn auth_check(request: Request, next: Next, expected_key: String) -> Response {
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            if token == expected_key {
                next.run(request).await
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(serde_json::json!({"error": "invalid or missing API key"})),
                )
                    .into_response()
            }
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({"error": "invalid or missing API key"})),
        )
            .into_response(),
    }
}

/// Start the HTTP server on the given address.
pub async fn start(api: Arc<TeidelumApi>, bind: &str, port: u16) -> anyhow::Result<()> {
    let app = build_router(api);
    let addr = format!("{bind}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("HTTP server listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
