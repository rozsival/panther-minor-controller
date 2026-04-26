#!/usr/bin/env bash
set -euo pipefail

# ── Configuration ─────────────────────────────────────────────────────────────
INSTALL_DIR="/opt/panther-minor-controller"
SERVICE_NAME="panther-minor-controller"
BIN_URL="https://github.com/rozsival/panther-minor-controller/releases/latest/download/panther-minor-controller"
BIN_PATH="${INSTALL_DIR}/bin/panther-minor-controller"

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

# ── Step 0: Verify existing installation ─────────────────────────────────────
if [[ ! -f "$BIN_PATH" ]]; then
  log_error "Binary not found at ${BIN_PATH}. Install first by running:\n  sudo ./install-app.sh"
fi

log_info "Found existing binary at ${BIN_PATH}."

# ── Step 1: Download new binary ──────────────────────────────────────────────
log_info "Downloading latest panther-minor-controller binary..."

mkdir -p "${INSTALL_DIR}/bin"
TMPBIN="${INSTALL_DIR}/bin/panther-minor-controller.tmp"
curl -fsSL "$BIN_URL" -o "$TMPBIN"
chmod +x "$TMPBIN"

log_success "Downloaded to ${TMPBIN}."

# ── Step 2: Confirm overwrite ────────────────────────────────────────────────
echo ""
log_warn "This will overwrite the existing binary:"
echo "  ${BIN_PATH}"
echo ""
read -rp "Continue? [y/N] " confirm
if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
  log_info "Aborted. No changes made."
  rm -f "$TMPBIN"
  exit 0
fi

# ── Step 3: Replace binary & restart ─────────────────────────────────────────
log_info "Stopping service..."
systemctl stop "$SERVICE_NAME" || true

log_info "Replacing binary..."
mv "$TMPBIN" "$BIN_PATH"
chmod +x "$BIN_PATH"

log_success "Binary updated to ${BIN_PATH}."

log_info "Starting service..."
systemctl start "$SERVICE_NAME"

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo -e "\033[0;32m╔═══════════════════════════════════════════╗\033[0m"
echo -e "\033[0;32m║  🔄  Panther Minor Controller updated!   ║\033[0m"
echo -e "\033[0;32m╠═══════════════════════════════════════════╣\033[0m"
printf "\033[0;32m║  Binary   : %-27s║\033[0m\n" "${BIN_PATH}"
printf "\033[0;32m║  Service  : %-27s║\033[0m\n" "$SERVICE_NAME"
echo -e "\033[0;32m╚═══════════════════════════════════════════╝\033[0m"
echo ""

log_info "Useful commands:"
echo "  systemctl status ${SERVICE_NAME}"
echo "  journalctl -u ${SERVICE_NAME} -f"
