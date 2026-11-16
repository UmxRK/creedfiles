#!/bin/bash
# Screenshots every surface of a mode into tests/out/<mode>/ and dumps layout JSON.
# Usage (inside the Arch test root, as tester): tests/visual.sh <mode> [WxH] [scale]
set -uo pipefail
mode=${1:-creed}; res=${2:-1920x1080}; scale=${3:-1}
OUT=/src/tests/out/$mode-$res
rm -rf "$OUT"; mkdir -p "$OUT"
export CREED_CONFIG=/tmp/creed-test/cfg-$mode
mkdir -p $CREED_CONFIG
printf '[general]\nmode = "%s"\n' "$mode" > $CREED_CONFIG/overrides.toml
S=/src/tests/session.sh
$S stop >/dev/null 2>&1
$S start "$res" "$scale"
eval "$($S env)"
shot() { creed-shot "$OUT/$1.png"; }
# Some content: a notification, then the base desktop.
notify-send -a Firefox -i firefox "Download complete" "creed-dotfiles-main.zip (2.4 MB) finished downloading." >/dev/null 2>&1
sleep 0.6
shot 00-desktop
for s in launcher control dashboard notif-center overview clipboard emoji power snap calendar; do
  $S cmd "open $s" >/dev/null
  sleep 1.2
  shot "10-$s"
  $S cmd dump-layout > "$OUT/10-$s.json"
  $S cmd "close $s" >/dev/null
  sleep 0.9
done
$S cmd "open overview apps" >/dev/null; sleep 1.2; shot 20-launchpad; $S cmd "close overview" >/dev/null; sleep 0.8
$S cmd "open launcher" >/dev/null; sleep 0.8; $S cmd "type fi" >/dev/null; sleep 0.8; shot 21-launcher-search; $S cmd "close launcher" >/dev/null; sleep 0.8
$S cmd "open launcher" >/dev/null; sleep 0.8; $S cmd "type 12*(3+4)" >/dev/null; sleep 0.8; shot 22-launcher-calc; $S cmd "close launcher" >/dev/null; sleep 0.8
# On-screen displays: volume, muted mic, brightness, game mode.
for o in "volume 0.6" "mic 0" "brightness 0.4" "game 1"; do
  $S cmd "osd $o" >/dev/null; sleep 0.9; shot "30-osd-${o%% *}"; $S cmd dump-layout > "$OUT/30-osd-${o%% *}.json"; sleep 2.2
done
# macOS app menus (Edit is the richest one).
if [[ $mode == mac-* ]]; then
  r=$($S cmd "click bar Edit menu"); [ "$r" = ok ] || { echo "✗ $mode: Edit menu not clickable ($r)"; exit 1; }
  sleep 0.8; shot 40-menu-edit; $S cmd dump-layout > "$OUT/40-menu-edit.json"; $S cmd "close menu" >/dev/null; sleep 0.6
fi
$S cmd dump-layout > "$OUT/00-desktop.json"
pid=$(cat /tmp/creed-test/shell.pid)
grep -E "^(Pss|Rss):" /proc/$pid/smaps_rollup > "$OUT/memory.txt"
$S stop >/dev/null 2>&1
echo "$OUT"
