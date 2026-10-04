# 📡 API Reference

> A small JSON API over plain HTTP for reading the workstation's power state and pressing its power button. It is
> what the dashboard uses, and it is reachable from any client on your Tailscale network.

**Related:** [Dashboard](dashboard.md) · [Architecture](architecture.md#-request-flow) ·
[Networking & security](networking.md) · [Operations](operations.md#-status-probe)

---

## 📋 Endpoint overview

Base URL: `http://<pi-tailscale-hostname>:8080` (or your `PORT`). No authentication; requests need no body.

| Method | Path             | Purpose                | Relay behavior                   | Rejected (`400`) when |
| ------ | ---------------- | ---------------------- | -------------------------------- | --------------------- |
| `GET`  | `/`              | HTML dashboard         | —                                | —                     |
| `GET`  | `/api/health`    | Health, version, state | —                                | —                     |
| `GET`  | `/api/status`    | Power state            | —                                | —                     |
| `POST` | `/api/power-on`  | Power on               | Short press 0.5s                 | Already **on**        |
| `POST` | `/api/power-off` | Graceful ACPI shutdown | Short press 0.5s                 | Already **off**       |
| `POST` | `/api/shutdown`  | Forced power-off       | Long press 5s                    | Already **off**       |
| `POST` | `/api/reset`     | Hard reset             | 5s press → 2s pause → 0.5s press | Already **off**       |

Every `/api/*` response is `application/json`. Any other method/path combination returns `404`.

## 🩺 Read endpoints

### `GET /api/health`

```bash
curl http://pi-zero:8080/api/health
```

```json
{
  "status": "healthy",
  "version": "X.Y.Z",
  "power_on": false,
  "poll_ms": 2000
}
```

| Field      | Type    | Description                                 |
| ---------- | ------- | ------------------------------------------- |
| `status`   | string  | Always `"healthy"` when the server responds |
| `version`  | string  | Controller version (`Cargo.toml`)           |
| `power_on` | boolean | Current power state                         |
| `poll_ms`  | number  | Configured `STATUS_POLL_MS`                 |

### `GET /api/status`

```bash
curl http://pi-zero:8080/api/status
```

```json
{
  "power_on": false,
  "poll_ms": 2000
}
```

`power_on` comes from the TCP status probe when `STATUS_HOST` and `STATUS_PORT` are configured; otherwise it reflects
the controller's last action — see [Power state tracking](architecture.md#-power-state-tracking).

## 🔘 Action endpoints

```bash
curl -X POST http://pi-zero:8080/api/power-on
```

The response is sent **after** the relay sequence completes, so a request takes at least as long as its press.

### Success — `200`

```json
{
  "status": "success",
  "action": "power-on",
  "message": "Short press (0.5s) sent",
  "expected_delay_ms": 60000,
  "confirmation_poll_ms": 5000
}
```

| Endpoint         | `action`    | `message`                                         | `expected_delay_ms` | Expected state |
| ---------------- | ----------- | ------------------------------------------------- | ------------------: | -------------- |
| `/api/power-on`  | `power-on`  | `Short press (0.5s) sent`                         |             `60000` | on             |
| `/api/power-off` | `power-off` | `Graceful shutdown signal sent (0.5s)`            |             `30000` | off            |
| `/api/shutdown`  | `shutdown`  | `Force shutdown (5s) sent`                        |             `15000` | off            |
| `/api/reset`     | `reset`     | `Hard reset sequence sent (5s + 2s pause + 0.5s)` |             `75000` | on             |

- `expected_delay_ms` — how long the workstation typically needs before `/api/status` can reflect the new state.
- `confirmation_poll_ms` — recommended polling interval while waiting for confirmation (always `5000`).

### 🛡️ State guards

Each action is rejected with `400` when the device is already in the state the action would put it in — or, for
reset, when there is nothing to reset. The relay is not touched.

```json
{
  "error": "Already on",
  "message": "Device is already powered on"
}
```

| Endpoint                                        | `error`       | `message`                       |
| ----------------------------------------------- | ------------- | ------------------------------- |
| `/api/power-on`                                 | `Already on`  | `Device is already powered on`  |
| `/api/power-off`, `/api/shutdown`, `/api/reset` | `Already off` | `Device is already powered off` |

## 🚫 Errors

| Status | When                                               | Body                       |
| ------ | -------------------------------------------------- | -------------------------- |
| `400`  | State guard rejected the action                    | `{ "error", "message" }`   |
| `404`  | Unknown path or wrong method (`PUT /api/power-on`) | `{ "error": "Not found" }` |

---

## ❓ FAQ

### How do I wait for an action to take effect in a script?

Do what the dashboard does: sleep `expected_delay_ms`, then poll `GET /api/status` every `confirmation_poll_ms` until
`power_on` matches the expected state.

```bash
curl -fsS -X POST http://pi-zero:8080/api/power-on && sleep 60
until curl -fsS http://pi-zero:8080/api/status | jq -e '.power_on' >/dev/null; do sleep 5; done
```

### Why did `/api/reset` return `400` when the workstation was hung?

Reset and shutdown are only accepted while the controller reports the device as **on**. A hung workstation often
stops answering on the probe port, so the probe reports it as off and both actions are rejected; only power-on is
accepted. In that case use the physical button, or check `/api/status` before choosing the action.

### Is there authentication?

No. Access control is the network boundary: the API is reachable only through Tailscale — see
[Networking & security](networking.md).

### Is the API versioned?

No. Changes to endpoints show up in the
[release notes](https://github.com/rozsival/panther-minor-controller/releases), which are generated from the
Conventional Commit messages since the previous tag.
