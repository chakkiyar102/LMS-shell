// Lively Sidekick: flight, coach marks, taps, drag, modes, reduced motion and dialogs, in the real UI build
// with the Tauri bridge stubbed (stub.mjs). Usage: node lively-check.mjs OUT_DIR URL [only-letter]
import { chromium } from "playwright";
import { page as open } from "./stub.mjs";
const S = process.argv[2], URL = process.argv[3];
const only = process.argv[4] || "";
const run = (k) => !only || only === k;
const out = [];
const ok = (name, cond, extra = "") => { out.push(`${cond ? "PASS" : "FAIL"} ${name}${extra ? " - " + extra : ""}`); };
const today = new Date().toDateString();
// quiet start: no nudge (studied_today already shown, first launch snoozed), so only what's under test happens
const calm = { sidekick_today: today, sidekick_snooze: JSON.stringify({ "c/first_launch": Date.now() + 1e9, "c/back_after_break": Date.now() + 1e9, "c/studied_today": Date.now() + 1e9 }) };
const page = (b, o = {}) => open(b, { S, URL, allDone: false, days: 0, today: true, ...o, storage: { ...calm, ...(o.storage || {}) } });
// the continue card 80 px from the top, the home hero just scrolled away (layout-independent)
const scrollContinue = (p) => p.evaluate(() => {
  const m = document.querySelector("#course-pane"), c = document.querySelector('[data-sidekick-anchor="continue"]');
  if (!c) return;
  const by = c.getBoundingClientRect().top - 80;
  if (m && m.scrollHeight > m.clientHeight) m.scrollTop += by; else window.scrollBy(0, by);
});
const scrollTo = (p, y) => p.evaluate((y) => { const m = document.querySelector("#course-pane"); if (m) m.scrollTop = y; window.scrollTo(0, y); }, y);
// the sprite's box, and whether it covers any text or control (the same selectors sidekick.rs avoids)
const covers = (p) => p.evaluate(() => {
  const k = document.querySelector(".sidekick-companion").getBoundingClientRect();
  const hit = [...document.querySelectorAll("p, h1, h2, h3, h4, li, button, a, input, textarea, select, label, img, video, .quiz, .prose")]
    .filter((e) => !e.closest(".sidekick-companion-root")).map((e) => [e, e.getBoundingClientRect()])
    .filter(([, r]) => r.width > 0 && k.left + 8 < r.right && k.right - 8 > r.left && k.top + 8 < r.bottom && k.bottom - 8 > r.top);
  return hit.map(([e]) => e.tagName + "." + e.className).slice(0, 3);
});
const anims = (p) => p.locator(".sidekick-companion").evaluate((e) => e.getAnimations().map((a) => [a.playState, JSON.stringify(a.effect.getKeyframes().slice(0, 2).map((k) => k.transform || k.opacity))]));
const seenAll = (except) => JSON.stringify(["where-to", "continue", "first-lesson", "quick-check", "practice", "first-exercise", "first-check", "ai-provider", "streak", "certificate"].filter((m) => !except.includes(m)));
const browser = await chromium.launch();
try {
  if (run("E")) {
    // E: the "continue" coach mark: Sidekick flies beside the card, a ring goes round it, once only
    const { ctx, p } = await page(browser, { storage: { sidekick_coach_last: "0", sidekick_coach_seen: seenAll(["continue"]) } });
    await p.waitForTimeout(1200);
    await scrollContinue(p);
    // the greeting quip goes first; the coach mark waits for it, then Sidekick flies to the card
    await p.waitForFunction(() => document.querySelector(".sidekick-coach") && document.querySelector(".sidekick-companion.flying img[src$='fly.png']"), null, { polling: "raf", timeout: 12000 }).catch(() => {});
    const mid = await anims(p);
    const midSrc = await p.locator(".sidekick-companion img").getAttribute("src");
    await p.screenshot({ path: `${S}/lively-E-mid-flight.png` });
    ok("E1 mid-flight: a running WAAPI transform arc and the fly pose", mid.some(([s, k]) => s === "running" && /translate.*rotate/.test(k)) && /\/(fly|landing)\.png$/.test(midSrc), JSON.stringify(mid) + " " + midSrc);
    await p.waitForTimeout(1600);
    const target = await p.locator('[data-sidekick-anchor="continue"]').boundingBox();
    const sidekick = await p.locator(".sidekick-companion").boundingBox();
    const overlap = sidekick.x < target.x + target.width && sidekick.x + sidekick.width > target.x && sidekick.y < target.y + target.height && sidekick.y + sidekick.height > target.y;
    ok("E2 the flight ends beside the card, never on it", !overlap && (await anims(p)).length === 0, JSON.stringify({ sidekick, target }));
    ok("E3 and covers no text or control", (await covers(p)).length === 0, JSON.stringify(await covers(p)));
    const after = await p.evaluate(() => window.__srcs);
    ok("E4 takes off, flies, lands, then points at it", /takeoff.*fly.*landing.*point-(left|right|up|down)/.test(after.map((s) => s.split("/").pop()).join(" ")), after.join(" "));
    const ring = await p.locator(".sidekick-ring").boundingBox();
    ok("E5 a spotlight ring round the target", ring && Math.abs(ring.x - (target.x - 6)) < 3 && Math.abs(ring.width - (target.width + 12)) < 3, JSON.stringify(ring));
    const coach = p.locator(".sidekick-coach");
    ok("E6 the coach mark: title, polite live text, Show me and Got it", /Pick up where you left off/.test(await coach.textContent({ timeout: 2000 }).catch(() => "")) && await coach.locator("[aria-live=polite]").count() === 1
      && await coach.getByRole("button", { name: "Show me" }).count() === 1 && await coach.getByRole("button", { name: "Got it" }).count() === 1);
    await p.screenshot({ path: `${S}/lively-E-coach-continue.png` });
    // keyboard: Tab reaches the coach's buttons, Esc closes it
    await coach.getByRole("button", { name: "Show me" }).focus();
    ok("E7 keyboard reaches Show me", await p.evaluate(() => document.activeElement?.textContent) === "Show me");
    await p.keyboard.press("Escape");
    await p.waitForTimeout(200);
    ok("E8 Esc dismisses it", await coach.count() === 0);
    const seen = await p.evaluate(() => localStorage.getItem("sidekick_coach_seen"));
    ok("E9 shown marks are remembered on this device", /continue/.test(seen || ""), seen);
    // once only: even with the wait over, the same mark never comes back
    await p.evaluate(() => localStorage.setItem("sidekick_coach_last", "0"));
    await p.reload(); await p.waitForTimeout(1200); await scrollContinue(p); await p.waitForTimeout(3500);
    ok("E10 a coach mark shows once", await p.locator(".sidekick-coach").count() === 0);
    await ctx.close();
  }
  if (run("E")) {
    // E11: a mark swept away at once (a scroll on the next frame) wasn't read: it isn't remembered, and comes back
    const { ctx, p } = await page(browser, { storage: { sidekick_coach_last: "0", sidekick_coach_seen: seenAll(["continue"]) } });
    await p.waitForTimeout(1200);
    await scrollContinue(p);
    await p.locator(".sidekick-coach").waitFor({ timeout: 12000 }).catch(() => {});
    await p.evaluate(() => { const m = document.querySelector("#course-pane"); if (m) m.scrollTop += 600; window.scrollBy(0, 600); });
    await p.waitForTimeout(400);
    const gone = await p.locator(".sidekick-coach").count() === 0;
    const seen = await p.evaluate(() => localStorage.getItem("sidekick_coach_seen"));
    ok("E11 a coach mark scrolled away at once is not remembered as seen", gone && !/"continue"/.test(seen || ""), `${gone} ${seen}`);
    await p.evaluate(() => localStorage.setItem("sidekick_coach_last", "0"));
    await p.reload(); await p.waitForTimeout(1200); await scrollContinue(p);
    await p.locator(".sidekick-coach").waitFor({ timeout: 12000 }).catch(() => {});
    ok("E12 so it comes back another time", await p.locator(".sidekick-coach").count() === 1);
    await ctx.close();
  }
  if (run("R")) {
    // R: unprompted coach marks follow familiarity: every 45 s for someone new, rarely once they know the app
    let { ctx, p } = await page(browser, { storage: { sidekick_coach_last: String(Date.now() - 30_000), sidekick_coach_seen: seenAll(["continue"]), sidekick_familiar: "{}" } });
    await p.waitForTimeout(1000); await scrollContinue(p); await p.waitForTimeout(3500);
    ok("R1 new here, 30 s after the last one: no coach mark yet", await p.locator(".sidekick-coach").count() === 0);
    await p.evaluate(() => localStorage.setItem("sidekick_coach_last", String(Date.now() - 46_000)));
    await p.waitForTimeout(2500);
    ok("R2 new here, 46 s after: it shows", await p.locator(".sidekick-coach").count() === 1);
    await ctx.close();
    ({ ctx, p } = await page(browser, { storage: { sidekick_coach_last: String(Date.now() - 240_000), sidekick_coach_seen: seenAll(["continue"]), sidekick_familiar: JSON.stringify({ course: 0.75 }) } }));
    await p.waitForTimeout(1000); await scrollContinue(p); await p.waitForTimeout(4000);
    ok("R3 at home in the app, 4 minutes after: still quiet (the gap has grown)", await p.locator(".sidekick-coach").count() === 0);
    await ctx.close();
  }
  if (run("F")) {
    // F: reduced motion: Sidekick still goes where help is, but fades there; no flight, no tricks
    const { ctx, p } = await page(browser, { reduce: true, storage: { sidekick_coach_last: "0", sidekick_coach_seen: seenAll(["continue"]) } });
    await p.waitForTimeout(1200); await scrollContinue(p);
    await p.locator(".sidekick-coach").waitFor({ timeout: 12000 }).catch(() => {});
    await p.waitForTimeout(800);
    const srcs = await p.evaluate(() => window.__srcs);
    ok("F1 reduced motion: no takeoff, flight or landing poses", !srcs.some((s) => /(fly|takeoff|landing)\.png$/.test(s)), JSON.stringify(srcs));
    ok("F2 reduced motion: Sidekick still arrives beside the card", await p.locator(".sidekick-companion:not(.hidden)").count() === 1 && (await covers(p)).length === 0);
    await p.keyboard.press("Escape");
    await p.locator(".sidekick-tab").click();
    await p.locator(".pane-sidekick").click();
    await p.waitForTimeout(100);
    const anim = await p.locator(".pane-sidekick .sidekick-fx").evaluate((e) => getComputedStyle(e).animationName);
    ok("F3 reduced motion: a tap's trick doesn't animate", anim === "none", anim);
    await ctx.close();
  }
  if (run("M")) {
    // M: modes. Quiet: Sidekick stays in his tab, silent until clicked. Off: the tab only, for asking.
    let { ctx, p } = await page(browser, { storage: { sidekick_mode: "quiet", sidekick_coach_last: "0", sidekick_coach_seen: "[]" } });
    await p.waitForTimeout(1200); await scrollContinue(p); await p.waitForTimeout(4000);
    ok("M1 Quiet: no coach mark, no quip, Sidekick in his tab", await p.locator(".sidekick-coach").count() === 0 && await p.locator(".sidekick-quip.on").count() === 0 && await p.locator(".sidekick-companion.hidden").count() === 1 && await p.locator(".sidekick-tab").count() === 1);
    ok("M2 Quiet: never flew", !(await p.evaluate(() => window.__srcs)).some((s) => /fly\.png$/.test(s)));
    await p.locator(".sidekick-tab").click();
    await p.locator(".sidekick-where-toggle").click();
    ok("M3 Quiet: a click still opens Sidekick and Where to?", await p.locator(".sidekick-where .sidekick-chips .chip").count() === 4);
    await ctx.close();
    ({ ctx, p } = await page(browser, { storage: { sidekick_mode: "off", sidekick_coach_last: "0", sidekick_coach_seen: "[]" } }));
    await p.waitForTimeout(1200); await scrollContinue(p); await p.waitForTimeout(3000);
    ok("M4 Off: no flying Sidekick, no coach mark, no quip; the tab stays", await p.locator(".sidekick-companion:not(.hidden)").count() === 0 && await p.locator(".sidekick-coach").count() === 0 && await p.locator(".sidekick-quip.on").count() === 0 && await p.locator(".sidekick-tab").count() === 1);
    await ctx.close();
  }
  if (run("T")) {
    // T: taps on Sidekick in his pane: a trick each tap, never the same twice running; 3 quick taps: dizzy; 7: the secret
    const { ctx, p } = await page(browser);
    await p.waitForTimeout(1500);
    await p.locator(".sidekick-tab").click();
    await p.waitForTimeout(500);
    await p.evaluate(() => { window.__fx = []; new MutationObserver(() => { const c = document.querySelector(".pane-sidekick .sidekick-fx")?.className.split(" ").find((x) => x.startsWith("t-")); if (c && window.__fx[window.__fx.length - 1] !== c) window.__fx.push(c); }).observe(document.querySelector(".pane-sidekick"), { subtree: true, attributes: true, attributeFilter: ["class"] }); });
    const k = p.locator(".pane-sidekick");
    await k.click();
    await p.waitForTimeout(120);
    ok("T1 one tap: a trick plays", (await p.evaluate(() => window.__fx)).length === 1);
    await p.screenshot({ path: `${S}/lively-T-trick.png` });
    for (let i = 0; i < 12; i++) { await p.waitForTimeout(900); await k.click(); await p.waitForTimeout(150); await k.click(); }
    const fx = (await p.evaluate(() => window.__fx)).filter((c) => c !== "t-dizzy");
    ok("T2 the trick pool never repeats back to back", fx.length >= 12 && fx.every((c, i) => i === 0 || c !== fx[i - 1]), fx.join(" "));
    await p.waitForTimeout(1200);
    await k.click(); await p.waitForTimeout(120); await k.click(); await p.waitForTimeout(120); await k.click();
    await p.waitForTimeout(250);
    ok("T3 three quick taps: dizzy", /dizzy\.png$/.test(await k.locator("img").getAttribute("src")) && /t-dizzy/.test(await k.locator(".sidekick-fx").getAttribute("class")));
    ok("T4 and he says so in his header", (await p.locator(".pane-title span").textContent()).length > 3 && !/Coaching you|Straight answers/.test(await p.locator(".pane-title span").textContent()));
    await p.waitForTimeout(2200);
    for (let i = 0; i < 7; i++) { await k.click(); await p.waitForTimeout(110); }
    await p.waitForTimeout(150);
    ok("T5 seven quick taps: the secret move", /t-secret/.test(await k.locator(".sidekick-fx").getAttribute("class")));
    await ctx.close();
  }
  if (run("D")) {
    // D: a window that shrinks under a Sidekick out pointing pulls him back on screen (sweep: 1024 -> 800x600 left him at y 605-677)
    const { ctx, p } = await page(browser, { viewport: { width: 1024, height: 768 }, storage: { sidekick_coach_last: "0", sidekick_coach_seen: seenAll(["continue"]) } });
    await p.waitForTimeout(1200); await scrollContinue(p);
    await p.locator(".sidekick-coach").waitFor({ timeout: 12000 }).catch(() => {});
    await p.waitForTimeout(1800);
    await p.setViewportSize({ width: 800, height: 600 });
    await p.waitForTimeout(500);
    const k = await p.locator(".sidekick-companion").boundingBox();
    ok("D1 after a resize Sidekick is wholly on screen", k && k.x >= 0 && k.y >= 0 && k.x + k.width <= 800 && k.y + k.height <= 600, JSON.stringify(k));
    await ctx.close();
  }
  if (run("W")) {
    // W: a dialog (What's new) is open: Sidekick never moves, coaches or speaks
    const { ctx, p } = await page(browser, { extra: { get_whats_new: { version: "9.9.9", title: "New things", new: ["A thing"], fixed: [] } }, storage: { sidekick_coach_last: "0", sidekick_coach_seen: "[]" } });
    await p.locator(".news").waitFor({ timeout: 6000 }).catch(() => {});
    await scrollContinue(p); await p.waitForTimeout(4000);
    const srcs = await p.evaluate(() => window.__srcs);
    ok("W1 dialog open: no flight, no coach mark, no quip", !srcs.some((s) => /(fly|takeoff)\.png$/.test(s)) && await p.locator(".sidekick-coach").count() === 0 && await p.locator(".sidekick-quip.on").count() === 0 && (await anims(p)).length === 0, JSON.stringify(srcs));
    ok("W2 dialog open: Sidekick stays home", await p.locator(".sidekick-companion.hidden").count() === 1);
    await ctx.close();
  }
  if (run("L")) {
    // L: a lesson: the first-lesson and quick-check coach marks, Sidekick pointing at the target
    const { ctx, p } = await page(browser, { storage: { sidekick_coach_last: "0", sidekick_coach_seen: seenAll(["first-lesson"]) } });
    await p.waitForTimeout(1500);
    await p.evaluate(() => { const b = [...document.querySelectorAll("button")].find((b) => /Lesson 1\.2|Continue|1\.2/.test(b.textContent)); b?.click(); });
    await p.waitForTimeout(900);
    await p.evaluate(() => document.querySelector('[data-sidekick-anchor="complete"]')?.scrollIntoView({ block: "center" }));
    await p.locator(".sidekick-coach").waitFor({ timeout: 12000 }).catch(() => {});
    await p.waitForTimeout(1600);
    ok("L1 first lesson: a coach mark at Mark complete", /Finished reading/.test(await p.locator(".sidekick-coach").textContent().catch(() => "")));
    ok("L2 Sidekick beside it covers nothing", (await covers(p)).length === 0, JSON.stringify(await covers(p)));
    await p.screenshot({ path: `${S}/lively-L-coach-first-lesson.png` });
    await p.locator(".sidekick-coach").getByRole("button", { name: "Show me" }).click({ timeout: 3000 }).catch(() => {});
    await p.waitForTimeout(300);
    ok("L3 Show me focuses the control and pulses the ring", await p.evaluate(() => document.activeElement?.closest('[data-sidekick-anchor="complete"]') !== null) && await p.locator(".sidekick-ring.pulse").count() === 1);
    await ctx.close();
  }
  if (run("S")) {
    // S: Settings with a provider that still needs its key: the AI provider coach mark, and the companion beside it
    const settings = { provider: { kind: "open_ai", base_url: "https://api.openai.com/v1", model: "gpt-4o-mini" }, has_api_key: false, practice_url: "" };
    const seen = JSON.stringify(["where-to", "continue", "first-lesson", "quick-check", "practice", "first-exercise", "first-check", "streak", "certificate"]);
    const { ctx, p } = await page(browser, { settings, storage: { sidekick_coach_last: "0", sidekick_coach_seen: seen } });
    await p.waitForTimeout(1200);
    await p.locator("button[aria-label=Settings]").click();
    await p.locator(".sidekick-coach").waitFor({ timeout: 10000 }).catch(() => {});
    await p.waitForTimeout(1500);
    ok("S1 Settings opens with the AI provider coach mark", /AI provider/.test(await p.locator(".sidekick-coach").textContent().catch(() => "")));
    const sec = await p.locator('[data-sidekick-anchor="ai-provider"]').boundingBox();
    const kb = await p.locator(".sidekick-companion").boundingBox();
    // beside the section if there's room, or (no room beside it) pointing from his tab: never on it
    ok("S2 Sidekick beside the section, not on it", sec && (kb ? !(kb.x < sec.x + sec.width && kb.x + kb.width > sec.x && kb.y < sec.y + sec.height && kb.y + kb.height > sec.y) : await p.locator(".sidekick-peek:not(.away)").count() === 1), JSON.stringify({ kb, sec }));
    await p.screenshot({ path: `${S}/lively-S-ai-provider.png` });
    await ctx.close();
  }
  if (run("P")) {
    // P: a nudge at the tab opens wholly inside the window, clear of the text-size control and the page's primary
    // buttons; on a phone it is a slim strip (sweep: the check-graded nudge covered Resubmit at 390 px)
    for (const [vw, vh] of [[800, 600], [1024, 768], [1440, 900], [390, 844]]) {
      const { ctx, p } = await page(browser, { viewport: { width: vw, height: vh }, allDone: false, days: null, today: false, storage: { sidekick_snooze: "{}" } });
      await p.locator(".sidekick-bubble").waitFor({ timeout: 8000 }).catch(() => {});
      await p.waitForTimeout(400);
      const r = await p.evaluate(() => {
        const box = (e) => { const b = e.getBoundingClientRect(); return [b.left, b.top, b.right, b.bottom]; };
        const b = document.querySelector(".sidekick-bubble"), z = document.querySelector(".zoom:not(.hidden)");
        const prim = [...document.querySelectorAll(".btn.primary")].filter((e) => !e.closest(".sidekick-companion-root") && e.getBoundingClientRect().width > 0).map(box);
        return { b: b && box(b), z: z && box(z), prim, strip: b?.classList.contains("strip") };
      });
      const hit = (a, c) => a[0] < c[2] && a[2] > c[0] && a[1] < c[3] && a[3] > c[1];
      const inside = r.b && r.b[0] >= 0 && r.b[1] >= 0 && r.b[2] <= vw - 40 + 1 && r.b[3] <= vh;
      const clear = (!r.z || !hit(r.b, r.z)) && !r.prim.some((x) => hit(r.b, x));
      ok(`P ${vw}x${vh}: the nudge is in the window, left of the tab, clear of the text-size control and primary buttons${vw < 640 ? " (a strip)" : ""}`, inside && clear && (vw >= 640 || r.strip), JSON.stringify(r));
      await p.screenshot({ path: `${S}/lively-P-${vw}.png` });
      await ctx.close();
    }
  }
} finally { await browser.close(); }
console.log(out.join("\n"));
process.exit(out.some((l) => l.startsWith("FAIL")) ? 1 : 0);
