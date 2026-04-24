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
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse GPIO pin from environment or use default
    let gpio_pin: Option<u8> = std::env::var("POWER_CONTROLLER_GPIO_PIN")
        .ok()
        .and_then(|v| v.parse().ok());

    let relay = Relay::new(gpio_pin)?;

    let state = AppState {
        relay: Arc::new(Mutex::new(relay)),
    };

    // Bind to localhost only — never accept remote connections
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
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

    // Only allow POST for API endpoints, GET for dashboard
    match (method.as_str(), path.as_str()) {
        ("GET", "/api/health") => Ok(json_response(
            StatusCode::OK,
            &serde_json::json!({
                "status": "healthy",
                "version": env!("CARGO_PKG_VERSION")
            }),
        )),

        ("GET", "/") => Ok(Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(
                dashboard_html(env!("CARGO_PKG_VERSION"))
                    .into_bytes()
                    .into(),
            )
            .unwrap()),

        ("POST", "/api/power-on") => {
            let mut relay = state.relay.lock().await;
            relay.short_press().await?;
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
            let mut relay = state.relay.lock().await;
            relay.long_press().await?;
            Ok(json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "status": "success",
                    "action": "power-off",
                    "message": "Long press (5s) sent"
                }),
            ))
        }

        ("POST", "/api/reset") => {
            let mut relay = state.relay.lock().await;
            relay.hard_reset().await?;
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
    }

    impl TestState {
        fn new() -> Self {
            let mock = MockRelay::new();
            let calls = mock.calls.clone();
            Self {
                relay: Arc::new(Mutex::new(mock)),
                calls,
            }
        }

        fn app_state(&self) -> AppState {
            AppState {
                relay: self.relay.clone(),
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

    // ── Power Off ────────────────────────────────────────────────────

    #[tokio::test]
    async fn power_off_returns_200() {
        let state = test_state();
        let resp = handle_request(request("POST", "/api/power-off"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn power_off_calls_long_press() {
        let state = test_state();
        assert_eq!(state.call_count("long_press"), 0);

        handle_request(request("POST", "/api/power-off"), state.app_state())
            .await
            .unwrap();

        assert_eq!(state.call_count("long_press"), 1);
    }

    #[tokio::test]
    async fn power_off_returns_correct_json() {
        let state = test_state();
        let resp = handle_request(request("POST", "/api/power-off"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["status"], "success");
        assert_eq!(json["action"], "power-off");
        assert_eq!(json["message"], "Long press (5s) sent");
    }

    // ── Reset ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn reset_returns_200() {
        let state = test_state();
        let resp = handle_request(request("POST", "/api/reset"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn reset_calls_hard_reset() {
        let state = test_state();
        assert_eq!(state.call_count("hard_reset"), 0);

        handle_request(request("POST", "/api/reset"), state.app_state())
            .await
            .unwrap();

        assert_eq!(state.call_count("hard_reset"), 1);
    }

    #[tokio::test]
    async fn reset_returns_correct_json() {
        let state = test_state();
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
    async fn multiple_power_on_calls_accumulate() {
        let state = test_state();
        for _ in 0..5 {
            handle_request(request("POST", "/api/power-on"), state.app_state())
                .await
                .unwrap();
        }
        assert_eq!(state.call_count("short_press"), 5);
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

        assert_eq!(state.call_count("short_press"), 2);
        assert_eq!(state.call_count("long_press"), 1);
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
            "/api/reset",
        ];
        let methods = ["GET", "POST", "POST", "POST"];

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
}
