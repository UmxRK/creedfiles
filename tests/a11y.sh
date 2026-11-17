#!/bin/bash
# Screen-reader support: with a11y.screen_reader on, every bar/panel control must appear in the
# AT-SPI tree (what Orca reads) with a role and a non-empty name.
set -uo pipefail
export CREED_DATA=/src CREED_CONFIG=/tmp/creed-test/cfg-a11y
rm -rf $CREED_CONFIG; mkdir -p $CREED_CONFIG
printf '[general]\nmode = "%s"\n[a11y]\nscreen_reader = true\n' "${1:-creed}" > $CREED_CONFIG/overrides.toml
S=/src/tests/session.sh
$S stop >/dev/null 2>&1; eval "$($S env 2>/dev/null)"
$S start 1920x1080 1 >/dev/null; eval "$($S env)"
/usr/lib/at-spi-bus-launcher --launch-immediately >/dev/null 2>&1 &
bl=$!
sleep 1.5
# What Orca does when it starts: tell applications accessibility is wanted.
gdbus call --session --dest org.a11y.Bus --object-path /org/a11y/bus --method org.freedesktop.DBus.Properties.Set org.a11y.Status IsEnabled '<true>' >/dev/null
# Restart the shell so it starts with an assistive technology present.
kill "$(cat /tmp/creed-test/shell.pid)"; sleep 0.5
${CREED_BIN:-/src/target/release/creed-shell} > /tmp/creed-test/shell.log 2>&1 &
echo $! > /tmp/creed-test/shell.pid
for i in $(seq 50); do [ -S $XDG_RUNTIME_DIR/creed/shell.sock ] && break; sleep 0.1; done
sleep 1.5
$S cmd "open control" >/dev/null; sleep 1.5
A11Y_MODE=${1:-creed} python3 - <<'PY'
import gi, sys
gi.require_version("Atspi", "2.0")
from gi.repository import Atspi
import os, time
# Names every mode must announce (Windows says "Do not disturb", macOS says "Focus").
need = [("Wi-Fi / Network / AirPort", ["wi-fi", "network", "airport"]), ("Clock or calendar", ["clock", "calendar"])]
if os.environ.get("A11Y_MODE") != "mac-aqua":
    # Tiger had no Do Not Disturb, and shows Bluetooth only when an adapter exists.
    need += [("Bluetooth", ["bluetooth"]), ("Do Not Disturb / Focus", ["do not disturb", "focus"])]
RAW = {"network", "volume", "battery", "bluetooth", "tray", "clock", "launcher", "status"}


def snapshot():
    desk = Atspi.get_desktop(0)
    apps = [desk.get_child_at_index(i) for i in range(desk.get_child_count())]
    shell = [a for a in apps if a and "creed" in (a.get_name() or "").lower()]
    nodes = []

    def walk(o, d=0):
        if d > 12: return
        try:
            nodes.append((o.get_role_name(), o.get_name() or ""))
            for i in range(o.get_child_count()):
                c = o.get_child_at_index(i)
                if c: walk(c, d + 1)
        except Exception as e:
            print("walk error", e)
    for a in shell: walk(a)
    return apps, shell, nodes


# The panel may still be opening on a busy machine: poll for up to 10 s instead of trusting one look.
for _ in range(20):
    apps, shell, nodes = snapshot()
    inter = [(r, n) for r, n in nodes if r in ("button", "push button", "toggle button", "slider", "list item", "entry", "check box")]
    unnamed = [(r, n) for r, n in inter if not n.strip() or n.strip() in RAW]
    names = " ".join(n for _, n in inter).lower()
    hits = {label: any(a in names for a in alts) for label, alts in need}
    if shell and len(inter) >= 10 and not unnamed and all(hits.values()):
        break
    time.sleep(0.5)
if not shell:
    print("✗ creed-shell not on the accessibility bus (apps: %s)" % [a.get_name() for a in apps if a]); sys.exit(1)
print(f"  {len(nodes)} accessible nodes, {len(inter)} interactive")
for r, n in (nodes if os.environ.get("A11Y_ALL") else inter[:12]): print(f"    {r}: {n}")
ok = len(inter) >= 10 and not unnamed
print(("✓" if ok else "✗") + f" every interactive control has a real name ({len(unnamed)} missing: {unnamed[:4]})")
for label, hit in hits.items():
    print(("✓" if hit else "✗") + f" '{label}' is announced"); ok = ok and hit
sys.exit(0 if ok else 1)
PY
rc=$?
kill $bl 2>/dev/null; $S stop >/dev/null 2>&1
exit $rc
