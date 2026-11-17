#!/bin/bash
# Renders every promo video with Blender (EEVEE) and encodes it. Resumable: finished frames are kept.
# Usage: promo/render.sh [video...]   Output: promo/videos/<video>.mp4 (1080x1920, 30 fps, 15 s)
cd "$(dirname "$0")/.." || exit 1
export LIBGL_ALWAYS_SOFTWARE=${LIBGL_ALWAYS_SOFTWARE:-1} PROMO_OUT=${PROMO_OUT:-promo/out}
vids=("$@"); [ ${#vids[@]} -eq 0 ] && vids=(seven glass win11 material ram aqua command tweaks era details)
for v in "${vids[@]}"; do
  echo "$(date +%T) render $v"
  blender -b --python promo/gen.py -- "$v" anim >"$PROMO_OUT.$v.log" 2>&1
  n=$(ls "$PROMO_OUT/$v"/f_*.jpg 2>/dev/null | wc -l)
  [ "$n" -eq 450 ] || { echo "$v: only $n frames"; continue; }
  ffmpeg -y -loglevel error -framerate 30 -i "$PROMO_OUT/$v/f_%04d.jpg" -c:v libx264 -preset slow -crf 15 -pix_fmt yuv420p -movflags +faststart "promo/videos/$v.mp4" && echo "$(date +%T) done $v"
done
echo ALLDONE
