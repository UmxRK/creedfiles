#!/bin/bash
# Full verification. Run as root inside an Arch root that has: hyprland, sway, grim deps,
# fonts and icon themes (see tests/README in the repo history) with this tree at /src and a
# user "tester" (uid 1000). Exits non-zero on the first failing stage.
set -uo pipefail
cd /src || exit 1
stage() { printf '\n\033[1m▶ %s\033[0m\n' "$*"; }
fail() { printf '\033[31m✗ %s\033[0m\n' "$*"; exit 1; }

stage "Unit tests"
cargo test --release --workspace 2>&1 | grep -E "^test result|FAILED|panicked" || true
cargo test --release --workspace >/dev/null 2>&1 || fail "unit tests"

stage "Build"
cargo build --release >/dev/null 2>&1 || fail "build"
install -m755 target/release/creed /usr/local/bin/creed
install -m755 target/release/creed-shell /usr/local/bin/creed-shell

stage "Every mode + variant validates with Hyprland --verify-config"
su tester -c tests/verify-modes.sh || fail "hyprland configs"

stage "Generated app themes pass the apps' own parsers (foot, starship, Lua, Qt…)"
su tester -c tests/apps.sh || fail "app themes"

stage "CLI: every command, bad input and config round trips"
su tester -c tests/cli.sh || fail "cli"

stage "Every shell dispatcher is accepted by Hyprland"
su tester -c tests/dispatchers.sh || fail "dispatchers"

stage "Tweaks: apply → re-apply → status → revert (byte-identical), cleanup safety"
tests/tweaks.sh | tail -25 || fail "tweaks"

stage "setup → uninstall round trip"
tests/roundtrip.sh || fail "round trip"

stage "Memory gate (PSS, all surfaces mapped, 200 panel cycles)"
for m in creed win11 win10 mac-tahoe mac-sequoia mac-bigsur mac-aqua; do
  r=$(su tester -c "tests/ram.sh 1920x1080 1 20 60 $m" | grep -E '^PSS|PASS|FAIL' | paste -sd' ')
  echo "  $m 1080p: $r"; echo "$r" | grep -q PASS || fail "RAM $m"
done
for m in creed mac-tahoe win11; do
  r=$(su tester -c "tests/ram.sh 3840x2160 1 10 120 $m" | grep -E '^PSS|PASS|FAIL' | paste -sd' ')
  echo "  $m 4K: $r"; echo "$r" | grep -q PASS || fail "RAM 4K $m"
done

stage "Screenshots of every surface in every mode → tests/out/"
for d in modes/*/; do m=$(basename "$d"); su tester -c "tests/visual.sh $m" >/dev/null || fail "visual $m"; done
python3 tests/check-layout.py tests/out || fail "layout geometry"

printf '\n\033[32m✓ All checks passed\033[0m\n'
