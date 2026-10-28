# Course corpus schema

All files are YAML. Ids are kebab-case, and each file is named `<id>.yaml`. The app's loader is `src-tauri/src/content.rs`; `tools/validate.py` checks everything below. The sample course `content/courses/brightline-basics/` uses every part of it.

Check content after any edit:

```bash
python3 tools/validate.py          # schema, counts, answer ranges, ids, media
python3 tools/test_validate.py     # the validator's own tests
```

The loader fails loudly: a malformed file stops the app at startup rather than silently dropping a lesson. Keys the loader does not declare (such as `sources`, `thread` or `answer_notes`) are ignored, so they never reach the app.

## Layout

```
content/courses/
  <course-id>/
    course.yaml
    lessons/<lesson-id>.yaml
    checks/<check-id>.yaml
    exercises/<exercise-id>.yaml
    assessment/final.yaml          # named by course.yaml's `assessment:`
    videos/                        # optional: mp4, poster image, vtt captions
  _shared/
    <name>/{lessons,checks,exercises}/   # items several courses pull in with `shared:`
    app-tour.yaml                  # the first-run tour (version 0 = no tour)
content/drafts/<course-id>/        # same layout; validated, never bundled into the app
```

Folders whose name starts with `_` are not courses. `lessons/`, `checks/` and `exercises/` must exist in every course, even when empty.

## course.yaml

| Key | Required | Notes |
|---|---|---|
| `id` | yes | Kebab-case; matches the folder name |
| `title` | yes | Full title |
| `short` | no | Top-menu label; falls back to `title` |
| `summary` | yes | One or two sentences |
| `order` | no | Menu order, ascending; default 100. Ties keep folder-name order |
| `shared` | no | List of `_shared/<name>` folders to pull lessons, checks and exercises from |
| `gated` | no | `true` locks the course behind an access code; default `false` |
| `thread` | no | Authoring note: the example that runs through the course. Not loaded |
| `sessions` | yes | List of sessions (below) |
| `assessment` | yes | File name (without `.yaml`) under `assessment/`, normally `final` |

Each session has `id`, `title`, `outcome`, `lessons` (a list of lesson ids), `check` and `exercise` (one id each), and an optional `phase` label. This `sessions:` list is the course spine: it drives both the app's navigation and the validator. A lesson file that no session lists is still loaded but never shown, and the validator reports it.

Shared items are validated like any other, and their progress is stored once, so finishing a shared lesson or check in one course counts in every course that lists it. Drafts may omit the final assessment (the validator prints a note); a course under `content/courses/` may not.

## lessons/<id>.yaml

```yaml
id: 1-2-writing-and-formatting
session: s1                 # the session id from course.yaml
number: '1.2'               # a string, so quote it
title: Writing and formatting notes
duration_min: 4             # whole minutes
objective: One sentence, what the learner can do after this lesson.
video: null                 # optional, see "Lesson video"
body: |                     # the reading version of the lesson, markdown, 150 to 1,100 words
  ...
  ![What the learner sees, one plain sentence](assets/media/<file>.webp)   # optional media line, own paragraph
key_points:                 # 3 to 6 strings; the AI tutor is grounded only on these plus the body
  - ...
gotchas:                    # optional strings, symptom then cause then fix
help:                       # optional "Read more" links
  - title: Searching your notes        # required
    url: https://docs.example.com/search
                            # must start with HELP_URL_PREFIX in shared/src/brand.rs
                            # ("https://" by default, which accepts any https link)
quick_check:                # exactly 3 questions (see "Questions")
  - ...
sources:                    # at least one; where each claim comes from. Not loaded
  - { kind: internal, ref: "Product guide, section 3" }
unverified: []              # optional: claims no source confirmed; keep them out of body and questions
```

The quick check's pass mark is fixed in code at 0.66, not set in YAML.

## Questions

Three kinds, used by quick checks, session checks and the final:

```yaml
- kind: single              # or multi
  prompt: ...
  options:                  # at least 2
    - { text: ..., feedback: ... }     # feedback on every option, saying why it is right or wrong
  answer: 0                 # single: an index; multi: a list of distinct indices

- kind: match
  prompt: Match each need to the tool that fits it
  left:  [row A, row B, row C]
  right: [choice 1, choice 2]          # a choice may be used by several rows
  answer: [0, 1, 1]                     # the right-index for each left row
  feedback: [why A, why B, why C]       # one per left row
```

- Start the correct option's feedback with "Correct". When any option's feedback starts that way, the validator checks that those options are exactly the `answer`.
- **Options are shuffled at load.** `single` and `multi` options are reordered with a shuffle seeded from the prompt text, so the order is the same on every view and every launch, and the answer indices move with their options. Write options in whatever order reads best. Editing a prompt changes its option order. `match` rows and choices keep their authored order.
- Scoring: `single` scores 1 or 0. `multi` scores (correct picks minus wrong picks) divided by the number of correct options, floored at 0. `match` scores the share of rows matched correctly. A quiz score is the average across its questions.

## checks/<id>.yaml

```yaml
id: s1-check
session: s1                 # authoring note; not loaded
title: Session 1 check
pass_mark: 0.75             # a fraction above 0 and at most 1; 0.75 is the convention
questions: [ ...exactly 10 questions... ]
```

## exercises/<id>.yaml

```yaml
id: s1-set-up-your-workspace
session: s1                 # authoring note; not loaded
title: ...
minutes: 20
brief: |                    # what the learner gets, markdown
setup: [ ...prerequisites, as strings... ]           # optional
acceptance_criteria: [ ...observable, testable statements... ]   # required; the AI coach uses them as stages
reference_walkthrough: |    # required; unlocked after the learner submits a write-up, markdown
common_mistakes: [ ... ]    # optional; not loaded
sources: [ ... ]            # optional; not loaded
```

Exercises are not graded. The AI review is advisory.

## assessment/final.yaml

```yaml
id: final
title: Final assessment
minutes: 20                 # shown as guidance, not enforced
blurb: One sentence shown on the course card and the assessment start screen.
pass_mark: 0.75             # a fraction above 0 and at most 1
fixture: |                  # markdown shown as the assessment's introduction (a scenario, a brief)
tasks:                      # at least 3, each with a unique id and at least 1 question
  - { id: organise, minutes: 7, questions: [...] }
answer_notes: ...           # optional, grader-only; not loaded
```

A learner passes when the average score across every question of every task is at least `pass_mark`. The certificate needs every session check and the final passed (best scores count, retries are unlimited).

## Lesson video

`video:` is optional; a lesson without one shows text only. It takes one of three forms:

```yaml
video: my-course/videos/1-1-welcome.mp4             # a path relative to content/courses/

video: https://cdn.example.com/lesson.mp4           # passed through, not file-checked

video:
  src: my-course/videos/1-1-welcome.mp4
  poster: my-course/videos/1-1-welcome.jpg
  captions: my-course/videos/1-1-welcome.vtt        # WebVTT
  chapters:
    - { t: 0, title: "Introduction" }               # t in seconds
```

Local `src`, `poster` and `captions` files must exist (while drafting, paths resolve against `content/drafts/` and then `content/courses/`).

## Media in lessons

A line of its own `![caption](url)` inside `body` (or an exercise's `brief` or `reference_walkthrough`) renders as a captioned figure: `.webp`, `.png` or `.gif` as an image, `.mp4` as a muted loop, and a YouTube or `player.vimeo.com/video/<id>` link as an embed. Bundled media lives in `ui/assets/media/` and is referenced as `assets/media/<file>`; the validator checks those files exist. Media lines are left out of the tutor's context and the word count.

## _shared/app-tour.yaml

The first-run tour, shown full screen once after onboarding and again whenever `version` goes up.

```yaml
version: 0                  # 0 is the stub: the app treats it as "no tour"
title: Your course tour
src: _shared/videos/app-tour.mp4      # relative to content/courses, or an https:// URL
poster: _shared/videos/app-tour.jpg   # optional
captions: _shared/videos/app-tour.vtt # optional
duration_s: 0
chapters: []                # optional, { t, title } as for lessons
```

The app also shows no tour when the file is missing, does not parse, or names a video that is not on disk. A missing poster or captions file is dropped. To ship a tour, add the video, fill in `duration_s` and `chapters`, and set `version: 1`.

## Rules for authors

- Every option needs feedback that explains why it is right or wrong.
- Never state a product behaviour you cannot back up. If no source confirms a claim, leave it out of the body and questions and list it under `unverified:`.
- Quote list items that contain `: `, or YAML turns them into maps and the loader rejects the file.
- Distractors are plausible mistakes a learner would actually make, not jokes.
- Keep one example running through each course (the `thread:` note in `course.yaml`).
- Use fictional names for people and companies, and no real customer data.
