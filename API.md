# 📡 API Reference

Panther Minor Controller exposes a REST API for monitoring and controlling the workstation's power state. All endpoints are accessible through your Tailscale network.

## Health

```http
GET /api/health
```

Returns system health information:

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "power_on": false,
  "poll_ms": 2000
}
```

## Status

```http
GET /api/status
```

Returns current power state:

```json
{
  "power_on": false,
  "poll_ms": 2000
}
```

## Power On

```http
POST /api/power-on
```

Sends a short press (0.5s) to power on the workstation. Returns `400` if already on.

## Power Off

```http
POST /api/power-off
```

Sends a short press (0.5s) to trigger a graceful ACPI shutdown. Returns `400` if already off.

## Shutdown

```http
POST /api/shutdown
```

Sends a long press (5s) to force shutdown the workstation. Returns `400` if already off.

## Hard Reset

```http
POST /api/reset
```

Performs a hard reset sequence: 5s off → 2s pause → 0.5s on. Returns `400` if already off.

## Error responses

Unknown paths return `404` JSON:

```json
{
  "error": "Not found"
}
```
