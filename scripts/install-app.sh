#!/usr/bin/env bash
set -euo pipefail

# ── Configuration ─────────────────────────────────────────────────────────────
INSTALL_DIR="/opt/panther-minor-controller"
SERVICE_NAME="panther-minor-controller"
RUN_USER="${USER:-root}"
SYSTEMD_UNIT="/etc/systemd/system/${SERVICE_NAME}.service"
BIN_URL="https://github.com/rozsival/panther-minor-controller/releases/latest/download/panther-minor-controller"
ENV_FILE="${INSTALL_DIR}/env"

# ── Helpers ───────────────────────────────────────────────────────────────────
log_info() { printf '\033[0;34m[INFO]\033[0m  %s\n' "$*"; }
log_success() { printf '\033[0;32m[OK]\033[0m    %s\n' "$*"; }
log_warn() { printf '\033[1;33m[WARN]\033[0m  %s\n' "$*"; }
log_error() {
  printf '\033[0;31m[ERROR]\033[0m %s\n' "$*" >&2
  exit 1
}

# ── Pre-flight ────────────────────────────────────────────────────────────────
[[ $EUID -eq 0 ]] || log_error "This script must be run as root (use sudo)."

# ── Step 1: Download binary ──────────────────────────────────────────────────
log_info "Downloading panther-minor-controller binary..."

mkdir -p "${INSTALL_DIR}/bin"
curl -fsSL "$BIN_URL" -o "${INSTALL_DIR}/bin/panther-minor-controller"
chmod +x "${INSTALL_DIR}/bin/panther-minor-controller"

log_success "Binary installed to ${INSTALL_DIR}/bin/panther-minor-controller."

# ── Step 2: Environment variables ─────────────────────────────────────────────
log_info "Configuring environment variables..."
cat >"$ENV_FILE" <<EOF
# Panther Minor Controller environment
GPIO_PIN=17
PORT=8080
EOF

chmod 600 "$ENV_FILE"
log_success "Environment file written to $ENV_FILE."

# ── Step 3: Systemd service ──────────────────────────────────────────────────
log_info "Installing systemd service..."
cat >"$SYSTEMD_UNIT" <<EOF
[Unit]
Description=Panther Minor Controller
After=tailscaled.service
Wants=tailscaled.service

[Service]
Type=simple
ExecStart=${INSTALL_DIR}/bin/panther-minor-controller
EnvironmentFile=${ENV_FILE}
Restart=on-failure
RestartSec=5
User=${RUN_USER}
Group=${RUN_USER}

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now "$SERVICE_NAME"
log_success "Systemd service '${SERVICE_NAME}' enabled and started."

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo -e "\033[0;32m╔═══════════════════════════════════════════╗\033[0m"
echo -e "\033[0;32m║  🖲️  Panther Minor Controller installed! ║\033[0m"
echo -e "\033[0;32m╠═══════════════════════════════════════════╣\033[0m"
printf "\033[0;32m║  Binary   : %-27s║\033[0m\n" "${INSTALL_DIR}/bin/panther-minor-controller"
printf "\033[0;32m║  Service  : %-27s║\033[0m\n" "$SERVICE_NAME"
printf "\033[0;32m║  Config   : %-27s║\033[0m\n" "$ENV_FILE"
printf "\033[0;32m║  Unit     : %-27s║\033[0m\n" "$SYSTEMD_UNIT"
echo -e "\033[0;32m╚═══════════════════════════════════════════╝\033[0m"
echo ""

log_warn "⚠  Review and customize GPIO_PIN and PORT as needed."
echo ""

log_info "Useful commands:"
echo "  systemctl status ${SERVICE_NAME}"
echo "  journalctl -u ${SERVICE_NAME} -f"
echo "  systemctl restart ${SERVICE_NAME}"
