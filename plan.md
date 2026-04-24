# Panther Minor Controller: Implementation Plan

## 1. Architecture Overview

A minimalist, highly secure hardware appliance for remote power management of the Panther Minor server. Built around a
Raspberry Pi Zero 2 W operating exclusively within a Tailscale network, utilizing a statically compiled Rust binary and
a single-channel relay to control motherboard power states.

## 2. Hardware Specification

- **Controller:** Raspberry Pi Zero 2 W.
- **Actuator:** 1-channel 5V relay module.
- **Wiring:** Relay connected to the motherboard's front-panel PWR (Power Switch) pins.

## 3. Repository Structure

**Repository Name:** `panther-minor-controller` (Public)
A unified "appliance" repository containing both the application source code and the operating system provisioning
scripts.

```text
├── src/                  # Rust web application source code
├── Cargo.toml            # Rust dependencies (including 'rppal')
├── scripts/
│   ├── setup-device.sh   # OS hardening and network configuration
│   └── install-app.sh    # Automated deployment and systemd setup
├── .github/workflows/    # CI/CD pipelines
└── README.md
```

## 4. Implementation Phases

### Phase 1: Operating System Initialization

1. Flash **Raspberry Pi OS Lite** (64-bit) onto the SD card using Raspberry Pi Imager.
2. Pre-configure local Wi-Fi and inject SSH public keys for passwordless authentication.
3. Boot the device and join it to the Tailscale network (Tailnet).

### Phase 2: Device Hardening & Setup (`scripts/setup-device.sh`)

Extract the absolute minimum requirements from the existing `panther-minor` repository to create a stripped-down
provisioning script:

1. Configure the default shell environment.
2. Install essential utilities.
3. Perform SSH hardening (disable root login, disable password authentication).
4. Configure the firewall to restrict all incoming traffic exclusively to the `tailscale0` interface.

### Phase 3: The Rust Application (`src/`)

Develop a minimalist, dependency-free HTTP server (e.g., using `hyper` or standard libraries) serving a simple UI
control panel.

- **Target Architecture:** `aarch64-unknown-linux-musl` (statically linked).
- **Hardware Control:** Use the `rppal` crate for direct GPIO manipulation.
- **Endpoints / Actions:**
  - `/` - Dashboard displaying the UI.
  - `/api/power-on` - Short press (0.5s relay activation).
  - `/api/power-off` - Long press (5s relay activation).
  - `/api/reset` - Emulated hard reset (5s relay activation -> 2s pause -> 0.5s relay activation).
- **Networking:** The application will bind to a local port (e.g., `8080`) over raw HTTP, relying entirely on
  Tailscale's underlying WireGuard encryption for security.

### Phase 4: CI/CD Pipeline (`.github/workflows/release.yml`)

Implement a secure, automated build process for public distribution.

1. **Runner:** Standard GitHub-hosted Ubuntu runner (avoiding self-hosted runners for public repositories to prevent
   security vulnerabilities).
2. **Toolchain:** Utilize `cross` (via Docker) to cross-compile for the `aarch64` target.
3. **Optimization:** Implement `Swatinem/rust-cache` to mitigate long compilation times.
4. **Release Assets:** Upon a new Git tag, the CI will create a GitHub Release containing:

- The compiled `power_controller` binary.
- The `setup-device.sh` script.
- The `install-app.sh` script.

### Phase 5: Deployment Strategy (`scripts/install-app.sh`)

Provide a seamless, single-command installation method for the final appliance:

1. The user executes a standard bash pipe:
   `curl -s https://github.com/rozsival/panther-minor-controller/releases/latest/download/install-app.sh | sudo sh`.
2. The script downloads the compiled binary to `/opt/power_controller/`.
3. The script generates, registers, and enables a `systemd` service ensuring the application starts automatically on
   boot, specifically after `tailscaled.service` is online.
