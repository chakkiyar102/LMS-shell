# The course tour

A full-screen video that plays once after onboarding and shows learners around the app. A **Course tour** button in the top bar replays it. LMS Shell ships the player and an empty slot, but no film.

## How it works

- `content/courses/_shared/app-tour.yaml` describes the tour. `content::app_tour` (in `src-tauri/src/content.rs`) loads it.
- The committed file has `version: 0`, the stub. Version 0 means "no tour": it never opens, and the top-bar button is hidden.
- The tour opens by itself when its `version` is above 0 and newer than the version this device has watched (`shared::tour_due`). The watched version is stored in the learner database, so replaying an older cut never re-arms a newer one.
- On the first run, Continue unlocks when the video ends, is played to within 2 seconds of the end, or fails to load. A broken video never traps the learner.
- A missing poster or captions file is dropped, not fatal. A missing local video means no tour.

## Add a tour

1. Make the film. A screen recording of the app with the sample course (or your own) is plenty: 60 to 120 seconds, 1920 by 1080, H.264 MP4. The browser harness in `tools/ui-check/` can drive the real UI for repeatable captures (see `tools/ui-check/tour.mjs`).
2. Put the files under `content/courses/_shared/videos/`:
   - `app-tour.mp4`
   - `app-tour.jpg` (poster, optional)
   - `app-tour.vtt` (captions, optional but recommended)
3. Edit `app-tour.yaml`:

   ```yaml
   version: 1
   title: Your course tour
   src: _shared/videos/app-tour.mp4
   poster: _shared/videos/app-tour.jpg
   captions: _shared/videos/app-tour.vtt
   duration_s: 94.5
   chapters:
     - { t: 0, title: Home }
     - { t: 22, title: Lessons }
     - { t: 48, title: Checks and the final }
     - { t: 71, title: Your guide }
   ```

   Paths are relative to `content/courses/`. `src` may also be an `https://` URL.
4. Run the app with `LMS_CONTENT_DIR` pointing at `content/`. The tour opens after onboarding (or press **Course tour**).
5. Shipping a new cut later? Raise `version` and every learner sees it once more.

Large videos make the app bundle bigger, since `content/courses/` is bundled as a resource. Hosting the film and pointing `src` at an `https://` URL keeps the download small.
