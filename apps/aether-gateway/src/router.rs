use std::path::PathBuf;

use axum::extract::Request;
use axum::http::{header, HeaderValue, Method};
use axum::response::{IntoResponse, Response};
use axum::routing::any;
use axum::Router;
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};
use tracing::warn;

use aether_runtime::{prometheus_response, ConcurrencyError, DistributedConcurrencyError};

use super::{api, handlers::proxy::proxy_request, middleware, state::AppState};

pub fn build_router() -> Result<Router, reqwest::Error> {
    Ok(build_router_with_state(AppState::new()?))
}

#[derive(Clone, Debug)]
struct FrontendStaticState {
    static_dir: PathBuf,
    index_html: PathBuf,
}

pub fn build_router_with_state(state: AppState) -> Router {
    let cors_state = state.clone();
    let mut router = Router::<AppState>::new();
    router = api::mount_core_routes(router);
    router = api::mount_operational_routes(router);
    router = api::mount_ai_routes(router);
    router = api::mount_public_support_routes(router);
    router = api::mount_oauth_routes(router);
    router = api::mount_internal_routes(router);
    router = api::mount_admin_routes(router);
    let mut router = router
        .route("/{*path}", any(proxy_request))
        .layer(axum::middleware::from_fn(middleware::access_log_middleware))
        .with_state(state);
    if cors_state.frontdoor_cors().is_some() {
        router = router.layer(axum::middleware::from_fn_with_state(
            cors_state,
            middleware::frontdoor_cors_middleware,
        ));
    }
    middleware::apply_cf_header_stripping(router)
}

pub fn attach_static_frontend(router: Router, static_dir: impl Into<PathBuf>) -> Router {
    let static_dir = static_dir.into();
    let index_html = static_dir.join("index.html");
    middleware::apply_cf_header_stripping(router.layer(axum::middleware::from_fn_with_state(
        FrontendStaticState {
            static_dir,
            index_html,
        },
        frontend_static_middleware,
    )))
}

async fn frontend_static_middleware(
    axum::extract::State(frontend): axum::extract::State<FrontendStaticState>,
    request: Request,
    next: axum::middleware::Next,
) -> Response {
    let path = request.uri().path().to_string();
    if !matches!(request.method(), &Method::GET | &Method::HEAD)
        || frontend_path_bypasses_static(&path)
    {
        return next.run(request).await;
    }

    if frontend_path_targets_static_asset(&path) {
        return serve_static_asset(&frontend.static_dir, &path, request).await;
    }

    serve_frontend_index(&frontend.index_html, request).await
}

fn frontend_path_bypasses_static(path: &str) -> bool {
    matches!(
        path,
        "/health" | "/test-connection" | crate::constants::READYZ_PATH
    ) || path.starts_with("/api/")
        || path.starts_with("/v1/")
        || path.starts_with("/v1beta/")
        || path.starts_with("/upload/")
        || path.starts_with("/_gateway/")
        || path.starts_with("/.well-known/")
}

fn frontend_path_targets_static_asset(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|segment| !segment.is_empty() && segment.contains('.'))
}

fn frontend_asset_cache_control(path: &str) -> HeaderValue {
    if path.starts_with("/assets/") {
        HeaderValue::from_static("public, max-age=31536000, immutable")
    } else {
        HeaderValue::from_static("no-cache")
    }
}

fn frontend_index_cache_control() -> HeaderValue {
    HeaderValue::from_static("no-cache, no-store, must-revalidate")
}

async fn serve_static_asset(static_dir: &PathBuf, path: &str, request: Request) -> Response {
    match ServeDir::new(static_dir).oneshot(request).await {
        Ok(mut response) => {
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, frontend_asset_cache_control(path));
            response.into_response()
        }
        Err(err) => {
            warn!(error = %err, "failed to serve frontend static asset");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn serve_frontend_index(index_html: &PathBuf, request: Request) -> Response {
    match ServeFile::new(index_html).oneshot(request).await {
        Ok(mut response) => {
            response
                .headers_mut()
                .insert(header::CACHE_CONTROL, frontend_index_cache_control());
            response.into_response()
        }
        Err(err) => {
            warn!(error = %err, "failed to serve frontend index");
            axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[cfg(test)]
mod frontend_static_tests {
    use std::path::PathBuf;

    use axum::body::Body;
    use http::Request;
    use tower::ServiceExt;

    use super::{attach_static_frontend, header, HeaderValue};

    fn test_static_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "aether-frontend-static-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time before unix epoch")
                .as_nanos(),
        ));
        std::fs::create_dir_all(dir.join("assets")).expect("create static dir");
        std::fs::write(dir.join("index.html"), "<html><body>aether</body></html>")
            .expect("write index");
        std::fs::write(dir.join("assets/app.js"), "console.log('aether');").expect("write asset");
        dir
    }

    #[tokio::test]
    async fn index_html_is_served_with_no_store_cache_policy() {
        let static_dir = test_static_dir("index-cache");
        let app = attach_static_frontend(axum::Router::new(), &static_dir);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/admin/pool")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("serve index");

        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static(
                "no-cache, no-store, must-revalidate"
            )),
        );

        std::fs::remove_dir_all(static_dir).expect("cleanup static dir");
    }

    #[tokio::test]
    async fn hashed_assets_are_served_as_immutable() {
        let static_dir = test_static_dir("asset-cache");
        let app = attach_static_frontend(axum::Router::new(), &static_dir);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/assets/app.js")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("serve asset");

        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static(
                "public, max-age=31536000, immutable"
            )),
        );

        std::fs::remove_dir_all(static_dir).expect("cleanup static dir");
    }
}

pub(crate) async fn metrics(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl axum::response::IntoResponse {
    prometheus_response(&state.metric_samples().await)
}

#[derive(Debug)]
pub(crate) enum RequestAdmissionError {
    Local(ConcurrencyError),
    Distributed(DistributedConcurrencyError),
}

pub async fn serve_tcp(bind: &str) -> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let router = build_router()?;
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}
