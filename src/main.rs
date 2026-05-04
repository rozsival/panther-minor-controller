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
use hyper::{Request, Response, StatusCode, header};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tokio::time::{Duration, sleep, timeout};

const DEFAULT_PORT: u16 = 8080;
const DEFAULT_STATUS_POLL_MS: u64 = 2000;
const DEFAULT_CONFIRMATION_POLL_MS: u64 = 5000;
const STATUS_CONNECT_TIMEOUT_MS: u64 = 1000;
const POWER_ON_EXPECTED_DELAY_MS: u64 = 60_000;
const POWER_OFF_EXPECTED_DELAY_MS: u64 = 30_000;
const SHUTDOWN_EXPECTED_DELAY_MS: u64 = 15_000;
const RESET_EXPECTED_DELAY_MS: u64 = 75_000;

#[derive(Clone, Debug)]
struct StatusProbe {
    host: String,
    port: u16,
}

impl StatusProbe {
    fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }

    fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// Shared application state.
#[derive(Clone)]
struct AppState {
    relay: Arc<Mutex<dyn RelayTrait>>,
    /// Whether the device is currently powered on.
    power_on: std::sync::Arc<std::sync::RwLock<bool>>,
    /// Status polling interval in milliseconds (default: 2000).
    poll_ms: u64,
    /// Optional TCP reachability target used to detect the real device status.
    status_probe: Option<StatusProbe>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse GPIO pin from environment or use default
    let gpio_pin: Option<u8> = std::env::var("GPIO_PIN").ok().and_then(|v| v.parse().ok());

    let relay = Relay::new(gpio_pin)?;

    // Parse port from environment (default: 8080).
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_PORT);

    // Parse status poll interval from environment (default: 2000ms).
    let poll_ms: u64 = std::env::var("STATUS_POLL_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_STATUS_POLL_MS);
    let status_probe = status_probe_from_env()?;

    let state = AppState {
        relay: Arc::new(Mutex::new(relay)),
        power_on: Arc::new(std::sync::RwLock::new(false)),
        poll_ms,
        status_probe: status_probe.clone(),
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
    match status_probe {
        Some(probe) => println!("   Status probe: {}", probe.address()),
        None => println!("   Status probe: disabled (set STATUS_HOST and STATUS_PORT)"),
    }

    if state.status_probe.is_some() {
        let polling_state = state.clone();
        tokio::spawn(async move {
            status_poller(polling_state).await;
        });
    }

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

fn status_probe_from_env() -> Result<Option<StatusProbe>> {
    let status_host = std::env::var("STATUS_HOST")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let status_port = match std::env::var("STATUS_PORT") {
        Ok(value) => Some(
            value
                .parse::<u16>()
                .map_err(|e| AppError::Http(format!("Invalid STATUS_PORT '{value}': {e}")))?,
        ),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(AppError::Http(
                "STATUS_PORT must contain valid UTF-8 text".to_string(),
            ));
        }
    };

    match (status_host, status_port) {
        (Some(host), Some(port)) => Ok(Some(StatusProbe::new(host, port))),
        (None, None) => Ok(None),
        (Some(_), None) => Err(AppError::Http(
            "STATUS_PORT must be set when STATUS_HOST is configured".to_string(),
        )),
        (None, Some(_)) => Err(AppError::Http(
            "STATUS_HOST must be set when STATUS_PORT is configured".to_string(),
        )),
    }
}

async fn status_poller(state: AppState) {
    let Some(status_probe) = state.status_probe.clone() else {
        return;
    };

    loop {
        let power_on = probe_status(&status_probe).await;
        *state.power_on.write().unwrap() = power_on;
        sleep(Duration::from_millis(state.poll_ms)).await;
    }
}

async fn probe_status(status_probe: &StatusProbe) -> bool {
    matches!(
        timeout(
            Duration::from_millis(STATUS_CONNECT_TIMEOUT_MS),
            TcpStream::connect(status_probe.address()),
        )
        .await,
        Ok(Ok(_))
    )
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
        ("GET", "/api/health") => {
            let power_on = *state.power_on.read().unwrap();
            Ok(json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "status": "healthy",
                    "version": env!("CARGO_PKG_VERSION"),
                    "power_on": power_on,
                    "poll_ms": state.poll_ms
                }),
            ))
        }

        ("GET", "/api/status") => {
            let power_on = *state.power_on.read().unwrap();
            Ok(json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "power_on": power_on,
                    "poll_ms": state.poll_ms
                }),
            ))
        }

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
            if state.status_probe.is_none() {
                *state.power_on.write().unwrap() = true;
            }
            Ok(json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "status": "success",
                    "action": "power-on",
                    "message": "Short press (0.5s) sent",
                    "expected_delay_ms": POWER_ON_EXPECTED_DELAY_MS,
                    "confirmation_poll_ms": DEFAULT_CONFIRMATION_POLL_MS
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
            if state.status_probe.is_none() {
                *state.power_on.write().unwrap() = false;
            }
            Ok(json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "status": "success",
                    "action": "power-off",
                    "message": "Graceful shutdown signal sent (0.5s)",
                    "expected_delay_ms": POWER_OFF_EXPECTED_DELAY_MS,
                    "confirmation_poll_ms": DEFAULT_CONFIRMATION_POLL_MS
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
            if state.status_probe.is_none() {
                *state.power_on.write().unwrap() = false;
            }
            Ok(json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "status": "success",
                    "action": "shutdown",
                    "message": "Force shutdown (5s) sent",
                    "expected_delay_ms": SHUTDOWN_EXPECTED_DELAY_MS,
                    "confirmation_poll_ms": DEFAULT_CONFIRMATION_POLL_MS
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
            if state.status_probe.is_none() {
                *state.power_on.write().unwrap() = true;
            }
            Ok(json_response(
                StatusCode::OK,
                &serde_json::json!({
                    "status": "success",
                    "action": "reset",
                    "message": "Hard reset sequence sent (5s + 2s pause + 0.5s)",
                    "expected_delay_ms": RESET_EXPECTED_DELAY_MS,
                    "confirmation_poll_ms": DEFAULT_CONFIRMATION_POLL_MS
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
        /// Whether the device is powered on.
        power_on: Arc<std::sync::RwLock<bool>>,
        /// Status polling interval in ms.
        poll_ms: u64,
        /// Optional TCP status probe config.
        status_probe: Option<StatusProbe>,
    }

    impl TestState {
        fn new() -> Self {
            let mock = MockRelay::new();
            let calls = mock.calls.clone();
            Self {
                relay: Arc::new(Mutex::new(mock)),
                calls,
                power_on: Arc::new(std::sync::RwLock::new(false)),
                poll_ms: 2000,
                status_probe: None,
            }
        }

        fn with_status_probe(mut self, host: &str, port: u16) -> Self {
            self.status_probe = Some(StatusProbe::new(host, port));
            self
        }

        fn app_state(&self) -> AppState {
            AppState {
                relay: self.relay.clone(),
                power_on: self.power_on.clone(),
                poll_ms: self.poll_ms,
                status_probe: self.status_probe.clone(),
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

    // ── Status endpoint ──────────────────────────────────────────────

    #[tokio::test]
    async fn status_returns_200() {
        let state = test_state();
        let resp = handle_request(request("GET", "/api/status"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn status_returns_power_on_false() {
        let state = test_state();
        let resp = handle_request(request("GET", "/api/status"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["power_on"], false);
    }

    #[tokio::test]
    async fn status_returns_power_on_true_after_power_on() {
        let state = test_state();
        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        let resp = handle_request(request("GET", "/api/status"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["power_on"], true);
    }

    #[tokio::test]
    async fn status_returns_poll_ms() {
        let state = test_state();
        let resp = handle_request(request("GET", "/api/status"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["poll_ms"], 2000);
    }

    #[tokio::test]
    async fn status_has_json_content_type() {
        let state = test_state();
        let resp = handle_request(request("GET", "/api/status"), state.app_state())
            .await
            .unwrap();
        let ct = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(ct.starts_with("application/json"));
    }

    // ── Expected delay ───────────────────────────────────────────────

    #[tokio::test]
    async fn power_on_response_contains_expected_delay_ms() {
        let state = test_state();
        let resp = handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["expected_delay_ms"], POWER_ON_EXPECTED_DELAY_MS);
    }

    #[tokio::test]
    async fn power_off_response_contains_expected_delay_ms() {
        let state = test_state();
        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        let resp = handle_request(request("POST", "/api/power-off"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["expected_delay_ms"], POWER_OFF_EXPECTED_DELAY_MS);
    }

    #[tokio::test]
    async fn shutdown_response_contains_expected_delay_ms() {
        let state = test_state();
        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        let resp = handle_request(request("POST", "/api/shutdown"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["expected_delay_ms"], SHUTDOWN_EXPECTED_DELAY_MS);
    }

    #[tokio::test]
    async fn reset_response_contains_expected_delay_ms() {
        let state = test_state();
        handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        let resp = handle_request(request("POST", "/api/reset"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["expected_delay_ms"], RESET_EXPECTED_DELAY_MS);
    }

    #[tokio::test]
    async fn action_responses_include_confirmation_poll_ms() {
        let state = test_state();

        let power_on = handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        assert_eq!(
            body_json(power_on).await["confirmation_poll_ms"],
            DEFAULT_CONFIRMATION_POLL_MS
        );

        let power_off = handle_request(request("POST", "/api/power-off"), state.app_state())
            .await
            .unwrap();
        assert_eq!(
            body_json(power_off).await["confirmation_poll_ms"],
            DEFAULT_CONFIRMATION_POLL_MS
        );
    }

    #[tokio::test]
    async fn health_response_contains_poll_ms() {
        let state = test_state();
        let resp = handle_request(request("GET", "/api/health"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["poll_ms"], 2000);
    }

    #[tokio::test]
    async fn status_probe_reports_online_when_tcp_target_is_reachable() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let accept_task = tokio::spawn(async move {
            let _ = listener.accept().await.unwrap();
        });

        assert!(probe_status(&StatusProbe::new("127.0.0.1", addr.port())).await);

        accept_task.await.unwrap();
    }

    #[tokio::test]
    async fn power_on_does_not_optimistically_change_state_when_probe_is_enabled() {
        let state = test_state_with_status_probe();

        let resp = handle_request(request("POST", "/api/power-on"), state.app_state())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = handle_request(request("GET", "/api/status"), state.app_state())
            .await
            .unwrap();
        let json = body_json(resp).await;
        assert_eq!(json["power_on"], false);
    }

    fn test_state_with_status_probe() -> TestState {
        test_state().with_status_probe("127.0.0.1", 22)
    }
}
