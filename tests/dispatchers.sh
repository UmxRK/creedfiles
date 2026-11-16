#!/bin/bash
# Every Hyprland dispatcher creed-shell can send must be accepted by Hyprland itself.
set -uo pipefail
f=$(mktemp --suffix=.lua); i=0
while IFS= read -r d; do i=$((i+1)); echo "hl.bind(\"SUPER + ALT + CTRL + SHIFT + code:$((100+i))\", $d)" >> "$f"; done < <(/src/target/release/creed-shell --print-dispatches)
r=$(XDG_RUNTIME_DIR=/tmp Hyprland --verify-config -c "$f" 2>&1 | sed -n '/Config parsing result/,$p' | tail -n +2 | grep -v '^$')
if [ "$r" = "config ok" ]; then echo "✓ $i shell dispatchers accepted by Hyprland"; else echo "✗ dispatchers:"; echo "$r"; exit 1; fi
