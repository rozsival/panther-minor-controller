# 🔐 Networking & Security

> The controller can switch a workstation off, so it is never exposed beyond your Tailscale network. The Pi is
> hardened by `setup-device.sh`: UFW blocks every inbound port except SSH, SSH accepts keys only, and fail2ban bans
> brute-force attempts.

**Related:** [Installation](installation.md) · [API reference](api.md) · [Operations](operations.md) ·
[Sleep & Wake-on-LAN](wake-on-lan.md)

---

## 🛡️ Security model

| Layer     | Behavior                                                                                |
| --------- | --------------------------------------------------------------------------------------- |
| Tailscale | Private network trusted clients connect through; the service starts after `tailscaled`  |
| UFW       | Default deny inbound, allow outbound; only the SSH port is open                         |
| SSH       | Custom port, public-key only, no root login, `AllowUsers` restricted, no TCP forwarding |
| fail2ban  | `sshd` jail: 3 failures within 10 minutes → banned for 1 hour                           |
| App       | No authentication — the network boundary is the access control                          |

The controller listens on `0.0.0.0:<PORT>`. UFW drops that port on the Pi's LAN interface, while Tailscale's own
packet filter rules accept traffic arriving over the tailnet interface — so the dashboard and API are reachable from
tailnet devices only.

> [!IMPORTANT]
> Anyone on your tailnet can power the workstation on and off. Use
> [Tailscale access controls](https://tailscale.com/kb/1018/acls) to restrict which devices or users may reach the Pi.

## 🌐 Port reference

| Port   | Service    | Reachable from  | Configured by                           |
| ------ | ---------- | --------------- | --------------------------------------- |
| `2222` | SSH        | LAN and tailnet | `PANTHER_SSH_PORT` in `setup-device.sh` |
| `8080` | Controller | Tailnet only    | `PORT` in the environment file          |

Outbound, the controller only opens TCP connections to `STATUS_HOST:STATUS_PORT` for the
[status probe](operations.md#-status-probe).

## 🔑 SSH hardening

`setup-device.sh` backs up `/etc/ssh/sshd_config` to `sshd_config.orig`, removes every `sshd_config.d/*.conf` drop-in
and applies:

| Setting                           | Value                 |
| --------------------------------- | --------------------- |
| `Port`                            | `2222` (configurable) |
| `PubkeyAuthentication`            | `yes`                 |
| `AuthenticationMethods`           | `publickey`           |
| `PasswordAuthentication`          | `no`                  |
| `KbdInteractiveAuthentication`    | `no`                  |
| `ChallengeResponseAuthentication` | `no`                  |
| `UsePAM`                          | `no`                  |
| `PermitRootLogin`                 | `no`                  |
| `MaxAuthTries`                    | `3`                   |
| `LoginGraceTime`                  | `30`                  |
| `X11Forwarding`                   | `no`                  |
| `AllowTcpForwarding`              | `no`                  |
| `AllowUsers`                      | The allowed user      |

The config is validated with `sshd -t` before SSH restarts; an invalid result restores the backup.

## 🚪 Access patterns

```bash
# Dashboard and API — from any tailnet device
open http://pi-zero:8080
curl http://pi-zero:8080/api/status

# Shell — over Tailscale or the LAN
ssh -p 2222 <user>@pi-zero
```

---

## ❓ FAQ

### Can LAN devices that are not on Tailscale reach the dashboard?

No. UFW only allows the SSH port on non-Tailscale interfaces. Join the device to the tailnet.

### Can I use an SSH tunnel instead of Tailscale?

No. `AllowTcpForwarding no` disables port forwarding. Use Tailscale, or relax that setting in
`/etc/ssh/sshd_config` deliberately.

### Why is there no login on the dashboard?

The controller is single-user by design and relies on Tailscale identity. Tailscale ACLs are the place to restrict
who can reach it.

### Is traffic to the dashboard encrypted?

The HTTP itself is plain, but it travels inside Tailscale's WireGuard tunnel end to end.

### I got banned by fail2ban. How do I get back in?

Wait one hour, or from another session run `sudo fail2ban-client set sshd unbanip <your-ip>`.
