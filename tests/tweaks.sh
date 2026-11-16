#!/bin/bash
# Applies, checks and reverts every tweak against a temporary root + home, verifying that
# revert restores every touched file byte-for-byte. Runs as root inside the test root.
set -uo pipefail
BIN=/src/target/release/creed
fail=0
for toml in /src/tweaks/*/*/tweak.toml; do
  dir=$(dirname "$toml"); id=$(basename "$dir"); cat=$(basename "$(dirname "$dir")")
  R=$(mktemp -d); H=$(mktemp -d)
  # Pre-existing files to make sure originals come back exactly.
  mkdir -p $R/etc/sysctl.d $R/etc/systemd $R/etc/NetworkManager/conf.d $R/usr/bin $R/usr/lib/systemd/system-generators
  for c in gamemoded powerprofilesctl resolvectl systemctl paccache pacman journalctl sync; do touch $R/usr/bin/$c; done
  touch $R/usr/lib/systemd/system-generators/zram-generator
  # Fake hardware so laptop / Wi-Fi / NVIDIA tweaks are exercised too.
  mkdir -p $R/sys/class/power_supply/BAT0 $R/sys/class/net/wlan0/wireless $R/sys/class/drm/card0/device $R/proc
  echo Battery > $R/sys/class/power_supply/BAT0/type; echo 0x10de > $R/sys/class/drm/card0/device/vendor
  printf 'processor\t: 0\nvendor_id\t: GenuineIntel\n' > $R/proc/cpuinfo
  mkdir -p $R/proc/sys/kernel $R/lib/modules/6.18.1-test/kernel/drivers/misc; echo 6.18.1-test > $R/proc/sys/kernel/osrelease; touch $R/lib/modules/6.18.1-test/kernel/drivers/misc/ntsync.ko.zst
  echo "# original zram config" > $R/etc/systemd/zram-generator.conf
  before=$(cd $R && find . -type f -exec sha256sum {} + | sort; cd $H && find . -type f -exec sha256sum {} + | sort)
  priv=$(grep -c '^privileged = true' "$toml"); destr=$(grep -c '^destructive = true' "$toml")
  export CREED_ROOT=$R CREED_DATA=/src HOME=$H CREED_USER_HOME=$H CREED_STATE=$H/.state XDG_CONFIG_HOME=$H/.config
  # Force hardware predicates open: available= may restrict (nvidia/wifi/laptop); skip if unavailable.
  if [ "$destr" = 1 ]; then
    out=$($BIN tweak explain "$id" 2>&1) || { echo "✗ $id explain: $out"; fail=1; }
    echo "$out" | grep -q "cannot be restored" || { echo "✗ $id: destructive warning missing"; fail=1; }
    echo "✓ $cat/$id (destructive: plan shown, not run)"
    rm -rf $R $H; continue
  fi
  if [ "$priv" = 1 ]; then ap="$BIN tweak-root apply $id"; rv="$BIN tweak-root revert $id"; st="$BIN tweak-root status $id"
  else ap="$BIN tweak apply $id --yes"; rv="$BIN tweak revert $id"; st=""; fi
  out=$($ap 2>&1); rc=$?
  if [ $rc -ne 0 ]; then
    if echo "$out" | grep -q "not available"; then echo "· $cat/$id (not available on this test machine)"; rm -rf $R $H; continue; fi
    echo "✗ $id apply: $out"; fail=1; rm -rf $R $H; continue
  fi
  $ap >/dev/null 2>&1 || { echo "✗ $id: second apply failed"; fail=1; }
  if [ -n "$st" ]; then s=$($st 2>&1); [ "$s" = applied ] || { echo "✗ $id status after apply: $s"; fail=1; }; fi
  # Every declared file must now exist.
  for f in $(grep -oP '^touches = \[\K.*(?=\])' "$toml" | tr -d '",' ); do
    p=${f/#\~/$H}; [[ "$f" == /* ]] && p=$R$f
    [ -e "$p" ] || { echo "✗ $id did not create $f"; fail=1; }
  done
  $rv >/dev/null 2>&1 || { echo "✗ $id revert failed"; fail=1; }
  after=$(cd $R && find . -type f -not -path './var/lib/creed/*' -exec sha256sum {} + | sort; cd $H && find . -type f -not -path './.state/*' -exec sha256sum {} + | sort)
  if [ "$before" != "$after" ]; then echo "✗ $id: revert did not restore files"; diff <(echo "$before") <(echo "$after") | head -5; fail=1
  else echo "✓ $cat/$id"; fi
  rm -rf $R $H
done
# Destructive user cleanups must delete only what they list.
H=$(mktemp -d); export HOME=$H CREED_ROOT=/ CREED_STATE=$H/.state
mkdir -p $H/.cache/thumbnails/large $H/.config/discord/Cache $H/.config/discord/Local\ Storage $H/Documents $H/.local/share/Steam/steamapps/shadercache/570
echo x > $H/.cache/thumbnails/large/a.png; echo x > $H/.config/discord/Cache/b; echo keep > $H/.config/discord/Local\ Storage/login; echo keep > $H/Documents/thesis.odt
echo x > $H/.local/share/Steam/steamapps/shadercache/570/c
plan=$($BIN tweak explain app-caches)
echo "$plan" | grep -q "thumbnails" || { echo "✗ app-caches plan does not list thumbnails"; fail=1; }
$BIN tweak apply app-caches --yes >/dev/null 2>&1 && { echo "✗ app-caches ran without --confirm"; fail=1; }
$BIN tweak apply app-caches steam-shader-cache --yes --confirm >/dev/null 2>&1 || { echo "✗ cleanup apply failed"; fail=1; }
[ ! -e $H/.cache/thumbnails ] && [ ! -e $H/.config/discord/Cache ] && [ -z "$(ls $H/.local/share/Steam/steamapps/shadercache)" ] || { echo "✗ cleanup left caches behind"; fail=1; }
[ -f $H/Documents/thesis.odt ] && [ -f "$H/.config/discord/Local Storage/login" ] && [ -d $H/.local/share/Steam/steamapps/shadercache ] || { echo "✗ cleanup deleted user data!"; fail=1; }
[ $fail = 0 ] && echo "✓ cleanup deletes only caches (documents and logins kept)"
rm -rf $H
exit $fail
