# 📚 Panther Minor Controller Documentation

> Entry point to the Panther Minor Controller documentation. Each page covers **one domain**, opens with a
> one-paragraph summary, links its related pages, and ends with an FAQ where one is useful.

New here? Start with [Hardware & wiring](hardware.md), then [Installation](installation.md). The project overview and
quick start live in the [root README](../README.md).

---

## 🗂️ Document index

| Document                               | Read when you want to…                                | Key topics                                                               |
| -------------------------------------- | ----------------------------------------------------- | ------------------------------------------------------------------------ |
| [Architecture](architecture.md)        | Understand how the controller works                   | System map, components, relay implementations, request flow, power state |
| [Hardware & wiring](hardware.md)       | Buy parts and wire the relay                          | Bill of materials, wiring, GPIO pin, relay polarity, press timings       |
| [Installation](installation.md)        | Set up the Pi and install the controller              | Raspberry Pi OS, `setup-device.sh`, Tailscale, `install-app.sh`          |
| [Operations](operations.md)            | Configure, run, troubleshoot or update the controller | Environment variables, status probe, `systemd`, logs, `update-app.sh`    |
| [Dashboard](dashboard.md)              | Use the web interface                                 | Buttons, status indicator, confirmation dialogs, action verification     |
| [API reference](api.md)                | Automate power control from scripts or other tools    | Endpoints, response fields, state guards, errors                         |
| [Networking & security](networking.md) | Know who can reach the controller and how             | Tailscale, UFW, SSH hardening, fail2ban, ports                           |
| [Sleep & Wake-on-LAN](wake-on-lan.md)  | Suspend the workstation and wake it remotely          | BIOS, `ethtool`, `wakeonlan`, remote aliases                             |
| [Development](development.md)          | Contribute, run checks or cut a release               | Toolchain, simulation mode, tests, hooks, CI, releases, agent assets     |

## 🛤️ Reading paths

### First installation

1. [Hardware & wiring](hardware.md) — parts and the relay connection
2. [Installation](installation.md) — OS, device hardening, Tailscale, the service
3. [Operations](operations.md#-status-probe) — configure the status probe for accurate state
4. [Dashboard](dashboard.md) — press the first button

### Daily use

- [Dashboard](dashboard.md) or the [API reference](api.md) to control the workstation
- [Operations](operations.md) for logs, restarts and updates
- [Sleep & Wake-on-LAN](wake-on-lan.md) for power-saving suspend instead of shutdown

### Maintainers

- [Development](development.md) for checks, tests, commit rules and releases
- [Architecture](architecture.md) for the code layout and the power state model
- [API reference](api.md) for the contract every change must keep

## ✍️ Documentation conventions

| Rule            | Detail                                                                                                       |
| --------------- | ------------------------------------------------------------------------------------------------------------ |
| Location        | All user and maintainer docs live in `docs/`, one domain per file, kebab-case names                          |
| Page shape      | `# Title` → summary quote → **Related** line → sections → `## ❓ FAQ` (when useful)                          |
| Source of truth | Code and scripts (`src/main.rs` constants, `src/gpio.rs` timings, `scripts/*.sh`) win; docs explain them     |
| Agent context   | `AGENTS.md`, `.github/copilot-instructions.md` and `.agents/skills/*/SKILL.md` stay where their tools look   |
| Formatting      | Prettier (`printWidth: 120`) via `pnpm run prettier:write`; GitHub-flavored Markdown with alerts and Mermaid |
| Versions        | Only the root README carries the release tag (`/download/vX.Y.Z/`); the `release` skill bumps it             |
