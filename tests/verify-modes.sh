#!/bin/bash
# Generates every mode's configs into a temp HOME and validates the Hyprland config with
# Hyprland itself (`--verify-config`). Run as tester inside the Arch test root.
set -uo pipefail
fail=0
for m in $(ls /src/modes); do
  H=$(mktemp -d); export HOME=$H CREED_DATA=/src CREED_CONFIG=$H/.config/creed XDG_RUNTIME_DIR=/run/user/1000
  mkdir -p $CREED_CONFIG
  printf '[general]\nmode = "%s"\n' "$m" > $CREED_CONFIG/overrides.toml
  if ! out=$(/src/target/release/creed theme apply --no-reload 2>&1); then echo "✗ $m: theme apply failed: $out"; fail=1; continue; fi
  res=$(Hyprland --verify-config -c $CREED_CONFIG/generated/hyprland.lua 2>&1 | sed -n '/Config parsing result/,$p' | tail -n +2 | grep -v '^$')
  if [ "$res" = "config ok" ]; then echo "✓ $m: hyprland config ok ($(wc -l < $CREED_CONFIG/generated/hyprland.lua) lines)"; else echo "✗ $m:"; echo "$res" | head -10; fail=1; fi
  for f in $CREED_CONFIG/generated/*; do [ -s "$f" ] || { echo "✗ $m: empty $(basename $f)"; fail=1; }; done
  # Every glass/motion/scheme variant of this mode must also validate.
  for extra in 'glass.quality="low"' 'glass.quality="ultra"' 'glass.enabled=false' 'appearance.motion="off"' 'appearance.motion="minimal"' 'appearance.scheme="light"'; do
    k=${extra%%=*}; v=${extra#*=}; sec=${k%%.*}; key=${k#*.}
    printf '[general]\nmode = "%s"\n[%s]\n%s = %s\n' "$m" "$sec" "$key" "$v" > $CREED_CONFIG/overrides.toml
    /src/target/release/creed theme apply --no-reload >/dev/null 2>&1 || { echo "✗ $m [$extra] apply failed"; fail=1; continue; }
    r=$(Hyprland --verify-config -c $CREED_CONFIG/generated/hyprland.lua 2>&1 | sed -n '/Config parsing result/,$p' | tail -n +2 | grep -v '^$')
    [ "$r" = "config ok" ] || { echo "✗ $m [$extra]: $r" | head -5; fail=1; }
  done
  rm -rf $H
done
exit $fail
