#!/usr/bin/env bash
# ╭───────────────────────────────────────────────────────────────╮
# │ Creed installer - Arch-based systems + Hyprland ≥ 0.57         │
# │ Asks for your sudo password once (packages + system files).    │
# ╰───────────────────────────────────────────────────────────────╯
# Usage: ./install.sh [--yes] [--dry-run] [--update] [--no-tweaks] [--no-assets] [--mode <id>]
set -Eeuo pipefail

SRC=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
PREFIX=/usr
LOG_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/creed"
LOG="$LOG_DIR/install.log"
YES=0 DRY=0 UPDATE=0 TWEAKS=1 ASSETS=1 MODE=""

while [ $# -gt 0 ]; do
  case "$1" in
    --yes|-y) YES=1 ;;
    --dry-run) DRY=1 ;;
    --update) UPDATE=1 ;;
    --no-tweaks) TWEAKS=0 ;;
    --no-assets) ASSETS=0 ;;
    --mode) MODE=${2:-}; shift ;;
    -h|--help) sed -n '2,7p' "$0"; exit 0 ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
  shift
done

mkdir -p "$LOG_DIR"
: > "$LOG"
exec 3>&1
trap 'echo; echo "✗ Something went wrong on line $LINENO. Details: $LOG" >&3' ERR

# ───────────── UI (gum when available, plain prompts otherwise) ─────────────
has() { command -v "$1" >/dev/null 2>&1; }
interactive() { [ "$YES" = 0 ] && [ -t 0 ]; }
say() { if has gum; then gum style --foreground 212 "$*"; else printf '\033[1;35m%s\033[0m\n' "$*"; fi; }
info() { printf '  %s\n' "$*"; }
ok() { printf '  \033[32m✓\033[0m %s\n' "$*"; }
warn() { printf '  \033[33m!\033[0m %s\n' "$*"; }
confirm() { # confirm "question" [default yes=1]
  if ! interactive; then return 0; fi
  if has gum; then gum confirm "$1"; else read -rp "$1 [Y/n] " a; [[ ! "$a" =~ ^[Nn] ]]; fi
}
run() { # logs and (unless dry-run) executes
  echo "+ $*" >> "$LOG"
  if [ "$DRY" = 1 ]; then echo "  [dry-run] $*"; return 0; fi
  "$@" >> "$LOG" 2>&1
}
spin() { # spin "title" cmd...
  local title=$1; shift
  if [ "$DRY" = 1 ]; then echo "  [dry-run] $title: $*"; return 0; fi
  echo "+ $*" >> "$LOG"
  if has gum && [ -t 1 ]; then gum spin --title "$title" -- bash -c '"$@" >> "'"$LOG"'" 2>&1' _ "$@"
  else info "$title…"; "$@" >> "$LOG" 2>&1; fi
}
sudo_run() { run sudo "$@"; }

banner() {
  if has gum; then
    gum style --border rounded --border-foreground 212 --padding "1 3" --margin "1 0" \
      "Creed" "A soft, fast Hyprland desktop - and every OS look you like."
  else
    printf '\n  \033[1;35mCreed\033[0m - a soft, fast Hyprland desktop\n\n'
  fi
}

# ───────────── preflight ─────────────
preflight() {
  say "Checking your system"
  [ "$(id -u)" -ne 0 ] || { echo "Run the installer as your normal user, not root." >&2; exit 1; }
  if ! { [ -f /etc/arch-release ] || grep -qiE '^(ID|ID_LIKE)=.*arch' /etc/os-release 2>/dev/null; }; then
    echo "Creed supports Arch Linux and Arch-based distributions only." >&2; exit 1
  fi
  ok "Arch-based system"
  has sudo || { echo "sudo is required." >&2; exit 1; }
  if [ "$DRY" = 0 ]; then
    echo "  Creed needs your password once to install packages and system files."
    sudo -v || { echo "sudo authentication failed." >&2; exit 1; }
    # Keep sudo alive for the rest of the install.
    ( while true; do sudo -n true 2>/dev/null; sleep 50; kill -0 "$$" 2>/dev/null || exit; done ) &
  fi
  ok "Administrator access"
  if [ "$DRY" = 0 ] && ! curl -fsS --max-time 8 -o /dev/null https://archlinux.org 2>/dev/null; then
    warn "No internet connection detected - package installation may fail"
  fi
  if ! id -nG | tr ' ' '\n' | grep -qx wheel; then
    warn "You are not in the 'wheel' group: system tweaks will ask for a password each time"
  fi
}

hardware_summary() {
  say "Your hardware"
  local gpu=""
  gpu=$( (for d in /sys/class/drm/card[0-9]*; do [ -e "$d/device/vendor" ] || continue; case "$(cat "$d/device/vendor")" in 0x10de) echo NVIDIA;; 0x1002) echo AMD;; 0x8086) echo Intel;; esac; done) 2>/dev/null | sort -u | paste -sd+ - || true)
  info "GPU:     ${gpu:-unknown}"
  info "CPU:     $(grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2 | sed 's/^ //' || true)"
  info "RAM:     $(awk '/MemTotal/ {printf "%.1f GB", $2/1048576}' /proc/meminfo)"
  if grep -qs Battery /sys/class/power_supply/*/type; then info "Type:    laptop"; else info "Type:    desktop"; fi
}

# ───────────── packages ─────────────
PKGS_CORE=(base-devel hyprland hyprlock hypridle xdg-desktop-portal-hyprland xdg-desktop-portal-gtk
  qt5-wayland qt6-wayland qt6ct polkit wireplumber pipewire pipewire-pulse libpulse
  networkmanager bluez bluez-utils brightnessctl playerctl power-profiles-daemon
  grim slurp hyprpicker wl-clipboard cliphist libnotify jq curl git rust gum sassc
  python python-gobject gtk4 libadwaita cairo pango
  inter-font ttf-jetbrains-mono noto-fonts noto-fonts-emoji papirus-icon-theme adwaita-icon-theme adw-gtk-theme
  foot thunar gvfs starship btop pacman-contrib zram-generator)
PKGS_OPTIONAL=(hyprpolkitagent hyprsunset gamemode mangohud gamescope kitty sddm qt6-svg qt6-declarative)

install_packages() {
  say "Installing packages"
  local want=() missing=()
  for p in "${PKGS_CORE[@]}" "${PKGS_OPTIONAL[@]}"; do
    if pacman -Qq "$p" >/dev/null 2>&1; then continue; fi
    # rustup users already have cargo; the `rust` package would conflict with it.
    if [ "$p" = rust ] && has cargo; then continue; fi
    if pacman -Si "$p" >/dev/null 2>&1 || pacman -Sg "$p" >/dev/null 2>&1; then want+=("$p"); else missing+=("$p"); fi
  done
  for m in "${missing[@]}"; do
    if printf '%s\n' "${PKGS_CORE[@]}" | grep -qx "$m"; then warn "Package '$m' not found in your repositories"; fi
  done
  if [ ${#want[@]} -eq 0 ]; then ok "All packages already installed"; return; fi
  info "Installing ${#want[@]} packages: ${want[*]}"
  spin "Installing packages" sudo pacman -S --needed --noconfirm "${want[@]}"
  ok "Packages installed"
  local hv
  hv=$(Hyprland --version 2>/dev/null | awk 'NR==1{print $2}')
  if [ -n "$hv" ] && [ "$(printf '%s\n0.57.0\n' "$hv" | sort -V | head -1)" != "0.57.0" ]; then
    echo "Hyprland $hv is too old - Creed needs 0.57 or newer (Lua config). Update your system first." >&2; exit 1
  fi
}

enable_services() {
  for svc in NetworkManager bluetooth power-profiles-daemon; do
    if systemctl list-unit-files "$svc.service" >/dev/null 2>&1 && ! systemctl is-enabled "$svc" >/dev/null 2>&1; then
      sudo_run systemctl enable --now "$svc" || warn "Could not enable $svc"
    fi
  done
  ok "Services enabled"
}

# ───────────── build + install files ─────────────
build() {
  say "Building Creed"
  has cargo || { echo "cargo is missing (package 'rust')." >&2; exit 1; }
  spin "Compiling (first build takes a few minutes)" cargo build --release --locked --manifest-path "$SRC/Cargo.toml"
  ok "Built creed, creed-shell"
}

install_files() {
  say "Installing Creed files"
  local B="$SRC/target/release" D="$PREFIX/share/creed"
  sudo_run install -Dm755 "$B/creed" "$PREFIX/bin/creed"
  sudo_run install -Dm755 "$B/creed-shell" "$PREFIX/bin/creed-shell"
  sudo_run install -Dm755 "$SRC/settings/creed-settings" "$PREFIX/bin/creed-settings"
  sudo_run install -Dm755 "$SRC/helper/creed-helper" "$PREFIX/lib/creed/creed-helper"
  sudo_run install -Dm644 "$SRC/packaging/polkit/50-creed.rules" "$PREFIX/share/polkit-1/rules.d/50-creed.rules"
  sudo_run install -Dm644 "$SRC/packaging/creed-settings.desktop" "$PREFIX/share/applications/creed-settings.desktop"
  sudo_run install -Dm644 "$SRC/packaging/creed.svg" "$PREFIX/share/icons/hicolor/scalable/apps/creed-settings.svg"
  sudo_run rm -rf "$D.new"
  sudo_run install -d "$D.new"
  sudo_run cp -r "$SRC/modes" "$SRC/templates" "$SRC/tweaks" "$SRC/scripts" "$SRC/defaults.toml" "$D.new/"
  for set in Creed-Fluent Creed-Material; do
    sudo_run rm -rf "/usr/share/icons/$set"
    sudo_run cp -r "$SRC/icons/$set" "/usr/share/icons/$set"
    sudo_run chmod -R u=rwX,go=rX "/usr/share/icons/$set"
  done
  sudo_run chown -R root:root "$D.new"
  sudo_run chmod -R u=rwX,go=rX "$D.new"
  sudo_run rm -rf "$D.old"
  if [ -d "$D" ]; then sudo_run mv "$D" "$D.old"; fi
  sudo_run mv "$D.new" "$D"
  sudo_run rm -rf "$D.old"
  if [ -d "$SRC/sddm" ]; then
    for t in "$SRC"/sddm/*/; do
      n=$(basename "$t")
      sudo_run rm -rf "/usr/share/sddm/themes/$n"
      sudo_run install -d "/usr/share/sddm/themes/$n"
      sudo_run cp -r "$t". "/usr/share/sddm/themes/$n/"
    done
  fi
  ok "Installed to $PREFIX"
}

# ───────────── mode assets (icons) ─────────────
fetch_assets() {
  [ "$ASSETS" = 1 ] || return 0
  say "Mode assets"
  info "Each mode looks like its real OS with free look-alike themes from open-source projects:"
  info "  • Windows: Selawik font, Win11 icons, Fluent GTK theme + cursors, Bloom wallpapers"
  info "  • macOS:   MacTahoe/WhiteSur icons, GTK theme + cursors, Tahoe/Big Sur/Sonoma wallpapers"
  info "  • Creed:   Rubik and Gabarito fonts"
  info "About 250 MB, downloaded from GitHub into your home folder (no admin rights)."
  confirm "Download them now?" || { warn "Skipped - modes fall back to Papirus icons and drawn wallpapers (run: creed assets)"; return 0; }
  if [ "$DRY" = 1 ]; then echo "  [dry-run] bash $PREFIX/share/creed/scripts/fetch-assets.sh"; return 0; fi
  if bash "$PREFIX/share/creed/scripts/fetch-assets.sh" --log "$LOG"; then ok "All mode assets installed"
  else warn "Some assets could not be downloaded - run \`creed assets\` later to retry (see $LOG)"; fi
}

# ───────────── titlebar plugin ─────────────
build_titlebars() {
  say "Window titlebars"
  info "Windows and macOS modes draw real titlebars (– ▢ ✕ / traffic lights) with the"
  info "hyprbars plugin, compiled for your exact Hyprland version (about 20 seconds)."
  if [ "$DRY" = 1 ]; then echo "  [dry-run] bash $PREFIX/share/creed/scripts/build-hyprbars.sh"; return 0; fi
  if spin "Building hyprbars" bash "$PREFIX/share/creed/scripts/build-hyprbars.sh"; then ok "Titlebars ready"
  else warn "Could not build hyprbars now - Creed retries automatically at login (see $LOG)"; fi
}

# ───────────── tweaks ─────────────
choose_tweaks() {
  [ "$TWEAKS" = 1 ] || return 0
  say "Optimisations"
  info "Creed can tune your system. Nothing changes without your OK, every tweak is"
  info "explained first, and \`creed tweak revert <id>\` undoes it exactly."
  local json; json=$(creed tweak list --json 2>/dev/null) || { warn "Could not list tweaks"; return 0; }
  local selected=()
  local cats=(gaming network system battery)
  for cat in "${cats[@]}"; do
    mapfile -t ids < <(jq -r --arg c "$cat" '.[] | select(.category==$c and .status=="off" and .destructive==false) | .id' <<<"$json")
    [ ${#ids[@]} -gt 0 ] || continue
    mapfile -t rec < <(jq -r --arg c "$cat" '.[] | select(.category==$c and .status=="off" and .destructive==false and .recommended) | .id' <<<"$json")
    if interactive && has gum; then
      local labels=() preselect=()
      for id in "${ids[@]}"; do
        t=$(jq -r --arg i "$id" '.[] | select(.id==$i) | .title' <<<"$json")
        labels+=("$id - $t")
        if printf '%s\n' "${rec[@]}" | grep -qx "$id"; then preselect+=("$id - $t"); fi
      done
      local IFS_OLD=$IFS; IFS=,
      mapfile -t picked < <(gum choose --no-limit --height 12 --header "$cat (recommended items are pre-selected - space toggles)" \
        --selected "${preselect[*]:-}" "${labels[@]}" || true)
      IFS=$IFS_OLD
      for p in "${picked[@]}"; do selected+=("${p%% - *}"); done
    else
      selected+=("${rec[@]}")
    fi
  done
  [ ${#selected[@]} -gt 0 ] || { info "No tweaks selected."; return 0; }
  echo
  info "These will be applied:"
  for id in "${selected[@]}"; do info "  • $(jq -r --arg i "$id" '.[] | select(.id==$i) | .title' <<<"$json")"; done
  if interactive && has gum; then
    if gum confirm "Read the full explanation of each first?"; then
      for id in "${selected[@]}"; do creed tweak explain "$id"; echo "────────"; done | gum pager
    fi
  fi
  confirm "Apply ${#selected[@]} tweak(s)?" || { warn "Tweaks skipped"; return 0; }
  for id in "${selected[@]}"; do
    if [ "$DRY" = 1 ]; then echo "  [dry-run] creed tweak apply $id --yes"; continue; fi
    if CREED_PRIV=sudo creed tweak apply "$id" --yes >> "$LOG" 2>&1; then ok "$id"; else warn "$id failed (see $LOG)"; fi
  done
}

# ───────────── login screen ─────────────
setup_sddm() {
  has sddm || [ "$DRY" = 1 ] || return 0
  say "Login screen"
  local current_dm=""
  current_dm=$(systemctl list-unit-files --state=enabled 2>/dev/null | awk '/display-manager|gdm|lightdm|ly|greetd|sddm/ {print $1}' | head -1)
  if [ -n "$current_dm" ] && [ "$current_dm" != "sddm.service" ]; then
    info "You already use $current_dm - leaving your login screen alone."
    return 0
  fi
  confirm "Use SDDM with the Creed login theme?" || return 0
  sudo_run install -d /etc/sddm.conf.d
  if [ "$DRY" = 0 ]; then
    printf '# Managed by Creed\n[Theme]\nCurrent=creed\n' | sudo tee /etc/sddm.conf.d/creed-theme.conf >/dev/null
  fi
  sudo_run systemctl enable sddm.service || warn "Could not enable sddm"
  if confirm "Should the login screen follow your mode (Windows / macOS look)?"; then
    sudo_run install -d /etc/creed
    sudo_run touch /etc/creed/sddm-follow-mode
  fi
  ok "SDDM configured"
}

# ───────────── per-user setup ─────────────
user_setup() {
  say "Setting up your desktop"
  if [ "$DRY" = 1 ]; then echo "  [dry-run] creed setup --yes"; return 0; fi
  creed setup --yes >> "$LOG" 2>&1 || { echo "creed setup failed - see $LOG" >&2; exit 1; }
  ok "Configs installed (your originals are in ~/.config/creed-backup)"
  if [ -n "$MODE" ]; then
    creed mode set "$MODE" --no-reload >> "$LOG" 2>&1 && ok "Mode: $MODE" || warn "Unknown mode '$MODE'"
  fi
  mkdir -p "$HOME/.config/creed"
  printf 'method=git\nsource=%s\n' "$SRC" > "$HOME/.config/creed/.install"
  [ "$UPDATE" = 1 ] || touch "$HOME/.config/creed/.welcome-pending"
}

main() {
  banner
  preflight
  hardware_summary
  install_packages
  enable_services
  build
  install_files
  if [ "$UPDATE" = 1 ]; then
    build_titlebars
    creed theme apply >> "$LOG" 2>&1 || true
    creed reload >/dev/null 2>&1 || true
    say "Creed updated."
    exit 0
  fi
  fetch_assets
  build_titlebars
  user_setup
  choose_tweaks
  setup_sddm
  echo
  say "All done."
  # shellcheck disable=SC1111  # typographic quotes in user-facing text
  info "Log out and pick “Hyprland” at the login screen. Creed Settings opens on first login"
  info "so you can choose a mode. Useful commands: creed help · creed doctor · creed uninstall"
  info "Install log: $LOG"
}

main
