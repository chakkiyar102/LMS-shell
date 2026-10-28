#!/usr/bin/env bash
# A TEMPLATE for publishing a release that installed apps pick up through the in-app update pill.
# Read docs/updates.md first: updates are off until you turn them on and fill in the placeholders.
#
#   1. bump "version" in src-tauri/tauri.conf.json (and the Cargo.toml versions if you like)
#   2. add the matching entry at the top of content/whats-new.yaml
#   3. tools/release.sh
#
# What it does: checks the content and the AI gate, builds a signed macOS universal app + updater
# archive + .dmg (and, if you uncomment it, a Windows installer), writes latest.json and uploads
# everything as a GitHub release under version-free file names, so "releases/latest/download/..."
# links never go stale. Linux bundles need WebKitGTK and build on CI (.github/workflows/linux.yml).
#
# Placeholders to fill in once:
#   - plugins.updater.endpoints in src-tauri/tauri.conf.json: https://github.com/<owner>/<releases-repo>/releases/latest/download/latest.json
#   - plugins.updater.pubkey: the public key from `cargo tauri signer generate -w ~/.tauri/lms-shell.key`
#   - APPLE_SIGNING_IDENTITY: your Developer ID ("Developer ID Application: <Your Org> (<TEAMID>)"), or unset to skip signing
set -euo pipefail
cd "$(dirname "$0")/.."

CONF=src-tauri/tauri.conf.json
# releases repo (owner/name): taken from the updater endpoint in tauri.conf.json, the one place it is configured
REPO=$(python3 -c "import re,sys;m=re.search(r'github\.com/([^/\"]+/[^/\"]+)/releases', open('$CONF').read());print(m.group(1) if m else '')")
[ -n "$REPO" ] || { echo "Set plugins.updater.endpoints in $CONF first (docs/updates.md)."; exit 1; }
KEY="${TAURI_KEY_FILE:-$HOME/.tauri/lms-shell.key}"
[ -f "$KEY" ] || { echo "No updater signing key at $KEY (cargo tauri signer generate -w $KEY)."; exit 1; }
VERSION=$(python3 -c "import json;print(json.load(open('$CONF'))['version'])")
NAME=$(python3 -c "import json;print(json.load(open('$CONF'))['productName'])")
FILE=$(echo "$NAME" | tr ' ' '-')
grep -q "version: \"$VERSION\"" content/whats-new.yaml || { echo "Add a whats-new.yaml entry for $VERSION first."; exit 1; }
grep -q 'UPDATER_ENABLED: bool = true' shared/src/brand.rs || { echo "Set UPDATER_ENABLED to true in shared/src/brand.rs first."; exit 1; }
python3 tools/validate.py
sh tools/ai-gate.sh
[ -z "$(git status --porcelain)" ] || { echo "Commit your changes first."; exit 1; }

export TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY")"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
(cd src-tauri && cargo tauri build --target universal-apple-darwin --bundles app,dmg)

# Windows (optional): cross-build with cargo-xwin. Needs rustup target add x86_64-pc-windows-msvc,
# cargo install cargo-xwin, brew install llvm lld nsis.
# export PATH="$(brew --prefix)/opt/llvm/bin:$(brew --prefix)/opt/lld/bin:$PATH"
# (cd src-tauri && cargo tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis)

TARGET=$(cd src-tauri && cargo metadata --format-version 1 --no-deps | python3 -c "import json,sys;print(json.load(sys.stdin)['target_directory'])")
B="$TARGET/universal-apple-darwin/release/bundle"
OUT="dist/$VERSION"; mkdir -p "$OUT"
cp "$B/dmg/"*.dmg "$OUT/$FILE-macos-universal.dmg"
cp "$B/macos/$NAME.app.tar.gz" "$OUT/$FILE-macos.app.tar.gz"
SIG=$(cat "$B/macos/$NAME.app.tar.gz.sig")
NOTES=$(python3 -c "
import yaml,sys
for w in yaml.safe_load(open('content/whats-new.yaml')):
    if w['version'] == '$VERSION':
        print(w['title']); [print('- ' + n) for n in w.get('new', []) + w.get('fixed', [])]
")
python3 - "$OUT" "$VERSION" "$SIG" "$REPO" "$FILE" "$NOTES" <<'PY'
import json, sys, datetime
out, version, sig, repo, file, notes = sys.argv[1:7]
base = f"https://github.com/{repo}/releases/download/v{version}"
mac = {"signature": sig, "url": f"{base}/{file}-macos.app.tar.gz"}
json.dump({"version": version, "notes": notes, "pub_date": datetime.datetime.now(datetime.timezone.utc).isoformat(),
           "platforms": {"darwin-aarch64": mac, "darwin-x86_64": mac}}, open(f"{out}/latest.json", "w"), indent=2)
PY
gh release create "v$VERSION" -R "$REPO" --title "$NAME $VERSION" --notes "$NOTES" "$OUT"/*
echo "released $VERSION to $REPO"
