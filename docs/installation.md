# 🛠️ Installation

> From a blank microSD card to a running controller: flash Raspberry Pi OS, harden the Pi with `setup-device.sh`,
> join Tailscale, then install the controller as a `systemd` service with `install-app.sh`.

**Related:** [Hardware & wiring](hardware.md) · [Networking & security](networking.md) ·
[Operations](operations.md) · [Dashboard](dashboard.md)

---

## 🧰 Prerequisites

- The hardware assembled and wired — see [Hardware & wiring](hardware.md)
- 🍇 [Raspberry Pi OS Lite](https://www.raspberrypi.com/software/operating-systems/) **64-bit**, flashed with
  [Raspberry Pi Imager](https://www.raspberrypi.com/software/) with:
  - SSH enabled with **public-key authentication**
  - Wi-Fi configured
  - A non-root user with `sudo` privileges
- A [Tailscale](https://tailscale.com/) account for secure remote access

> [!NOTE]
> The release binary targets `aarch64-unknown-linux-gnu`, so the 64-bit OS image is required. `setup-device.sh`
> installs Tailscale from its Debian **trixie** repository, matching the current Raspberry Pi OS release.

## 🚀 Install

Each script is published as an asset of every [GitHub release](https://github.com/rozsival/panther-minor-controller/releases).
The versioned download commands live in the [root README quick start](../README.md#-quick-start); the steps below
explain what each one does.

### 1. Prepare the Raspberry Pi — `setup-device.sh`

SSH into the Pi, download `setup-device.sh` and run it with `sudo`. It prompts for four values, prints a summary and
asks for confirmation:

| Prompt       | Default                  | Override variable      | Used for                                    |
| ------------ | ------------------------ | ---------------------- | ------------------------------------------- |
| Server name  | Current hostname         | `PANTHER_SERVER_NAME`  | Git identity (`user.name`, `user.email`)    |
| Allowed user | User that invoked `sudo` | `PANTHER_ALLOWED_USER` | SSH `AllowUsers`, `gpio` group, shell setup |
| SSH port     | `2222`                   | `PANTHER_SSH_PORT`     | `sshd`, UFW and fail2ban                    |
| Timezone     | `Europe/Prague`          | `PANTHER_TIMEZONE`     | `timedatectl set-timezone`                  |

Override variables must be passed through `sudo` to take effect, e.g.
`sudo PANTHER_TIMEZONE=UTC bash setup-device.sh`.

#### What `setup-device.sh` configures

| #   | Step      | What it does                                                                                                                  |
| --- | --------- | ----------------------------------------------------------------------------------------------------------------------------- |
| 1   | Timezone  | Sets the system timezone                                                                                                      |
| 2   | Packages  | `apt update` + `upgrade`; installs `fail2ban`, `git`, `htop`, `jq`, `starship`, `tree`, `tmux`, `ufw`, `unattended-upgrades`  |
| 3   | Git       | Name, email, `pull.rebase true` and the `store` credential helper for the allowed user                                        |
| 4   | SSH       | Backs up `sshd_config`, removes drop-ins, sets the port, key-only auth, no root login, `AllowUsers`; validates with `sshd -t` |
| 5   | UFW       | Resets the firewall, denies all inbound traffic except the SSH port                                                           |
| 6   | GPIO      | Adds the allowed user to the `gpio` group                                                                                     |
| 7   | fail2ban  | `sshd` jail on the SSH port: 3 retries within 10 minutes → 1 hour ban                                                         |
| 8   | Tailscale | Installs the Tailscale agent                                                                                                  |
| 9   | Shell     | Starship prompt in `.bashrc`, `loginctl enable-linger` for the allowed user                                                   |

Full hardening details are in [Networking & security](networking.md).

> [!WARNING]
> When the script finishes, SSH listens on **port 2222** (or your chosen port) with **key-based authentication
> only**. Keep the current session open and verify a new one before disconnecting:
> `ssh -p 2222 <user>@<pi-ip>`.

### 2. Join Tailscale

Authenticate the Pi to your [Tailscale network](https://login.tailscale.com/admin/):

```bash
sudo tailscale up
```

Open the printed link to authenticate. From then on, reach the Pi through its Tailscale hostname (e.g. `pi-zero`).

> [!TIP]
> [Disable key expiry](https://login.tailscale.com/admin/machines) for the Pi so it does not drop off the tailnet.

### 3. Install the controller — `install-app.sh`

Download `install-app.sh` and run it with `sudo`. It:

| Step | What it does                                                                                                  |
| ---- | ------------------------------------------------------------------------------------------------------------- |
| 1    | Downloads the **latest** release binary to `/opt/panther-minor-controller/bin/panther-minor-controller`       |
| 2    | Writes `/opt/panther-minor-controller/env` (mode `600`) with default settings                                 |
| 3    | Installs `panther-minor-controller.service`, ordered after `tailscaled.service`, running as the invoking user |
| 4    | Enables and starts the service; restarts it automatically 5s after a failure                                  |

> [!IMPORTANT]
> Edit `/opt/panther-minor-controller/env` to match your setup — at least `STATUS_HOST` and `STATUS_PORT` for
> accurate power state — then `sudo systemctl restart panther-minor-controller`. Every variable is described in
> [Operations](operations.md#-configuration).

### 4. Open the dashboard

Browse to `http://<pi-tailscale-hostname>:8080` (or your custom `PORT`). The [Dashboard](dashboard.md) page explains
the controls.

## ✅ Next steps

1. Configure the [status probe](operations.md#-status-probe)
2. Bookmark [Service management](operations.md#-service-management) and [Updating](operations.md#-updating)
3. Optionally set up [Sleep & Wake-on-LAN](wake-on-lan.md)

---

## ❓ FAQ

### I lost SSH access after running `setup-device.sh`. What happened?

SSH moved to the configured port (default `2222`), accepts only public keys and only the allowed user. Connect with
`ssh -p 2222 <user>@<pi-ip>` using the key you configured in Raspberry Pi Imager. If `sshd -t` had rejected the new
config, the script would have restored the backup at `/etc/ssh/sshd_config.orig`.

### Can I re-run `setup-device.sh`?

Yes. It is safe to re-run: the original `sshd_config` backup is kept, UFW is reset and re-applied, and the Starship
line is only appended once. `AllowUsers` is also only added when missing, so switching to a different allowed user
later requires editing `/etc/ssh/sshd_config` by hand.

### Does `install-app.sh` keep my configuration when re-run?

No. It rewrites `/opt/panther-minor-controller/env` with defaults. Use `update-app.sh` to upgrade an existing
installation — see [Updating](operations.md#-updating).

### Can I install a specific version?

The scripts always download the binary from the **latest** release. For an older version, download
`panther-minor-controller` from that release's assets and replace `/opt/panther-minor-controller/bin/panther-minor-controller`
manually, then restart the service.

### The service fails with a GPIO permission error. Why?

The service runs as the user who invoked `sudo bash install-app.sh`, and that user needs to be in the `gpio` group.
Run `setup-device.sh` as the same user, or `sudo usermod -aG gpio <user>`, then restart the service.
