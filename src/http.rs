//! Authenticated Streamable HTTP entrypoint for trusted operators and gateways.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::Router;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio::sync::Mutex;
use tower::ServiceExt;

use crate::{server::DwgServer, service::Service};

async fn authenticate(State(expected): State<[u8; 32]>, request: Request, next: Next) -> Response {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    if let Some(token) = token {
        let actual = Sha256::digest(token.as_bytes());
        if bool::from(actual.as_slice().ct_eq(&expected)) {
            return next.run(request).await;
        }
    }
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, "Bearer")],
    )
        .into_response()
}

fn transport(
    server: DwgServer,
    config: StreamableHttpServerConfig,
) -> StreamableHttpService<DwgServer, LocalSessionManager> {
    StreamableHttpService::new(
        move || Ok(server.clone()),
        Arc::new(LocalSessionManager::default()),
        config,
    )
}

struct TenantGateway {
    cache_dir: PathBuf,
    font: Arc<ab_glyph::FontVec>,
    config: StreamableHttpServerConfig,
    gate: Mutex<()>,
}

async fn serve_tenant(State(gateway): State<Arc<TenantGateway>>, request: Request) -> Response {
    let tenant = request
        .headers()
        .get("x-dwg-account")
        .and_then(|value| value.to_str().ok());
    let Some(tenant) = tenant.filter(|id| {
        id.len() == 32
            && id
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    }) else {
        return (
            StatusCode::BAD_REQUEST,
            "Missing or invalid gateway account",
        )
            .into_response();
    };
    let cache = gateway.cache_dir.join(tenant);
    let _gate = gateway.gate.lock().await;
    let Ok(service) = Service::for_http(cache, gateway.font.clone()) else {
        eprintln!("dwg-mcp: unable to open a tenant cache");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    match transport(DwgServer::for_http(service), gateway.config.clone())
        .oneshot(request)
        .await
    {
        Ok(response) => response.into_response(),
        Err(never) => match never {},
    }
}

pub fn run(font_path: &str, cache_dir: PathBuf) -> Result<(), String> {
    let token = std::env::var("DWG_MCP_HTTP_TOKEN").unwrap_or_default();
    if token.len() < 32 || !token.bytes().all(|byte| byte.is_ascii_graphic()) {
        return Err("DWG_MCP_HTTP_TOKEN must be at least 32 visible ASCII characters".into());
    }
    let token_hash: [u8; 32] = Sha256::digest(token.as_bytes()).into();
    let bind = std::env::var("DWG_MCP_HTTP_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let hosts =
        std::env::var("DWG_MCP_HTTP_HOSTS").unwrap_or_else(|_| "localhost,127.0.0.1,::1".into());
    let hosts: Vec<_> = hosts
        .split(',')
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .collect();
    if hosts.is_empty() {
        return Err("DWG_MCP_HTTP_HOSTS must contain at least one hostname".into());
    }

    let font = Arc::new(dwg2png::text::load_font(font_path)?);
    let config = StreamableHttpServerConfig::default()
        .with_allowed_hosts(hosts)
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_max_request_body_bytes(64 * 1024);
    let routes = if std::env::var("DWG_MCP_HTTP_TENANTS").as_deref() == Ok("1") {
        let gateway = TenantGateway {
            cache_dir,
            font,
            config,
            gate: Mutex::new(()),
        };
        Router::new()
            .route("/mcp", axum::routing::any(serve_tenant))
            .with_state(Arc::new(gateway))
    } else {
        let service = Service::for_http(cache_dir, font).map_err(|error| error.to_string())?;
        Router::new().route_service("/mcp", transport(DwgServer::for_http(service), config))
    };
    let app = routes.layer(middleware::from_fn_with_state(token_hash, authenticate));
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("start runtime: {error}"))?;
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind(&bind)
            .await
            .map_err(|error| format!("bind HTTP server at {bind}: {error}"))?;
        axum::serve(listener, app)
            .await
            .map_err(|error| format!("serve HTTP: {error}"))
    })
}
