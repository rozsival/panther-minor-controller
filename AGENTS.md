# AGENTS.md

Panther Minor Controller is a remote control for [Panther Minor](https://github.com/rozsival/panther-minor) AI workstation via GPIO relay on Raspberry Pi Zero 2 W.
It allows power on/off, force shutdown, and hard reset through a simple web dashboard and API.

Rust 2024 · Tokio · Hyper 1.x · MIT

## Documentation

- `README.md` — minimal project overview, quick start, ownership; links to `docs/`
- `docs/README.md` — documentation index; one domain per file in `docs/` (follow its conventions when adding or editing docs)
- `docs/api.md` — API contract (endpoints, response fields, state guards); update it with every endpoint change
- `docs/operations.md` — environment variables and status probe; update it with every new or changed variable

## Commands

```bash
cargo build --workspace                 # Build app on current platform
cargo run                               # Run app on current platform
cargo test --workspace                  # Run all tests
cargo clippy --workspace -- -D warnings # Lint with Clippy
cargo check --workspace                 # Check types without building
cargo fmt                               # Format code with rustfmt
pnpm install                            # Install Node.js deps (linting, formatting)
pnpm run prettier:write                 # Format non-Rust files (Markdown, JSON, TOML, YAML)
```

## Architecture

```
src/
├── main.rs    # HTTP server, routing, tests
├── gpio.rs    # Relay abstraction (Linux rppal / macOS stub / test mock)
├── html.rs    # Dashboard HTML
└── error.rs   # AppError + Result alias
```

- `RelayTrait` (async trait): real on Linux (`rppal`), stub on macOS (prints actions), mock in tests.
- `AppState` = `relay` + `power_on` state + `poll_ms`, injected into handlers.
- Binds `0.0.0.0:8080`, accessible only via Tailscale.
- GPIO pin via `GPIO_PIN` env (default: BCM 17).
- Power-on state tracked internally; guards prevent duplicate actions.

## API

| Method | Path             | Action       | Relay Behavior                             |
| ------ | ---------------- | ------------ | ------------------------------------------ |
| GET    | `/`              | Dashboard    | —                                          |
| GET    | `/api/health`    | Health       | Returns status, version, power_on, poll_ms |
| GET    | `/api/status`    | Status       | Returns power_on, poll_ms                  |
| POST   | `/api/power-on`  | Power on     | Short press 0.5s                           |
| POST   | `/api/power-off` | Graceful off | Short press 0.5s (ACPI signal)             |
| POST   | `/api/shutdown`  | Force off    | Long press 5s                              |
| POST   | `/api/reset`     | Hard reset   | 5s off, 2s pause, 0.5s on                  |

- API responses are JSON. Unknown paths → 404 JSON.
- Idempotency: action endpoints return 400 without touching the relay — power-on when already on; power-off, shutdown, and reset when already off.

## Testing

`MockRelay` — instant, no sleeps, no GPIO. Call counts tracked in shared `Arc<Mutex<HashMap>>`.

```bash
cargo test --workspace # All
cargo test power_on    # Filter by name
```

## Env

- `GPIO_PIN` — BCM pin (default: 17)
- `PORT` — HTTP port (default: 8080)
- `STATUS_POLL_MS` — Status polling interval (default: 2000)
- `STATUS_HOST` + `STATUS_PORT` — TCP status probe target (both or neither; see `docs/operations.md`)

## Release

`opt-level = "z"`, LTO, strip, single codegen unit.
