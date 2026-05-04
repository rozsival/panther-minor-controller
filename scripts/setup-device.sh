#!/usr/bin/env bash
set -euo pipefail

# -- Color output --------------------------------------------------------------
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

# -- Interactive prompts -------------------------------------------------------
echo ""
echo "🖲️  Panther Minor Controller — Device Setup"
echo "============================================"

# Detect actual user (handles sudo context correctly)
# logname returns the original login name even when running under sudo
ACTUAL_USER="${SUDO_USER:-$(logname 2>/dev/null || echo root)}"

# Default values before prompts (prevents unbound variable errors)
PANTHER_SERVER_NAME="${PANTHER_SERVER_NAME:-${HOSTNAME}}"
PANTHER_ALLOWED_USER="${PANTHER_ALLOWED_USER:-${ACTUAL_USER}}"
PANTHER_SSH_PORT="${PANTHER_SSH_PORT:-2222}"
PANTHER_TIMEZONE="${PANTHER_TIMEZONE:-Europe/Prague}"

# Prompt for values (use defaults if non-interactive)
if [[ $- == *i* ]] || [[ -t 0 ]]; then
  read -r -p "Enter server name (default: ${PANTHER_SERVER_NAME}): " server_name_in
  [[ -n "$server_name_in" ]] && PANTHER_SERVER_NAME="$server_name_in"

  read -r -p "Enter allowed user (default: ${PANTHER_ALLOWED_USER}): " allowed_user_in
  [[ -n "$allowed_user_in" ]] && PANTHER_ALLOWED_USER="$allowed_user_in"

  read -r -p "Enter SSH port (default: ${PANTHER_SSH_PORT}): " ssh_port_in
  [[ -n "$ssh_port_in" ]] && PANTHER_SSH_PORT="$ssh_port_in"

  read -r -p "Enter timezone (default: ${PANTHER_TIMEZONE}): " timezone_in
  [[ -n "$timezone_in" ]] && PANTHER_TIMEZONE="$timezone_in"
fi

print_summary_table \
  "Setup summary" \
  "Server name" "$PANTHER_SERVER_NAME" \
  "Allowed user" "$PANTHER_ALLOWED_USER" \
  "SSH port" "$PANTHER_SSH_PORT" \
  "Timezone" "$PANTHER_TIMEZONE"

read -r -p "Proceed with setup? (y/N): " confirm
[[ "$confirm" =~ ^[Yy]$ ]] || {
  log_warn "Setup cancelled."
  exit 0
}

# -- Step 1: Timezone ---------------------------------------------------------
log_info "Setting timezone to $PANTHER_TIMEZONE..."
timedatectl set-timezone "$PANTHER_TIMEZONE"
log_success "Timezone set to $PANTHER_TIMEZONE."

# -- Step 2: Update & essential packages --------------------------------------
log_info "Updating system and installing essential packages..."
apt update -y
apt upgrade -y
apt install -y \
  fail2ban \
  git \
  htop \
  jq \
  starship \
  tree \
  tmux \
  ufw \
  unattended-upgrades

log_success "Essential packages installed."

# -- Step 3: Git --------------------------------------------------------------
log_info "Configuring Git for $PANTHER_ALLOWED_USER..."
sudo -u "$PANTHER_ALLOWED_USER" git config --global user.name "$PANTHER_SERVER_NAME"
sudo -u "$PANTHER_ALLOWED_USER" git config --global user.email "${PANTHER_ALLOWED_USER}@${PANTHER_SERVER_NAME}"
sudo -u "$PANTHER_ALLOWED_USER" git config --global pull.rebase true
sudo -u "$PANTHER_ALLOWED_USER" git config --global credential.helper store
log_success "Git configured for $PANTHER_ALLOWED_USER."

# -- Step 4: SSH hardening ---------------------------------------------------
log_info "Hardening SSH (port $PANTHER_SSH_PORT, key-only auth)..."

SSHD_CONFIG="/etc/ssh/sshd_config"
if [[ ! -f "${SSHD_CONFIG}.orig" ]]; then
  cp "$SSHD_CONFIG" "${SSHD_CONFIG}.orig"
  log_info "Original sshd_config backed up."
fi

# Remove any drop-in overrides
rm -f /etc/ssh/sshd_config.d/*.conf

# Apply settings via sed (portable, no augeas dependency)
declare -A sshd_settings=(
  [Port]="$PANTHER_SSH_PORT"
  [PasswordAuthentication]="no"
  [KbdInteractiveAuthentication]="no"
  [ChallengeResponseAuthentication]="no"
  [PubkeyAuthentication]="yes"
  [AuthenticationMethods]="publickey"
  [UsePAM]="no"
  [PermitRootLogin]="no"
  [MaxAuthTries]="3"
  [LoginGraceTime]="30"
  [X11Forwarding]="no"
  [AllowTcpForwarding]="no"
)

for key in "${!sshd_settings[@]}"; do
  value="${sshd_settings[$key]}"
  if grep -qE "^#${key} " "$SSHD_CONFIG" 2>/dev/null; then
    sed -i "s|^#${key} .*|${key} ${value}|" "$SSHD_CONFIG"
  elif grep -qE "^${key} " "$SSHD_CONFIG" 2>/dev/null; then
    sed -i "s|^${key} .*|${key} ${value}|" "$SSHD_CONFIG"
  else
    echo "${key} ${value}" >>"$SSHD_CONFIG"
  fi
done

# AllowUsers (append if not present)
if ! grep -qE "^AllowUsers " "$SSHD_CONFIG"; then
  echo "AllowUsers $PANTHER_ALLOWED_USER" >>"$SSHD_CONFIG"
fi

# Validate before applying
if ! sshd -t 2>&1; then
  log_error "sshd configuration is invalid — aborting to avoid locking you out."
  cp "${SSHD_CONFIG}.orig" "$SSHD_CONFIG"
  log_info "Restored original sshd_config."
fi

systemctl restart ssh
log_success "SSH hardened on port $PANTHER_SSH_PORT."

# -- Step 5: UFW --------------------------------------------------------------
log_info "Configuring UFW firewall..."
ufw --force reset
ufw default deny incoming
ufw default allow outgoing
ufw allow "${PANTHER_SSH_PORT}/tcp" comment 'SSH'
ufw --force enable
log_success "UFW enabled. Open ports: SSH($PANTHER_SSH_PORT). All other traffic blocked."

# -- Step 6: GPIO group -------------------------------------------------------
log_info "Granting ${PANTHER_ALLOWED_USER} GPIO access..."
groupadd -f gpio
usermod -aG gpio "$PANTHER_ALLOWED_USER"
log_success "${PANTHER_ALLOWED_USER} added to the gpio group."

# -- Step 7: fail2ban ---------------------------------------------------------
log_info "Configuring fail2ban..."

JAIL_LOCAL="/etc/fail2ban/jail.local"
cat >"$JAIL_LOCAL" <<EOF
[sshd]
enabled  = true
port     = ${PANTHER_SSH_PORT}
filter   = sshd
logpath  = /var/log/auth.log
maxretry = 3
bantime  = 1h
findtime = 10m
EOF

systemctl enable --now fail2ban
systemctl restart fail2ban
log_success "fail2ban configured and running."

# -- Step 8: Tailscale --------------------------------------------------------
log_info "Installing Tailscale..."
curl -fsSL "https://pkgs.tailscale.com/stable/debian/trixie.noarmor.gpg" |
  tee /usr/share/keyrings/tailscale-archive-keyring.gpg >/dev/null
curl -fsSL "https://pkgs.tailscale.com/stable/debian/trixie.tailscale-keyring.list" |
  tee /etc/apt/sources.list.d/tailscale.list >/dev/null

apt update -y
apt install -y tailscale

if command -v tailscale >/dev/null 2>&1; then
  log_success "Tailscale installed. Run 'sudo tailscale up' to authenticate."
else
  log_error "Tailscale installation failed."
fi

# -- Step 9: Shell + Starship -------------------------------------------------
log_info "Setting up shell with Starship prompt for $PANTHER_ALLOWED_USER..."

# Ensure home dir exists
mkdir -p "/home/$PANTHER_ALLOWED_USER"

BASHRC="/home/$PANTHER_ALLOWED_USER/.bashrc"
if ! grep -qF 'starship init bash' "$BASHRC" 2>/dev/null; then
  printf '\n# Starship prompt\neval "$(starship init bash)"\n' >>"$BASHRC"
  chown "$PANTHER_ALLOWED_USER:$PANTHER_ALLOWED_USER" "$BASHRC"
fi

loginctl enable-linger "$PANTHER_ALLOWED_USER" 2>/dev/null || true
log_success "Shell set up with Starship prompt for $PANTHER_ALLOWED_USER."

# -- Summary ------------------------------------------------------------------
print_summary_table \
  "Panther Minor Controller setup complete!" \
  "Timezone" "$PANTHER_TIMEZONE" \
  "SSH port" "$PANTHER_SSH_PORT" \
  "User" "$PANTHER_ALLOWED_USER" \
  "Tailscale" "installed" \
  "Firewall" "UFW active" \
  "fail2ban" "active" \
  "Starship" "configured"

log_warn "⚠  To finish Tailscale setup, run: sudo tailscale up"
echo ""

log_info "Reconnection: ssh -p $PANTHER_SSH_PORT $PANTHER_ALLOWED_USER@<server-ip>"
