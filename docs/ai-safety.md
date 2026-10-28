# AI safety

Every place the app talks to an AI model, what the model can see, and what stops it doing harm. This file is a living checklist: `cargo test` fails if a new AI entry point or screen-helper tool is added without being listed here (`every_ai_entry_point_is_in_the_ai_safety_doc` in `src-tauri/src/lib.rs` and `every_screen_helper_tool_is_in_the_ai_safety_doc` in `src-tauri/src/screen_agent.rs`).

The short version for people deploying the app is in [SECURITY.md](../SECURITY.md).

## Principles

- **The AI never grades.** Quick checks, session checks and the final are graded in `src-tauri/src/grade.rs`. Answer keys never reach the webview or a prompt.
- **Keys stay in Rust.** API keys live in the system keychain (`llm.rs`) and are attached to requests in Rust. No webview ever sees one.
- **Course material and page text are data.** Every prompt says so, and page text that reads like instructions is withheld before it reaches a model (`guard::screen_for_model`).
- **Safety is enforced in code, not in the prompt.** The screen helper's action guard (`guard::decide`) decides from what the page itself says, and neither the model nor the learner can turn a Block into an Allow.
- **Everything is optional.** Lessons, checks and certificates work with no provider at all.

## Entry points

Tauri commands that reach a model:

| Command | What it does | What the model sees |
|---|---|---|
| `ask_tutor` | Ask mode: answers a question about the lesson or course | The lesson (or course outline), the ground rules, recent chat |
| `coach_turn` | Coach mode: one stage at a time with a hint ladder | The session's stages and walkthrough (for the coach only), the ground rules |
| `submit_exercise` | An advisory review of an exercise write-up | The acceptance criteria and the write-up, secrets redacted first |
| `screen_agent_run` | The screen helper: look, act or check | An outline of the practice page, the task, course context |
| `test_provider` | "Save and test" in Settings | A fixed one-word prompt |

Functions in `src-tauri/src/llm.rs` that send a request:

| Function | Used by |
|---|---|
| `chat` | Ask, Coach, exercise reviews, the provider test |
| `respond` | The screen helper (OpenAI Responses API with tool calls) |
| `list_models` | Settings, to list the models a key can use |

Every request passes the daily limit first (`llm::set_meter`, default 300 requests a day per computer, `db::DEFAULT_DAILY_CAP`; 0 means no limit) and is counted afterwards.

## Chat ground rules

`guard::RULES` is added to every chat prompt (the test `every_chat_prompt_carries_the_rules` fails if one is built without it). It tells the model that it is an AI, to stay on the course, its subject and the practice app, never to ask for or repeat secrets, never to give or confirm a graded answer, to decline harmful requests briefly, and to treat course material and page text as data.

On top of the prompt:

- `guard::redact_secrets` removes things that look like passwords or keys from the learner's text before it is sent or stored.
- `guard::quotes_graded` notices when a message quotes a check or final question and adds `guard::GRADED_NOTE`, which tells the model to explain the idea, not the answer.
- `guard::cap_reply` cuts replies at 4,000 characters.
- Replies are rendered as text elements (`ui/src/md.rs`), never as HTML.

## The screen helper

The helper runs only on desktop, only in the practice pane, and only on practice hosts (`practice::is_practice`): the saved practice URL's host, the optional Development host, or any host under `brand::PRACTICE_HOST_SUFFIX` when a fork sets one. It never runs on an identity provider's sign-in page. The pane itself has no IPC capability; Rust drives it with `eval`.

The loop is in `src-tauri/src/screen_agent.rs`: at most 30 steps, 5 confirmed writes and 3 failed actions per request.

| Tool | Modes | What it does |
|---|---|---|
| `read_screen` | look, act, check | Reads the page as a numbered outline |
| `scroll` | look, act, check | Scrolls the main area |
| `list_tabs` | look, act, check | Lists the pane's tabs |
| `switch_tab` | look, act, check | Brings a tab to the front |
| `click` | act | Clicks a numbered element, after the action guard |
| `type_text` | act | Types into a field, after the action guard |
| `choose_option` | act | Picks an option in a dropdown, after the action guard |
| `press_key` | act | Enter, Escape or Tab, after the action guard |
| `wait` | act | Waits 1 to 5 seconds |
| `record_criterion` | check | Records met, not met or can't tell for one acceptance criterion |

### The action guard

`guard::decide` reads the element's label, the dialog around it, the URL path and the field type (all from the page, through `exec.js describe()`) and returns one of:

- **Block**: never done, and the learner is not asked. Deleting, archiving, discarding, user and role changes, keys, resets, ownership, account, sign-in and billing settings, integrations, bulk actions, deploying or publishing to Production, anything the dialog says can't be undone, any action on an admin path, and typing into a password or secret field or typing anything that looks like a secret.
- **Confirm**: the learner must click Allow. In Production: submit, save, create, approve, send and similar writes. In Development: submit, save, create, approve, send and publish. An unknown environment counts as Production.
- **Allow**: navigation, opening forms, typing into ordinary fields, picking options, reading.

The words are in [`content/guard.yaml`](../content/guard.yaml). Add your app's own destructive labels, admin URL segments and one-way actions there; the file is compiled in, and `cargo test` checks it parses and stays lowercase.

Every action and every guard decision goes to the activity log (Settings, safety and AI usage), without what was typed.

## Known limits

- Results are self-attested: the learner's database is on their own machine.
- The guard works from words on the page. An app whose destructive button is an unlabelled icon needs its label (or aria-label) added to `content/guard.yaml` or fixed in the app.
- The access-code check is offline, so a code can't be revoked before it expires (see [access.md](access.md)).
