#!/usr/bin/env bash
set -euo pipefail

# -- Configuration -------------------------------------------------------------
INSTALL_DIR="/opt/panther-minor-controller"
SERVICE_NAME="panther-minor-controller"
BIN_URL="https://github.com/rozsival/panther-minor-controller/releases/latest/download/panther-minor-controller"
BIN_PATH="${INSTALL_DIR}/bin/panther-minor-controller"

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

# -- Step 0: Verify existing installation -------------------------------------
if [[ ! -f "$BIN_PATH" ]]; then
  log_error "Binary not found at ${BIN_PATH}. Install first by running:\n  sudo ./install-app.sh"
fi

log_info "Found existing binary at ${BIN_PATH}."

# -- Step 1: Download new binary ----------------------------------------------
log_info "Downloading latest panther-minor-controller binary..."

mkdir -p "${INSTALL_DIR}/bin"
TMPBIN="${INSTALL_DIR}/bin/panther-minor-controller.tmp"
curl -fsSL "$BIN_URL" -o "$TMPBIN"
chmod +x "$TMPBIN"

log_success "Downloaded to ${TMPBIN}."

# -- Step 2: Confirm overwrite ------------------------------------------------
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

# -- Step 3: Replace binary & restart -----------------------------------------
log_info "Stopping service..."
systemctl stop "$SERVICE_NAME" || true

log_info "Replacing binary..."
mv "$TMPBIN" "$BIN_PATH"
chmod +x "$BIN_PATH"

log_success "Binary updated to ${BIN_PATH}."

log_info "Starting service..."
systemctl start "$SERVICE_NAME"

# -- Summary -------------------------------------------------------------------
print_summary_table \
  "Panther Minor Controller updated!" \
  "Binary" "${BIN_PATH}" \
  "Service" "$SERVICE_NAME"

log_info "Useful commands:"
echo "  systemctl status ${SERVICE_NAME}"
echo "  journalctl -u ${SERVICE_NAME} -f"
