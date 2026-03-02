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

    // If TEIDELUM_API_KEY is set, wrap all routes with auth middleware
    if std::env::var("TEIDELUM_API_KEY").is_ok() {
        app = app.layer(middleware::from_fn(auth_middleware));
    }

    app
}

/// Auth middleware: requires `Authorization: Bearer <key>` matching TEIDELUM_API_KEY.
async fn auth_middleware(request: Request, next: Next) -> Response {
    let expected = match std::env::var("TEIDELUM_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => return next.run(request).await,
    };

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            if token == expected {
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
