<div align="center">

# 🖲️ Panther Minor Controller

### Remote power control for the [Panther Minor](https://github.com/rozsival/panther-minor) AI workstation

![Platform](https://img.shields.io/badge/Platform-Raspberry%20Pi%20Zero%202%20W-0A84FF)
![Architecture](https://img.shields.io/badge/Architecture-ARM64-E01F27)
![Language](https://img.shields.io/badge/Language-Rust-FE5E00)

A lightweight, secure remote control in a single binary for a **Raspberry Pi Zero 2 W**. A relay wired across the
workstation's power button lets you **power on**, **power off**, **force shutdown** and **hard reset** it from a web
dashboard or a JSON API, with real-time status tracking.

**[📚 Documentation](docs/README.md)** · [Installation](docs/installation.md) · [Hardware](docs/hardware.md) ·
[API](docs/api.md)

</div>

---

## ✨ Highlights

| Feature                  | What it gives you                                                                 |
| ------------------------ | --------------------------------------------------------------------------------- |
| **Web dashboard**        | Responsive page with live status, state-aware buttons and confirmation dialogs    |
| **REST API**             | JSON endpoints for scripts and automation, with idempotent state guards           |
| **Status tracking**      | TCP reachability probe keeps the reported state aligned with the real machine     |
| **Secure remote access** | Tailscale-only access, key-only SSH on port `2222`, UFW and fail2ban              |
| **Zero-touch install**   | One script hardens the Pi, another installs the controller as a `systemd` service |

## 🚀 Quick start

**Requires** a Raspberry Pi Zero 2 W running Raspberry Pi OS Lite 64-bit, a 5V relay wired to the workstation's power
button header and a Tailscale account — see [Hardware & wiring](docs/hardware.md) and
[Installation](docs/installation.md#-prerequisites).

```bash
# 1. Prepare and harden the Pi
wget https://github.com/rozsival/panther-minor-controller/releases/download/v1.0.10/setup-device.sh -O setup-device.sh
sudo bash setup-device.sh && rm setup-device.sh

# 2. Reconnect on port 2222 and join Tailscale
ssh -p 2222 <user>@<pi-ip>
sudo tailscale up

# 3. Install the controller service
wget https://github.com/rozsival/panther-minor-controller/releases/download/v1.0.10/install-app.sh -O install-app.sh
sudo bash install-app.sh && rm install-app.sh
```

Point the status probe at the workstation in `/opt/panther-minor-controller/env`, restart the service, then open
`http://<pi-tailscale-hostname>:8080`.

> [!WARNING]
> After `setup-device.sh`, SSH accepts **keys only on port 2222**. Keep your current session open until a second one
> connects.

To update an existing installation later:

```bash
wget https://github.com/rozsival/panther-minor-controller/releases/download/v1.0.10/update-app.sh -O update-app.sh
sudo bash update-app.sh && rm update-app.sh
```

## 📚 Documentation

Everything else — architecture, hardware, configuration, dashboard, API, networking, Wake-on-LAN and development —
lives in **[docs/](docs/README.md)**.

## 👤 Ownership

| Item       | Details                                                                                       |
| ---------- | --------------------------------------------------------------------------------------------- |
| Maintainer | [@rozsival](https://github.com/rozsival) (see [`CODEOWNERS`](CODEOWNERS))                     |
| Issues     | [GitHub Issues](https://github.com/rozsival/panther-minor-controller/issues)                  |
| Companion  | [Panther Minor](https://github.com/rozsival/panther-minor) — the AI workstation this controls |
| License    | [MIT](LICENSE)                                                                                |
