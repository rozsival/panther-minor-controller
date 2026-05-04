#!/usr/bin/env bash
set -euo pipefail

# -- Configuration -------------------------------------------------------------
INSTALL_DIR="/opt/panther-minor-controller"
SERVICE_NAME="panther-minor-controller"

# Detect actual user (handles sudo context correctly)
# logname returns the original login name even when running under sudo
RUN_USER="${SUDO_USER:-$(logname 2>/dev/null || echo root)}"

SYSTEMD_UNIT="/etc/systemd/system/${SERVICE_NAME}.service"
BIN_URL="https://github.com/rozsival/panther-minor-controller/releases/latest/download/panther-minor-controller"
ENV_FILE="${INSTALL_DIR}/env"

# -- Helpers -------------------------------------------------------------------
log_info() { printf '\033[0;34m[INFO]\033[0m  %s\n' "$*"; }
log_success() { printf '\033[0;32m[OK]\033[0m    %s\n' "$*"; }
log_warn() { printf '\033[1;33m[WARN]\033[0m  %s\n' "$*"; }
log_error() {
  printf '\033[0;31m[ERROR]\033[0m %s\n' "$*" >&2
  exit 1
}

repeat_char() {
  local char="$1"
  local count="$2"
  local out=""

  while ((count > 0)); do
    out+="$char"
    ((count--))
  done

  printf '%s' "$out"
}

print_summary_table() {
  local title="$1"
  shift

  if (( $# % 2 != 0 )); then
    printf 'print_summary_table requires label/value pairs\n' >&2
    return 1
  fi

  local -a labels=()
  local -a values=()
  local label_width=0
  local inner_width=${#title}
  local i
  local row_text
  local border

  while (( $# > 0 )); do
    labels+=("$1")
    values+=("$2")
    if ((${#1} > label_width)); then
      label_width=${#1}
    fi
    shift 2
  done

  for ((i = 0; i < ${#labels[@]}; i++)); do
    row_text=$(printf "%-*s : %s" "$label_width" "${labels[i]}" "${values[i]}")
    if ((${#row_text} > inner_width)); then
      inner_width=${#row_text}
    fi
  done

  border="+$(repeat_char "-" $((inner_width + 2)))+"

  printf '\n\033[0;32m%s\033[0m\n' "$border"
  printf '\033[0;32m| %-*s |\033[0m\n' "$inner_width" "$title"
  printf '\033[0;32m|%s|\033[0m\n' "$(repeat_char "-" $((inner_width + 2)))"

  for ((i = 0; i < ${#labels[@]}; i++)); do
    row_text=$(printf "%-*s : %s" "$label_width" "${labels[i]}" "${values[i]}")
    printf '\033[0;32m| %-*s |\033[0m\n' "$inner_width" "$row_text"
  done

  printf '\033[0;32m%s\033[0m\n\n' "$border"
}

# -- Pre-flight ----------------------------------------------------------------
[[ $EUID -eq 0 ]] || log_error "This script must be run as root (use sudo)."

# -- Step 1: Download binary --------------------------------------------------
log_info "Downloading panther-minor-controller binary..."

mkdir -p "${INSTALL_DIR}/bin"
curl -fsSL "$BIN_URL" -o "${INSTALL_DIR}/bin/panther-minor-controller"
chmod +x "${INSTALL_DIR}/bin/panther-minor-controller"

log_success "Binary installed to ${INSTALL_DIR}/bin/panther-minor-controller."

# -- Step 2: Environment variables ---------------------------------------------
log_info "Configuring environment variables..."
cat >"$ENV_FILE" <<EOF
# Panther Minor Controller environment
GPIO_PIN=17
PORT=8080
EOF

chmod 600 "$ENV_FILE"
log_success "Environment file written to $ENV_FILE."

# -- Step 3: Systemd service --------------------------------------------------
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

# -- Summary -------------------------------------------------------------------
print_summary_table \
  "Panther Minor Controller installed!" \
  "Binary" "${INSTALL_DIR}/bin/panther-minor-controller" \
  "Service" "$SERVICE_NAME" \
  "Config" "$ENV_FILE" \
  "Unit" "$SYSTEMD_UNIT"

log_warn "⚠  Review and customize GPIO_PIN and PORT as needed."
echo ""

log_info "Useful commands:"
echo "  systemctl status ${SERVICE_NAME}"
echo "  journalctl -u ${SERVICE_NAME} -f"
echo "  systemctl restart ${SERVICE_NAME}"
