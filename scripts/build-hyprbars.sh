#!/bin/bash
# Builds the hyprbars plugin (window titlebars) for the Hyprland that is installed right
# now, the same way hyprpm resolves versions: use the plugin commit pinned for this
# Hyprland commit if there is one, otherwise the latest plugin source.
# Output: ${XDG_DATA_HOME:-~/.local/share}/creed/plugins/hyprbars.so (+ .abi stamp).
set -euo pipefail
out="${XDG_DATA_HOME:-$HOME/.local/share}/creed/plugins"
for c in git make g++ pkg-config jq Hyprland; do
  command -v "$c" >/dev/null || { echo "build-hyprbars: '$c' is missing (install base-devel, git, jq)" >&2; exit 1; }
done
info=$(Hyprland --version-json)
commit=$(jq -r .commit <<<"$info")
abi=$(jq -r .abiHash <<<"$info")
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
git clone -q https://github.com/hyprwm/hyprland-plugins.git "$tmp/hp"
pin=$(grep -oP "\[\"$commit\", \"\K[0-9a-f]+" "$tmp/hp/hyprpm.toml" | head -1 || true)
if [ -n "$pin" ]; then git -C "$tmp/hp" checkout -q "$pin"; fi
make -s -C "$tmp/hp/hyprbars" all >/dev/null
mkdir -p "$out"
install -m755 "$tmp/hp/hyprbars/hyprbars.so" "$out/hyprbars.so.new"
mv "$out/hyprbars.so.new" "$out/hyprbars.so"
echo "$abi" > "$out/hyprbars.abi"
echo "hyprbars built for Hyprland $(jq -r .version <<<"$info") (${pin:-latest})"
