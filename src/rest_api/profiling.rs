//! Production performance profiling HTTP endpoints (#1330).
//!
//! Routes are registered only when:
//! 1. The crate is built with `--features profiling`, and
//! 2. `REST_API_PROFILING_ENABLED=true` at process start.
//!
//! Endpoints sit on the protected REST router under `/api/v1/debug/pprof/...`
//! and require the existing Admin role (`api_admin` after `api_reader`).
//!
//! See `docs/operations/profiling-runbook.md`.

use axum::http::StatusCode;
use axum::Json;

use super::dto::ErrorResponse;

/// Environment variable that gates route registration at runtime.
pub const PROFILING_ENABLED_ENV: &str = "REST_API_PROFILING_ENABLED";

/// Default CPU sample duration when `seconds` is omitted.
pub const DEFAULT_CPU_SECONDS: u64 = 30;
/// Inclusive lower bound for CPU profile duration.
pub const MIN_CPU_SECONDS: u64 = 1;
/// Inclusive upper bound for CPU profile duration (keeps capture bounded).
pub const MAX_CPU_SECONDS: u64 = 60;

/// True when `REST_API_PROFILING_ENABLED` is a truthy value (`1`, `true`, `yes`, `on`).
pub fn profiling_runtime_enabled() -> bool {
    match std::env::var(PROFILING_ENABLED_ENV) {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => false,
    }
}

/// Parse and bound the `seconds` query parameter for CPU profiling.
pub fn parse_cpu_seconds(raw: Option<&str>) -> Result<u64, (StatusCode, Json<ErrorResponse>)> {
    let Some(raw) = raw else {
        return Ok(DEFAULT_CPU_SECONDS);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(DEFAULT_CPU_SECONDS);
    }
    let seconds: u64 = trimmed.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "invalid_parameter",
                "seconds must be a positive integer",
            )),
        )
    })?;
    if !(MIN_CPU_SECONDS..=MAX_CPU_SECONDS).contains(&seconds) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse::new(
                "invalid_parameter",
                &format!(
                    "seconds must be between {MIN_CPU_SECONDS} and {MAX_CPU_SECONDS} inclusive"
                ),
            )),
        ));
    }
    Ok(seconds)
}

/// Attach profiling routes when the Cargo feature and runtime flag are both on.
///
/// Without the `profiling` feature, this is a no-op so default builds never
/// expose pprof endpoints.
#[cfg(feature = "profiling")]
pub fn attach_profiling_routes<S>(router: axum::Router<S>) -> axum::Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    use axum::middleware;
    use axum::routing::get;

    use super::auth;

    if !profiling_runtime_enabled() {
        tracing::info!(
            "{} is not enabled; profiling HTTP endpoints are not registered",
            PROFILING_ENABLED_ENV
        );
        return router;
    }

    tracing::warn!(
        "REST API profiling endpoints enabled at /api/v1/debug/pprof/* (Admin auth required)"
    );

    router
        .route(
            "/api/v1/debug/pprof/profile",
            get(cpu_profile).route_layer(middleware::from_fn(auth::api_admin)),
        )
        .route(
            "/api/v1/debug/pprof/heap",
            get(heap_profile).route_layer(middleware::from_fn(auth::api_admin)),
        )
}

#[cfg(not(feature = "profiling"))]
pub fn attach_profiling_routes<S>(router: axum::Router<S>) -> axum::Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if profiling_runtime_enabled() {
        tracing::warn!(
            "{} is set but the operator binary was built without the `profiling` Cargo feature; \
             endpoints are unavailable",
            PROFILING_ENABLED_ENV
        );
    }
    router
}

#[cfg(feature = "profiling")]
mod handlers {
    use std::time::Duration;

    use axum::extract::Query;
    use axum::http::{header, HeaderValue, StatusCode};
    use axum::response::Response;
    use axum::Json;
    use serde::Deserialize;
    use tokio::sync::Mutex;

    use super::{parse_cpu_seconds, ErrorResponse, MAX_CPU_SECONDS};

    /// Serializes CPU captures so concurrent requests cannot stack profilers.
    static CPU_PROFILE_LOCK: Mutex<()> = Mutex::const_new(());

    #[derive(Debug, Deserialize)]
    pub struct CpuProfileQuery {
        /// Capture duration in seconds (1..=60). Defaults to 30.
        pub seconds: Option<String>,
        /// Optional format; only `proto` (default) is accepted.
        pub format: Option<String>,
    }

    pub async fn cpu_profile(
        Query(q): Query<CpuProfileQuery>,
    ) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
        let seconds = parse_cpu_seconds(q.seconds.as_deref())?;
        // Only protobuf is supported. SVG flamegraphs would pull in `inferno`
        // (CDDL-1.0), which is outside this repository's cargo-deny allowlist.
        // Operators render flamegraphs locally with `pprof` / `go tool pprof`.
        if let Some(format) = q.format.as_deref() {
            let format = format.trim().to_ascii_lowercase();
            if !format.is_empty() && format != "proto" {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse::new(
                        "invalid_parameter",
                        "format must be 'proto' (SVG flamegraphs are generated offline with pprof)",
                    )),
                ));
            }
        }

        let Ok(guard) = CPU_PROFILE_LOCK.try_lock() else {
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorResponse::new(
                    "profiler_busy",
                    "another CPU profile is already in progress",
                )),
            ));
        };

        let result = tokio::task::spawn_blocking(move || capture_cpu(seconds))
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse::new(
                        "profiler_error",
                        &format!("profiler task failed: {e}"),
                    )),
                )
            })?;

        drop(guard);
        result
    }

    fn capture_cpu(seconds: u64) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
        let guard = pprof::ProfilerGuardBuilder::default()
            .frequency(100)
            .blocklist(&["libc", "libgcc", "pthread", "vdso"])
            .build()
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse::new(
                        "profiler_error",
                        &format!("failed to start CPU profiler: {e}"),
                    )),
                )
            })?;

        std::thread::sleep(Duration::from_secs(seconds.min(MAX_CPU_SECONDS)));

        let report = guard.report().build().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "profiler_error",
                    &format!("failed to build CPU profile: {e}"),
                )),
            )
        })?;

        let profile = report.pprof().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "profiler_error",
                    &format!("failed to encode pprof profile: {e}"),
                )),
            )
        })?;
        use prost::Message;
        let mut buf = Vec::new();
        profile.encode(&mut buf).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "profiler_error",
                    &format!("failed to serialize pprof profile: {e}"),
                )),
            )
        })?;

        let mut response = Response::new(axum::body::Body::from(buf));
        *response.status_mut() = StatusCode::OK;
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        response.headers_mut().insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=\"cpu-profile.pb\""),
        );
        Ok(response)
    }

    pub async fn heap_profile() -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
        let Some(ctl) = jemalloc_pprof::PROF_CTL.as_ref() else {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse::new(
                    "heap_unavailable",
                    "jemalloc profiling control is not available on this platform/build",
                )),
            ));
        };

        let mut prof_ctl = ctl.lock().await;
        if !prof_ctl.activated() {
            drop(prof_ctl);
            jemalloc_pprof::activate_jemalloc_profiling().await;
            prof_ctl = ctl.lock().await;
        }
        if !prof_ctl.activated() {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse::new(
                    "heap_inactive",
                    "jemalloc heap profiling is not activated; ensure the image was built with \
                     `--features profiling` and MALLOC_CONF includes prof:true",
                )),
            ));
        }

        let pprof = prof_ctl.dump_pprof().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "profiler_error",
                    &format!("failed to dump heap profile: {e}"),
                )),
            )
        })?;

        let mut response = Response::new(axum::body::Body::from(pprof));
        *response.status_mut() = StatusCode::OK;
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        response.headers_mut().insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("attachment; filename=\"heap-profile.pb\""),
        );
        Ok(response)
    }
}

#[cfg(feature = "profiling")]
use handlers::{cpu_profile, heap_profile};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::middleware::{self, Next};
    use axum::response::Response;
    use axum::routing::get;
    use axum::{Extension, Router};
    use tower::ServiceExt;

    use crate::rest_api::auth::{api_admin, RequestIdentity};
    use crate::rest_api::oidc::ApiRole;
    use crate::rest_api::versioning::{self, VersionPolicy};

    use super::*;

    #[test]
    fn runtime_flag_parser_truthy_values() {
        assert!(!parse_truthy(""));
        assert!(!parse_truthy("0"));
        assert!(!parse_truthy("false"));
        assert!(parse_truthy("true"));
        assert!(parse_truthy("1"));
        assert!(parse_truthy("YES"));
        assert!(parse_truthy("on"));
    }

    fn parse_truthy(v: &str) -> bool {
        let v = v.trim().to_ascii_lowercase();
        matches!(v.as_str(), "1" | "true" | "yes" | "on")
    }

    #[test]
    fn cpu_seconds_defaults_and_bounds() {
        assert_eq!(parse_cpu_seconds(None).unwrap(), DEFAULT_CPU_SECONDS);
        assert_eq!(parse_cpu_seconds(Some("10")).unwrap(), 10);
        assert_eq!(parse_cpu_seconds(Some("1")).unwrap(), 1);
        assert_eq!(parse_cpu_seconds(Some("60")).unwrap(), 60);
        assert_eq!(
            parse_cpu_seconds(Some("0")).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            parse_cpu_seconds(Some("61")).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            parse_cpu_seconds(Some("abc")).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            parse_cpu_seconds(Some("-1")).unwrap_err().0,
            StatusCode::BAD_REQUEST
        );
    }

    async fn inject_admin(mut request: Request<Body>, next: Next) -> Response {
        request.extensions_mut().insert(RequestIdentity {
            subject: "test-admin".into(),
            roles: vec![ApiRole::Admin],
            auth_type: "test".into(),
            groups: vec![],
        });
        next.run(request).await
    }

    async fn inject_reader(mut request: Request<Body>, next: Next) -> Response {
        request.extensions_mut().insert(RequestIdentity {
            subject: "test-reader".into(),
            roles: vec![ApiRole::Reader],
            auth_type: "test".into(),
            groups: vec![],
        });
        next.run(request).await
    }

    async fn stub_ok() -> &'static str {
        "profile-ok"
    }

    fn admin_gated_app() -> Router {
        let policy = Arc::new(VersionPolicy::default());
        Router::new()
            .route(
                "/api/v1/debug/pprof/profile",
                get(stub_ok).route_layer(middleware::from_fn(api_admin)),
            )
            .route(
                "/api/v1/debug/pprof/heap",
                get(stub_ok).route_layer(middleware::from_fn(api_admin)),
            )
            .layer(middleware::from_fn(versioning::inject_api_version_headers))
            .layer(Extension(policy))
    }

    #[tokio::test]
    async fn unauthenticated_cpu_profile_rejected_by_admin_gate() {
        let app = admin_gated_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/debug/pprof/profile?seconds=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn unauthenticated_heap_profile_rejected_by_admin_gate() {
        let app = admin_gated_app();
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/debug/pprof/heap")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn reader_role_forbidden_for_profiling() {
        let app = admin_gated_app().layer(middleware::from_fn(inject_reader));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/debug/pprof/profile?seconds=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn admin_reaches_profiling_handler() {
        let app = admin_gated_app().layer(middleware::from_fn(inject_admin));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/debug/pprof/profile?seconds=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn profiling_paths_follow_api_v1_versioning() {
        let app = admin_gated_app().layer(middleware::from_fn(inject_admin));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/debug/pprof/heap")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get("deprecation").is_none());
    }

    #[tokio::test]
    async fn missing_authorization_rejected_before_handler() {
        // Mirrors api_reader's missing-Bearer behavior on the protected router.
        async fn require_bearer(
            headers: axum::http::HeaderMap,
            request: Request<Body>,
            next: Next,
        ) -> Result<Response, StatusCode> {
            match headers
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
            {
                Some(v) if v.starts_with("Bearer ") => Ok(next.run(request).await),
                _ => Err(StatusCode::UNAUTHORIZED),
            }
        }

        let app = admin_gated_app().layer(middleware::from_fn(require_bearer));
        let cpu = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/debug/pprof/profile?seconds=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cpu.status(), StatusCode::UNAUTHORIZED);

        let heap = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/debug/pprof/heap")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(heap.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn attach_without_runtime_flag_does_not_panic() {
        let _ = profiling_runtime_enabled();
        let router = Router::new().route("/health", get(stub_ok));
        let _ = attach_profiling_routes(router);
    }

    #[cfg(feature = "profiling")]
    #[tokio::test]
    async fn invalid_cpu_format_rejected() {
        let app = Router::new()
            .route("/api/v1/debug/pprof/profile", get(cpu_profile))
            .layer(middleware::from_fn(inject_admin));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/debug/pprof/profile?seconds=1&format=xml")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[cfg(feature = "profiling")]
    #[tokio::test]
    async fn invalid_cpu_seconds_rejected_by_handler() {
        let app = Router::new()
            .route("/api/v1/debug/pprof/profile", get(cpu_profile))
            .layer(middleware::from_fn(inject_admin));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/debug/pprof/profile?seconds=999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
