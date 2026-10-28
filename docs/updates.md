# Updates

LMS Shell carries the [Tauri updater](https://v2.tauri.app/plugin/updater/) wiring (check, download with progress, "Restart to update", and the What's new sheet afterwards), but it ships **off**: no endpoint, no keys, and `UPDATER_ENABLED` set to `false` in `shared/src/brand.rs`. With it off, the plugin is not registered, `check_update` reports no update and the update pill never appears.

What's new still works without the updater: the entry in `content/whats-new.yaml` whose `version` matches the app's version shows once, on the first launch of that version.

## Turn updates on

1. **Make a signing keypair.** Update archives are signed, and installed apps refuse anything not signed with your key.

   ```bash
   cargo tauri signer generate -w ~/.tauri/lms-shell.key
   ```

   Keep the private key out of the repository and back it up. Losing it means installed apps can never update again.

2. **Configure the plugin** in `src-tauri/tauri.conf.json`: add a `plugins` block with your public key and where `latest.json` will be published, and turn on updater artifacts.

   ```json
   "bundle": { "createUpdaterArtifacts": true, ... },
   "plugins": {
     "updater": {
       "pubkey": "<the contents of ~/.tauri/lms-shell.key.pub>",
       "endpoints": ["https://github.com/<owner>/<releases-repo>/releases/latest/download/latest.json"],
       "windows": { "installMode": "passive" }
     }
   }
   ```

3. **Flip the switch:** set `UPDATER_ENABLED` to `true` in `shared/src/brand.rs`.

4. **Publish a release.** Bump `version` in `tauri.conf.json`, add a matching entry at the top of `content/whats-new.yaml`, commit, and run the template script:

   ```bash
   TAURI_SIGNING_PRIVATE_KEY_PASSWORD=... tools/release.sh
   ```

   `tools/release.sh` validates the content, runs the AI gate, builds a signed macOS universal app and `.dmg`, writes `latest.json` and creates a GitHub release with `gh`. It reads the repository from your updater endpoint. Windows (cargo-xwin) is in the script, commented out; Linux builds on CI with `.github/workflows/linux.yml`. Adapt it to your own hosting if you don't use GitHub releases: the updater only needs a reachable `latest.json` and the signed archives it points to.

## Code signing

Updater signatures are not operating-system code signing. Without a macOS Developer ID and notarisation, or a Windows code-signing certificate, learners see a first-launch warning. Set `APPLE_SIGNING_IDENTITY` (and Tauri's notarisation variables) before running the release script to sign on macOS.

## Android

Android builds don't self-update; a new version arrives as a new APK. The updater code is compiled out on mobile.
