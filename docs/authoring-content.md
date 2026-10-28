# Authoring content

How to write a course for LMS Shell. The full key-by-key schema is in [content/SCHEMA.md](../content/SCHEMA.md); this guide is the workflow around it. The sample course, `content/courses/brightline-basics/`, uses every part and is the best template.

## The shape of a course

```
content/courses/<course-id>/
  course.yaml                 # title, summary, the sessions and what each one contains
  lessons/<lesson-id>.yaml    # one per lesson: objective, body, key points, a quick check
  checks/<check-id>.yaml      # one per session: the session check
  exercises/<exercise-id>.yaml# one per session: a hands-on task with acceptance criteria
  assessment/final.yaml       # the final assessment, in tasks
  videos/                     # optional lesson media
```

A course is a list of **sessions**. Each session has a few **lessons**, one **session check** and one **exercise**. After the last session comes the **final assessment**. The certificate needs every session check and the final passed; lessons and exercises count toward progress but not toward the certificate.

| Item | Graded by | Pass mark |
|---|---|---|
| Quick check (3 questions at the end of a lesson) | The app | Two of three (practice only, never blocks anything) |
| Session check | The app | `pass_mark` in the check file (0.75 by convention) |
| Final assessment | The app | `pass_mark` in `final.yaml` |
| Exercise | Nobody: the AI review is advisory | None |

Questions come in three kinds: `single` (one right option), `multi` (several) and `match` (pair each left item with a right one). Every option carries feedback, shown after grading. Options of `single` and `multi` questions are shuffled once, deterministically, so the order is stable between launches and the authored order (right answer first, say) never leaks.

## Workflow

1. Copy the sample course folder and rename it. The folder name is the course id.
2. Write `course.yaml` first: it is the spine the app and the validator both follow.
3. Write each lesson. Keep bodies short (the validator accepts 150 to 1,100 words) and end each with three quick-check questions.
4. Write a session check (10 questions) per session, an exercise per session and the final.
5. Run the validator after every few files:

   ```bash
   python3 tools/validate.py
   ```

6. Run the app against your working copy:

   ```bash
   cd src-tauri && LMS_CONTENT_DIR="$PWD/../content" cargo tauri dev
   ```

7. Remove the sample course (or keep it as a demo) and run `cargo test`: `real_corpus_loads` in `src-tauri/src/content.rs` checks the sample course, so update it to name your own.

Work in progress can live in `content/drafts/<course-id>/`: the validator checks it, but the app never loads it.

## Media

Lesson videos go in the course's `videos/` folder (MP4, a poster image and WebVTT captions), or point `video.src` at an `https://` URL. Images in lesson bodies are ordinary markdown. Everything under `content/courses/` is bundled into the app, so large files make the download bigger; hosting them keeps it small.

## Read more links

A lesson can list `help:` links (title and URL) that open in the learner's browser. They must be `https://` and start with `HELP_URL_PREFIX` from `shared/src/brand.rs` (any https link by default). Set the prefix to your docs site to keep links on it.

## Gated courses

Set `gated: true` in `course.yaml` to lock a course behind an access code. See [access.md](access.md).

## The companion's copy

The companion's nudges (`content/sidekick-nudges.yaml`) and hover help (`content/sidekick-help.yaml`) are content too. Write `{companion}` wherever the companion names itself; the app fills in the name from `shared/src/brand.rs`. `cargo test` checks both files: every nudge trigger is covered, every `data-help` key in the UI has an entry, lengths are within limits and there are no em or en dashes.

## Style

Plain, warm and short. Second person. No em or en dashes. Fictional people and companies only, with `.example` domains for addresses.
