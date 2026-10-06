# ⚙️ Operations

> Day-to-day running of the controller on the Pi: the environment file, the status probe, managing the `systemd`
> service, reading logs and updating to a new release.

**Related:** [Installation](installation.md) · [Architecture](architecture.md#-power-state-tracking) ·
[API reference](api.md) · [Hardware & wiring](hardware.md)

---

## 🧾 Configuration

The controller reads its settings from environment variables. On the Pi they live in
`/opt/panther-minor-controller/env`, loaded by the service through `EnvironmentFile=`.

| Variable         | Default | Description                                                                    |
| ---------------- | ------- | ------------------------------------------------------------------------------ |
| `GPIO_PIN`       | `17`    | **BCM** number of the relay signal pin — see [GPIO pin](hardware.md#-gpio-pin) |
| `PORT`           | `8080`  | HTTP port for the dashboard and API                                            |
| `STATUS_POLL_MS` | `2000`  | Probe interval of the backend and status refresh interval of the dashboard     |
| `STATUS_HOST`    | —       | Hostname or IP address the status probe connects to                            |
| `STATUS_PORT`    | —       | TCP port the status probe connects to                                          |

The file written by `install-app.sh`:

```bash
# Panther Minor Controller environment
GPIO_PIN=17
PORT=8080
STATUS_POLL_MS=2000
# STATUS_HOST=192.168.1.50
# STATUS_PORT=2222
```

> [!IMPORTANT]
> Restart the service after every change: `sudo systemctl restart panther-minor-controller`.

### Validation rules

| Situation                                               | Result                               |
| ------------------------------------------------------- | ------------------------------------ |
| `GPIO_PIN`, `PORT` or `STATUS_POLL_MS` unparsable       | Silently falls back to the default   |
| `STATUS_HOST` set without `STATUS_PORT` (or vice versa) | Startup fails with an explicit error |
| `STATUS_PORT` not a valid port number                   | Startup fails with an explicit error |
| `STATUS_HOST` empty or whitespace only                  | Treated as unset                     |

## 🔦 Status probe

With `STATUS_HOST` and `STATUS_PORT` set, a background task opens a TCP connection to that address every
`STATUS_POLL_MS`. A connection established within **1 second** means _on_; anything else means _off_. Without them,
the controller only knows what it last did — see [Power state tracking](architecture.md#-power-state-tracking).

Pick a port that is open exactly while the workstation is up. For a standard
[Panther Minor](https://github.com/rozsival/panther-minor) install, its hardened SSH port works well:

```bash
# Panther Minor's LAN IP and SSH port
STATUS_HOST=192.168.1.50
STATUS_PORT=2222
```

> [!WARNING]
> `systemd` does not strip trailing comments in environment files — keep comments on their own lines.

> [!TIP]
> Prefer the workstation's **LAN address**. A Tailscale hostname only answers once the workstation's Tailscale agent
> is up, which adds boot time to every confirmation.

Check the active probe target in the startup log:

```text
🖲️ Panther Minor Controller vX.Y.Z
   Listening on port 8080
   GPIO Pin: 17
   Status probe: 192.168.1.50:2222
```

## 🔧 Service management

| Task           | Command                                                    |
| -------------- | ---------------------------------------------------------- |
| Show status    | `systemctl status panther-minor-controller`                |
| Follow logs    | `journalctl -u panther-minor-controller -f`                |
| Restart        | `sudo systemctl restart panther-minor-controller`          |
| Stop           | `sudo systemctl stop panther-minor-controller`             |
| Enable on boot | `sudo systemctl enable panther-minor-controller` (default) |

| File                                                         | Purpose                                                                |
| ------------------------------------------------------------ | ---------------------------------------------------------------------- |
| `/opt/panther-minor-controller/bin/panther-minor-controller` | Binary                                                                 |
| `/opt/panther-minor-controller/env`                          | Environment file (mode `600`)                                          |
| `/etc/systemd/system/panther-minor-controller.service`       | Unit: `Restart=on-failure`, `RestartSec=5`, after `tailscaled.service` |

## 📜 System logs

Raspberry Pi OS keeps the `systemd` journal in RAM by default
(`/usr/lib/systemd/journald.conf.d/40-rpi-volatile-storage.conf`), so a hang followed by a power cycle wipes every log
that could explain it. `setup-device.sh` overrides this with
`/etc/systemd/journald.conf.d/90-panther-minor-controller.conf`:

| Setting           | Value        | Why                                                                          |
| ----------------- | ------------ | ---------------------------------------------------------------------------- |
| `Storage`         | `persistent` | Journal lives in `/var/log/journal` and survives reboots                     |
| `SystemMaxUse`    | `100M`       | Bounds disk use and SD card wear                                             |
| `SyncIntervalSec` | `1m`         | A hard power cut loses at most about a minute of logs instead of 5 (default) |

| Task                             | Command                                        |
| -------------------------------- | ---------------------------------------------- |
| List boots (each reboot is one)  | `journalctl --list-boots`                      |
| Last messages before the reboot  | `journalctl -b -1 -e`                          |
| Warnings and errors of that boot | `journalctl -b -1 -p warning`                  |
| Controller logs of that boot     | `journalctl -b -1 -u panther-minor-controller` |
| Disk used by the journal         | `journalctl --disk-usage`                      |

A boot whose log ends abruptly, without the usual shutdown messages, ended in a hang or power loss rather than a clean
reboot.

## ⬆️ Updating

Download and run `update-app.sh` — the versioned command is in the
[root README quick start](../README.md#-quick-start). The script:

1. Exits with an error pointing to `install-app.sh` if no binary is installed yet
2. Downloads the **latest** release binary to a temporary file
3. Asks for confirmation before overwriting (answering anything but `y` removes the download)
4. Stops the service, replaces the binary and starts the service again

The environment file and the `systemd` unit are left untouched. Confirm the new version in the dashboard footer or
with `curl http://<pi>:8080/api/health`.

---

## ❓ FAQ

### The dashboard says "Offline" but the workstation is running. Why?

Either the probe is not configured (state starts as off after every controller restart) or the probe target does not
answer. From the Pi, test the target directly:
`timeout 1 bash -c '</dev/tcp/<STATUS_HOST>/<STATUS_PORT>' && echo open || echo closed`.

### The service keeps restarting. Where do I look?

`journalctl -u panther-minor-controller -e`. Typical causes are a half-configured status probe
(`STATUS_HOST must be set when STATUS_PORT is configured`), a GPIO setup error (`GPIO setup failed: …`) or the port
already being in use.

### Can I lower `STATUS_POLL_MS` for faster updates?

Yes. Each probe costs one TCP connection attempt with a 1-second timeout, so values down to about `1000` are
reasonable. The dashboard picks up the new interval on its next page load.

### Does updating reset my configuration?

No. `update-app.sh` replaces only the binary. Re-running `install-app.sh` would overwrite the environment file.
