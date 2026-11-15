# Shared helpers for tweak scripts. $CREED_ROOT is "" on real systems (tests use a temp dir).
set -euo pipefail
R="${CREED_ROOT:-}"
live() { [ -z "$R" ]; }
# put <absolute path> <<'X' ... X   - writes stdin to $R<path>, creating parents.
put() { mkdir -p "$(dirname "$R$1")"; cat > "$R$1"; }
has() { command -v "$1" >/dev/null 2>&1; }
# Runs a command only on a real system.
run() { if live; then "$@" || true; fi; }
human() { numfmt --to=iec --suffix=B "${1:-0}" 2>/dev/null || echo "${1:-0} bytes"; }
du_b() { du -sb "$@" 2>/dev/null | awk '{s+=$1} END {print s+0}'; }
