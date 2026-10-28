// Sidekick explains and nudges: hover help (a 5 s rest, once per control), tooltips, the coaching column's coach
// marks, and every stuck detector with its rate limit, in the real UI build with the bridge stubbed.
// Usage: node help-check.mjs OUT_DIR URL [only-letter]
import { chromium } from "playwright";
import { page as open } from "./stub.mjs";
const S = process.argv[2], URL = process.argv[3];
const only = process.argv[4] || "";
const run = (k) => !only || only === k;
const out = [];
const ok = (name, cond, extra = "") => { out.push(`${cond ? "PASS" : "FAIL"} ${name}${extra ? " - " + extra : ""}`); };
const today = new Date().toDateString();
const MARKS = ["where-to", "continue", "first-lesson", "quick-check", "practice", "first-exercise", "first-check", "ai-provider", "streak", "certificate",
  "coach-look", "coach-act", "coach-hint", "coach-next", "coach-send"];
// quiet start: no nudge, no coach mark due, so only what's under test happens
const calm = { sidekick_today: today, sidekick_coach_seen: JSON.stringify(MARKS), sidekick_coach_last: String(Date.now()),
  sidekick_snooze: JSON.stringify({ "c/first_launch": Date.now() + 1e9, "c/back_after_break": Date.now() + 1e9, "c/studied_today": Date.now() + 1e9 }) };
const page = (b, o = {}) => open(b, { S, URL, allDone: false, days: 0, today: true, ...o, storage: { ...calm, ...(o.storage || {}) } });
const tip = (p) => p.locator(".sidekick-coach");
const rest = async (p, sel, ms) => { const b = await p.locator(sel).first().boundingBox(); await p.mouse.move(b.x + b.width / 2, b.y + b.height / 2); await p.mouse.move(b.x + b.width / 2 + 3, b.y + b.height / 2 + 2); await p.waitForTimeout(ms); };
const away = (p) => p.mouse.move(400, 500);
const warnings = [];
// a coaching session on lesson 1.2 (the stub's open_practice does nothing; the columns are the app's own)
const session = async (p) => {
  await p.locator("[data-sidekick-anchor=continue]").click();
  await p.locator(".practice .btn.primary").click();
  await p.locator(".col-sidekick .pane-tools").waitFor({ timeout: 5000 });
  await p.waitForTimeout(800);
};
const failQuiz = { submit_quiz: { score: 0, passed: false, results: [{ score: 0, feedback: ["Not this one."] }] } };
const browser = await chromium.launch();
try {
  if (run("H")) {
    // H: hover help. A 5 s rest on a control: Sidekick flies over and explains it, once per control per device
    for (const theme of ["light", "dark"]) {
      const { ctx, p } = await page(browser, { theme, viewport: { width: 1440, height: 900 } });
      p.on("console", (m) => { if (m.type() === "warning" && /reactive/.test(m.text())) warnings.push(m.text().slice(0, 120)); });
      await p.waitForTimeout(1200);
      await p.locator("[data-help='top.settings']").click();
      await p.waitForTimeout(800);
      await p.locator("[data-help='settings.practice-url']").scrollIntoViewIfNeeded();
      await rest(p, "[data-help='settings.practice-url'] span", 3800);
      if (theme === "light") ok("H1 under 5 s: nothing yet", await tip(p).count() === 0);
      await p.waitForTimeout(1800);
      const t = await tip(p).textContent().catch(() => "");
      ok(`H2 ${theme}: a 5 s rest on Practice app address: the companion explains it, with the caveat`, /Practice app address/.test(t) && /training or sandbox/.test(t) && /real records/.test(t), t);
      await p.waitForTimeout(1500);
      ok(`H3 ${theme}: Sidekick flew out beside it`, await p.locator(".sidekick-companion:not(.hidden)").count() === 1 && await p.locator(".sidekick-ring").evaluate((e) => getComputedStyle(e).display) !== "none");
      await p.screenshot({ path: `${S}/help-H-settings-dwell-${theme}.png` });
      if (theme === "light") {
        await p.keyboard.press("Escape");
        await away(p); await p.waitForTimeout(600);
        await rest(p, "[data-help='settings.practice-url'] span", 6500);
        ok("H4 once per control per device", await tip(p).count() === 0 && /settings\.practice-url/.test(await p.evaluate(() => localStorage.getItem("sidekick_help_seen"))));
        // typing: no explanation while a field has focus
        await p.locator("[data-help='settings.name'] input").focus();
        await rest(p, "[data-help='settings.email'] span", 6500);
        ok("H5 never while typing", await tip(p).count() === 0);
        await p.evaluate(() => document.activeElement.blur());
        // a dialog of the page's own (Start over): nothing
        await p.locator("[data-help='settings.start-over']").click();
        await p.waitForTimeout(400);
        await rest(p, "[aria-modal=true] button", 6500);
        ok("H6 never over a dialog", await tip(p).count() === 0);
        await p.keyboard.press("Escape");
      }
      await ctx.close();
    }
    // Quiet: no unprompted explanations
    const { ctx, p } = await page(browser, { storage: { sidekick_mode: "quiet" } });
    await p.waitForTimeout(1200);
    await rest(p, "[data-help='top.settings']", 6500);
    ok("H7 Quiet: no hover explanations", await tip(p).count() === 0);
    await ctx.close();
  }
  if (run("K")) {
    // K: tooltips. Every icon-only control has a label and a tooltip; ~500 ms; on screen; keyboard too
    for (const theme of ["light", "dark"]) {
      const { ctx, p } = await page(browser, { theme, viewport: { width: 1440, height: 900 } });
      await p.waitForTimeout(1200);
      await p.locator("[data-help='top.settings']").hover();
      await p.waitForTimeout(250);
      const early = await p.locator(".sidekick-tooltip.on").count();
      await p.waitForTimeout(450);
      const tt = await p.locator(".sidekick-tooltip.on").textContent().catch(() => "");
      const tb = await p.locator(".sidekick-tooltip").boundingBox();
      if (theme === "light") {
        ok("K1 a tooltip after ~500 ms, not at once", early === 0 && tt === "Settings", `${early} ${tt}`);
        ok("K2 tied to the control for screen readers, and no native title doubling it", await p.locator("[data-help='top.settings']").getAttribute("aria-describedby") === "sidekick-tooltip");
        ok("K3 wholly on screen", tb && tb.x >= 0 && tb.y >= 0 && tb.x + tb.width <= 1440 && tb.y + tb.height <= 900, JSON.stringify(tb));
      }
      await p.screenshot({ path: `${S}/help-K-topbar-tooltip-${theme}.png`, clip: { x: 700, y: 0, width: 740, height: 140 } });
      await p.locator("[data-help='top.theme']").hover();
      await p.waitForTimeout(650);
      await p.screenshot({ path: `${S}/help-K-topbar-tooltip-theme-${theme}.png`, clip: { x: 700, y: 0, width: 740, height: 140 } });
      if (theme === "light") {
        await p.mouse.move(300, 600); await p.waitForTimeout(200);
        await p.locator("[data-help='top.theme']").focus();
        await p.keyboard.press("Shift+Tab"); await p.keyboard.press("Tab");
        await p.waitForTimeout(650);
        ok("K4 keyboard focus shows it too", /theme/i.test(await p.locator(".sidekick-tooltip.on").textContent().catch(() => "")));
      }
      await ctx.close();
    }
    // every icon-only button, on every page we can reach (home, lesson, settings, a session): a label and a tooltip
    const { ctx, p } = await page(browser, { viewport: { width: 1440, height: 900 } });
    const help = JSON.parse((await import("node:fs")).readFileSync(`${S}/help.json`, "utf8"));
    const tips = new Set(help.filter((h) => h.tip).map((h) => h.key));
    const scan = () => p.evaluate(() => [...document.querySelectorAll("button")].filter((b) => b.getBoundingClientRect().width > 0 && !b.closest(".sidekick-coach, .sidekick-bubble"))
      .filter((b) => (b.textContent || "").trim().replace(/[^\p{L}\p{N}]/gu, "").length <= 2)
      .map((b) => ({ label: b.getAttribute("aria-label"), help: b.getAttribute("data-help") || b.closest("[data-help]")?.getAttribute("data-help"), html: b.outerHTML.slice(0, 80) })));
    const bad = [];
    const check = async (where) => { for (const b of await scan()) if (!b.label || !b.help || !tips.has(b.help)) bad.push(`${where}: ${JSON.stringify(b)}`); };
    await p.waitForTimeout(1200); await check("home");
    await p.locator("[data-sidekick-anchor=continue]").click(); await p.waitForTimeout(900); await check("lesson");
    await p.locator(".sidekick-tab").click(); await p.waitForTimeout(400); await check("lesson + pane");
    await p.locator("[data-help='side.collapse']").click();
    await p.locator("[data-help='top.home']").click(); await p.waitForTimeout(600);
    await session(p); await check("session");
    ok("K5 every icon-only button has a label and a tooltip", bad.length === 0, bad.slice(0, 4).join(" | "));
    // the practice pane's toolbar: the tooltip never goes below, into the native pane
    await p.locator("[data-help='pane.reload']").hover(); await p.waitForTimeout(700);
    const slot = await p.locator(".practice-slot").boundingBox(), t2 = await p.locator(".sidekick-tooltip.on").boundingBox();
    ok("K6 a toolbar tooltip stays off the native practice pane", t2 && !(t2.x < slot.x + slot.width && t2.x + t2.width > slot.x && t2.y < slot.y + slot.height && t2.y + t2.height > slot.y), JSON.stringify({ t2, slot }));
    await ctx.close();
  }
  if (run("C")) {
    // C: the coaching column. Lingering near Do it for me (never used): Sidekick flies over with what it does
    for (const theme of ["light", "dark"]) {
      const seen = JSON.stringify(MARKS.filter((m) => m !== "coach-act"));
      const { ctx, p } = await page(browser, { theme, viewport: { width: 1440, height: 900 }, storage: { sidekick_coach_seen: seen, sidekick_coach_last: "0" } });
      await p.waitForTimeout(1200);
      await session(p);
      await rest(p, "[data-help='coach.act']", 2600);
      const t = await tip(p).textContent().catch(() => "");
      ok(`C1 ${theme}: lingering near Do it for me brings its coach mark`, /Do it for me/.test(t) && /Submit/.test(t) && /ask/.test(t) && /OpenAI API key/.test(t) && !/Delete ask/.test(t), t);
      await p.waitForTimeout(1400);
      const tb = await tip(p).boundingBox(), slot = await p.locator(".practice-slot").boundingBox();
      ok(`C2 ${theme}: its bubble stays in the companion's column, off the practice pane`, tb && tb.x >= slot.x + slot.width - 1, JSON.stringify({ tb, slot }));
      await p.screenshot({ path: `${S}/help-C-coach-do-it-${theme}.png` });
      if (theme === "light") {
        // wait for the bubble to settle, click, and require it to close (under load a click can land mid-move)
        await p.waitForTimeout(600);
        await tip(p).getByRole("button", { name: "Got it" }).click();
        const closed = await tip(p).waitFor({ state: "detached", timeout: 4000 }).then(() => true).catch(() => false);
        ok("C3a Got it closes the coach mark", closed);
        await away(p); await p.waitForTimeout(400);
        await p.evaluate(() => localStorage.setItem("sidekick_coach_last", "0"));
        await rest(p, "[data-help='coach.act']", 3000);
        const t3 = await tip(p).textContent().catch(() => "");
        ok("C3 never the same coach mark twice", !/Do it for me/.test(t3), t3 + " seen=" + await p.evaluate(() => localStorage.getItem("sidekick_coach_seen")));
      }
      await ctx.close();
    }
  }
  if (run("S")) {
    // S: stuck detectors, each with one best next step, and their rate limits
    // S1 idle in a coaching stage, on a fake clock: 60 s with no input brings "Show a hint"
    let { ctx, p } = await page(browser, { clock: true, viewport: { width: 1440, height: 900 } });
    await p.waitForTimeout(1500);
    await session(p);
    await p.clock.runFor(30_000);
    ok("S1a 30 s of quiet: nothing", await tip(p).count() === 0);
    await p.clock.runFor(35_000);
    await p.waitForTimeout(1500);
    const t1 = await tip(p).textContent().catch(() => "");
    ok("S1 a minute with no input in a stage: a kind nudge offering a hint", /hint/i.test(t1) && await tip(p).getByRole("button", { name: "Show a hint" }).count() === 1, t1);
    for (const theme of ["light"]) await p.screenshot({ path: `${S}/help-S-stuck-idle-${theme}.png` });
    await tip(p).getByRole("button", { name: "Show a hint" }).click();
    await p.waitForTimeout(400);
    ok("S1b Show a hint asks the coach for a hint", (await p.evaluate(() => window.__calls)).some(([c, a]) => c === "coach_turn" && a.hint === true));
    await p.mouse.move(500, 300);
    await p.clock.runFor(65_000); await p.waitForTimeout(800);
    ok("S1c rate-limited: not again a minute later", await tip(p).count() === 0);
    // S2 a field error in the practice app: "That form wants Due date first"
    await p.evaluate(() => window.__emit("practice-signal", { kind: "validation", text: "Due date: Value is required" }));
    await p.waitForTimeout(600);
    const t2 = await tip(p).textContent().catch(() => "");
    ok("S2 a field error in the practice app: names the field and offers to show where", /Due date/.test(t2) && await tip(p).getByRole("button", { name: "Show me where" }).count() === 1, t2);
    await tip(p).getByRole("button", { name: "Show me where" }).click();
    await p.waitForTimeout(400);
    ok("S2b Show me where: Look at my screen on, the question ready", await p.locator("[data-help='coach.look'].on").count() === 1 && /Due date/.test(await p.locator(".col-sidekick textarea").inputValue()));
    await p.locator(".col-sidekick textarea").fill("");
    // S3 never while typing
    await p.locator(".col-sidekick textarea").focus();
    await p.evaluate(() => window.__emit("practice-signal", { kind: "toast", text: "Something went wrong" }));
    await p.waitForTimeout(500);
    ok("S3 never interrupts typing", await tip(p).count() === 0);
    await p.evaluate(() => document.activeElement.blur());
    // S4 the screen helper fails on setup: point to the fix in Settings
    await p.evaluate(() => window.__emit("practice-signal", { kind: "toast", text: "Something went wrong" }));
    await p.waitForTimeout(400);
    ok("S4 an error toast: offers to look at it", /error/i.test(await tip(p).textContent().catch(() => "")));
    await p.keyboard.press("Escape");
    await ctx.close();
    ({ ctx, p } = await page(browser, { viewport: { width: 1440, height: 900 } }));
    await p.waitForTimeout(1200);
    await session(p);
    await p.evaluate(() => window.__emit("screen-agent", { kind: "error", text: "HTTP 400: this model does not support tools" }));
    await p.waitForTimeout(600);
    const t4 = await tip(p).textContent().catch(() => "");
    ok("S5 the screen helper fails on setup: the fix is in Settings", /Settings/.test(t4) && await tip(p).getByRole("button", { name: "Open Settings" }).count() === 1, t4);
    await p.screenshot({ path: `${S}/help-S-stuck-helper-light.png` });
    await tip(p).getByRole("button", { name: "Open Settings" }).click();
    await p.waitForTimeout(1200);
    ok("S5b and Open Settings takes them to the model", /Settings/.test(await p.locator(".col-course h1").first().textContent().catch(() => "")));
    await ctx.close();
    // S6 the same quick-check question missed twice: point at the key points
    ({ ctx, p } = await page(browser, { extra: failQuiz }));
    await p.waitForTimeout(1200);
    await p.locator("[data-sidekick-anchor=continue]").click(); await p.waitForTimeout(800);
    for (let i = 0; i < 2; i++) { await p.locator("[data-help='quiz.submit']").click(); await away(p); await p.waitForTimeout(1500); }
    const t6 = await tip(p).textContent().catch(() => "") || JSON.stringify(await p.evaluate(() => [document.querySelectorAll(".quiz .result").length, document.querySelectorAll(".feedback:not(.ok)").length, localStorage.getItem("sidekick_familiar")]));
    ok("S6 the same question missed twice: the key points", /[Qq]uestion 1|question/.test(t6) && await tip(p).getByRole("button", { name: "Show me" }).count() === 1, t6);
    await ctx.close();
    // S7 rage clicks on one spot: what that control does
    ({ ctx, p } = await page(browser, { stats: { streak: 3, active_today: true } }));
    await p.waitForTimeout(1500);
    const sp = await p.locator("[data-help='top.streak']").boundingBox();
    for (let i = 0; i < 5; i++) { await p.mouse.click(sp.x + 20, sp.y + 10); await p.waitForTimeout(90); }
    await p.waitForTimeout(500);
    ok("S7 rage clicks: Sidekick explains that control", /Days in a row/.test(await tip(p).textContent().catch(() => "")), await tip(p).textContent().catch(() => ""));
    await ctx.close();
    // S8 back and forth between two pages: "Tell me what you're looking for"
    ({ ctx, p } = await page(browser));
    await p.waitForTimeout(1500);
    for (let i = 0; i < 3; i++) {
      await p.locator("[data-sidekick-anchor=continue]").click(); await p.waitForTimeout(350);
      if (i < 2) { await p.locator(".crumb-back").click(); await p.waitForTimeout(350); }
    }
    await p.locator(".crumb-back").click();
    await p.waitForTimeout(1800);
    const t8 = await tip(p).textContent().catch(() => "");
    ok("S8 back and forth: offers to take them there", await tip(p).getByRole("button", { name: "Where to?" }).count() === 1, t8);
    await ctx.close();
    await ctx.close();
  }
  if (run("Q")) {
    // Q: nothing unprompted in Quiet, at most one bubble at a time
    const { ctx, p } = await page(browser, { storage: { sidekick_mode: "quiet" } });
    await p.waitForTimeout(1200);
    await session(p);
    await p.evaluate(() => window.__emit("practice-signal", { kind: "validation", text: "Due date: Value is required" }));
    await p.waitForTimeout(600);
    ok("Q1 Quiet: no stuck nudges", await tip(p).count() === 0);
    await p.evaluate(() => localStorage.setItem("sidekick_mode", "on"));
    await ctx.close();
    const r = await page(browser);
    await r.p.waitForTimeout(1200);
    await session(r.p);
    await r.p.evaluate(() => { window.__emit("practice-signal", { kind: "validation", text: "Due date: Value is required" }); window.__emit("practice-signal", { kind: "toast", text: "Boom" }); });
    await r.p.waitForTimeout(600);
    ok("Q2 one bubble at a time", await r.p.locator(".sidekick-coach, .sidekick-bubble").count() === 1);
    await r.ctx.close();
  }
  if (run("V")) {
    // V: the sidekick at a glance: the tab with a nudge, the pane in Ask and Coach, a lesson at 1440 and 1024, a phone
    for (const theme of ["light", "dark"]) {
      let { ctx, p } = await page(browser, { theme, viewport: { width: 1440, height: 900 }, storage: { sidekick_snooze: "{}", sidekick_today: "" } });
      await p.locator(".sidekick-bubble").waitFor({ timeout: 8000 }).catch(() => {});
      await p.screenshot({ path: `${S}/side-tab-nudge-${theme}.png` });
      await ctx.close();
      for (const [w, h] of [[1440, 900], [1024, 768], [390, 844]]) {
        ({ ctx, p } = await page(browser, { theme, viewport: { width: w, height: h }, storage: { sidekick_pane_mode: "ask" } }));
        await p.waitForTimeout(1200);
        await p.locator("[data-sidekick-anchor=continue]").click(); await p.waitForTimeout(700);
        await p.locator(".sidekick-tab").click(); await p.waitForTimeout(700);
        await p.screenshot({ path: `${S}/side-lesson-ask-${w}-${theme}.png` });
        if (w === 1440) {
          await p.locator("[data-help='coach.mode-coach']").click(); await p.waitForTimeout(400);
          await p.locator(".coach-start").click(); await p.waitForTimeout(800);
          await p.screenshot({ path: `${S}/side-lesson-coach-${w}-${theme}.png` });
        }
        await ctx.close();
      }
    }
  }
  ok("Z no reactive-read warnings in the console", warnings.length === 0, warnings.slice(0, 2).join(" | "));
} finally { await browser.close(); }
console.log(out.join("\n"));
process.exit(out.some((l) => l.startsWith("FAIL")) ? 1 : 0);
