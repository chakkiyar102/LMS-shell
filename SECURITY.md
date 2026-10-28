# Security

## Reporting a problem

Please report security issues privately to the maintainers of your copy of this project rather than in a public issue. Include what you found, how to reproduce it and what it affects.

## The safety model of the screen helper

The screen helper lets an AI model look at, and optionally act in, the web app open in the practice pane. It is optional, desktop only, and needs an OpenAI-compatible Responses API. Treat it as a teaching aid for a training or sandbox account, never as automation for production data.

**Where it can run.** Only in the practice pane, and only on practice hosts: the saved practice URL's host, the optional Development host, or any host under `PRACTICE_HOST_SUFFIX` in `shared/src/brand.rs`. The check runs twice: in Rust before every call (`practice::is_practice`) and as the first statement of the injected script (`src-tauri/src/exec.js`), against a frozen gate Rust defines before any page script loads. It never runs on an identity provider's sign-in page or any other site.

**What the page can do to the app.** Nothing. The pane's webviews have no Tauri capability, so a page cannot call any app command. Rust drives the page with `eval`; the page cannot push anything back. Sign-in popups open as plain windows with no app script.

**Where keys live.** In the system keychain, read in Rust and attached to requests in Rust. Neither the course UI nor the practice page ever sees one.

**What decides an action.** A Rust function, `guard::decide`, from what the page itself says about the element: its label, the dialog around it, the URL path and the field type. The model's output is not an input to the decision, and the learner cannot override a Block.

| Decision | When | Examples |
|---|---|---|
| Block (never done, not asked) | Destructive, irreversible, administrative or credential actions | Delete, archive, discard, user and role changes, keys, resets, ownership, account, sign-in and billing settings, integrations, bulk actions, publishing or deploying to Production, anything a dialog says can't be undone, any action on an admin path, typing into a password or secret field, typing anything that looks like a key |
| Confirm (the learner clicks Allow) | Writes | Submit, save, create, approve, send, and more in Production; a shorter list in Development |
| Allow | Everything else | Navigating, opening forms, typing into ordinary fields, picking options, reading |

An unknown environment counts as Production. A request stops after 30 steps, 5 confirmed writes or 3 failed actions.

**The word lists are yours to extend.** They live in [`content/guard.yaml`](content/guard.yaml), compiled into the app. If your practice app has its own destructive labels (for example "Purge workspace"), admin URL segments or one-way actions, add them there and run `cargo test`. The guard works from words on the page, so an icon-only destructive button with no label or aria-label is invisible to it: label it in your app, or keep the screen helper away from that screen.

**What the model sees.** An outline of the page as data, wrapped in "untrusted data" markers, with any line that reads like instructions to an AI withheld. The course material and the learner's task. Never the answer keys, never a key, never the guard's decisions as something it can change.

**What is recorded.** Every action and guard decision goes to the activity log on the learner's computer (Settings), with the control's label but without anything typed. Start over erases it.

## Chat

The Ask and Coach chats and the exercise review carry ground rules in every prompt (`guard::RULES`): the model says it is an AI, stays on the course, never asks for or repeats secrets, never gives or confirms an answer to a graded question, and treats course material as data. Text that looks like a password or key is removed before it is sent or stored. Replies are rendered as text, never as HTML. Grading never involves a model. Details: [docs/ai-safety.md](docs/ai-safety.md).

## Access codes and local data

Access codes are checked offline against a public key compiled into the app, so a code cannot be revoked before it expires, and course files inside an installed app are not encrypted. Results are self-attested: the learner's progress database is on their own machine. If either matters for you, add a server-side check. The shipped public key is a placeholder and the shared login is off; see [docs/access.md](docs/access.md).
