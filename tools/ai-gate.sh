#!/bin/sh
# The release gate for the companion's AI features (docs/ai-safety.md). tools/release.sh runs it and
# stops on any failure. Run it yourself before pushing a change to a prompt, a model setting, a
# guardrail (content/guard.yaml, src-tauri/src/guard.rs) or a screen-helper tool.
#   sh tools/ai-gate.sh            (needs cargo, trunk and tools/ui-check/node_modules: npm install there)
# What it runs:
#   - cargo tests for the app and shared: the action policy (guard.rs), prompts carry the ground rules,
#     every AI entry point and tool is listed in docs/ai-safety.md, the daily limit, the activity log, Start over
#   - ui-check: the companion in a browser, and the executor on fixture pages, including a delete confirmation dialog
set -e
ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
cargo test -p lms-shell -p shared
UI_CHECK_PORT=${UI_CHECK_PORT:-8817} sh tools/ui-check/run.sh
echo "AI gate passed"
