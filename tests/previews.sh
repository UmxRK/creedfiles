#!/bin/bash
# Renders modes/<id>/preview.png (640×360) from a live headless session.
# Uses a clean HOME so previews only contain Creed's own assets (drawn wallpapers, bundled icon
# sets) - never the third-party look-alike themes/wallpapers a user may have downloaded.
S=/src/tests/session.sh
export HOME=/tmp/creed-test/prev-home; rm -rf "$HOME"; mkdir -p "$HOME"
for d in /src/modes/*/; do m=$(basename "$d")
  export CREED_CONFIG=/tmp/creed-test/cfg-prev-$m; rm -rf $CREED_CONFIG; mkdir -p $CREED_CONFIG
  printf '[general]\nmode = "%s"\n' "$m" > $CREED_CONFIG/overrides.toml
  $S stop >/dev/null 2>&1; $S start 1920x1080 >/dev/null; eval "$($S env)"
  case $m in
    win*) $S cmd "open launcher" >/dev/null; sleep 1.2 ;;
    creed) $S cmd "open control" >/dev/null; sleep 1.2 ;;
    *) sleep 1 ;;
  esac
  creed-shot /tmp/creed-test/prev-$m.png
  magick /tmp/creed-test/prev-$m.png -resize 640x360 -strip -quality 92 PNG24:/src/modes/$m/preview.png
  $S stop >/dev/null 2>&1
done
ls -la /src/modes/*/preview.png
