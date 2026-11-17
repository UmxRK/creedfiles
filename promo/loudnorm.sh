#!/bin/bash
# Two-pass EBU R128 normalisation: -14 LUFS integrated, -1.5 dBTP (TikTok/Instagram targets).
# Usage: promo/loudnorm.sh IN.wav OUT.wav
set -euo pipefail
m=$(ffmpeg -hide_banner -i "$1" -af loudnorm=I=-14:TP=-1.5:LRA=11:print_format=json -f null - 2>&1 | sed -n '/^{/,/^}/p')
g() { echo "$m" | python3 -c "import json,sys; print(json.load(sys.stdin)['$1'])"; }
ffmpeg -y -loglevel error -i "$1" -af "loudnorm=I=-14:TP=-1.5:LRA=11:measured_I=$(g input_i):measured_TP=$(g input_tp):measured_LRA=$(g input_lra):measured_thresh=$(g input_thresh):offset=$(g target_offset):linear=true" -ar 48000 "$2"
