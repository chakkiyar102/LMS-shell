# Contributing

Thanks for helping. This project is a starter that people fork and rebrand, so changes should keep it generic: no product names, no real people, no organisation-specific URLs or keys.

## Set up

Follow the [Quick start](README.md#quick-start), then:

```bash
cargo test --workspace
cargo check -p ui --target wasm32-unknown-unknown
python3 tools/validate.py && python3 tools/test_validate.py
(cd tools/ui-check && npm install) && sh tools/ui-check/run.sh
```

All four should pass before you open a pull request.

## How the code is organised

Read [docs/architecture.md](docs/architecture.md) first. The short version:

- Anything both sides need (types, pure rules) goes in `shared/`, with unit tests there.
- Every UI-to-backend call is a Tauri command. Adding one takes three edits: the function in `src-tauri/src/lib.rs` (and `generate_handler!`), its name in `COMMANDS` in `src-tauri/build.rs`, and `allow-<name-with-dashes>` in `src-tauri/capabilities/main.json`. The test `every_command_is_declared_and_allowed` fails if you miss one.
- Names and rules a fork would change belong in `shared/src/brand.rs`, not scattered through the code.
- The companion's copy lives in `content/sidekick-nudges.yaml`, `content/sidekick-help.yaml` and `ui/src/sidekick/lines.rs`. Write `{companion}` in the YAML wherever it names itself.

## Rules that the tests enforce

- Answer keys and API keys never reach a webview. Grading stays in `src-tauri/src/grade.rs`.
- Every new AI entry point or screen-helper tool is listed in [docs/ai-safety.md](docs/ai-safety.md).
- Every chat prompt carries `guard::RULES`.
- Every `data-help` attribute in the UI has an entry in `content/sidekick-help.yaml`.
- The guard's word lists in `content/guard.yaml` stay lowercase, without apostrophes.

## Writing style

User-facing text is plain and warm: short sentences, no jargon, no guilt. No em or en dashes in copy or docs (use a comma, a colon or parentheses). Placeholder people and companies are fictional; use names like Jordan Hale or Brightline, and `.example` domains.

## Pull requests

Keep each pull request to one change, say what it changes and how you tested it, and include screenshots for UI changes. By contributing you agree that your contribution is licensed under the MIT licence of this project.
