#!/bin/bash
# Validates the generated app themes with the apps' own parsers, for every mode × scheme.
set -uo pipefail
fail=0
for d in /src/modes/*/; do m=$(basename "$d")
  for scheme in dark light; do
    H=$(mktemp -d); export HOME=$H CREED_DATA=/src CREED_CONFIG=$H/.config/creed XDG_RUNTIME_DIR=$H/run
    mkdir -p $CREED_CONFIG $XDG_RUNTIME_DIR
    printf '[general]\nmode = "%s"\n[appearance]\nscheme = "%s"\n' "$m" "$scheme" > $CREED_CONFIG/overrides.toml
    creed theme apply --no-reload >/dev/null 2>&1 || { echo "✗ $m/$scheme: theme apply failed"; fail=1; continue; }
    G=$CREED_CONFIG/generated
    # foot: a config that includes the generated colours must pass foot's own check.
    printf '[main]\ninclude=%s/foot-colors.ini\n' "$G" > $H/foot.ini
    out=$(foot --check-config -c $H/foot.ini 2>&1) || { echo "✗ $m/$scheme foot: $out"; fail=1; }
    # starship: must parse (print-config fails on bad TOML) and must not warn.
    out=$(STARSHIP_CONFIG=$G/starship.toml starship print-config 2>&1 >/dev/null)
    [ -z "$out" ] || { echo "✗ $m/$scheme starship: $out"; fail=1; }
    # kitty colours: every line "key value", keys known to kitty.
    awk 'NF && $1 !~ /^#/ && NF<2 {print "bad line: " $0; e=1} END{exit e}' $G/kitty-colors.conf || { echo "✗ $m/$scheme kitty"; fail=1; }
    # Lua configs: syntax.
    for f in $G/*.lua; do luac -p "$f" 2>/dev/null || { echo "✗ $m/$scheme $(basename $f): $(luac -p $f 2>&1)"; fail=1; }; done
    # CSS: balanced braces, no unrendered template tags anywhere.
    for f in $G/gtk.css $G/userChrome.css; do
      python3 - "$f" <<'PY' || { echo "✗ $m/$scheme $(basename $f)"; fail=1; }
import sys,re
s=open(sys.argv[1]).read()
s2=re.sub(r'/\*.*?\*/','',s,flags=re.S)
assert s2.count('{')==s2.count('}'), 'unbalanced braces'
PY
    done
    if grep -l '{{' $G/* >/dev/null 2>&1; then echo "✗ $m/$scheme: unrendered template tag in $(grep -l '{{' $G/*)"; fail=1; fi
    # Qt colour scheme: INI with 21-entry active/inactive/disabled palettes.
    python3 - $G/qt-colors.conf <<'PY' || { echo "✗ $m/$scheme qt-colors"; fail=1; }
import sys,configparser
c=configparser.ConfigParser(); c.read(sys.argv[1])
for k in ('active_colors','inactive_colors','disabled_colors'):
    v=c['ColorScheme'][k].split(',')
    assert len(v)>=20, (k,len(v))
    assert all(x.strip().startswith('#') for x in v), k
PY
    # MangoHud: key=value or bare flags only.
    awk -F= 'NF && $1 !~ /^#/ && $1 !~ /^[a-z_0-9]+$/ {print "bad key " $1; e=1} END{exit e}' $G/mangohud.conf || { echo "✗ $m/$scheme mangohud"; fail=1; }
    # hyprlock: every block opened is closed.
    python3 -c "import sys; s=open('$G/hyprlock.conf').read(); assert s.count('{')==s.count('}')" || { echo "✗ $m/$scheme hyprlock braces"; fail=1; }
    rm -rf $H
  done
  [ $fail = 0 ] && echo "✓ $m: foot, starship, kitty, Lua, CSS, Qt, MangoHud, hyprlock"
done
exit $fail
