#!/bin/bash
# Downloads the free look-alike assets that make each mode match its real OS, into the user's
# home (no admin rights). Safe to re-run; each item is independent, so one failed download
# never blocks the others. Nothing here is shipped inside Creed itself - every asset comes
# straight from its own open-source project:
#
#   Windows   Selawik font (Microsoft, OFL) · Win11 icon theme (GPL-3) · Fluent GTK theme and
#             cursors (GPL-3) · Bloom wallpapers from Win11OS-kde (GPL-3)
#   macOS     MacTahoe icon theme, cursors (GPL-3) and GTK theme (MIT) · WhiteSur icons (GPL-3)
#             · Tahoe wallpapers from MacTahoe-gtk-theme · Big Sur / Sonoma / Ventura / Monterey
#             wallpapers from WhiteSur-wallpapers (MIT)
#   Creed     Rubik and Gabarito fonts (Google Fonts, OFL)
#
# Usage: fetch-assets.sh [--only windows|macos|creed] [--log FILE]
set -uo pipefail
ONLY=""; LOG=/dev/null
while [ $# -gt 0 ]; do
  case $1 in
    --only) ONLY=$2; shift ;;
    --log) LOG=$2; shift ;;
    *) echo "unknown option $1" >&2; exit 2 ;;
  esac
  shift
done
DATA=${XDG_DATA_HOME:-$HOME/.local/share}
ICONS=$DATA/icons; THEMES=$HOME/.themes; FONTS=$DATA/fonts; WALLS=$DATA/creed/wallpapers
mkdir -p "$ICONS" "$THEMES" "$FONTS" "$WALLS"
TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
fails=0
ok() { printf '  \033[32m✓\033[0m %s\n' "$1"; }
bad() { printf '  \033[33m!\033[0m %s\n' "$1"; fails=$((fails + 1)); }
want() { [ -z "$ONLY" ] || [ "$ONLY" = "$1" ]; }

clone() { git clone -q --depth 1 "https://github.com/$1.git" "$TMP/${1#*/}" >>"$LOG" 2>&1; }
get() { curl -fsSL --retry 2 --max-time 120 -o "$2" "$1" >>"$LOG" 2>&1; }
# get_wall URL NAME: downloads to a temp file, keeps it only when it decodes as an image.
get_wall() {
  local tmp="$TMP/w.jpg"
  if get "$1" "$tmp" && head -c 3 "$tmp" | od -An -tx1 | grep -q "ff d8 ff"; then mv "$tmp" "$WALLS/$2.jpg" && return 0; fi
  return 1
}

if want windows; then
  echo "Windows look-alike assets"
  if get https://github.com/microsoft/Selawik/releases/download/1.01/Selawik_Release.zip "$TMP/selawik.zip"; then
    mkdir -p "$FONTS/selawik"
    python3 -c "import zipfile,sys; z=zipfile.ZipFile(sys.argv[1]); [open(sys.argv[2]+'/'+n.split('/')[-1],'wb').write(z.read(n)) for n in z.namelist() if n.endswith('.ttf')]" "$TMP/selawik.zip" "$FONTS/selawik" && ok "Selawik font" || bad "Selawik font (unzip failed)"
  else bad "Selawik font"; fi
  if clone yeyushengfan258/Win11-icon-theme && (cd "$TMP/Win11-icon-theme" && ./install.sh -d "$ICONS" >>"$LOG" 2>&1); then ok "Win11 icon theme"; else bad "Win11 icon theme"; fi
  if clone vinceliuice/Fluent-icon-theme && (cd "$TMP/Fluent-icon-theme" && ./install.sh -d "$ICONS" >>"$LOG" 2>&1 && cd cursors && ./install.sh >>"$LOG" 2>&1); then ok "Fluent icons and cursors"; else bad "Fluent icons and cursors"; fi
  if command -v sassc >/dev/null && clone vinceliuice/Fluent-gtk-theme \
     && (cd "$TMP/Fluent-gtk-theme" && ./install.sh -d "$THEMES" -c dark -c light -s standard --tweaks round >>"$LOG" 2>&1 \
         && ./install.sh -d "$THEMES" -c dark -c light -s standard --tweaks square >>"$LOG" 2>&1); then ok "Fluent GTK theme"
  else bad "Fluent GTK theme (needs sassc)"; fi
  W=https://raw.githubusercontent.com/yeyushengfan258/Win11OS-kde/main/wallpaper
  if get_wall "$W/Win11OS-dark/contents/images/3840x2400.jpg" bloom-dark && get_wall "$W/Win11OS-light/contents/images/3840x2400.jpg" bloom-light; then ok "Bloom wallpapers"; else bad "Bloom wallpapers"; fi
fi

if want macos; then
  echo "macOS look-alike assets"
  if clone vinceliuice/MacTahoe-icon-theme && (cd "$TMP/MacTahoe-icon-theme" && ./install.sh -d "$ICONS" -t default >>"$LOG" 2>&1 && cd cursors && ./install.sh >>"$LOG" 2>&1); then ok "MacTahoe icons and cursors"; else bad "MacTahoe icons and cursors"; fi
  if clone vinceliuice/WhiteSur-icon-theme && (cd "$TMP/WhiteSur-icon-theme" && ./install.sh -d "$ICONS" >>"$LOG" 2>&1); then ok "WhiteSur icons (Big Sur → Sequoia)"; else bad "WhiteSur icons"; fi
  if command -v sassc >/dev/null && clone vinceliuice/MacTahoe-gtk-theme \
     && (cd "$TMP/MacTahoe-gtk-theme" && ./install.sh -d "$THEMES" -c dark -c light -o normal >>"$LOG" 2>&1); then ok "MacTahoe GTK theme"
  else bad "MacTahoe GTK theme (needs sassc)"; fi
  T=https://raw.githubusercontent.com/vinceliuice/MacTahoe-gtk-theme/main/wallpaper
  if get_wall "$T/MacTahoe-day.jpeg" tahoe-light && get_wall "$T/MacTahoe-night.jpeg" tahoe-dark; then ok "Tahoe wallpapers"; else bad "Tahoe wallpapers"; fi
  S=https://raw.githubusercontent.com/vinceliuice/WhiteSur-wallpapers/main/4k
  got=0
  for pair in "WhiteSur-light bigsur-light" "WhiteSur-dark bigsur-dark" "Sonoma-light sonoma-light" "Sonoma-dark sonoma-dark" \
              "Ventura-light ventura-light" "Ventura-dark ventura-dark" "Monterey-light monterey-light" "Monterey-dark monterey-dark"; do
    set -- $pair
    get_wall "$S/$1.jpg" "$2" && got=$((got + 1))
  done
  [ $got = 8 ] && ok "Big Sur, Monterey, Ventura and Sonoma wallpapers" || bad "macOS wallpapers ($got of 8)"
fi

if want creed; then
  echo "Creed assets"
  G=https://raw.githubusercontent.com/google/fonts/main/ofl
  mkdir -p "$FONTS/creed"
  if get "$G/rubik/Rubik%5Bwght%5D.ttf" "$FONTS/creed/Rubik[wght].ttf" && get "$G/gabarito/Gabarito%5Bwght%5D.ttf" "$FONTS/creed/Gabarito[wght].ttf"; then ok "Rubik and Gabarito fonts"; else bad "Rubik and Gabarito fonts"; fi
fi

command -v fc-cache >/dev/null && fc-cache -f "$FONTS" >/dev/null 2>&1
exit $(( fails > 0 ))
