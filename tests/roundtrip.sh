#!/bin/bash
# setup → uninstall must restore a user's original files byte-for-byte. Run as root.
set -uo pipefail
id rt >/dev/null 2>&1 || useradd -m rt
H=/home/rt
rm -rf $H/.config $H/.bashrc $H/.zshrc; mkdir -p $H/.config/hypr $H/.config/foot $H/.config/gtk-3.0
echo '-- my own hyprland config' > $H/.config/hypr/hyprland.lua
printf '[main]\nfont=monospace:size=11\n' > $H/.config/foot/foot.ini
echo '/* my gtk tweaks */' > $H/.config/gtk-3.0/gtk.css
printf 'alias ll="ls -l"\nexport EDITOR=vim\n' > $H/.bashrc
chown -R rt $H
snap() { (cd $H && find . -type f -not -path './.config/creed*' -not -path './.cache/*' -not -path './.local/*' -exec sha256sum {} + | sort); }
before=$(snap)
su rt -c "XDG_RUNTIME_DIR=/tmp creed setup --yes" >/dev/null 2>&1 || { echo "✗ setup failed"; exit 1; }
grep -q Creed $H/.config/hypr/hyprland.lua || { echo "✗ setup did not install the entry file"; exit 1; }
grep -q "creed starship" $H/.bashrc || { echo "✗ setup did not add the prompt"; exit 1; }
su rt -c "XDG_RUNTIME_DIR=/tmp creed uninstall --yes" >/dev/null 2>&1 || { echo "✗ uninstall failed"; exit 1; }
after=$(snap)
if [ "$before" = "$after" ]; then echo "✓ setup → uninstall restores every original file byte-for-byte"; else echo "✗ files differ:"; diff <(echo "$before") <(echo "$after"); exit 1; fi
