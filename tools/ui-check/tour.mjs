// Visual tour of the app as built: the real UI build (trunk) in Chromium, the Tauri bridge answered from
// real command output dumped by src-tauri/examples/tour_dump.rs (the sample course, a seeded temp db).
// Learner: Jordan Hale (fictional), learning "Getting started with Brightline Notes" (a fictional notes app).
//
// Run all (from the repo root; any dist built by `trunk build`, e.g. the one tools/ui-check/run.sh leaves in OUT/dist):
//   LMS_CONTENT_DIR=content cargo run -p lms-shell --example tour_dump -- /tmp/tour-data.json
//   (cd ui && trunk build --dist /tmp/tour-dist)
//   node tools/ui-check/tour.mjs /tmp/tour-data.json /tmp/tour-dist OUT_DIR
//
// Only some shots: node tools/ui-check/tour.mjs DATA DIST OUT_DIR [PORT] [shot-prefix]
// The README screenshots (1280x840, light and dark), written straight into docs/screenshots/:
//   node tools/ui-check/tour.mjs /tmp/tour-data.json /tmp/tour-dist docs/screenshots 8823 readme
//
// Serves the dist and the lesson media itself (with Range, so videos seek). Uses Playwright's Chromium; set
// TOUR_CHANNEL=chrome to use the installed Google Chrome instead (Playwright's Chromium has no H.264, so a
// fork whose lessons have MP4 videos wants that). Every expectation that fails is printed on stderr and
// makes the run exit 1; OUT_DIR/captions.txt lists every shot with what it shows.
import { chromium } from "playwright";
import fs from "node:fs";
import { serve, COMPANION } from "./tour-lib.mjs";

const [DATA, DIST, OUT, PORT_ARG, ONLY] = process.argv.slice(2);
fs.mkdirSync(OUT, { recursive: true });
const { data, TOUR_NOTE, COURSE, server, open } = await serve(DATA, DIST, Number(PORT_ARG || 8823));

let problems = 0;
const fail = (...a) => { problems++; console.error(...a); };
const shots = [];
const want = (n) => !ONLY || n.startsWith(ONLY);
async function shot(p, name, caption, opts = {}) {
  await p.screenshot({ path: `${OUT}/${name}.png`, ...opts });
  shots.push(`${name}.png: ${caption}`);
  console.log(`${name}.png`);
}
const scrollPane = (p, y) => p.evaluate((y) => { const m = document.querySelector("#course-pane"); if (m) m.scrollTop = y; window.scrollTo(0, y); }, y);
/** Seek the lesson video and wait for the frame. */
async function seek(p, t) {
  return p.evaluate(async (t) => {
    const v = document.querySelector("video");
    if (!v) return "no video";
    if (v.readyState < 1) await new Promise((r) => { v.addEventListener("loadedmetadata", r, { once: true }); setTimeout(r, 8000); });
    v.currentTime = t;
    await new Promise((r) => { v.addEventListener("seeked", r, { once: true }); setTimeout(r, 8000); });
    await new Promise((r) => setTimeout(r, 400));
    return `${v.readyState} ${v.currentTime.toFixed(1)}/${v.duration.toFixed(1)} ${v.videoWidth}x${v.videoHeight}`;
  }, t);
}
// suppress the once-a-day "studied today" nudge where it would cover what a shot is about
const today = () => new Date().toDateString();
// every coach mark (ui/src/sidekick/coach.rs)
const MARKS = ["where-to", "continue", "first-lesson", "quick-check", "practice", "first-exercise", "first-check", "ai-provider", "streak",
  "coach-look", "coach-act", "coach-hint", "coach-next", "coach-send", "certificate"];
// a calm page for stills: no nudge, no coach mark, no hover help due
const QUIET = () => ({ sidekick_today: today(), sidekick_coach_seen: JSON.stringify(MARKS), sidekick_coach_last: String(Date.now()),
  sidekick_snooze: JSON.stringify(Object.fromEntries(["first_launch", "back_after_break", "studied_today", "all_lessons_done_todo", "session_lessons_done", "lesson_done_next"].map((t) => [`${COURSE}/${t}`, Date.now() + 1e10]))) });
const course = data.main.by_course[COURSE].course;
const sessionNo = (i) => String(i + 1).padStart(2, "0");
const firstLesson = course.sessions[0].lessons[0], midLesson = course.sessions[0].lessons[1];
// the session with a check not passed yet (the dump has session 1's passed)
const openSession = Math.max(0, course.sessions.findIndex((s) => s.check_best == null));
/** Home → session card → lesson row, the way a learner gets there. */
async function toLesson(p, s, l) {
  await p.locator(".welcome-title").waitFor();
  await p.locator(".session-card", { hasText: sessionNo(s) }).first().click();
  await p.locator(".lesson-row", { hasText: course.sessions[s].lessons[l].number }).click();
  await p.locator("article.lesson").waitFor();
}
async function toCheck(p, s) {
  await p.locator(".welcome-title").waitFor();
  await p.locator(".session-card", { hasText: sessionNo(s) }).first().click();
  await p.locator(".action-card", { hasText: "Session check" }).click();
  await p.locator(".options, .match").first().waitFor();
}
async function answerAll(p) {
  for (const g of await p.locator(".question").all()) {
    const r = g.locator("input[type=radio], input[type=checkbox]").first();
    if (await r.count()) await r.check();
    for (const s of await g.locator("select").all()) await s.selectOption({ index: 1 });
  }
}
/** README stills: wait out the companion's short line at the tab (a quip), so the shot shows the page. */
const calm = (p) => p.waitForFunction(() => !document.querySelector(".sidekick-quip.on"), null, { timeout: 8000 }).catch(() => fail("a quip stayed up"));
const toSettings = async (p) => { await p.locator(".welcome-title").waitFor(); await p.locator("button[aria-label=Settings]").click(); await p.locator("form.profile-form").waitFor(); };

const browser = await chromium.launch(process.env.TOUR_CHANNEL ? { channel: process.env.TOUR_CHANNEL } : {});
try {
  // 0. the README screenshots: 1280x840, nothing popping up unless the shot is about it
  if (want("readme")) {
    const README = { width: 1280, height: 840 };
    for (const theme of ["light", "dark"]) {
      const { ctx, p } = await open(browser, { profile: false, theme, viewport: README });
      await p.locator(".onboard-form").waitFor();
      await p.locator(".onboard-form input").first().fill("Jordan Hale");
      await p.locator(".onboard-form input[type=email]").fill("jordan.hale@brightline.example");
      await p.locator(".onboard-form select").selectOption("learner");
      await p.locator("#onboard-account").fill("notes.brightline.example");
      await p.locator(".onboard-form h2").click(); // blur, no focus ring on the last field
      await p.waitForTimeout(400);
      await shot(p, `onboarding-${theme}`, `Onboarding (${theme}): name, work email, role and the optional practice app address`);
      await ctx.close();
    }
    for (const theme of ["light", "dark"]) {
      const { ctx, p } = await open(browser, { theme, viewport: README, storage: QUIET() });
      await p.locator(".welcome-title").waitFor();
      await p.waitForTimeout(1800);
      await calm(p);
      if (await p.locator(".sidekick-bubble, .sidekick-coach").count()) fail(`home-${theme}: a bubble is up over the README shot`);
      await shot(p, `home-${theme}`, `Course home (${theme}), mid-way: welcome banner, continue card, progress, streak and the ${COMPANION} tab`);
      await ctx.close();
    }
    {
      const { ctx, p } = await open(browser, { viewport: README, storage: QUIET() });
      await toLesson(p, 0, 1);
      await p.waitForTimeout(900);
      await calm(p);
      await shot(p, "lesson-light", `Lesson ${midLesson.number} ${midLesson.title}: the session rail, the lesson body and its key points`);
      await ctx.close();
    }
    {
      const { ctx, p } = await open(browser, { viewport: README, quiz: "mixed", storage: QUIET() });
      await toCheck(p, openSession);
      await answerAll(p);
      await p.locator(".quiz-bar .btn.primary").first().click();
      await p.locator(".quiz-bar .result").first().waitFor();
      // the first wrong answer's feedback in the middle of the window, a right one above it
      await p.locator(".feedback:not(.ok)").first().evaluate((e) => e.closest(".question").scrollIntoView({ block: "center" }));
      await p.waitForTimeout(600);
      await calm(p);
      await shot(p, "check-light", `Session ${openSession + 1} check, graded: Correct and Not quite with feedback under each answer`);
      await ctx.close();
    }
    {
      const { ctx, p } = await open(browser, { viewport: README, storage: QUIET() });
      await toSettings(p);
      await p.locator("details.caps summary").first().click();
      // the AI provider section at the top, clear of the sticky top bar
      await p.locator("details.caps").first().evaluate((e) => { const s = e.closest("section"); s.style.scrollMarginTop = "84px"; s.scrollIntoView({ block: "start" }); });
      await p.waitForTimeout(500);
      await calm(p);
      if (!(await p.locator("details.caps[open] table").first().isVisible().catch(() => false))) fail("settings-light: the Which one should I pick? table should be open");
      await shot(p, "settings-light", "Settings, AI provider: the choices and Which one should I pick? open, the chosen provider's column lit");
      await ctx.close();
    }
    {
      const msgs = [
        { role: "user", content: "What's the difference between a notebook and a tag?" },
        { role: "assistant", content: "A note lives in exactly **one notebook**, like a folder. **Tags** cut across notebooks:\n\n- give a note as many tags as you like\n- click a tag to see every note that has it\n\nFor the garden day, keep the plan in its own notebook and tag the jobs `#volunteers`." },
      ];
      const { ctx, p } = await open(browser, { viewport: README, extra: { get_messages: msgs }, storage: { ...QUIET(), sidekick_pane_mode: "ask" } });
      await toLesson(p, 0, 2);
      await p.locator(".sidekick-tab").click();
      await p.locator(".sidekick-side-pane .msg.assistant").first().waitFor();
      await p.waitForTimeout(900);
      await calm(p);
      await shot(p, "companion-light", `${COMPANION}, the companion, opened from its tab beside a lesson: a question about the lesson and its formatted answer`);
      await ctx.close();
    }
  }

  // 1. onboarding
  if (want("01") || want("02") || want("03")) {
    const { ctx, p } = await open(browser, { profile: false });
    await p.locator(".onboard-form").waitFor();
    const shown = await p.locator(".onboard-form select").evaluate((s) => [s.value, s.selectedOptions[0]?.textContent]);
    if (shown[0] !== "" || shown[1] !== "Choose your role") fail("role select should show the placeholder, shows:", shown);
    await shot(p, "01-onboarding-empty", "Onboarding, empty: name, work email, the role select on its Choose your role placeholder");
    await p.locator(".onboard-form input").first().fill("Jordan Hale");
    await p.locator(".onboard-form input[type=email]").fill("jordan.hale@brightline.example");
    await p.locator(".onboard-form button[type=submit]").click();
    await p.waitForTimeout(300);
    // the select is required: the browser's own validation stops the submit before save_profile
    if (await p.evaluate(() => window.__calls.some(([c]) => c === "save_profile"))) fail("Start learning with no role should not save the profile");
    if (await p.locator(".onboard-form select").evaluate((s) => s.validity.valid)) fail("the role select should be invalid while on Choose your role");
    await shot(p, "01b-onboarding-role-required", "No role chosen: the select still reads Choose your role, and Start learning goes nowhere");
    await p.reload();
    await p.locator(".onboard-form").waitFor();
    await p.waitForTimeout(500);
    await p.locator(".onboard-form input").first().fill("Jordan Hale");
    await p.locator(".onboard-form input[type=email]").fill("jordan.hale@brightline.example");
    // a native <select> popup never reaches a screenshot: show its options in place (size=4) instead
    await p.locator(".onboard-form select").evaluate((s) => { s.size = 4; s.style.height = "auto"; s.focus(); });
    await shot(p, "02-onboarding-role-options", "Onboarding, the role choices (select drawn open with size=4, a native popup can't be captured)");
    await p.locator(".onboard-form select").evaluate((s) => { s.size = 0; s.style.height = ""; });
    // a learner: no access code asked
    await p.locator(".onboard-form select").selectOption("learner");
    await p.waitForTimeout(200);
    if (await p.locator(".onboard-form input[type=password]").count()) fail("a Learner should not be asked for an access code at onboarding");
    if ((await p.locator(".onboard-form button[type=submit]").textContent()) !== "Start learning") fail("a Learner's submit should read Start learning");
    // staff and partners are offered the access code (brand::ROLES_ASKING_CODE), with I'll add it later
    for (const [role, label] of [["staff", "Staff"], ["partner", "Partner"]]) {
      await p.locator(".onboard-form select").selectOption(role);
      await p.waitForTimeout(300);
      if (!(await p.locator(".onboard-form input[type=password]").count())) fail(`${label} should get the access code field`);
      if (!(await p.locator(".onboard-form button", { hasText: "add it later" }).count())) fail(`${label} should get I'll add it later`);
      if ((await p.locator(".onboard-form button[type=submit]").textContent()) !== "Unlock and start") fail(`${label}'s submit should read Unlock and start`);
      const note = await p.locator(".onboard-form p.meta[aria-live]").textContent();
      if (!/access code/.test(note)) fail(`${role} role note:`, note);
      await shot(p, `03${role === "staff" ? "a" : "b"}-onboarding-${role}-code`, `Onboarding, ${label}: username and access code fields, Unlock and start, and I'll add it later`);
    }
    await p.locator(".onboard-form input[type=email]").fill("jordan@mailinator.com");
    await p.locator(".onboard-form button", { hasText: "add it later" }).click();
    await p.waitForTimeout(300);
    if (!/disposable/.test(await p.locator(".onboard-form .notice.error").textContent().catch(() => ""))) fail("a disposable address should be refused");
    await shot(p, "03c-onboarding-disposable-email-error", "Onboarding, Partner with a disposable (mailinator.com) address: Use your work email, temporary or disposable addresses aren't accepted");
    await ctx.close();
  }

  // 2. course home
  if (want("04") || want("05")) {
    const { ctx, p } = await open(browser, {});
    await p.locator(".welcome-title").waitFor();
    await p.waitForTimeout(1800); // the companion's start-up nudge, if any
    await shot(p, "04-home-midway", `${course.title}, mid-way: welcome banner (Keep going, Jordan), the continue card, the ${COMPANION} tab`);
    await p.keyboard.press("Escape");
    await p.locator(".card.overview").evaluate((e) => e.scrollIntoView({ block: "start" }));
    await p.waitForTimeout(900);
    await shot(p, "05-home-progress-sessions", "Progress ring, streak, activity strip and session cards");
    await ctx.close();
  }
  if (want("09")) {
    const { ctx, p } = await open(browser, { scenario: "fresh" });
    await p.locator(".welcome-title").waitFor();
    await p.locator(".sidekick-bubble").waitFor({ timeout: 8000 }).catch(() => {});
    await p.waitForTimeout(600);
    if (!new RegExp(`I'm ${COMPANION}`).test(await p.locator(".sidekick-bubble").textContent().catch(() => ""))) fail(`first launch: ${COMPANION}'s hello nudge should name it`);
    if (/\{companion\}/.test(await p.content())) fail("a {companion} placeholder reached the page");
    await shot(p, "09-home-first-launch-hello-nudge", `First launch (fresh db): Glad you're here, Jordan; ${COMPANION}'s hello nudge at its tab`);
    await ctx.close();
  }

  // 3. a lesson, and a session check
  if (want("12")) {
    const { ctx, p } = await open(browser, { storage: { sidekick_today: today() } });
    await toLesson(p, 0, 1);
    if (await p.locator("article.lesson video").count()) console.log("  video:", await seek(p, 20));
    await p.waitForTimeout(500);
    await shot(p, "12-lesson", `Lesson ${midLesson.number} ${midLesson.title}: session rail, body, key points`);
    await p.locator(".quiz").first().evaluate((e) => e.scrollIntoView({ block: "start" })).catch(() => fail("the lesson has no quick check"));
    await p.waitForTimeout(400);
    await shot(p, "12b-lesson-quick-check-practice", "The same lesson further down: the quick check and Try it for real");
    await ctx.close();
  }
  if (want("14")) {
    const { ctx, p } = await open(browser, { quiz: "mixed", storage: { sidekick_today: today() } });
    await toCheck(p, openSession);
    await p.waitForTimeout(500);
    await shot(p, "14-session-check", `Session ${openSession + 1} check: its questions, options shuffled at load`);
    await answerAll(p);
    await p.locator(".quiz-bar .btn.primary").first().click();
    await p.locator(".quiz-bar .result").first().waitFor();
    if ((await p.locator(".feedback").count()) !== (await p.locator(".question").count())) fail("every graded question should carry feedback");
    await p.locator(".quiz-bar").last().evaluate((e) => e.scrollIntoView({ block: "center" }));
    await p.waitForTimeout(400);
    await shot(p, "14b-session-check-graded", "Graded: the score, pass or not yet, Resubmit");
    await ctx.close();
  }

  // 4. the companion
  if (want("15") || want("16")) {
    // at rest the companion peeks from its tab on the right edge, and a scroll doesn't send it off; it only flies
    // out to walk the learner to something (a coach mark's Show me), then flies home
    const marks = JSON.stringify(MARKS.filter((m) => m !== "continue"));
    const { ctx, p } = await open(browser, { storage: { sidekick_today: today(), sidekick_coach_seen: marks, sidekick_coach_last: "0" } });
    await p.locator(".welcome-title").waitFor();
    const atRest = () => p.evaluate(() => ({
      peek: !!document.querySelector(".sidekick-tab-wrap .sidekick-peek:not(.away) img"),
      out: !!document.querySelector(".sidekick-companion:not(.hidden)"),
      flying: !!document.querySelector(".sidekick-companion.flying"),
    }));
    const r0 = await atRest();
    if (!r0.peek || r0.out) fail("at rest the companion should peek from its tab, not stand on the page:", JSON.stringify(r0));
    // the Continue coach mark (Show me): the companion flies out of the tab to the card; catch it mid-flight
    const t0 = Date.now();
    let caught = false;
    while (Date.now() - t0 < 15000 && !caught) {
      const src = await p.locator(".sidekick-companion img").getAttribute("src").catch(() => "");
      if (/\/(fly|takeoff)\.png$/.test(src || "") && await p.locator(".sidekick-companion.flying").count()) {
        const arc = await p.locator(".sidekick-companion").evaluate((e) => e.getAnimations().some((a) => a.playState === "running" && /translate/.test(JSON.stringify(a.effect.getKeyframes()))));
        if (!arc) fail("mid-flight the companion should be on a running WAAPI arc");
        const coach = p.locator(".sidekick-coach");
        if (!(await coach.count()) || !(await coach.getByRole("button", { name: "Show me" }).count())) fail("the companion should only fly out for a coach mark with Show me");
        await shot(p, "15-companion-mid-flight", `${COMPANION} flies out of its tab for a coach mark's Show me (WAAPI arc): ${src.split("/").pop()}`);
        caught = true;
      }
      else await p.waitForTimeout(40);
    }
    if (!caught) fail("no flight caught for the Continue coach mark:", await p.evaluate(() => window.__srcs));
    else {
      // it lands beside the card, never on it, and the tab's peek gives way while it's out
      await p.waitForTimeout(1600);
      const card = await p.locator('[data-sidekick-anchor="continue"]').boundingBox();
      const k = await p.locator(".sidekick-companion").boundingBox();
      if (card && k && k.x < card.x + card.width && k.x + k.width > card.x && k.y < card.y + card.height && k.y + k.height > card.y) fail("the companion landed on the card:", JSON.stringify({ k, card }));
      if ((await atRest()).peek) fail("the tab's peek should hide while the companion is out");
      await p.locator(".sidekick-coach").getByRole("button", { name: "Got it" }).click();
      await p.waitForTimeout(1800);
      const r2 = await atRest();
      if (!r2.peek || r2.out) fail("after Got it the companion should be back in its tab:", JSON.stringify(r2));
    }
    await p.locator(".sidekick-tab").click();
    await p.locator(".sidekick-where-toggle").click();
    await p.locator(".sidekick-where .sidekick-chips").waitFor();
    const chips = await p.locator(".sidekick-where .sidekick-chips .chip").allTextContents();
    if (chips.length !== 4) fail("Where to? should offer four chips:", chips);
    await p.waitForTimeout(500);
    await shot(p, "16-companion-where-to", `${COMPANION}'s pane, opened from the tab on the right edge: Where to? with its chips and the ask box`);
    await ctx.close();
  }
  if (want("27")) {
    const { ctx, p } = await open(browser, { storage: { sidekick_today: today(), sidekick_coach_last: "0", sidekick_coach_seen: JSON.stringify(MARKS.filter((m) => m !== "first-lesson")) } });
    await toLesson(p, 0, 0);
    await p.locator('[data-sidekick-anchor="complete"]').waitFor();
    await p.evaluate(() => document.querySelector('[data-sidekick-anchor="complete"]').scrollIntoView({ block: "center" }));
    await p.locator(".sidekick-coach").waitFor({ timeout: 15000 }).catch(() => fail("no first-lesson coach mark"));
    await p.waitForTimeout(1800);
    await shot(p, "27-coach-mark-first-lesson", `Coach mark on a first lesson: the ring round Mark complete, ${COMPANION} beside it pointing, Show me / Got it`);
    await ctx.close();
  }
  if (want("28") || want("29")) {
    const { ctx, p } = await open(browser, { storage: { sidekick_today: today() } });
    await p.locator(".welcome-title").waitFor();
    await p.waitForTimeout(1500);
    await scrollPane(p, 900);
    await p.waitForTimeout(3500);
    await p.locator(".sidekick-tab").click();
    const k = p.locator(".pane-sidekick");
    await k.click();
    await p.waitForTimeout(260);
    const fx = await k.locator(".sidekick-fx").getAttribute("class");
    await shot(p, "28-companion-tap-trick", `One tap on ${COMPANION} in its pane: a trick (${fx.replace("sidekick-fx", "").trim()}) with its line in the header`);
    await p.waitForTimeout(1500);
    await k.click(); await p.waitForTimeout(120); await k.click(); await p.waitForTimeout(120); await k.click();
    await p.waitForTimeout(500);
    await shot(p, "29-companion-dizzy", `Three quick taps: ${COMPANION} gets dizzy and says so`);
    await ctx.close();
  }

  // 5. settings and the certificate
  if (want("18")) {
    const { ctx, p } = await open(browser, { storage: { sidekick_today: today() } });
    await toSettings(p);
    await p.waitForTimeout(1500);
    await shot(p, "18-settings-profile", "Settings: Profile (name, work email, role)");
    await p.getByRole("heading", { name: "Access" }).scrollIntoViewIfNeeded();
    await p.waitForTimeout(400);
    const text = await p.locator("form.unlock").locator("xpath=ancestor::section[1]").textContent();
    if (!text.includes("Gated courses open with the username and access code you were given.")) fail("Access copy:", text);
    await shot(p, "18b-settings-access", "Settings with no access code saved: the Access section");
    await p.locator("details.caps summary").first().click();
    await p.locator("details.caps").first().evaluate((e) => e.scrollIntoView({ block: "center" }));
    await p.waitForTimeout(400);
    await shot(p, "18c-settings-provider-guide", "Settings, AI provider: Which one should I pick? open, the chosen provider's column lit");
    await p.locator("label.choice", { hasText: "ChatGPT plan" }).click();
    await p.locator("details.caps").first().evaluate((e) => e.scrollIntoView({ block: "center" }));
    await p.waitForTimeout(300);
    await shot(p, "18d-settings-provider-guide-chatgpt", "Same, with ChatGPT plan chosen");
    await ctx.close();
  }
  if (want("18e") || want("18f") || want("18g")) {
    // a Partner with a code saved: Remove access code asks first, then locks gated courses again
    const { ctx, p } = await open(browser, { role: "partner", code: true, storage: { sidekick_today: today() } });
    await p.locator(".welcome-title").waitFor();
    if (await p.locator("nav.tabs .tab-lock").count()) fail("with a code saved, no tab should show a lock");
    await toSettings(p);
    const remove = p.locator("form.unlock button", { hasText: "Remove access code" });
    await remove.waitFor();
    await remove.evaluate((e) => e.closest("section").scrollIntoView({ block: "center" }));
    await p.waitForTimeout(400);
    await shot(p, "18e-settings-access-code-saved", "Settings as a Partner with a code saved: Remove access code beside Unlock course");
    await remove.click();
    await p.locator(".confirm-inline").waitFor();
    await p.waitForTimeout(200);
    const f1 = await p.evaluate(() => document.activeElement?.textContent);
    if (f1 !== "Keep it") fail("the inline confirm should focus Keep it, focus is on:", f1);
    await shot(p, "18f-settings-remove-code-confirm", "Remove access code, confirmed inline: Keep it (focused) or Remove access code");
    await p.keyboard.press("Escape");
    await p.waitForTimeout(200);
    if (await p.locator(".confirm-inline").count()) fail("Escape did not cancel the inline confirm");
    const f2 = await p.evaluate(() => document.activeElement?.textContent);
    if (f2 !== "Remove access code") fail("focus should return to Remove access code, is on:", f2);
    await remove.click();
    await p.locator(".confirm-inline button.danger").click();
    await p.locator("section.block .result.pass", { hasText: "Access code removed" }).waitFor();
    await p.waitForTimeout(500);
    if (await p.locator("form.unlock button", { hasText: "Remove access code" }).count()) fail("Remove access code should be gone once removed");
    if (!(await p.evaluate(() => window.__calls.some(([c]) => c === "remove_access_code")))) fail("remove_access_code was not called");
    await shot(p, "18g-settings-access-code-removed", "Code removed: Access code removed, gated courses are locked on this computer");
    await ctx.close();
  }
  for (const theme of ["light", "dark"]) {
    const name = `18h-settings-start-over-dialog-${theme}`;
    if (!want(name)) continue;
    const { ctx, p } = await open(browser, { theme, storage: { sidekick_today: today() } });
    await toSettings(p);
    const btn = p.locator("button", { hasText: "Start over on this computer" });
    await btn.scrollIntoViewIfNeeded();
    await p.waitForTimeout(400);
    if (theme === "light") await shot(p, "18h-settings-start-over-zone", "Bottom of Settings: the Start over block");
    await btn.click();
    await p.locator("section.confirm").waitFor();
    await p.waitForTimeout(400);
    const focused = await p.evaluate(() => document.activeElement?.textContent);
    if (focused !== "Cancel") fail("the Start over dialog should focus Cancel, focus is on:", focused);
    for (let i = 0; i < 3; i++) await p.keyboard.press("Tab");
    if (!(await p.evaluate(() => !!document.activeElement?.closest("section.confirm")))) fail("Tab left the Start over dialog");
    await p.evaluate(() => document.querySelector("section.confirm button")?.focus());
    await shot(p, name, `Start over on this computer? Cancel focused, Erase and start over in --bad (${theme})`);
    await p.keyboard.press("Escape");
    await p.waitForTimeout(250);
    if (await p.locator("section.confirm").count()) fail("Escape did not cancel the Start over dialog");
    const back = await p.evaluate(() => document.activeElement?.textContent);
    if (back !== "Start over on this computer") fail("focus should return to the Start over button, is on:", back);
    await ctx.close();
  }
  if (want("19")) {
    const { ctx, p } = await open(browser, { storage: { sidekick_today: today() } });
    await p.locator(".welcome-title").waitFor();
    await p.locator(".session-card.final", { hasText: "Your certificate" }).click();
    await p.locator("ul.todo").waitFor();
    await p.waitForTimeout(3500); // the companion walks to the to-do list first
    await shot(p, "19-certificate-todo", "Certificate: still to do, each item linked to its check or the final");
    await ctx.close();
  }

  // 6. the course tour: only when content/courses/_shared/app-tour.yaml has one (version > 0)
  if ((want("23") || want("26")) && !data.app_tour) console.log("  (no course tour in this content: tour shots 23-26 skipped)");
  if (data.app_tour && want("23")) {
    const { ctx, p } = await open(browser, { tour: "due", storage: { sidekick_today: today() } });
    await p.locator("section.tour").waitFor();
    const cont = p.locator("section.tour .tour-foot .btn");
    console.log("  tour video:", await seek(p, 2));
    await p.evaluate(() => document.querySelector("section.tour video").pause());
    await p.keyboard.press("Escape");
    await p.mouse.click(8, 8); // the backdrop
    await p.waitForTimeout(300);
    if (!(await p.locator("section.tour").count())) fail("first-run tour closed on Escape or the backdrop");
    if ((await cont.getAttribute("aria-disabled")) !== "true") fail("Continue should be disabled mid-tour");
    await shot(p, "23-tour-first-run", `First run: the tour opens full screen after onboarding; Continue disabled, the line says why${TOUR_NOTE}`);
    await ctx.close();
  }
  if (data.app_tour && want("26")) {
    const { ctx, p } = await open(browser, { storage: { sidekick_today: today() } });
    await p.locator(".welcome-title").waitFor();
    await p.locator("button[aria-label='Course tour']").click();
    await p.locator("section.tour").waitFor();
    await p.evaluate(() => document.querySelector("section.tour video")?.pause());
    if ((await p.locator("section.tour .tour-foot .btn").getAttribute("aria-disabled")) !== "false") fail("replay Continue should be enabled");
    await shot(p, "26-tour-replay", `Replay from the top bar: close button, Continue open from the start${TOUR_NOTE}`);
    await p.keyboard.press("Escape");
    await p.waitForTimeout(300);
    if (await p.locator("section.tour").count()) fail("Escape did not close the replay");
    await ctx.close();
  }
  if (!data.app_tour && want("25")) {
    const { ctx, p } = await open(browser, { storage: { sidekick_today: today() } });
    await p.locator(".welcome-title").waitFor();
    if (await p.locator("button[aria-label='Course tour']").count()) fail("with no course tour, the top bar should have no Course tour button");
    if (await p.locator("section.tour").count()) fail("with no course tour, nothing should open");
    await ctx.close();
  }

  // 7. dark theme
  if (want("20")) {
    const { ctx, p } = await open(browser, { theme: "dark" });
    await p.locator(".welcome-title").waitFor();
    await p.waitForTimeout(1800);
    await shot(p, "20-dark-home", "Dark theme: course home");
    await ctx.close();
  }
  if (want("21")) {
    const { ctx, p } = await open(browser, { theme: "dark", storage: { sidekick_today: today() } });
    await toLesson(p, 1, 1);
    await p.waitForTimeout(500);
    await shot(p, "21-dark-lesson", "Dark theme: a lesson");
    await ctx.close();
  }
  if (want("22")) {
    const { ctx, p } = await open(browser, { theme: "dark", storage: { sidekick_today: today() } });
    await p.locator(".welcome-title").waitFor();
    await p.locator(".session-card.final", { hasText: "Your certificate" }).click();
    await p.locator("ul.todo").waitFor();
    await p.waitForTimeout(3500);
    await shot(p, "22-dark-certificate-todo", "Dark theme: certificate to-do list");
    await ctx.close();
  }

  // 8. no practice app address set: the coaching session says so where the learner looks, and goes to Settings
  if (want("30")) {
    const NO_URL = "Add your practice app's address in Settings, under Practice app, to open it here.";
    for (const android of [false, true]) {
      const { ctx, p } = await open(browser, { fail: { open_practice: NO_URL }, storage: { sidekick_today: today() }, viewport: android ? { width: 390, height: 844 } : { width: 1280, height: 800 } });
      if (android) { await p.addInitScript(() => Object.defineProperty(navigator, "userAgent", { get: () => "Mozilla/5.0 (Linux; Android 14) Chrome/120" })); await p.reload(); await p.waitForTimeout(1500); }
      await toLesson(p, 0, 1);
      await p.locator(".practice .btn.primary").click();
      if (android) await p.locator(".srail-btn[aria-label=Practice]").click();
      await p.waitForTimeout(800);
      const shown = await p.evaluate(() => [...document.querySelectorAll(".practice-error")].filter((e) => {
        const r = e.getBoundingClientRect();
        return !e.closest("[aria-hidden=true]") && r.height > 0 && r.top >= 0 && r.bottom <= innerHeight && getComputedStyle(e).visibility !== "hidden";
      }).length);
      if (!shown) fail(`no-URL message not on screen (${android ? "Android" : "desktop"})`);
      const pw = await p.locator(".practice p").first().evaluate((e) => e.getBoundingClientRect().width);
      if (!android && pw < 200) fail("the coaching block's text is squeezed in the session's course column:", pw);
      await shot(p, android ? "30b-no-practice-url-android" : "30-no-practice-url", android
        ? "Android, no practice app address set: tapping Practice brings the message into view, with Open Settings"
        : `No practice app address set: the course column and ${COMPANION}'s column say where to add it, with Open Settings`);
      if (!android) {
        await p.locator(".pane-body .practice-error button", { hasText: "Open Settings" }).click();
        await p.locator("form.profile-form").waitFor();
        await p.waitForTimeout(700);
        const f = await p.evaluate(() => document.activeElement?.id);
        if (f !== "practice-url") fail("Open Settings should focus the Practice app field, focus is on:", f);
        if (await p.locator(".srail").count()) fail("Open Settings should end the session");
      }
      await ctx.close();
    }
  }
  // 9. model replies are formatted (shared::md → elements, never HTML): bold, a list, code; no raw markers
  for (const theme of ["light", "dark"]) {
    if (!want(`31-${theme}`) && !want("31")) continue;
    const reply = "Good start. Before you write, what do you think a **notebook** is for, and what is a **tag**?\n\nThen, in the note:\n- Add a *Volunteers* heading\n- Add a **checklist** of jobs\n- Tag it `#garden-day`\n\n1. Save\n2. Share it with the team";
    const coach = { title: "Coaching session", stages: ["Plan the notebook", "Write the notes", "Share it"], stage: 0, hints_used: 0,
      messages: [{ role: "assistant", content: reply }, { role: "user", content: "a **notebook** holds notes?" }, { role: "assistant", content: "Yes: each *note* lives in one notebook. <b>not html</b>" }] };
    const { ctx, p } = await open(browser, { theme, extra: { get_coach: coach }, storage: { sidekick_today: today() }, viewport: { width: 1280, height: 800 } });
    await toLesson(p, 0, 1);
    await p.locator(".practice .btn.primary").click();
    await p.locator(".pane-body .msg.assistant").first().waitFor();
    await p.waitForTimeout(500);
    const r = await p.evaluate(() => {
      const ms = [...document.querySelectorAll(".pane-body .msg.assistant")];
      return { strong: ms[0]?.querySelectorAll("strong").length, li: ms[0]?.querySelectorAll("ul li").length, ol: ms[0]?.querySelectorAll("ol li").length, code: ms[0]?.querySelectorAll("code").length,
        raw: ms.some((m) => /\*\*|`/.test(m.textContent)), html: !!ms[1]?.querySelector("b"), user: document.querySelector(".pane-body .msg.user")?.textContent };
    });
    if (r.strong < 3 || r.li !== 3 || r.ol !== 2 || r.code !== 1 || r.raw || r.html) fail("coach reply not formatted as expected:", JSON.stringify(r));
    if (r.user !== "a **notebook** holds notes?") fail("the learner's own message should stay as typed:", r.user);
    await shot(p, `31-coach-formatted-${theme}`, `Coaching session (${theme}): replies with bold, italic, a bullet and a numbered list and inline code; the learner's own words as typed; HTML in a reply stays text`);
    await ctx.close();
  }
  // 10. What's new after an update: a real modal (Got it focused, Tab stays in, Escape dismisses and says so)
  if (want("32")) {
    const { ctx, p } = await open(browser, { news: { version: "0.2.0", title: "Calmer nudges", new: ["Search inside a lesson."], fixed: [`${COMPANION}'s speech bubble stays inside the window.`] } });
    await p.locator("section.news").waitFor();
    await p.waitForTimeout(400);
    const f = await p.evaluate(() => document.activeElement?.textContent);
    if (f !== "Got it") fail("What's new should focus Got it, focus is on:", (f || "").slice(0, 40));
    for (let i = 0; i < 3; i++) await p.keyboard.press(i === 1 ? "Shift+Tab" : "Tab");
    if (!(await p.evaluate(() => !!document.activeElement?.closest("section.news")))) fail("Tab left the What's new dialog");
    await shot(p, "32-whats-new", "What's new after an update: Got it focused; Tab stays inside, Escape or the backdrop dismisses");
    await p.keyboard.press("Escape");
    await p.waitForTimeout(300);
    if (await p.locator("section.news").count()) fail("Escape did not dismiss What's new");
    if (!(await p.evaluate(() => window.__calls.some(([c]) => c === "mark_whats_new_seen")))) fail("Escape should mark the notes seen");
    await ctx.close();
  }
  // 11. the practice pane's toolbar: always in the app's own UI (Back, Forward, Reload, Home, Open in browser,
  // Hide, Close), a notice on a sign-in page, a tab strip once the app opens a second tab, and Close (button or
  // Shift-Cmd-W) that really gets the pane out of the way
  for (const theme of ["light", "dark"]) {
    if (!want(`33-${theme}`) && !want("33")) continue;
    const { ctx, p } = await open(browser, { theme, storage: { sidekick_today: today() }, viewport: { width: 1280, height: 800 } });
    await toLesson(p, 0, 1);
    await p.locator(".practice .btn.primary").click();
    const bar = p.locator(".col-practice .pane-bar");
    await bar.waitFor();
    const labels = await bar.locator("button").evaluateAll((bs) => bs.map((b) => b.getAttribute("aria-label")));
    for (const l of ["Back", "Forward", "Reload", "Home", "Open in browser", "Close the practice app"]) if (!labels.includes(l)) fail(`pane toolbar is missing ${l}:`, labels);
    const placed = () => p.evaluate(() => window.__calls.filter(([c]) => c === "place_practice").at(-1)?.[1]?.rect);
    await p.locator(".pane-bar button[aria-label=Back]").click();
    if (!(await p.evaluate(() => window.__calls.some(([c, a]) => c === "practice_nav" && a.action === "back")))) fail("Back should call practice_nav back");
    // on a company sign-in page: the notice, and the pane moves down below it (never over the toolbar)
    await p.evaluate(() => window.__emit("practice-nav", { host: "login.example.com", practice: false, refused: false, tabs: [{ id: 0, title: "Sign in", host: "login.example.com" }], active: 0 }));
    await p.locator(".pane-notice").waitFor();
    await p.waitForTimeout(300);
    const notice = await p.locator(".pane-notice").textContent();
    if (!/Signing in with your company's sign-in page/.test(notice)) fail("sign-in notice:", notice);
    const nb = await p.locator(".pane-notice").boundingBox();
    const r1 = await placed();
    if (!r1 || r1.y < nb.y + nb.height - 1) fail("the pane must sit below the notice:", JSON.stringify(r1), JSON.stringify(nb));
    await shot(p, `33-pane-signin-${theme}`, `Practice pane toolbar (${theme}) on a company sign-in page: Back, More, Hide, Close, and the notice`);
    await p.evaluate(() => window.__emit("practice-nav", { host: "login.example.com", practice: false, refused: true, tabs: [{ id: 0, title: "Sign in", host: "login.example.com" }], active: 0 }));
    await p.waitForTimeout(200);
    if (!/doesn't allow embedded browsers/.test(await p.locator(".pane-notice").textContent())) fail("refused notice missing");
    await shot(p, `33-pane-refused-${theme}`, `Practice pane (${theme}): the sign-in page refused the embedded browser, with what to do`);
    // the app opened a second tab, on its Development address: the strip, the new tab in front with its Dev pill
    await p.evaluate(() => window.__emit("practice-nav", { host: "staging.notes.brightline.example", practice: true, refused: false, active: 1, dev: true, tabs: [
      { id: 0, title: "Brightline Notes · Garden day", host: "notes.brightline.example", env: "prod" },
      { id: 1, title: "Brightline Notes · Settings", host: "staging.notes.brightline.example", env: "dev" }] }));
    await p.locator(".pane-tabs").waitFor();
    await p.waitForTimeout(300);
    if (await p.locator(".pane-notice").count()) fail("no notice on a practice app page");
    const on = await p.locator(".pane-tab.on").textContent();
    if (!/Settings/.test(on)) fail("the new tab should be the active one:", on);
    const tb = await p.locator(".pane-tabs").boundingBox();
    const r2 = await placed();
    if (!r2 || r2.y < tb.y + tb.height - 1) fail("the pane must sit below the tab strip:", JSON.stringify(r2), JSON.stringify(tb));
    const pills = await p.locator(".pane-tab .env-pill").allTextContents();
    if (pills.join() !== "Prod,Dev") fail("each tab should say Prod or Dev:", pills);
    if (!(await p.locator(".pane-tab.dev.on").count())) fail("the Development tab should be marked dev");
    await shot(p, `33-pane-tabs-${theme}`, `Practice pane (${theme}): a second tab, on the Development address (Dev pill); click a tab to switch, x to close it`);
    // + offers Production and Development, inline in the strip (a drop-down would sit under the native pane)
    await p.locator(".pane-tab-plus").click();
    await p.locator(".pane-tab-menu").waitFor();
    const mb = await p.locator(".pane-tab-menu").boundingBox();
    if (!mb || mb.y + mb.height > tb.y + tb.height + 1) fail("the + choices must stay inside the strip:", JSON.stringify(mb));
    await shot(p, `33-pane-new-tab-${theme}`, `Practice pane (${theme}): + opens a Production or a Development tab`);
    await p.locator(".pane-tab-menu button", { hasText: "Development" }).click();
    if (!(await p.evaluate(() => window.__calls.some(([c, a]) => c === "practice_tab" && a.action === "dev")))) fail("+ Development should open a Development tab");
    await p.locator(".pane-tab", { hasText: "Garden day" }).locator("button[role=tab]").click();
    if (!(await p.evaluate(() => window.__calls.some(([c, a]) => c === "practice_tab" && a.action === "switch" && a.id === 0)))) fail("clicking a tab should switch to it");
    await p.locator(".pane-tab", { hasText: "Settings" }).locator(".pane-tab-x").click();
    if (!(await p.evaluate(() => window.__calls.some(([c, a]) => c === "practice_tab" && a.action === "close" && a.id === 1)))) fail("x should close that tab");
    // Close: the practice column goes, and the pane with it (a zero rect parks it)
    await p.locator(".pane-bar .pane-close").click();
    await p.waitForTimeout(300);
    if (!(await p.locator(".col-practice.closed").count())) fail("Close should close the practice column");
    const r3 = await placed();
    if (!r3 || r3.w !== 0) fail("after Close the pane should be parked:", JSON.stringify(r3));
    // the shortcut, and the macOS menu item's event, do the same from anywhere
    await p.locator(".srail-btn[aria-label=Practice]").click();
    await p.waitForTimeout(200);
    await p.keyboard.press(process.platform === "darwin" ? "Meta+Shift+W" : "Control+Shift+W");
    await p.waitForTimeout(200);
    if (!(await p.locator(".col-practice.closed").count())) fail("Shift-Cmd-W should close the practice column");
    await p.locator(".srail-btn[aria-label=Practice]").click();
    await p.evaluate(() => window.__emit("practice-close", null));
    await p.waitForTimeout(200);
    if (!(await p.locator(".col-practice.closed").count())) fail("the menu's practice-close should close the practice column");
    // End: the session goes and the backend is told to close the pane
    await p.evaluate(() => { window.__calls.length = 0; });
    await p.locator(".srail-btn.end").click();
    await p.waitForTimeout(300);
    if (!(await p.evaluate(() => window.__calls.some(([c]) => c === "close_practice")))) fail("End should close the pane");
    await ctx.close();
  }
  // 12. a role change in Settings saves the profile and opens or locks nothing by itself
  if (want("34")) {
    const { ctx, p } = await open(browser, { role: "learner", storage: { sidekick_today: today() } });
    const errs = [];
    p.on("pageerror", (e) => errs.push(e.message));
    await toSettings(p);
    await p.locator("form.profile-form select").selectOption("partner");
    await p.locator("form.profile-form button[type=submit]").click();
    await p.waitForTimeout(800);
    const saved = await p.evaluate(() => window.__calls.filter(([c]) => c === "save_profile").map(([, a]) => a.profile.role));
    if (saved.join() !== "partner") fail("save_profile calls:", saved);
    if (await p.locator("nav.tabs .tab-lock").count()) fail("a role change should not lock an open course");
    if (errs.length) fail("page errors on a role change:", errs);
    await shot(p, "34-role-change", "Settings, Learner to Partner: the profile is saved, no course opens or locks by role alone");
    await ctx.close();
  }
  // 13. onboarding's optional practice app address: empty is fine, a wrong address says why and blocks Start,
  // a right one is saved to Settings (save_settings: the backend adds https:// and checks it again)
  for (const theme of ["light", "dark"]) {
    if (!want("35")) continue;
    const { ctx, p } = await open(browser, { profile: false, theme });
    await p.locator(".onboard-form").waitFor();
    await p.locator(".onboard-form input").first().fill("Jordan Hale");
    await p.locator(".onboard-form input[type=email]").fill("jordan.hale@brightline.example");
    await p.locator(".onboard-form select").selectOption("learner");
    const acct = p.locator("#onboard-account");
    if ((await acct.getAttribute("placeholder")) !== "e.g. app.example.com") fail("account placeholder:", await acct.getAttribute("placeholder"));
    await shot(p, `35-onboarding-account-${theme}`, `Onboarding (${theme}): the optional Your practice app field under the role, with its hint`);
    await acct.fill("http://notes.brightline.example");
    await p.waitForTimeout(150);
    if (!/https:\/\//.test(await p.locator(".onboard-form .field-error").textContent().catch(() => ""))) fail("a plain http address should ask for https://");
    await shot(p, `35b-onboarding-account-error-${theme}`, `Onboarding (${theme}): a plain http:// address says why, inline`);
    await p.locator(".onboard-form button[type=submit]").click();
    await p.waitForTimeout(300);
    const calls = await p.evaluate(() => window.__calls.map(([c]) => c));
    if (calls.includes("save_profile")) fail("Start went ahead with an invalid practice app address");
    await acct.fill("Notes.Brightline.example");
    await p.evaluate(() => { const inv = window.__TAURI__.core.invoke; window.__TAURI__.core.invoke = async (c, a) => {
      sessionStorage.setItem("__log", JSON.stringify([...JSON.parse(sessionStorage.getItem("__log") || "[]"), [c, a]])); return inv(c, a); }; });
    await p.locator(".onboard-form button[type=submit]").click();
    await p.waitForTimeout(800);
    const saved = await p.evaluate(() => JSON.parse(sessionStorage.getItem("__log") || "[]").filter(([c]) => c === "save_settings").map(([, a]) => a.practice_url));
    if (saved.join() !== "Notes.Brightline.example") fail("save_settings practice_url calls:", JSON.stringify(saved));
    await ctx.close();
  }
  if (want("35c")) {
    // empty: no settings write at all
    const { ctx, p } = await open(browser, { profile: false });
    await p.locator(".onboard-form").waitFor();
    await p.locator(".onboard-form input").first().fill("Jordan Hale");
    await p.locator(".onboard-form input[type=email]").fill("jordan.hale@brightline.example");
    await p.locator(".onboard-form select").selectOption("learner");
    await p.evaluate(() => { const inv = window.__TAURI__.core.invoke; window.__TAURI__.core.invoke = async (c, a) => {
      sessionStorage.setItem("__log", JSON.stringify([...JSON.parse(sessionStorage.getItem("__log") || "[]"), [c, a]])); return inv(c, a); }; });
    await p.locator(".onboard-form button[type=submit]").click();
    await p.waitForTimeout(800);
    const calls = await p.evaluate(() => JSON.parse(sessionStorage.getItem("__log") || "[]").map(([c]) => c));
    if (!calls.includes("save_profile")) fail("onboarding with an empty practice app field didn't save the profile:", calls);
    if (calls.includes("save_settings")) fail("an empty practice app field should not touch Settings");
    await ctx.close();
  }
} finally {
  await browser.close();
  server.close();
}
// the README set goes straight into docs/screenshots: no captions file there
if (ONLY !== "readme") fs.writeFileSync(`${OUT}/captions.txt`, shots.join("\n") + "\n");
console.log(`${shots.length} shots, ${problems ? problems + " problem(s)" : "no problems"}`);
process.exit(problems ? 1 : 0);
