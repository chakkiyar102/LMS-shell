# Architecture

How LMS Shell is put together. The app is Rust end to end: a Tauri 2 backend and a Leptos UI compiled to WebAssembly, with a small shared crate for the types that cross between them.

Related guides: [authoring-content.md](authoring-content.md), [access.md](access.md), [ai-safety.md](ai-safety.md), [updates.md](updates.md), [course-tour.md](course-tour.md).

## Workspace layout

The root `Cargo.toml` declares the members `shared`, `src-tauri` and `ui`. `ui` is left out of `default-members` because it only runs in the browser target; Trunk builds it.

| Folder | What lives there |
|---|---|
| `src-tauri/` | The backend, crate `lms-shell`, library `lms_shell_lib`. `src/main.rs` only calls `lms_shell_lib::run()`. |
| `ui/` | The Leptos UI (crate `ui`), built and served by Trunk (`ui/Trunk.toml`, port 1420). |
| `shared/` | Types and pure logic both sides use, and `brand.rs`, every name and rule a fork changes. |
| `content/` | Courses, What's new, the companion's nudges and hover help, the guard's word lists. |
| `tools/` | Validator, access-code tool, shared-login hash tool, placeholder art, browser checks, release template. |

### Backend (`src-tauri/src/`)

| File | Responsibility |
|---|---|
| `lib.rs` | `App` state, every Tauri command, `run()` (plugins, setup, the invoke handler) and unit tests. |
| `content.rs` | Loads the YAML corpus, shuffles quiz options, renders lesson markdown, builds media URLs, loads the course tour. |
| `grade.rs` | Grading for `single`, `multi` and `match` questions. |
| `db.rs` | SQLite schema and queries. |
| `llm.rs` | Provider calls (Ollama, OpenAI, OpenAI-compatible, Anthropic), model listing, key storage, the daily-limit meter. |
| `codex.rs` | The ChatGPT plan provider through the Codex CLI (desktop only). |
| `practice.rs` | The practice pane: hosts, tabs, sign-in popups, placement, the executor injection and `exec()`. |
| `exec.js` | The executor injected into practice pages: reads the screen as an outline and performs one action. |
| `screen_agent.rs` | The screen helper's agent loop (desktop only). |
| `guard.rs` | The action guard, secret redaction, untrusted page text, the chat ground rules. |
| `access.rs` | Access codes and the shared login (see [access.md](access.md)). |
| `maildns.rs` | Whether an email domain exists in DNS (MX, then A/AAAA, 3 s timeout, offline allows). |
| `cert.rs` | The PDF certificate writer used on Android. |

### UI (`ui/src/`)

| File | Responsibility |
|---|---|
| `main.rs` | Pages and components. Pages are a `Page` enum in a shared `Ctx`, not a router. |
| `api.rs` | `invoke()` and event `listen()` bindings to Tauri. |
| `sidekick.rs` and `sidekick/` | The companion: tab, pane, flight, nudges, hover help, coach marks, stuck detection, tricks. |
| `welcome.rs` | The greeting on the course home. |
| `md.rs` | Renders AI replies as elements, never HTML. |

Styles are in `ui/style.css` (the design tokens are at the top; `--brand` is the one colour to change) and `ui/sidekick.css`.

## How the pieces fit

```
 main window
 +------------------------------------------------------------------+
 |  Leptos UI (WebAssembly)                 Practice pane            |
 |  pages, quizzes, companion -- invoke --> (native child webviews,  |
 |                              |            exec.js injected)       |
 +------------------------------|---------------------^--------------+
                                v                     | eval only, no IPC
                     Tauri backend (Rust) ------------+
        content.rs (corpus)   db.rs (SQLite)   llm.rs (providers, keys)
        grade.rs (answers)    practice.rs      screen_agent.rs + guard.rs
```

One rule shapes most of the code: **answer keys and API keys never reach a webview.**

- The UI gets question views (prompt and option texts only). Grading runs in `grade.rs`.
- Grader-only YAML keys are not declared in the Rust types, so serde drops them at load.
- API keys are attached to requests in `llm.rs` and nowhere else.
- The practice pane cannot call the backend at all.

### Development run

```bash
cd src-tauri
LMS_CONTENT_DIR="$PWD/../content" cargo tauri dev
```

`cargo tauri dev` runs `trunk serve` in `ui/` and opens the window. `LMS_CONTENT_DIR` must point at the folder that contains `courses/`; without it, a build reads the copy bundled into its resources.

## Commands, build.rs and capabilities

Every UI-to-backend call is a Tauri command. Because `src-tauri/build.rs` declares an app manifest, each command is opt-in per capability. To add one:

1. Write the function in `src-tauri/src/lib.rs` with `#[tauri::command(rename_all = "snake_case")]` and add it to `tauri::generate_handler![...]` in `run()`.
2. Add its name to `COMMANDS` in `src-tauri/build.rs`.
3. Add `allow-<command-name-with-dashes>` to `src-tauri/capabilities/main.json`.

Skip step 2 or 3 and the call fails at runtime with a permission error, not at compile time. The test `every_command_is_declared_and_allowed` catches it. The capability applies to the `main` window only; the practice pane has none, so it can call nothing.

The UI passes arguments in snake_case, for example `api::call("get_lesson", json!({ "course_id": ..., "lesson_id": ... }))`.

## Course media

Lesson videos, posters and captions are local files served through Tauri's asset protocol. `tauri.conf.json` allows `$RESOURCE/content/**`, and `run()` also allows the actual `courses/` folder at runtime (it can be `LMS_CONTENT_DIR` in development). `content::media_url` turns a path under `content/courses/` into an asset URL; `https://` URLs pass through. The CSP allows `https:` media, so a lesson can point at a hosted video.

## Learner database

Everything a learner does is in SQLite at `<app data dir>/learner.db`. The folder comes from Tauri's `app_data_dir()`, named by the `identifier` in `tauri.conf.json` (`org.lmsshell.desktop` until you change it): on macOS `~/Library/Application Support/<identifier>/`.

| Table | Holds |
|---|---|
| `lesson_progress` | Video position and completion per lesson |
| `attempts` | Quick check, session check and final attempts |
| `exercise_submissions` | Write-ups and their AI review |
| `messages` | Ask and Coach threads |
| `certificates` | Certificate id and issue date per course |
| `activity` | Event counts per local day (streak and activity grid) |
| `app_settings` | Key-value settings: profile, provider, practice URLs, access code, seen markers, daily limit |
| `sidekick_audit` | The companion's activity log: screen-helper actions and guard decisions, without anything typed |
| `ai_usage` | AI requests and tokens per local day, for the daily limit |

Progress keys are prefixed with the course id (`<course>/<item>`). Items from a shared folder use `shared/<item>`, so their progress counts in every course that includes them.

**Settings, Start over on this computer** (`reset_app`) empties every table, compacts the file and erases the saved AI keys. The UI then clears its local storage and reloads into onboarding.

## The companion

The companion (display name `companion_name!()` in `shared/src/brand.rs`, internal name "sidekick") lives in a tab on the right edge and a side pane.

- **Nudges** are authored in `content/sidekick-nudges.yaml` and picked by `shared::pick_nudge`, which is unit tested. The YAML writes `{companion}` where it names itself; the backend fills in the configured name.
- **Hover help and tooltips** are in `content/sidekick-help.yaml`, keyed by the UI's `data-help` attributes. A test fails if a `data-help` key has no entry.
- **Short reactions** are in `ui/src/sidekick/lines.rs`.
- **Art** is one PNG per pose in `ui/assets/mascot/`; a missing pose falls back to a simpler one (`shared::pose_fallback`), then to `ui/assets/icon.png`.
- The mode (On, Quiet, Off) and nudge bookkeeping live in the webview's `localStorage`.

## The practice pane

`practice.rs` opens the learner's practice URL as a child webview of the main window, sized by the same formula that sizes the UI's columns. Links that open a new window become tabs (on a practice host) or open in the system browser (anywhere else). Sized popups become small sign-in windows that share the pane's cookies and close themselves when the sign-in returns to the practice app. The backend owns the pane's lifecycle: it closes every tab and popup when the session ends and whenever the main webview reloads, so a pane is never left orphaned over the app.

The screen helper (`screen_agent.rs`) drives `exec.js` through `practice::exec`, one typed tool call per step, each checked by `guard::decide`. See [ai-safety.md](ai-safety.md) and [SECURITY.md](../SECURITY.md).

## Access gating

A course with `gated: true` is locked unless a valid access code, or the shared login, is stored on the device. The role never opens a course by itself. See [access.md](access.md).

## Email checks

`shared::clean_profile` runs in the UI and again in `save_profile`: a name, a plausible email, the optional staff domain rule, and (when `brand::CHECK_DISPOSABLE_EMAIL` is on) the disposable-domain list and common webmail typos. When `brand::CHECK_EMAIL_DNS` is on, the backend also asks DNS whether the domain exists; an offline or slow lookup always allows the address.
