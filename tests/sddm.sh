#!/bin/bash
# Renders every SDDM theme with the real greeter (test mode) and screenshots it.
set -uo pipefail
S=/src/tests/session.sh; fail=0
for t in /src/sddm/*/; do n=$(basename $t)
  $S stop >/dev/null 2>&1; $S start 1920x1080 1 >/dev/null; eval "$($S env)"
  QT_QPA_PLATFORM=wayland QT_QUICK_BACKEND=software sddm-greeter-qt6 --test-mode --theme "$t" > /tmp/creed-test/sddm-$n.log 2>&1 &
  p=$!; sleep 6
  creed-shot /tmp/creed-test/sddm-$n.png
  kill $p 2>/dev/null; wait $p 2>/dev/null
  if grep -E "qrc:|file:.*(Error|error|ReferenceError|TypeError|is not defined|Cannot)" /tmp/creed-test/sddm-$n.log | grep -v "Cannot open" | head -3 | grep -q .; then
    echo "✗ $n: QML errors:"; grep -E "Error|is not defined|Cannot" /tmp/creed-test/sddm-$n.log | head -5; fail=1
  else echo "✓ $n renders without QML errors"; fi
  $S stop >/dev/null 2>&1
done
exit $fail
