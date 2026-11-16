#!/bin/bash
# CLI behaviour: every documented command runs, bad input fails cleanly (non-zero exit, a
# message, never a panic), and config edits round-trip. Runs without a desktop session.
set -uo pipefail
H=$(mktemp -d); trap 'rm -rf "$H"' EXIT
export HOME=$H CREED_DATA=/src CREED_CONFIG=$H/.config/creed XDG_RUNTIME_DIR=$H/run CREED_STATE=$H/state
mkdir -p "$CREED_CONFIG" "$XDG_RUNTIME_DIR"
fail=0
ok() { local d=$1; shift; out=$("$@" 2>&1); rc=$?
  if [ $rc -ne 0 ] || grep -q panicked <<<"$out"; then echo "✗ $d (exit $rc): ${out:0:200}"; fail=1; else echo "✓ $d"; fi; }
bad() { local d=$1; shift; out=$("$@" 2>&1); rc=$?
  if [ $rc -eq 0 ]; then echo "✗ $d: accepted bad input"; fail=1
  elif grep -q panicked <<<"$out"; then echo "✗ $d: panicked"; fail=1
  elif [ -z "$out" ]; then echo "✗ $d: failed silently"; fail=1
  else echo "✓ $d → ${out:0:90}"; fi; }
eq() { local d=$1 want=$2; shift 2; got=$("$@" 2>&1)
  if [ "$got" = "$want" ]; then echo "✓ $d"; else echo "✗ $d: got '$got', want '$want'"; fail=1; fi; }

ok "help" creed help
ok "version" creed version
ok "mode list" creed mode list
for m in $(cd /src/modes && ls); do ok "mode set $m" creed mode set "$m"; eq "mode current = $m" "$m" sh -c "creed mode current | awk '{print \$1}'"; done
ok "mode set back to creed" creed mode set creed
ok "theme apply" creed theme apply --no-reload
ok "config path" creed config path
ok "config validate" creed config validate
ok "config dump is JSON" sh -c 'creed config dump | python3 -c "import json,sys; json.load(sys.stdin)"'
ok "hardware" creed hardware
ok "hardware --json" sh -c 'creed hardware --json | python3 -c "import json,sys; json.load(sys.stdin)"'
ok "tweak list" creed tweak list
ok "output into a closed pipe does not panic" sh -c 'creed tweak list 2>"$HOME/err" | head -1 >/dev/null; creed help 2>>"$HOME/err" | head -1 >/dev/null; ! grep -q panicked "$HOME/err"' 
ok "tweak list --json" sh -c 'creed tweak list --json | python3 -c "import json,sys; json.load(sys.stdin)"'
ok "tweak recommended" creed tweak recommended
ok "tweak status" creed tweak status
ok "tweak explain zram" creed tweak explain zram
ok "plugins status" creed plugins status

# Config round trips.
ok "config set bar.size 44" creed config set bar.size 44
eq "config get bar.size" "44" creed config get bar.size
ok "config set --mode" creed config set style.radius 9 --mode
eq "per-mode value" "9.0" creed config get style.radius
ok "config unset --mode" creed config unset style.radius --mode
ok "config unset bar.size" creed config unset bar.size
ok "config set list value" creed config set style.desktop_widgets '["weather","calendar"]'
eq "list round-trips" '["weather", "calendar"]' creed config get style.desktop_widgets
ok "config unset list" creed config unset style.desktop_widgets
ok "config validate after edits" creed config validate

# Bad input.
bad "unknown command" creed frobnicate
bad "mode set unknown" creed mode set nonexistent
bad "mode set path traversal" creed mode set ../../etc
bad "config get unknown key" creed config get bar.nope
bad "config set unknown key" creed config set bar.nope 1
bad "config set wrong type" creed config set bar.size big
bad "config set out of range" creed config set bar.size 9000
bad "config set unknown module" creed config set bar.left '["nope"]'
bad "config set unknown widget" creed config set style.desktop_widgets '["nope"]'
bad "tweak explain unknown" creed tweak explain nope/nope
bad "tweak apply unknown" creed tweak apply nope/nope --yes
bad "cleanup without --confirm" creed tweak apply app-caches --yes
bad "osd unknown kind" creed osd sideways
bad "assets unknown group" creed assets linux
bad "wallpaper missing file" creed wallpaper /nonexistent/x.png
bad "shell command with no shell" creed toggle launcher
eq "config still valid after bad input" "0" sh -c 'creed config validate >/dev/null 2>&1; echo $?'
exit $fail
