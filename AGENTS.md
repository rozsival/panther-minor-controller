# AGENTS.md

Panther Minor Controller is a remote control for [Panther Minor](https://github.com/rozsival/panther-minor) AI workstation via GPIO relay on Raspberry Pi Zero 2 W.
It allows power on/off and hard reset through a simple web dashboard and API.

Rust 2021 · Tokio · Hyper 1.x · MIT

## Commands

```bash
cargo build
cargo run
cargo test
cargo clippy
cargo fmt
```

## Architecture

```
src/
├── main.rs    # HTTP server, routing, tests
├── gpio.rs    # Relay abstraction (Linux rppal / macOS stub / test mock)
├── html.rs    # Dashboard HTML
└── error.rs   # AppError + Result alias
```

- `RelayTrait` (async trait): real on Linux, stub on macOS, mock in tests.
- `AppState` = `Arc<Mutex<dyn RelayTrait>>`, injected into handlers.
- Binds `127.0.0.1:8080` only — no remote access.
- GPIO pin via `POWER_CONTROLLER_GPIO_PIN` env (default: BCM 17).

## API

| Method | Path             | Action     | Relay Behavior         |
| ------ | ---------------- | ---------- | ---------------------- |
| GET    | `/`              | Dashboard  | —                      |
| GET    | `/api/health`    | Health     | —                      |
| POST   | `/api/power-on`  | Power on   | Short press 0.5s       |
| POST   | `/api/power-off` | Power off  | Long press 5s          |
| POST   | `/api/reset`     | Hard reset | 5s off, 2s pause, 0.5s |

API responses JSON. Unknown paths → 404 JSON.

## Testing

`MockRelay` — instant, no sleeps, no GPIO. Call counts in shared `HashMap` (no relay lock needed).

```bash
cargo test --workspace # All
cargo test power_on    # Filter by name
```

## Files

```text
Cargo.{toml,lock}
src/{main, gpio, html, error}.rs
scripts/
package.json
lefthook.yml
README.md
```

## Env

`POWER_CONTROLLER_GPIO_PIN` — BCM pin (default: 17)

## Release

`opt-level = "z"`, LTO, strip, single codegen unit.
