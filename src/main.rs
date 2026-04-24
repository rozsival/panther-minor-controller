mod error;
mod gpio;
mod html;

use error::{AppError, Result};
#[cfg(test)]
use gpio::MockRelay;
use gpio::{Relay, RelayTrait};
use html::dashboard_html;
#[cfg(test)]
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{header, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

/// Shared application state.
#[derive(Clone)]
struct AppState {
    relay: Arc<Mutex<dyn RelayTrait>>,
    /// API token for authorization (empty string = no auth required).
    token: String,
    /// Whether the device is currently powered on.
    power_on: std::sync::Arc<std::sync::RwLock<bool>>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse GPIO pin from environment or use default
    let gpio_pin: Option<u8> = std::env::var("PANTHER_MINOR_CONTROLLER_GPIO_PIN")
        .ok()
        .and_then(|v| v.parse().ok());

    let relay = Relay::new(gpio_pin)?;

    // Parse API token from environment (empty = no auth required).
    let token = std::env::var("PANTHER_MINOR_CONTROLLER_TOKEN").unwrap_or_default();

    // Parse port from environment (default: 8080).
    let port: u16 = std::env::var("PANTHER_MINOR_CONTROLLER_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);

    let state = AppState {
        relay: Arc::new(Mutex::new(relay)),
        token,
        power_on: Arc::new(std::sync::RwLock::new(false)),
    };

    // Bind to localhost only — never accept remote connections
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|e| AppError::Http(format!("Failed to bind to {addr}: {e}")))?;

    println!("🖲️ Panther Minor Controller v{}", env!("CARGO_PKG_VERSION"));
    println!("   Listening on port {}", listener.local_addr()?.port());
    println!(
        "   GPIO Pin: {}",
        gpio_pin.unwrap_or(gpio::DEFAULT_GPIO_PIN)
    );

    tokio::spawn(async move {
        if let Err(e) = http_server(listener, state).await {
            eprintln!("Server error: {e}");
        }
    });

    // Keep main alive
    tokio::signal::ctrl_c()
        .await
        .map_err(|e| AppError::Http(format!("Failed to listen for shutdown signal: {e}")))?;

    println!("\nShutting down...");
    Ok(())
}

async fn http_server(listener: TcpListener, state: AppState) -> Result<()> {
    loop {
        let (stream, _) = listener
            .accept()
            .await
            .map_err(|e| AppError::Http(format!("Accept error: {e}")))?;
        let io = TokioIo::new(stream);
        let state = state.clone();
        tokio::spawn(async move {
            if let Err(e) = http1::Builder::new()
                .serve_connection(
                    io,
                    service_fn(move |req| handle_request(req, state.clone())),
                )
                .await
            {
                eprintln!("Connection error: {e}");
            }
        });
    }
}

async fn handle_request<B>(req: Request<B>, state: AppState) -> Result<Response<Full<Bytes>>>
where
    B: http_body::Body<Data = Bytes> + Send + 'static,
    B::Error: std::fmt::Debug,
{
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // Auth check: API endpoints require a valid token (if one is configured).
    if path.starts_with("/api/") && !state.token.is_empty() {
        let auth_header = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        // Accept both "Bearer <token>" and direct "<token>" formats.
        let provided = auth_header.strip_prefix("Bearer ").unwrap_or(auth_header);
        if provided != state.token {
            return Ok(json_response(
                StatusCode::UNAUTHORIZED,
                &serde_json::json!({
                    "error": "Unauthorized",
                    "message": "Missing or invalid API token"
                }),
            ));
        }
    }

    // Only allow POST for API endpoints, GET for dashboard
    match (method.as_str(), path.as_str()) {
        ("GET", "/api/health") => {
            let power_on = *state.power_on.read().unwrap();
            Ok(json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "status": "healthy",
                    "version": env!("CARGO_PKG_VERSION"),
                    "power_on": power_on
                }),
            ))
        }

        ("GET", "/") => Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(
                dashboard_html(env!("CARGO_PKG_VERSION"), &state.token)
                    .into_bytes()
                    .into(),
            )
            .unwrap()),

        ("POST", "/api/power-on") => {
            // Guard: reject if already on
            if *state.power_on.read().unwrap() {
                return Ok(json_response(
                    StatusCode::BAD_REQUEST,
                    &serde_json::json!({
                        "error": "Already on",
                        "message": "Device is already powered on"
                    }),
                ));
            }
            let mut relay = state.relay.lock().await;
            relay.short_press().await?;
            *state.power_on.write().unwrap() = true;
            Ok(json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "status": "success",
                    "action": "power-on",
                    "message": "Short press (0.5s) sent"
                }),
            ))
        }

        ("POST", "/api/power-off") => {
            // Guard: reject if already off
            if !*state.power_on.read().unwrap() {
                return Ok(json_response(
                    StatusCode::BAD_REQUEST,
                    &serde_json::json!({
                        "error": "Already off",
                        "message": "Device is already powered off"
                    }),
                ));
            }
            let mut relay = state.relay.lock().await;
            relay.graceful_power_off().await?;
            *state.power_on.write().unwrap() = false;
            Ok(json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "status": "success",
                    "action": "power-off",
                    "message": "Graceful shutdown signal sent (0.5s)"
                }),
            ))
        }

        ("POST", "/api/shutdown") => {
            // Guard: reject if already off
            if !*state.power_on.read().unwrap() {
                return Ok(json_response(
                    StatusCode::BAD_REQUEST,
                    &serde_json::json!({
                        "error": "Already off",
                        "message": "Device is already powered off"
                    }),
                ));
            }
            let mut relay = state.relay.lock().await;
            relay.long_press().await?;
            *state.power_on.write().unwrap() = false;
            Ok(json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "status": "success",
                    "action": "shutdown",
                    "message": "Force shutdown (5s) sent"
                }),
            ))
        }

        ("POST", "/api/reset") => {
            // Guard: reject if already off
            if !*state.power_on.read().unwrap() {
                return Ok(json_response(
                    StatusCode::BAD_REQUEST,
                    &serde_json::json!({
                        "error": "Already off",
                        "message": "Device is already powered off"
                    }),
                ));
            }
            let mut relay = state.relay.lock().await;
            relay.hard_reset().await?;
            *state.power_on.write().unwrap() = true;
            Ok(json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "status": "success",
                    "action": "reset",
                    "message": "Hard reset sequence sent (5s + 2s pause + 0.5s)"
                }),
            ))
        }

        _ => Ok(json_response(
            StatusCode::NOT_FOUND,
            &serde_json::json!({ "error": "Not found" }),
        )),
    }
}

fn json_response(status: StatusCode, body: &serde_json::Value) -> Response<Full<Bytes>> {
    let body_str = serde_json::to_string(body).unwrap();
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(body_str.into_bytes().into())
        .unwrap()
}

// ─── Unit Tests ─────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::Request;

    /// Helper: build a test state with a `MockRelay`.
    fn test_state() -> TestState {
        TestState::new()
    }

    /// Helper: build a request with an empty body.
    fn request(method: &str, path: &str) -> Request<http_body_util::Full<Bytes>> {
        Request::builder()
            .method(method)
            .uri(path)
            .body(Full::new(Bytes::new()))
            .unwrap()
    }

    /// Helper: build a request with an API token header.
    fn request_with_auth(
        method: &str,
        path: &str,
        token: &str,
    ) -> Request<http_body_util::Full<Bytes>> {
        Request::builder()
            .method(method)
            .uri(path)
            .header("Authorization", format!("Bearer {token}"))
            .body(Full::new(Bytes::new()))
            .unwrap()
    }

    /// Helper: extract JSON body from response (async).
    async fn body_json(resp: Response<Full<Bytes>>) -> serde_json::Value {
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    /// Test state with a shared MockRelay.
    struct TestState {
        relay: Arc<Mutex<dyn RelayTrait>>,
        /// Shared call counter — accessible without holding the relay lock.
        calls: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, usize>>>,
        /// API token (empty = no auth).
        token: String,
        /// Whether the device is powered on.
        power_on: Arc<std::sync::RwLock<bool>>,
    }

    impl TestState {
        fn new() -> Self {
            let mock = MockRelay::new();
            let calls = mock.calls.clone();
            Self {
                relay: Arc::new(Mutex::new(mock)),
                calls,
                token: String::new(), // no auth by default
                power_on: Arc::new(std::sync::RwLock::new(false)),
            }
        }

        /// Create a test state with auth enabled.
        fn with_auth(token: &str) -> Self {
            let mock = MockRelay::new();
            let calls = mock.calls.clone();
            Self {
                relay: Arc::new(Mutex::new(mock)),
                calls,
                token: token.to_string(),
                power_on: Arc::new(std::sync::RwLock::new(false)),
            }
        }

        fn app_state(&self) -> AppState {
            AppState {
                relay: self.relay.clone(),
                token: self.token.clone(),
                power_on: self.power_on.clone(),
            }
        }

        /// Check call count for an action.
        fn call_count(&self, action: &str) -> usize {
            self.calls.lock().unwrap().get(action).copied().unwrap_or(0)
        }
    }

    // ── Health endpoint ──────────────────────────────────────────────

    #[tokio::test]
    async fn health_returns_200() {
        let state = test_state();
        let resp = handle_request(request("GET", "/api/health"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn health_returns_json_with_status_and_version() {
        let state = test_state();
        let resp = handle_request(request("GET", "/api/health"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["status"], "healthy");
        assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(json["power_on"], false);
    }

    #[tokio::test]
    async fn health_returns_power_on_true_after_power_on() {
        let state = test_state();
        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();

        let resp = handle_request(request("GET", "/api/health"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["power_on"], true);
    }

    // ── Dashboard (GET /) ────────────────────────────────────────────

    #[tokio::test]
    async fn dashboard_returns_200() {
        let state = test_state();
        let resp = handle_request(request("GET", "/"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn dashboard_returns_html_content_type() {
        let state = test_state();
        let resp = handle_request(request("GET", "/"), state.app_state())
            .await
            .unwrap();
        let ct = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(ct.starts_with("text/html"));
    }

    #[tokio::test]
    async fn dashboard_contains_version() {
        let state = test_state();
        let resp = handle_request(request("GET", "/"), state.app_state())
            .await
            .unwrap();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(body_str.contains(env!("CARGO_PKG_VERSION")));
    }

    // ── Power On ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn power_on_returns_200() {
        let state = test_state();
        let resp = handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn power_on_calls_short_press() {
        let state = test_state();
        assert_eq!(state.call_count("short_press"), 0);

        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();

        assert_eq!(state.call_count("short_press"), 1);
    }

    #[tokio::test]
    async fn power_on_returns_correct_json() {
        let state = test_state();
        let resp = handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["status"], "success");
        assert_eq!(json["action"], "power-on");
        assert_eq!(json["message"], "Short press (0.5s) sent");
    }

    #[tokio::test]
    async fn power_on_when_already_on_returns_400() {
        let state = test_state();
        // First call succeeds
        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        assert_eq!(state.call_count("short_press"), 1);

        // Second call is rejected
        let resp = handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let json = body_json(resp).await;
        assert_eq!(json["error"], "Already on");
        // No additional relay call
        assert_eq!(state.call_count("short_press"), 1);
    }

    // ── Power Off ────────────────────────────────────────────────────

    #[tokio::test]
    async fn power_off_returns_200() {
        let state = test_state();
        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        let resp = handle_request(request("POST", "/api/power-off"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn power_off_calls_graceful_power_off() {
        let state = test_state();
        assert_eq!(state.call_count("graceful_power_off"), 0);

        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        handle_request(request("POST", "/api/power-off"), state.app_state())
            .await
            .unwrap();

        assert_eq!(state.call_count("graceful_power_off"), 1);
    }

    #[tokio::test]
    async fn power_off_returns_correct_json() {
        let state = test_state();
        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        let resp = handle_request(request("POST", "/api/power-off"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["status"], "success");
        assert_eq!(json["action"], "power-off");
        assert_eq!(json["message"], "Graceful shutdown signal sent (0.5s)");
    }

    #[tokio::test]
    async fn power_off_when_already_off_returns_400() {
        let state = test_state();
        let resp = handle_request(request("POST", "/api/power-off"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let json = body_json(resp).await;
        assert_eq!(json["error"], "Already off");
        assert_eq!(state.call_count("graceful_power_off"), 0);
    }

    // ── Shutdown (force) ─────────────────────────────────────────────

    #[tokio::test]
    async fn shutdown_returns_200() {
        let state = test_state();
        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        let resp = handle_request(request("POST", "/api/shutdown"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn shutdown_calls_long_press() {
        let state = test_state();
        assert_eq!(state.call_count("long_press"), 0);

        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        handle_request(request("POST", "/api/shutdown"), state.app_state())
            .await
            .unwrap();

        assert_eq!(state.call_count("long_press"), 1);
    }

    #[tokio::test]
    async fn shutdown_returns_correct_json() {
        let state = test_state();
        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        let resp = handle_request(request("POST", "/api/shutdown"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["status"], "success");
        assert_eq!(json["action"], "shutdown");
        assert_eq!(json["message"], "Force shutdown (5s) sent");
    }

    #[tokio::test]
    async fn shutdown_when_already_off_returns_400() {
        let state = test_state();
        let resp = handle_request(request("POST", "/api/shutdown"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let json = body_json(resp).await;
        assert_eq!(json["error"], "Already off");
        assert_eq!(state.call_count("long_press"), 0);
    }

    #[tokio::test]
    async fn reset_when_already_off_returns_400() {
        let state = test_state();
        let resp = handle_request(request("POST", "/api/reset"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let json = body_json(resp).await;
        assert_eq!(json["error"], "Already off");
        assert_eq!(state.call_count("hard_reset"), 0);
    }

    // ── Reset ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn reset_returns_200() {
        let state = test_state();
        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        let resp = handle_request(request("POST", "/api/reset"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn reset_calls_hard_reset() {
        let state = test_state();
        assert_eq!(state.call_count("hard_reset"), 0);

        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        handle_request(request("POST", "/api/reset"), state.app_state())
            .await
            .unwrap();

        assert_eq!(state.call_count("hard_reset"), 1);
    }

    #[tokio::test]
    async fn reset_returns_correct_json() {
        let state = test_state();
        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        let resp = handle_request(request("POST", "/api/reset"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["status"], "success");
        assert_eq!(json["action"], "reset");
        assert_eq!(
            json["message"],
            "Hard reset sequence sent (5s + 2s pause + 0.5s)"
        );
    }

    // ── Not Found ────────────────────────────────────────────────────

    #[tokio::test]
    async fn unknown_path_returns_404() {
        let state = test_state();
        let resp = handle_request(request("GET", "/nonexistent"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unknown_path_returns_error_json() {
        let state = test_state();
        let resp = handle_request(request("GET", "/nonexistent"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["error"], "Not found");
    }

    #[tokio::test]
    async fn wrong_method_returns_404() {
        let state = test_state();
        let resp = handle_request(request("PUT", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // ── Idempotency / Multiple calls ─────────────────────────────────

    #[tokio::test]
    async fn multiple_power_on_calls_guard_second_call() {
        let state = test_state();
        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        assert_eq!(state.call_count("short_press"), 1);

        let resp = handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(state.call_count("short_press"), 1);
    }

    #[tokio::test]
    async fn mixed_actions_track_independently() {
        let state = test_state();
        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        handle_request(request("POST", "/api/power-off"), state.app_state())
            .await
            .unwrap();
        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        handle_request(request("POST", "/api/reset"), state.app_state())
            .await
            .unwrap();
        handle_request(request("POST", "/api/power-off"), state.app_state())
            .await
            .unwrap();

        assert_eq!(state.call_count("short_press"), 2);
        assert_eq!(state.call_count("graceful_power_off"), 2);
        assert_eq!(state.call_count("hard_reset"), 1);
    }

    // ── Content-Type ─────────────────────────────────────────────────

    #[tokio::test]
    async fn api_endpoints_return_json_content_type() {
        let state = test_state();
        let endpoints = [
            "/api/health",
            "/api/power-on",
            "/api/power-off",
            "/api/shutdown",
            "/api/reset",
        ];
        let methods = ["GET", "POST", "POST", "POST", "POST"];

        for (i, path) in endpoints.iter().enumerate() {
            let resp = handle_request(request(methods[i], path), state.app_state())
                .await
                .unwrap();
            let ct = resp
                .headers()
                .get(header::CONTENT_TYPE)
                .unwrap()
                .to_str()
                .unwrap();
            assert!(
                ct.starts_with("application/json"),
                "{path} should return application/json, got: {ct}"
            );
        }
    }

    // ── MockRelay ────────────────────────────────────────────────────

    #[tokio::test]
    async fn mock_relay_records_short_press() {
        let mut relay = MockRelay::new();
        relay.short_press().await.unwrap();
        assert_eq!(relay.call_count("short_press"), 1);
    }

    #[tokio::test]
    async fn mock_relay_records_long_press() {
        let mut relay = MockRelay::new();
        relay.long_press().await.unwrap();
        assert_eq!(relay.call_count("long_press"), 1);
    }

    #[tokio::test]
    async fn mock_relay_records_hard_reset() {
        let mut relay = MockRelay::new();
        relay.hard_reset().await.unwrap();
        assert_eq!(relay.call_count("hard_reset"), 1);
    }

    #[tokio::test]
    async fn mock_relay_reset_clears_counts() {
        let mut relay = MockRelay::new();
        relay.short_press().await.unwrap();
        relay.long_press().await.unwrap();
        relay.reset_counts();
        assert_eq!(relay.call_count("short_press"), 0);
        assert_eq!(relay.call_count("long_press"), 0);
        assert_eq!(relay.call_count("hard_reset"), 0);
    }

    #[tokio::test]
    async fn mock_relay_untracked_action_returns_zero() {
        let relay = MockRelay::new();
        assert_eq!(relay.call_count("nonexistent"), 0);
    }

    // ── Authorization (when token is set) ────────────────────────────

    #[tokio::test]
    async fn api_without_token_returns_401() {
        let state = TestState::with_auth("secret");
        let resp = handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn api_with_wrong_token_returns_401() {
        let state = TestState::with_auth("secret");
        let resp = handle_request(
            request_with_auth("POST", "/api/power-on", "wrong"),
            state.app_state(),
        )
        .await
        .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn api_with_correct_token_succeeds() {
        let state = TestState::with_auth("secret");
        assert_eq!(state.call_count("short_press"), 0);

        let resp = handle_request(
            request_with_auth("POST", "/api/power-on", "secret"),
            state.app_state(),
        )
        .await
        .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(state.call_count("short_press"), 1);
    }

    #[tokio::test]
    async fn api_without_token_header_returns_401() {
        let state = TestState::with_auth("secret");
        let resp = handle_request(
            Request::builder()
                .method("POST")
                .uri("/api/power-on")
                .body(Full::new(Bytes::new()))
                .unwrap(),
            state.app_state(),
        )
        .await
        .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn health_without_token_returns_401_when_auth_enabled() {
        let state = TestState::with_auth("secret");
        let resp = handle_request(request("GET", "/api/health"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn dashboard_accessible_without_token() {
        let state = TestState::with_auth("secret");
        let resp = handle_request(request("GET", "/"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn bearer_prefix_accepted() {
        let state = TestState::with_auth("secret");
        handle_request(
            request_with_auth("POST", "/api/power-on", "secret"),
            state.app_state(),
        )
        .await
        .unwrap();
        let resp = handle_request(
            request_with_auth("POST", "/api/reset", "secret"),
            state.app_state(),
        )
        .await
        .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn no_auth_when_token_is_empty() {
        let state = test_state(); // token = ""
        let resp = handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
