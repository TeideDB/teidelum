use std::sync::Arc;

use axum::http::StatusCode;
use axum::{
    extract::Request,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use tokio_util::sync::CancellationToken;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

use crate::api::TeidelumApi;
use crate::chat::handlers::{chat_routes, ChatState};
use crate::chat::ws::ws_upgrade;
use crate::mcp::Teidelum;
use crate::routes;

/// Build the axum router with all routes, CORS, and optional auth.
pub fn build_router(
    api: Arc<TeidelumApi>,
    hub: Arc<crate::chat::hub::Hub>,
    data_dir: Option<std::path::PathBuf>,
    ct: CancellationToken,
) -> Router {
    let chat_state: crate::chat::handlers::AppState = Arc::new(ChatState {
        api: api.clone(),
        hub: hub.clone(),
        data_dir,
        dm_create_lock: tokio::sync::Mutex::new(()),
        reads_lock: tokio::sync::Mutex::new(()),
        settings_lock: tokio::sync::Mutex::new(()),
        channel_create_lock: tokio::sync::Mutex::new(()),
        channel_join_lock: tokio::sync::Mutex::new(()),
        pin_lock: tokio::sync::Mutex::new(()),
        reaction_lock: tokio::sync::Mutex::new(()),
        register_lock: tokio::sync::Mutex::new(()),
    });

    // Data API routes (protected by optional API key)
    let mut data_api = Router::new()
        .merge(routes::api_routes())
        .with_state(api.clone());

    // MCP Streamable HTTP endpoint (also protected by optional API key)
    let mcp_api = api;
    let mcp_hub = hub.clone();
    let mcp_service = StreamableHttpService::new(
        move || Ok(Teidelum::new_with_hub(mcp_api.clone(), mcp_hub.clone())),
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig {
            stateful_mode: true,
            cancellation_token: ct.child_token(),
            ..Default::default()
        },
    );
    data_api = data_api.nest_service("/mcp", mcp_service);

    // If TEIDELUM_API_KEY is set, apply auth only to data API and MCP routes (not chat/ws/files)
    if let Ok(key) = std::env::var("TEIDELUM_API_KEY") {
        if !key.is_empty() {
            data_api = data_api.layer(middleware::from_fn(move |req, next| {
                let key = key.clone();
                async move { auth_check(req, next, key).await }
            }));
        } else {
            tracing::warn!("TEIDELUM_API_KEY is set but empty — data API and MCP endpoints are unauthenticated");
        }
    } else {
        tracing::warn!("TEIDELUM_API_KEY not set — data API and MCP endpoints are unauthenticated");
    }

    // Chat routes (protected by JWT, not API key)
    let mut app = Router::new()
        .merge(data_api)
        .merge(chat_routes(chat_state.clone()))
        .route(
            "/files/{id}/{filename}",
            axum::routing::get(crate::chat::files::files_download).with_state(chat_state.clone()),
        )
        .route("/ws", axum::routing::get(ws_upgrade).with_state(chat_state))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::OPTIONS,
                ])
                .allow_headers([
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::HeaderName::from_static("last-event-id"),
                    axum::http::header::HeaderName::from_static("mcp-protocol-version"),
                    axum::http::header::HeaderName::from_static("mcp-session-id"),
                ])
                .expose_headers([
                    axum::http::header::HeaderName::from_static("mcp-session-id"),
                    axum::http::header::HeaderName::from_static("mcp-protocol-version"),
                ]),
        );

    // Serve SvelteKit static build — fallback after API routes.
    // For SPA routing, unknown paths serve index.html with 200 so the
    // client-side router handles them.
    let ui_dir = std::path::Path::new("ui/build");
    if ui_dir.exists() {
        let index_html: &'static str = Box::leak(
            std::fs::read_to_string(ui_dir.join("index.html"))
                .unwrap_or_default()
                .into_boxed_str(),
        );
        let serve_dir = ServeDir::new(ui_dir);
        app = app
            .fallback_service(serve_dir)
            .layer(axum::middleware::from_fn(
                move |req: Request, next: Next| async move {
                    let resp = next.run(req).await;
                    if resp.status() == StatusCode::NOT_FOUND {
                        (
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "text/html")],
                            index_html,
                        )
                            .into_response()
                    } else {
                        resp
                    }
                },
            ));
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
pub async fn start(
    api: Arc<TeidelumApi>,
    hub: Arc<crate::chat::hub::Hub>,
    data_dir: Option<std::path::PathBuf>,
    bind: &str,
    port: u16,
) -> anyhow::Result<()> {
    // Validate TEIDE_CHAT_SECRET at startup
    match std::env::var("TEIDE_CHAT_SECRET") {
        Ok(s) if s.len() >= 32 => {}
        Ok(s) if !s.is_empty() => {
            anyhow::bail!(
                "TEIDE_CHAT_SECRET must be at least 32 bytes (got {})",
                s.len()
            );
        }
        Ok(_) => {
            tracing::warn!("TEIDE_CHAT_SECRET is set but empty — chat auth will not work");
        }
        Err(_) => {
            tracing::warn!("TEIDE_CHAT_SECRET not set — chat auth will not work");
        }
    }

    let ct = CancellationToken::new();
    let app = build_router(api, hub, data_dir, ct.clone());
    let addr = format!("{bind}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("HTTP server listening on {addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move { ct.cancelled().await })
        .await?;
    Ok(())
}
