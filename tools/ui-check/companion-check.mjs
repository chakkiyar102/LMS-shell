// The companion (display name Guide; .sidekick-* in the code) with the Tauri bridge stubbed (no backend, no
// practice account): the tab on the right edge, the pane it opens, the nudges said at the tab. Usage: node companion-check.mjs OUT_DIR URL [only-letter]
import { chromium } from "playwright";
import { page as open } from "./stub.mjs";
const S = process.argv[2], URL = process.argv[3];
const only = process.argv[4] || "";
const page = (browser, o) => open(browser, { S, URL, ...o });
const out = [];
const ok = (name, cond, extra = "") => { out.push(`${cond ? "PASS" : "FAIL"} ${name}${extra ? " - " + extra : ""}`); };
const browser = await chromium.launch();
try {
  if (!only || only === "A") {
  // A: all lessons done, checks missing: the certificate to-do nudge, said at Sidekick's tab
  let { ctx, p } = await page(browser, { allDone: true, days: 0, today: false });
  const bubble = p.locator(".sidekick-bubble");
  await bubble.waitFor({ timeout: 8000 }).catch(() => {});
  const text = await bubble.textContent().catch(() => "");
  ok("A1 to-do nudge shows once all lessons are done", /Every lesson is done/.test(text || ""), JSON.stringify(text));
  ok("A2 nudge names the first missing check", /session 1 check/.test(text || ""));
  const tab = await p.locator(".sidekick-tab").boundingBox();
  const bb = await bubble.boundingBox();
  ok("A3 the nudge opens beside the tab, wholly in the window", tab && bb && bb.x + bb.width <= tab.x + 1 && bb.x >= 0 && bb.y >= 0, JSON.stringify({ tab, bb }));
  ok("A3b no bottom-right launcher, and no second Sidekick at rest", await p.locator(".launcher").count() === 0 && await p.locator(".sidekick-companion.hidden").count() === 1);
  ok("A3c the tab reserves its 40 px", await p.evaluate(() => getComputedStyle(document.body).paddingRight) === "40px");
  await p.screenshot({ path: `${S}/sidekick-A-home.png` });
  await p.keyboard.press("Escape");
  ok("A4 Esc closes the bubble", await bubble.count() === 0);
  await p.locator(".sidekick-tab").click();
  await p.locator(".sidekick-side-pane").waitFor({ timeout: 2000 }).catch(() => {});
  ok("A5 the tab opens the pane over the page", await p.locator(".sidekick-side-pane .sidekick-pane").count() === 1 && await p.locator(".sidekick-tab").count() === 0);
  await p.locator(".sidekick-where-toggle").click();
  ok("A6 Where to? in the pane: four chips", await p.locator(".sidekick-where .sidekick-chips .chip").count() === 4);
  ok("A6b Where to? shows no raw error", !/invalid type|Error:/.test(await p.locator(".sidekick-where").textContent() || ""));
  await p.screenshot({ path: `${S}/sidekick-A-where.png` });
  await p.locator(".sidekick-where .chip", { hasText: "Next check" }).click();
  await p.waitForTimeout(600);
  ok("A7 Next check chip opens the session 1 check", /Session 1 check/.test(await p.locator("h1").first().textContent() || ""));
  await p.reload(); await p.waitForTimeout(1500);
  ok("A8 the pane remembers it was open", await p.locator(".sidekick-side-pane").count() === 1);
  await p.locator("[data-help='side.collapse']").click();
  await p.reload(); await p.waitForTimeout(1500);
  ok("A9 and that it was tucked away", await p.locator(".sidekick-side-pane").count() === 0 && await p.locator(".sidekick-tab").count() === 1);
  await ctx.close();
  }
  if (!only || only === "B") {
  // B: first launch -> hello nudge; Not now snoozes it for 7 days
  let { ctx, p } = await page(browser, { allDone: false, days: null, today: false });
  await p.locator(".sidekick-bubble").waitFor({ timeout: 8000 }).catch(() => {});
  ok("B1 first launch says hello", /I'm Guide/.test(await p.locator(".sidekick-bubble").textContent().catch(() => "") || ""));
  await p.locator(".sidekick-bubble button", { hasText: "Not now" }).click();
  const snooze = await p.evaluate(() => localStorage.getItem("sidekick_snooze"));
  ok("B2 Not now stores a 7-day snooze for course/trigger", /"c\/first_launch":\d+/.test(snooze || ""), snooze);
  const shown = await p.evaluate(() => localStorage.getItem("sidekick_shown"));
  ok("B3 the shown nudge is counted", /^\[\d+(\.\d+)?\]$/.test(shown || ""), shown);
  await p.reload();
  await p.waitForTimeout(2500);
  ok("B4 a snoozed nudge stays quiet after a restart", await p.locator(".sidekick-bubble").count() === 0);
  await p.evaluate(() => localStorage.setItem("sidekick_mode", "off"));
  await p.reload(); await p.waitForTimeout(1500);
  ok("B5 Off: no flying Sidekick, no bubble; the tab stays for asking", await p.locator(".sidekick-companion:not(.hidden)").count() === 0 && await p.locator(".sidekick-bubble").count() === 0 && await p.locator(".sidekick-tab").count() === 1);
  await ctx.close();
  }
  if (!only || only === "C") {
  // C: reduced motion: the pane appears without sliding, the halo is still, the ghost placeholder doesn't type
  let { ctx, p } = await page(browser, { allDone: false, days: 0, today: false, reduce: true });
  await p.waitForTimeout(1500);
  const halo = await p.locator(".sidekick-halo").evaluate((e) => getComputedStyle(e).animationName);
  ok("C1 reduced motion: the tab's halo is still", halo === "none", halo);
  await p.locator(".sidekick-tab").click();
  const slide = await p.locator(".sidekick-side-pane").evaluate((e) => getComputedStyle(e).animationName);
  ok("C2 reduced motion: the pane appears without sliding", slide === "none", slide);
  await p.locator("[data-help='coach.mode-ask']").click();
  const a = await p.locator(".pane-compose textarea").getAttribute("placeholder");
  await p.waitForTimeout(900);
  const b = await p.locator(".pane-compose textarea").getAttribute("placeholder");
  ok("C3 reduced motion: the composer's placeholder doesn't type", a === b && !/▍/.test(b || ""), `${a} | ${b}`);
  await ctx.close();
  }
  if (!only || only === "D") {
  // D: the pane, in each mode, on a lesson: Ask offers chips for the page, the composer types examples; Coach offers to coach this lesson
  let { ctx, p } = await page(browser, { allDone: false, days: 0, today: true, storage: { sidekick_today: new Date().toDateString(), sidekick_pane_mode: "ask" } });
  await p.waitForTimeout(1200);
  await p.locator("[data-sidekick-anchor=continue]").click();
  await p.waitForTimeout(800);
  await p.locator(".sidekick-tab").click();
  await p.waitForTimeout(400);
  const chips = await p.locator(".ask-chips .chip").allTextContents();
  ok("D1 an empty Ask pane offers chips for this page", chips.includes("What's on this lesson?") && chips.includes("Quiz me") && chips.includes("Where was I?"), JSON.stringify(chips));
  const ph = [];
  for (let i = 0; i < 4; i++) { ph.push(await p.locator(".pane-compose textarea").getAttribute("placeholder")); await p.waitForTimeout(250); }
  ok("D2 the composer's placeholder types example questions", new Set(ph).size >= 3 && ph.some((x) => /^W/.test(x || "")), JSON.stringify(ph));
  await p.locator(".ask-chips .chip", { hasText: "Quiz me" }).click();
  await p.waitForTimeout(500);
  const asked = (await p.evaluate(() => window.__calls)).find(([c]) => c === "ask_tutor");
  ok("D3 a chip asks Sidekick, about this lesson", asked && /Quiz me/.test(asked[1].question) && asked[1].lesson_id === "1.2", JSON.stringify(asked));
  await p.locator("[data-help='coach.mode-coach']").click();
  await p.waitForTimeout(500);
  ok("D4 Coach offers to coach this lesson", /Coach me through 1\.2/.test(await p.locator(".coach-start").textContent().catch(() => "")));
  ok("D5 and to practise in the practice app", await p.locator(".sidekick-side-pane [data-help='lesson.practice']").count() === 1);
  await p.locator(".coach-start").click();
  await p.waitForTimeout(600);
  ok("D6 Coach me through it: stages and Sidekick's first message", await p.locator(".sidekick-side-pane .stages li").count() === 3 && /Garden day/.test(await p.locator(".sidekick-side-pane .pane-body").textContent()));
  await p.reload(); await p.waitForTimeout(1500);
  ok("D7 the mode is remembered", await p.locator("[data-help='coach.mode-coach'][aria-checked=true]").count() === 1);
  await ctx.close();
  }
  if (!only || only === "K") {
  // K: Sidekick on the progress card wears the mood for the learner's stage, says a real number, and opens Ask Sidekick
  const cases = [
    ["determined", { days: 0, today: true, stats: { lessons_done: 1 } }, "encourage", /1 of 4 lessons done/],
    ["feeling low", { days: 3, today: false, stats: { lessons_done: 1 } }, "mood-low", /3 lessons to go/],
    ["proud", { days: 0, today: true, allDone: true, stats: { checks_passed: 2, final_passed: true } }, "trophy", /Every check and the final passed/],
  ];
  for (const [mood, o, pose, line] of cases) {
    const { ctx, p } = await page(browser, { allDone: false, ...o });
    const k = p.locator(".card.overview .card-sidekick");
    await k.waitFor({ timeout: 8000 }).catch(() => {});
    const src = await k.locator("img").getAttribute("src").catch(() => "");
    const text = await k.textContent().catch(() => "");
    ok(`K1 card Guide is ${mood}: ${pose}.png and a line with real numbers`, src === `assets/mascot/${pose}.png` && text.includes(`Guide is ${mood}`) && line.test(text), JSON.stringify({ src, text }));
    if (mood === "determined") {
      ok("K2 the banner keeps a hello, not a second mood Sidekick", await p.locator(".welcome img.sidekick").getAttribute("src") === "assets/mascot/wave.png" && await p.locator("img.sidekick[src$='encourage.png']").count() === 1);
      await p.keyboard.press("Escape");
      await k.click();
      await p.locator(".sidekick-side-pane").waitFor({ timeout: 2000 }).catch(() => {});
      ok("K3 tapping card Sidekick opens Ask Sidekick", await p.locator(".sidekick-side-pane .sidekick-pane").count() === 1);
    }
    await ctx.close();
  }
  // Off keeps him (it's progress, not a nudge) but still; reduced motion stills him too
  for (const [name, o] of [["Off", { storage: { sidekick_mode: "off" } }], ["reduced motion", { reduce: true }]]) {
    const { ctx, p } = await page(browser, { allDone: false, days: 0, today: true, ...o });
    const k = p.locator(".card.overview .card-sidekick");
    await k.waitFor({ timeout: 8000 }).catch(() => {});
    const anim = await k.evaluate((e) => [getComputedStyle(e.querySelector(".card-sidekick-bob")).animationName, getComputedStyle(e.querySelector("img")).animationName]).catch(() => null);
    ok(`K4 ${name}: card Sidekick shows, without motion`, anim && anim.every((a) => a === "none"), JSON.stringify(anim));
    await ctx.close();
  }
  }
} finally { await browser.close(); }
console.log(out.join("\n"));
process.exit(out.some((l) => l.startsWith("FAIL")) ? 1 : 0);
