#!/bin/bash
# RAM gate: PSS of every Creed process with bar + dock + wallpaper + notification daemon mapped,
# measured after cycling every transient surface N times and idling.
# Usage (as tester inside the Arch test root): tests/ram.sh [WxH] [scale] [cycles] [limit_mb] [mode]
set -uo pipefail
res=${1:-1920x1080}; scale=${2:-1}; cycles=${3:-20}; limit=${4:-60}; mode=${5:-creed}
export CREED_CONFIG=/tmp/creed-test/cfg-ram
mkdir -p $CREED_CONFIG
printf '[general]\nmode = "%s"\n%s\n' "$mode" "${EXTRA_TOML:-}" > $CREED_CONFIG/overrides.toml
S=/src/tests/session.sh
$S stop >/dev/null 2>&1
$S start "$res" "$scale"
eval "$($S env)"
pss() { local t=0; for p in $(pgrep -u "$(id -u)" -f 'creed-shell|creed-helper|creed ' ); do v=$(awk '/^Pss:/{print $2}' /proc/$p/smaps_rollup 2>/dev/null); t=$((t + ${v:-0})); done; echo $t; }
before=$(pss)
notify-send -a Test "Hello" "Notification daemon check" >/dev/null 2>&1
for i in $(seq "$cycles"); do
  for s in ${SURFACES:-launcher control dashboard notif-center overview clipboard emoji power snap calendar}; do
    $S cmd "open $s" >/dev/null; sleep 0.25; $S cmd "close $s" >/dev/null; sleep 0.25
  done
done
sleep 4
# Every always-on surface must be mapped while we measure.
state=$($S cmd state)
after=$(pss)
pid=$(cat /tmp/creed-test/shell.pid)
t0=$(awk '{print $14+$15}' /proc/$pid/stat); sleep 3; t1=$(awk '{print $14+$15}' /proc/$pid/stat)
idle_ticks=$((t1 - t0))
echo "mode=$mode res=$res scale=$scale cycles=$cycles"
echo "surfaces: $state"
echo "PSS start: $((before/1024)) MB   PSS after cycling: $((after/1024)) MB   limit: $limit MB   idle CPU: ${idle_ticks} ticks/3s"
awk '/^[0-9a-f]+-[0-9a-f]+ /{name=$6} /^Pss:/{if ($2>400) printf "  %6d kB %s\n", $2, name}' /proc/$pid/smaps | sort -rn | head -8
$S stop >/dev/null 2>&1
need="bar wallpaper"
grep -qE '^enabled = false' /src/modes/$mode/mode.toml 2>/dev/null && awk '/^\[dock\]/{f=1;next} /^\[/{f=0} f&&/^enabled = false/{x=1} END{exit !x}' /src/modes/$mode/mode.toml || need="$need dock"
for k in $need; do echo "$state" | grep -q "\"$k\"" || { echo "FAIL: $k not mapped"; exit 1; }; done
[ "$idle_ticks" -le 3 ] || { echo "FAIL: shell is busy while idle"; exit 1; }
[ $((after/1024)) -le "$limit" ] && echo PASS || { echo FAIL; exit 1; }
