#!/usr/bin/env python3
"""Builds icons/Creed-Material: Google Material Symbols Rounded (Apache-2.0) under the
freedesktop symbolic names Creed's shell asks for. Run from the repo root; needs network.
The result is committed, so users never run this."""
import os
import re
import sys
import urllib.request

# freedesktop name -> (material symbol, filled?)
MAP = {
    "applications-games-symbolic": ("stadia_controller", True),
    "input-gaming-symbolic": ("stadia_controller", True),
    "audio-input-microphone-symbolic": ("mic", True),
    "microphone-disabled-symbolic": ("mic_off", True),
    "audio-volume-high-symbolic": ("volume_up", True),
    "audio-volume-medium-symbolic": ("volume_down", True),
    "audio-volume-low-symbolic": ("volume_mute", True),
    "audio-volume-muted-symbolic": ("volume_off", True),
    "avatar-default-symbolic": ("account_circle", True),
    "bluetooth-active-symbolic": ("bluetooth", False),
    "bluetooth-disabled-symbolic": ("bluetooth_disabled", False),
    "display-brightness-symbolic": ("light_mode", True),
    "document-open-recent-symbolic": ("history", False),
    "edit-find-symbolic": ("search", False),
    "system-search-symbolic": ("search", False),
    "edit-paste-symbolic": ("content_paste", False),
    "emblem-system-symbolic": ("settings", False),
    "face-smile-symbolic": ("mood", False),
    "focus-windows-symbolic": ("select_window", False),
    "folder-documents-symbolic": ("folder", True),
    "go-down-symbolic": ("keyboard_arrow_down", False),
    "go-up-symbolic": ("keyboard_arrow_up", False),
    "go-next-symbolic": ("chevron_right", False),
    "go-previous-symbolic": ("chevron_left", False),
    "keyboard-symbolic": ("keyboard", False),
    "media-playback-pause-symbolic": ("pause", True),
    "media-playback-start-symbolic": ("play_arrow", True),
    "media-skip-backward-symbolic": ("skip_previous", True),
    "media-skip-forward-symbolic": ("skip_next", True),
    "network-offline-symbolic": ("signal_wifi_off", False),
    "network-wired-symbolic": ("lan", False),
    "network-wireless-acquiring-symbolic": ("wifi_find", False),
    "network-wireless-disabled-symbolic": ("wifi_off", False),
    "network-wireless-signal-excellent-symbolic": ("wifi", False),
    "network-wireless-symbolic": ("wifi", False),
    "network-wireless-signal-good-symbolic": ("network_wifi_3_bar", False),
    "network-wireless-signal-ok-symbolic": ("network_wifi_2_bar", False),
    "network-wireless-signal-weak-symbolic": ("network_wifi_1_bar", False),
    "night-light-symbolic": ("nightlight", True),
    "notifications-disabled-symbolic": ("notifications_off", False),
    "notifications-symbolic": ("notifications", False),
    "preferences-system-notifications-symbolic": ("notifications", False),
    "object-select-symbolic": ("check", False),
    "power-profile-balanced-symbolic": ("balance", False),
    "power-profile-performance-symbolic": ("bolt", True),
    "power-profile-power-saver-symbolic": ("energy_savings_leaf", True),
    "system-lock-screen-symbolic": ("lock", False),
    "system-log-out-symbolic": ("logout", False),
    "system-reboot-symbolic": ("restart_alt", False),
    "system-shutdown-symbolic": ("power_settings_new", False),
    "view-app-grid-symbolic": ("apps", False),
    "view-dual-symbolic": ("splitscreen_left", False),
    "view-fullscreen-symbolic": ("fullscreen", False),
    "view-grid-symbolic": ("grid_view", False),
    "weather-clear-symbolic": ("sunny", True),
    "weather-clear-night-symbolic": ("clear_night", True),
    "weather-few-clouds-symbolic": ("partly_cloudy_day", True),
    "weather-few-clouds-night-symbolic": ("partly_cloudy_night", True),
    "weather-fog-symbolic": ("foggy", True),
    "weather-overcast-symbolic": ("cloud", True),
    "weather-showers-symbolic": ("rainy", True),
    "weather-snow-symbolic": ("weather_snowy", True),
    "weather-storm-symbolic": ("thunderstorm", True),
    "window-close-symbolic": ("close", False),
    "window-pin-symbolic": ("keep", False),
    # Extra glyphs used by the Material layout.
    "view-refresh-symbolic": ("refresh", False),
    "screenshot-symbolic": ("screenshot_region", False),
    "color-select-symbolic": ("colorize", False),
    "coffee-symbolic": ("coffee", True),
    "dark-mode-symbolic": ("contrast", False),
    "speaker-symbolic": ("speaker", False),
    "checkbox-checked-symbolic": ("check_box", True),
    "alarm-symbolic": ("timer", False),
    "calendar-symbolic": ("calendar_month", False),
    "task-due-symbolic": ("done_outline", False),
    "list-add-symbolic": ("add", False),
    "list-remove-symbolic": ("remove", False),
    "edit-clear-all-symbolic": ("clear_all", False),
    "memory-symbolic": ("memory", False),
    "drive-harddisk-symbolic": ("hard_drive", False),
    "swap-symbolic": ("swap_horiz", False),
    "music-note-symbolic": ("music_note", False),
}
for lvl in range(0, 101, 10):
    icon = "battery_full" if lvl >= 95 else f"battery_{max(0, min(6, round(lvl / 100 * 6)))}_bar"
    MAP[f"battery-level-{lvl}-symbolic"] = (icon, True)
    MAP[f"battery-level-{lvl}-charging-symbolic"] = ("battery_charging_full" if lvl >= 95 else f"battery_charging_{[20, 20, 20, 30, 50, 50, 60, 80, 80, 90, 90][lvl // 10]}", True)

BASE = "https://raw.githubusercontent.com/google/material-design-icons/master/symbols/web/{n}/materialsymbolsrounded/{n}{f}_24px.svg"


def fetch(name, fill):
    for f in (["_fill1", ""] if fill else [""]):
        try:
            with urllib.request.urlopen(BASE.format(n=name, f=f), timeout=20) as r:
                return r.read().decode()
        except Exception:
            continue
    return None


def main():
    out = "icons/Creed-Material/symbolic"
    os.makedirs(out, exist_ok=True)
    missing = []
    for fd, (name, fill) in sorted(MAP.items()):
        svg = fetch(name, fill)
        if not svg:
            missing.append(f"{fd} ({name})")
            continue
        # Symbolic icons are recoloured by the shell: make the glyph a single plain fill.
        svg = re.sub(r'\s(width|height)="24"', "", svg, count=2)
        svg = svg.replace("<path ", '<path fill="#bebebe" ', 1) if 'fill=' not in svg.split("<path", 1)[1].split(">", 1)[0] else svg
        svg = svg.replace('viewBox="0 -960 960 960"', 'viewBox="0 -960 960 960" width="24" height="24"')
        with open(f"{out}/{fd}.svg", "w") as fh:
            fh.write(svg)
    with open("icons/Creed-Material/index.theme", "w") as fh:
        fh.write("[Icon Theme]\nName=Creed Material\nComment=Google Material Symbols Rounded (Apache-2.0) under freedesktop names, for Creed mode\n"
                 "Inherits=Papirus-Dark,Adwaita,hicolor\nDirectories=symbolic\n\n[symbolic]\nSize=24\nMinSize=8\nMaxSize=512\nType=Scalable\nContext=Status\n")
    if missing:
        print("missing:", *missing, sep="\n  ", file=sys.stderr)
        sys.exit(1)
    print(f"{len(MAP)} icons")


if __name__ == "__main__":
    main()
