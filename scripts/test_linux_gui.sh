#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="${BFD_LINUX_GUI_BIN:-$ROOT/target/release/burst-frame-deduplicator-gtk}"
CLI="${BFD_LINUX_CLI_BIN:-$ROOT/target/release/burst-frame-deduplicator}"
SOURCE_IMAGE="${BFD_TEST_SOURCE_IMAGE:-$ROOT/assets/app-icon.png}"

if [[ "$(uname -s)" != Linux ]]; then
  printf 'The Linux GUI smoke test must run on Linux.\n' >&2
  exit 1
fi
for command in Xvfb dbus-run-session metacity python3 xdotool; do
  command -v "$command" >/dev/null || {
    printf 'Missing Linux GUI test prerequisite: %s\n' "$command" >&2
    exit 1
  }
done
python3 -c 'import importlib.util; assert importlib.util.find_spec("dogtail.tree") is not None'

if [[ ! -x "$APP" || ! -x "$CLI" ]]; then
  cargo build \
    --manifest-path "$ROOT/Cargo.toml" \
    --release \
    --features linux-gui \
    --bin burst-frame-deduplicator \
    --bin burst-frame-deduplicator-gtk
fi

WORK="$(mktemp -d "${TMPDIR:-/tmp}/bfd-linux-gui-test.XXXXXX")"
DISPLAY_NUMBER="${BFD_TEST_DISPLAY_NUMBER:-99}"
export DISPLAY=":$DISPLAY_NUMBER"
cleanup() {
  if [[ -n "${XVFB_PID:-}" ]]; then
    kill "$XVFB_PID" 2>/dev/null || true
  fi
  if [[ "${BFD_KEEP_TEST_WORK:-0}" == 1 ]]; then
    printf 'Preserved Linux GUI test files at %s\n' "$WORK" >&2
  else
    rm -rf "$WORK"
  fi
}
trap cleanup EXIT

mkdir -p "$WORK/photos" "$WORK/config/burst-frame-deduplicator" "$WORK/cache" "$WORK/data"
if [[ ! -f "$SOURCE_IMAGE" ]]; then
  printf 'Linux GUI test source image is unavailable: %s\n' "$SOURCE_IMAGE" >&2
  exit 1
fi
EXTENSION="${SOURCE_IMAGE##*.}"
FRAME_ONE="frame_0001.$EXTENSION"
FRAME_TWO="frame_0002.$EXTENSION"
cp "$SOURCE_IMAGE" "$WORK/photos/$FRAME_ONE"
cp "$WORK/photos/$FRAME_ONE" "$WORK/photos/$FRAME_TWO"
"$CLI" scan "$WORK/photos" \
  --out "$WORK/run" \
  --no-refine \
  --detector off \
  --workers 1

python3 - "$WORK/config/burst-frame-deduplicator/config.json" "$WORK/run" <<'PY'
import json
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
path.write_text(json.dumps({
    "locale": "en",
    "appearance": "system",
    "recent_runs": [sys.argv[2]],
    "tutorial_progress": {
        "schema_version": 1,
        "outcome": "completed",
        "updated_at": "2026-01-01T00:00:00Z",
    },
}, indent=2) + "\n")
PY

Xvfb "$DISPLAY" -screen 0 1600x1000x24 -nolisten tcp >"$WORK/xvfb.log" 2>&1 &
XVFB_PID=$!
sleep 1
export GDK_BACKEND=x11
export LIBGL_ALWAYS_SOFTWARE=1
export XDG_CONFIG_HOME="$WORK/config"
export XDG_CACHE_HOME="$WORK/cache"
export XDG_DATA_HOME="$WORK/data"
export GTK_MODULES=gail:atk-bridge
export NO_AT_BRIDGE=0
export APP WORK FRAME_ONE FRAME_TWO EXTENSION

dbus-run-session -- bash <<'SESSION'
set -euo pipefail
metacity --sm-disable >"$WORK/metacity.log" 2>&1 &
WM_PID=$!
"$APP" >"$WORK/app.log" 2>&1 &
APP_PID=$!
session_cleanup() {
  status=$?
  if ((status != 0)) && [[ -f "$WORK/app.log" ]]; then
    printf '%s\n' '--- Linux GUI application log ---' >&2
    cat "$WORK/app.log" >&2
  fi
  kill "$APP_PID" "$WM_PID" 2>/dev/null || true
  return "$status"
}
trap session_cleanup EXIT
sleep 3

python3 <<'PY'
import time
from dogtail.predicate import GenericPredicate
from dogtail.tree import root

app = root.application("burst-frame-deduplicator-gtk")
buttons = app.findChildren(GenericPredicate(roleName="push button"), recursive=True)
next(button for button in buttons if button.name.startswith("photos ")).doActionNamed("click")
deadline = time.monotonic() + 10
while time.monotonic() < deadline:
    try:
        preview = app.findChild(
            GenericPredicate(roleName="push button", name="Open image preview"),
            recursive=True,
            retry=False,
        )
        preview.doActionNamed("click")
        break
    except Exception:
        time.sleep(0.1)
else:
    raise RuntimeError("review did not expose an image preview control")
PY

for _ in $(seq 1 100); do
  FIRST="$(xdotool search --name "$FRAME_ONE" 2>/dev/null | tail -1 || true)"
  [[ -n "$FIRST" ]] && break
  sleep 0.1
done
[[ -n "${FIRST:-}" ]]
if [[ "${EXTENSION,,}" =~ ^(3fr|arw|cr2|cr3|dng|erf|iiq|kdc|mef|mos|mrw|nef|nrw|orf|pef|raf|raw|rw2|rwl|sr2|srf|srw)$ ]]; then
  for _ in $(seq 1 100); do
    EMBEDDED="$(find "$WORK/run/native_previews" -type f -name '*_embedded.preview' -size +0c -print -quit 2>/dev/null || true)"
    [[ -n "$EMBEDDED" ]] && break
    sleep 0.1
  done
  [[ -n "${EMBEDDED:-}" ]]
  identify "$EMBEDDED" >/dev/null
fi
xdotool windowactivate "$FIRST" key Right
for _ in $(seq 1 100); do
  SECOND="$(xdotool search --name "$FRAME_TWO" 2>/dev/null | tail -1 || true)"
  [[ -n "$SECOND" ]] && break
  sleep 0.1
done
[[ -n "${SECOND:-}" ]]
xdotool windowactivate "$SECOND" key ctrl+q
for _ in $(seq 1 50); do
  kill -0 "$APP_PID" 2>/dev/null || break
  sleep 0.1
done
if kill -0 "$APP_PID" 2>/dev/null; then
  printf 'Ctrl+Q did not quit while the preview was active.\n' >&2
  exit 1
fi
wait "$APP_PID"
trap - EXIT
kill "$WM_PID" 2>/dev/null || true
wait "$WM_PID" 2>/dev/null || true
SESSION

printf 'Linux GUI smoke test passed.\n'
