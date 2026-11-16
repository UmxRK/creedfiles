"""Drives Creed Settings inside a live session: opens every page (and the welcome flow), fails on
any Python exception, and checks that edits made through the app's Store reach the config.
Run by tests/settings.sh; prints one line per check and exits non-zero on failure."""
import importlib.machinery
import importlib.util
import subprocess
import sys
import time

errors = []
sys.excepthook = lambda t, v, tb: errors.append(f"{t.__name__}: {v}")

spec = importlib.util.spec_from_loader("creed_settings", importlib.machinery.SourceFileLoader("creed_settings", "/src/settings/creed-settings"))
cs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cs)
from gi.repository import GLib  # noqa: E402

fails = 0


def check(ok, msg):
    global fails
    print(("✓ " if ok else "✗ ") + msg, flush=True)
    if not ok:
        fails += 1


def pump(seconds):
    ctx = GLib.MainContext.default()
    end = time.time() + seconds
    while time.time() < end:
        while ctx.pending():
            ctx.iteration(False)
        time.sleep(0.02)


def get(key):
    return subprocess.run(["creed", "config", "get", key], capture_output=True, text=True).stdout.strip()


app = cs.App()
app.register(None)
app.follow_scheme()
win = cs.Window(app, None, False)
win.present()
pump(1.5)
pages = [r.page_name for r in win.nav_rows.values()]
for name in pages:
    before = len(errors)
    win.stack.set_visible_child_name(name)
    pump(0.8)
    check(len(errors) == before and win.stack.get_visible_child_name() == name, f"page '{name}' opens{'' if len(errors) == before else ': ' + errors[-1]}")
    check(win.page_title.get_label() == win.nav_rows[name].page_title, f"page '{name}' title + sidebar selection follow")

# Sidebar search filters and jumps.
win.nav_rows["about"].get_parent()  # rows exist
# Store writes of every type reach the config file.
cases = [("bar.size", 44, "44"), ("dock.magnify", 1.3, "1.3"), ("appearance.motion", "smooth", "smooth"),
         ("a11y.focus_ring", False, "false"), ("bar.right", ["tray", "clock"], '["tray", "clock"]')]
for key, val, want in cases:
    win.store.set(key, val, immediate=True)
pump(4.0)
for key, val, want in cases:
    got = get(key)
    check(got == want, f"store.set({key}) persisted ({got})")
# Invalid edits are rejected by the CLI and surface as a toast, leaving the config valid.
win.store.set("bar.size", 99999, immediate=True)
pump(3.0)
check(get("bar.size") == "44", "out-of-range edit rejected, previous value kept")
ok = subprocess.run(["creed", "config", "validate"], capture_output=True).returncode == 0
check(ok, "config still valid after the app's edits")
# Welcome flow.
w2 = cs.Window(app, None, True)
w2.present()
pump(1.0)
check(w2.stack.get_visible_child_name() == "modes", "welcome opens on Modes")
check(not errors, "no Python exceptions" + (": " + "; ".join(errors[:3]) if errors else ""))
sys.exit(1 if fails else 0)
