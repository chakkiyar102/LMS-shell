#!/bin/sh
# Browser checks for the companion (nudges, flight, coach marks, taps, drag, modes, hover help, chat log) and
# the practice-pane executor: the real UI build (trunk) in Chromium with the Tauri bridge stubbed. No backend,
# no key, no practice account. The companion's display name comes from shared/src/brand.rs companion_name!()
# ("Guide"): the stub replaces {companion} in the help and nudge YAML the way the backend's named() does.
#
# Setup once:  (cd tools/ui-check && npm install)     # playwright 1.56.1 and its Chromium
# Usage:       [UI_CHECK_PORT=8817] sh tools/ui-check/run.sh [OUT_DIR]
#              (needs trunk, node, and /usr/bin/python3 with PyYAML; builds the UI into OUT_DIR/dist)
#
# The screenshot tour runs separately, from real command output dumped by src-tauri/examples/tour_dump.rs,
# against the OUT_DIR/dist this script built:
#   LMS_CONTENT_DIR=content cargo run -p lms-shell --example tour_dump -- OUT_DIR/tour-data.json
#   node tools/ui-check/tour.mjs OUT_DIR/tour-data.json OUT_DIR/dist OUT_DIR/tour              # run all shots
#   node tools/ui-check/tour.mjs OUT_DIR/tour-data.json OUT_DIR/dist docs/screenshots 8823 readme  # README set
set -e
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
OUT=${1:-$(mktemp -d)}
mkdir -p "$OUT"
COMPANION=$(sed -n 's/.*macro_rules! companion_name { () => { "\([^"]*\)" } }.*/\1/p' "$ROOT/shared/src/brand.rs")
for f in nudges help; do
  /usr/bin/python3 -c "import yaml,json,sys;json.dump(yaml.safe_load(open(sys.argv[1]).read().replace('{companion}',sys.argv[3])),open(sys.argv[2],'w'))" \
    "$ROOT/content/sidekick-$f.yaml" "$OUT/$f.json" "${COMPANION:-Guide}"
done
(cd "$ROOT/ui" && CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$ROOT/target} trunk build --dist "$OUT/dist" >/dev/null)
PORT=${UI_CHECK_PORT:-8817}
python3 -m http.server "$PORT" -d "$OUT/dist" >/dev/null 2>&1 &
SERVER=$!
trap 'kill $SERVER' EXIT
sleep 1
node "$ROOT/tools/ui-check/companion-check.mjs" "$OUT" "http://127.0.0.1:$PORT/"
node "$ROOT/tools/ui-check/lively-check.mjs" "$OUT" "http://127.0.0.1:$PORT/"
node "$ROOT/tools/ui-check/help-check.mjs" "$OUT" "http://127.0.0.1:$PORT/"
# the chat log: anchored to the bottom above the composer, pinned while at the bottom, the New messages pill
node "$ROOT/tools/ui-check/chat-check.mjs" "$OUT" "http://127.0.0.1:$PORT/"
# the executor (src-tauri/src/exec.js) on its fixture pages; needs no server
node "$ROOT/tools/ui-check/exec-check.mjs" "$OUT"
echo "screenshots in $OUT"
