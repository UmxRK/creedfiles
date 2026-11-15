#!/bin/bash
# Starts a headless Wayland session (sway, pixman renderer) with creed-shell inside it.
# Usage: tests/session.sh start WIDTHxHEIGHT [scale] | stop | shot FILE | cmd "<shell ipc>"
# Runs as user "tester" inside the Arch test root; CREED_* point at the source tree.
set -euo pipefail
RT=/run/user/1000
export XDG_RUNTIME_DIR=$RT
export CREED_DATA=${CREED_DATA:-/src}
export CREED_CONFIG=${CREED_CONFIG:-/home/tester/.config/creed}
BIN=${CREED_BIN:-/src/target/release/creed-shell}
STATE=/tmp/creed-test
mkdir -p $STATE
case "${1:-}" in
  start)
    res=${2:-1920x1080}; scale=${3:-1}
    mkdir -p $RT && chmod 700 $RT
    cat > $STATE/sway.conf <<CFG
output HEADLESS-1 resolution $res scale $scale
default_border none
seat * hide_cursor 100
CFG
    # dbus session shared by sway, the shell and test clients.
    dbus-daemon --session --fork --address=unix:path=$RT/bus --print-pid > $STATE/dbus.pid
    export DBUS_SESSION_BUS_ADDRESS=unix:path=$RT/bus
    WLR_BACKENDS=headless WLR_RENDERER=pixman WLR_LIBINPUT_NO_DEVICES=1 sway -c $STATE/sway.conf > $STATE/sway.log 2>&1 &
    echo $! > $STATE/sway.pid
    for i in $(seq 50); do ls $RT/wayland-* >/dev/null 2>&1 && break; sleep 0.1; done
    export WAYLAND_DISPLAY=$(basename $(ls $RT/wayland-? | head -1))
    echo "$WAYLAND_DISPLAY" > $STATE/display
    $BIN > $STATE/shell.log 2>&1 &
    echo $! > $STATE/shell.pid
    for i in $(seq 100); do [ -S $RT/creed/shell.sock ] && break; sleep 0.1; done
    sleep 1.5
    ;;
  stop)
    for p in shell sway; do [ -f $STATE/$p.pid ] && kill $(cat $STATE/$p.pid) 2>/dev/null || true; done
    [ -f $STATE/dbus.pid ] && kill $(cat $STATE/dbus.pid) 2>/dev/null || true
    sleep 0.5; rm -f $STATE/*.pid $RT/wayland-* $RT/bus; rm -rf $RT/creed
    ;;
  shot)
    WAYLAND_DISPLAY=$(cat $STATE/display) grim "$2"
    ;;
  cmd)
    shift; printf '%s\n' "$*" | socat - UNIX-CONNECT:$RT/creed/shell.sock 2>/dev/null || python3 -c "
import socket,sys
s=socket.socket(socket.AF_UNIX); s.connect('$RT/creed/shell.sock'); s.sendall((sys.argv[1]+'\n').encode()); s.shutdown(socket.SHUT_WR)
print(s.recv(1<<20).decode().strip())" "$*"
    ;;
  env)
    echo "export XDG_RUNTIME_DIR=$RT WAYLAND_DISPLAY=$(cat $STATE/display) DBUS_SESSION_BUS_ADDRESS=unix:path=$RT/bus"
    ;;
esac
