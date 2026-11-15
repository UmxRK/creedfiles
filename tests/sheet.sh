#!/bin/bash
# Contact sheet for a mode: desktop, launcher, control, dashboard, notification center, overview.
d=/src/tests/out/$1-${2:-1920x1080}
magick \( $d/00-desktop.png $d/10-launcher.png $d/10-control.png -resize 33.333% +append \) \
       \( $d/10-dashboard.png $d/10-notif-center.png $d/10-overview.png -resize 33.333% +append \) \
       -append /src/tests/out/sheet-$1.png
