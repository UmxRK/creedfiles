#!/bin/bash
# Creed Settings in a live headless session: every page, edits, welcome flow.
set -uo pipefail
export CREED_DATA=/src CREED_CONFIG=/tmp/creed-test/cfg-settings
rm -rf $CREED_CONFIG; mkdir -p $CREED_CONFIG
printf '[general]\nmode = "creed"\n' > $CREED_CONFIG/overrides.toml
S=/src/tests/session.sh
$S stop >/dev/null 2>&1; $S start 1920x1080 1 >/dev/null; eval "$($S env)"
export GDK_BACKEND=wayland GSK_RENDERER=cairo
python3 /src/tests/settings_driver.py 2> /tmp/creed-test/settings-driver.log
rc=$?
grep -E "Traceback|Error" /tmp/creed-test/settings-driver.log | grep -v -E "portal|MESA|Gdk-WARNING" | head -5
$S stop >/dev/null 2>&1
exit $rc
