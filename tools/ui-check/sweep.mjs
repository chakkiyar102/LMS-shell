// Layout sweep: every screen, light and dark, at five window sizes and both text-size extremes, with
// programmatic checks (things outside the window or clipped, overlapping controls, cut-off text,
// horizontal page scroll, contrast of text on its real background, the companion's bubble/quip/coach inside the
// window and off the top bar, the text-size control and primary buttons, console errors).
//
//   node tools/ui-check/sweep.mjs /tmp/tour-data.json /tmp/tour-dist OUT_DIR [PORT] [only-screen-prefix] [sizes]
//   BUBBLES=1 node tools/ui-check/sweep.mjs ...   (the bubble pass only)
//
// Same data and stubbed bridge as tour.mjs (tour-lib.mjs). Writes OUT_DIR/<screen>-<theme>-<size>.png,
// OUT_DIR/issues.json and a contact sheet per screen (sheet-<screen>.png). Exits 1 when a check fails.
//
// The bubble pass (always run, or alone with BUBBLES=1): on every screen at 390, 800, 1024 and 1440, each
// kind of Sidekick bubble is put up (the debug build's __sidekickShow hook): a quip, a nudge, and a coach mark and
// hover help on the control nearest the page's primary button and on the text-size control. Any bubble
// outside the window, over the top bar, the text-size control, a primary button or the session rail, or
// (away from a phone's strip) over another control's centre fails it. Its shots: OUT_DIR/bubbles/.
import { chromium } from "playwright";
import fs from "node:fs";
import { serve } from "./tour-lib.mjs";

const [DATA, DIST, OUT, PORT_ARG, ONLY, SIZES_ARG] = process.argv.slice(2);
fs.mkdirSync(OUT, { recursive: true });
const { data, server, open } = await serve(DATA, DIST, Number(PORT_ARG || 8825));
const ALL_SIZES = { phone: [390, 844], s800: [800, 600], s1024: [1024, 768], s1440: [1440, 900], s1920: [1920, 1080] };
const SIZES = SIZES_ARG ? Object.fromEntries(SIZES_ARG.split(",").map((k) => [k, ALL_SIZES[k]])) : ALL_SIZES;
const today = () => new Date().toDateString();
const quiet = { sidekick_today: today() };
const LONG = { name: "Maximiliana Alexandrovna Featherstonehaugh-Worthington de la Vega", email: "maximiliana.featherstonehaugh-worthington@brightline-consulting-partners.example" };
const NEWS = { version: "0.2.0", title: "Calmer nudges", new: ["Search inside a lesson.", "The companion points at what's next."], fixed: ["The companion's speech bubble stays inside the window."] };

/** In the page: every problem with the layout as it stands (`sidekickOnly`: only Sidekick's bubbles). */
function audit(sidekickOnly = false) {
  const out = [];
  const vw = innerWidth, vh = innerHeight;
  const vis = (e) => {
    const r = e.getBoundingClientRect();
    if (r.width < 1 || r.height < 1) return false;
    for (let x = e; x; x = x.parentElement) {
      const s = getComputedStyle(x);
      if (s.display === "none" || s.visibility === "hidden" || Number(s.opacity) === 0) return false;
    }
    return true;
  };
  const name = (e) => {
    const t = (e.getAttribute("aria-label") || e.textContent || e.getAttribute("placeholder") || "").trim().replace(/\s+/g, " ").slice(0, 40);
    return `${e.tagName.toLowerCase()}${e.className && typeof e.className === "string" ? "." + e.className.trim().split(/\s+/).join(".") : ""} "${t}"`;
  };
  const inSidekick = (e) => !!e.closest(".sidekick-companion-root");
  // 1. horizontal page scroll
  const se = document.scrollingElement;
  if (!sidekickOnly) {
    if (se.scrollWidth > vw + 1) out.push({ check: "hscroll", what: `page scrolls sideways: ${se.scrollWidth} > ${vw}` });
    const pane = document.getElementById("course-pane");
    if (pane && pane.scrollWidth > pane.clientWidth + 1 && getComputedStyle(pane).overflowX !== "hidden") out.push({ check: "hscroll", what: `#course-pane scrolls sideways: ${pane.scrollWidth} > ${pane.clientWidth}` });
    // 2. controls outside the window or clipped by an overflow ancestor
    const scrollsX = (e) => { for (let x = e.parentElement; x; x = x.parentElement) { const s = getComputedStyle(x).overflowX; if ((s === "auto" || s === "scroll") && x.scrollWidth > x.clientWidth + 1) return true; } return false; };
    for (const e of [...document.querySelectorAll("button, a[href], input:not([type=radio]):not([type=checkbox]), select, textarea, [role=button], summary, label.opt, label.choice")].filter(vis)) {
      const r = e.getBoundingClientRect();
      if ((r.right > vw + 1 || r.left < -1) && !scrollsX(e)) out.push({ check: "offscreen-x", what: `${name(e)} at x ${Math.round(r.left)}..${Math.round(r.right)} (window ${vw})` });
      for (let x = e.parentElement; x && x !== document.body; x = x.parentElement) {
        const s = getComputedStyle(x);
        if (s.overflowX === "visible" && s.overflowY === "visible") continue;
        if (s.overflowY !== "visible" && x.scrollHeight > x.clientHeight + 1 && s.overflowY !== "hidden") break; // a scroller: fine
        const p = x.getBoundingClientRect();
        if ((s.overflowX === "hidden" || s.overflowX === "clip") && (r.left < p.left - 1 || r.right > p.right + 1)) { out.push({ check: "clipped", what: `${name(e)} cut by ${name(x)} sideways` }); break; }
        if ((s.overflowY === "hidden" || s.overflowY === "clip") && (r.top < p.top - 1 || r.bottom > p.bottom + 1) && r.height < p.height) { out.push({ check: "clipped", what: `${name(e)} cut by ${name(x)} vertically` }); break; }
      }
  }
  }
  const ctrls = [...document.querySelectorAll("button, a[href], input:not([type=radio]):not([type=checkbox]), select, textarea, [role=button], summary, label.opt, label.choice")].filter(vis);
  // 3. a control covered by something else (checked at its centre, only when on screen)
  for (const e of ctrls) {
    if (inSidekick(e)) continue;
    const r = e.getBoundingClientRect();
    const cx = (r.left + r.right) / 2, cy = (r.top + r.bottom) / 2;
    if (cx < 0 || cy < 0 || cx > vw || cy > vh) continue;
    const hit = document.elementFromPoint(cx, cy);
    if (!hit || e.contains(hit) || hit.contains(e) || hit.closest("label")?.contains(e)) continue;
    // under a modal, or scrolled under the sticky bar or a fixed control (a phone's Sidekick strip is one, kept
    // off the top bar and primary buttons by check 6): by design, not a layout bug
    const over = hit.closest(".scrim, [aria-modal=true], section.tour, header.top, .zoom, .sidekick-tab-wrap, .sidekick-side-pane, .srail, .sidekick-bubble.strip, .sidekick-coach.strip");
    if (over && !over.contains(e) && !e.closest("[aria-modal=true], section.tour")) continue;
    // a scroller's edge clips it (sticky heads, etc) is reported as covered too: real for the learner
    if (!sidekickOnly || inSidekick(hit)) out.push({ check: inSidekick(hit) ? "sidekick-covers" : "covered", what: `${name(e)} covered by ${name(hit)}` });
  }
  if (!sidekickOnly) {
    // 4. text cut off (ellipsis or hidden overflow) where it is not meant to be
    for (const e of document.querySelectorAll("body *")) {
      if (!e.firstChild || ![...e.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim())) continue;
      if (!vis(e) || e.clientWidth <= 2 || e.clientHeight <= 2) continue; // visually hidden (sr-only) text is meant to be clipped
      const s = getComputedStyle(e);
      const cut = (s.textOverflow === "ellipsis" || s.overflowX === "hidden" || s.overflowX === "clip") && e.scrollWidth > e.clientWidth + 1;
      const cutY = (s.overflowY === "hidden" || s.overflowY === "clip" || s.webkitLineClamp !== "none") && e.scrollHeight > e.clientHeight + 2;
      if (cut || cutY) out.push({ check: "text-cut", what: `${name(e)} (${cut ? "width" : "height"} ${cut ? e.scrollWidth + ">" + e.clientWidth : e.scrollHeight + ">" + e.clientHeight})` });
  }
  // 5. contrast of each text run on its real background
  const rgba = (c) => { const m = c.match(/rgba?\(([^)]+)\)/); if (!m) return null; const p = m[1].split(/[ ,/]+/).filter(Boolean).map(Number); return [p[0], p[1], p[2], p.length > 3 ? p[3] : 1]; };
  const parseColor = (c) => {
    if (c.startsWith("rgb")) return rgba(c);
    // color(srgb r g b / a) and oklch etc: let the canvas resolve it
    const cv = document.createElement("canvas"); cv.width = cv.height = 1; const g = cv.getContext("2d"); g.fillStyle = "#000"; g.fillStyle = c; g.fillRect(0, 0, 1, 1); const d = g.getImageData(0, 0, 1, 1).data;
    const a = c.match(/\/\s*([\d.]+%?)\s*\)$/); return [d[0], d[1], d[2], a ? (a[1].endsWith("%") ? parseFloat(a[1]) / 100 : Number(a[1])) : 1];
  };
  const lum = ([r, g, b]) => { const f = (v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; }; return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b); };
  const over = (top, bot) => { const a = top[3]; return [top[0] * a + bot[0] * (1 - a), top[1] * a + bot[1] * (1 - a), top[2] * a + bot[2] * (1 - a), 1]; };
  const bgOf = (e) => {
    const layers = [];
    for (let x = e; x; x = x.parentElement) {
      const s = getComputedStyle(x);
      if (s.backgroundImage !== "none" && !s.backgroundImage.startsWith("url")) return null; // a gradient: unknown
      const c = parseColor(s.backgroundColor);
      if (c && c[3] > 0) { layers.push(c); if (c[3] >= 1) break; }
    }
    let bg = [255, 255, 255, 1];
    if (!layers.length || layers[layers.length - 1][3] < 1) bg = parseColor(getComputedStyle(document.body).backgroundColor) || bg;
    for (const l of layers.reverse()) bg = over(l, bg);
    return bg;
  };
  const seen = new Set();
  for (const e of document.querySelectorAll("body *")) {
    if (![...e.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim().length > 1)) continue;
    if (!vis(e) || e.closest("[aria-hidden=true], :disabled, .sidekick-companion-root video, option") || e.matches(":disabled, option")) continue;
    const r = e.getBoundingClientRect();
    if (r.bottom < 0 || r.top > vh) continue; // only what's on screen
    const s = getComputedStyle(e);
    let fg = parseColor(s.color); if (!fg) continue;
    let op = 1; for (let x = e; x; x = x.parentElement) op *= Number(getComputedStyle(x).opacity);
    const bg = bgOf(e); if (!bg) continue;
    fg = over([fg[0], fg[1], fg[2], fg[3] * op], bg);
    const L1 = lum(fg), L2 = lum(bg);
    const ratio = (Math.max(L1, L2) + 0.05) / (Math.min(L1, L2) + 0.05);
    const px = parseFloat(s.fontSize), bold = Number(s.fontWeight) >= 700;
    const need = px >= 24 || (bold && px >= 18.66) ? 3 : 4.5;
    if (ratio < need - 0.05) {
      const key = `${name(e)}|${ratio.toFixed(1)}`;
      if (!seen.has(key)) { seen.add(key); out.push({ check: "contrast", what: `${name(e)} ${ratio.toFixed(2)}:1 < ${need} (${s.color} on rgb(${bg.slice(0, 3).map(Math.round)}) ${px}px)` }); }
    }
  }
  }
  // 6. Sidekick's bubble, quip and coach mark: inside the window, off the top bar, text-size control and primary buttons
  const R = (e) => e.getBoundingClientRect();
  const hits = (a, b) => a.left < b.right - 1 && a.right > b.left + 1 && a.top < b.bottom - 1 && a.bottom > b.top + 1;
  const keep = [...document.querySelectorAll("header.top:not(.hidden), .zoom:not(.hidden), .btn.primary, .srail")].filter(vis).filter((e) => !inSidekick(e));
  for (const sel of [".sidekick-bubble", ".sidekick-quip.on", ".sidekick-coach", ".sidekick-companion", ".sidekick-tab", ".sidekick-side-pane"]) {
    for (const e of document.querySelectorAll(sel)) {
      if (!vis(e)) continue;
      const r = R(e);
      if (r.left < -1 || r.top < -1 || r.right > vw + 1 || r.bottom > vh + 1) out.push({ check: "sidekick-out", what: `${sel} outside the window: ${[r.left, r.top, r.right, r.bottom].map(Math.round)} in ${vw}x${vh}` });
      if ([".sidekick-companion", ".sidekick-tab", ".sidekick-side-pane"].includes(sel)) continue;
      for (const k of keep) if (hits(r, R(k))) out.push({ check: "sidekick-covers", what: `${sel} over ${name(k)}` });
    }
  }
  return out;
}

const issues = [];
const shots = [];
const browser = await chromium.launch(process.env.TOUR_CHANNEL ? { channel: process.env.TOUR_CHANNEL } : {});
const settle = (p, ms = 700) => p.waitForTimeout(ms);

// ---- the bubble pass: every kind of Sidekick bubble on every screen, at four widths ----
const BUBBLES_ONLY = process.env.BUBBLES === "1";
const BUBBLE_SIZES = { phone: [390, 844], s800: [800, 600], s1024: [1024, 768], s1440: [1440, 900] };
const BUBBLE_TEXT = "Checking where this opens: a line about as long as Sidekick's usual explanations, so that it wraps over two or three lines.";
const bubbleIssues = [];
let bubbleShows = 0;
/** In the page: where to point a coach mark and hover help: the control with help nearest the primary button (or the middle), and the text-size control. */
function bubbleTargets() {
  const vis = (e) => { const r = e.getBoundingClientRect(); return r.width > 0 && r.height > 0 && r.bottom > 0 && r.top < innerHeight && r.right > 0 && r.left < innerWidth && !e.closest(".sidekick-companion-root"); };
  for (const e of document.querySelectorAll("[data-bubble-check]")) e.removeAttribute("data-bubble-check");
  const prim = [...document.querySelectorAll(".btn.primary")].find(vis);
  const c = prim ? prim.getBoundingClientRect() : { left: innerWidth / 2, right: innerWidth / 2, top: innerHeight / 2, bottom: innerHeight / 2 };
  const d = (e) => { const r = e.getBoundingClientRect(); return Math.hypot((r.left + r.right - c.left - c.right) / 2, (r.top + r.bottom - c.top - c.bottom) / 2); };
  const near = [...document.querySelectorAll("[data-help]")].filter((e) => vis(e) && e !== prim && !e.closest(".zoom")).sort((a, b) => d(a) - d(b))[0];
  const out = [];
  if (near) { near.setAttribute("data-bubble-check", ""); out.push("[data-bubble-check]"); }
  const z = document.querySelector(".zoom:not(.hidden) [data-help='text.bigger']");
  if (z && vis(z)) out.push(".zoom [data-help='text.bigger']");
  return out;
}
async function bubblePass(name, opts, go) {
  const { ctx, p } = await open(browser, { theme: "light", storage: quiet, viewport: { width: 1024, height: 768 }, ...opts });
  try {
    await go(p);
    // no companion yet (onboarding): nothing of Sidekick's to place
    if (!(await p.locator(".sidekick-companion-root").count())) { await ctx.close(); return; }
    if (!(await p.evaluate(() => typeof window.__sidekickShow === "function"))) throw new Error("no __sidekickShow: sweep needs the debug build (trunk build without --release)");
    fs.mkdirSync(`${OUT}/bubbles`, { recursive: true });
    for (const [label, [w, h]] of Object.entries(BUBBLE_SIZES)) {
      await p.setViewportSize({ width: w, height: h });
      await settle(p, 400);
      const targets = await p.evaluate(bubbleTargets);
      for (const [kind, target] of [["quip", null], ["nudge", null], ...targets.flatMap((t) => [["mark", t], ["help", t]])]) {
        await p.evaluate(([k, t, text]) => window.__sidekickShow(k, text, t), [kind, target, BUBBLE_TEXT]);
        await settle(p, 650);
        bubbleShows++;
        const found = await p.evaluate(audit, true);
        for (const i of found) bubbleIssues.push({ screen: name, size: label, kind, target, ...i });
        if (found.length) await p.screenshot({ path: `${OUT}/bubbles/${name}-${label}-${kind}${target?.includes("zoom") ? "-zoom" : ""}.png` });
      }
      await p.evaluate(() => window.__sidekickShow("none", "", null));
    }
  } catch (e) { bubbleIssues.push({ screen: name, size: "-", check: "navigate", what: String(e).slice(0, 300) }); }
  await ctx.close();
}

async function sweep(name, opts, go, { sizes = SIZES, themes = ["light", "dark"], zoom = true, after } = {}) {
  if (ONLY && !name.startsWith(ONLY)) return;
  await bubblePass(name, opts, go);
  if (BUBBLES_ONLY) return;
  for (const theme of themes) {
    const first = Object.values(sizes)[0];
    const { ctx, p } = await open(browser, { theme, storage: quiet, viewport: { width: first[0], height: first[1] }, ...opts });
    const logs = [];
    p.on("console", (m) => { if (m.type() === "error" || m.type() === "warning") logs.push(`${m.type()}: ${m.text().slice(0, 200)}`); });
    p.on("pageerror", (e) => logs.push(`pageerror: ${e.message}`));
    try { await go(p); } catch (e) { issues.push({ screen: name, theme, size: "-", check: "navigate", what: String(e).slice(0, 300) }); await ctx.close(); continue; }
    const run = async (label) => {
      await settle(p);
      for (const i of await p.evaluate(audit)) issues.push({ screen: name, theme, size: label, ...i });
      const f = `${name}-${theme}-${label}.png`;
      await p.screenshot({ path: `${OUT}/${f}` });
      shots.push({ screen: name, file: f });
      if (after) await after(p, label, theme);
    };
    for (const [label, [w, h]] of Object.entries(sizes)) { await p.setViewportSize({ width: w, height: h }); await run(label); }
    if (zoom && (await p.locator(".zoom:not(.hidden)").count())) {
      for (const [dir, lab] of [["Larger text", "a140"], ["Smaller text", "a80"]]) {
        for (let i = 0; i < 6; i++) { const b = p.locator(`.zoom button[aria-label="${dir}"]`); if (await b.isDisabled()) break; await b.click(); }
        for (const [label, [w, h]] of Object.entries({ phone: [390, 844], s1024: [1024, 768] })) { await p.setViewportSize({ width: w, height: h }); await run(`${label}-${lab}`); }
      }
    }
    for (const l of logs) issues.push({ screen: name, theme, size: "-", check: "console", what: l });
    await ctx.close();
  }
}
const home = (p) => p.locator(".welcome-title").waitFor();
const session = async (p, n = "01") => { await home(p); await p.locator(".session-card", { hasText: n }).first().click(); await p.locator(".lesson-row").first().waitFor(); };
const lesson = async (p, n = "1.2", s = "01") => { await session(p, s); await p.locator(".lesson-row", { hasText: n }).click(); await p.locator("article.lesson").waitFor(); };
const answerAll = async (p) => {
  for (const g of await p.locator(".question").all()) {
    const r = g.locator("input[type=radio], input[type=checkbox]").first();
    if (await r.count()) await r.check();
    for (const s of await g.locator("select").all()) await s.selectOption({ index: 1 });
  }
};

try {
  await sweep("onboard", { profile: false }, async (p) => { await p.locator(".onboard-form").waitFor(); });
  await sweep("onboard-partner", { profile: false }, async (p) => {
    await p.locator(".onboard-form").waitFor();
    await p.locator(".onboard-form input").first().fill(LONG.name);
    await p.locator(".onboard-form input[type=email]").fill("max.featherstonehaugh-worthington@brightline-consulting-partners.example");
    await p.locator(".onboard-form select").selectOption("partner");
  });
  await sweep("onboard-learner-error", { profile: false }, async (p) => {
    await p.locator(".onboard-form").waitFor();
    await p.locator(".onboard-form input").first().fill(LONG.name);
    await p.locator(".onboard-form input[type=email]").fill("max@mailinator.com");
    await p.locator(".onboard-form select").selectOption("learner");
    await p.locator(".onboard-form button[type=submit]").click();
    await p.locator(".onboard-form .notice.error, .onboard-form [role=alert]").first().waitFor({ timeout: 3000 });
  });
  await sweep("onboard-staff-code", { profile: false }, async (p) => {
    await p.locator(".onboard-form").waitFor();
    await p.locator(".onboard-form select").selectOption("staff");
    await p.locator(".onboard-form input[type=email]").fill("jordan@brightline.example");
  });
  // a wrong access code in Settings → Access (the sample course isn't gated, so no locked course screen)
  await sweep("unlock", {}, async (p) => {
    await home(p); await p.locator("button[aria-label=Settings]").click(); await p.locator("form.unlock").waitFor();
    await p.locator("form.unlock input").first().fill("brightline-partner");
    await p.locator("form.unlock input[type=password]").fill("20500.WRONG");
    await p.locator("form.unlock button[type=submit]").click();
    await p.locator("form.unlock .notice.error").waitFor();
  });
  await sweep("home", { storage: {} }, home);
  await sweep("home-fresh", { scenario: "fresh", storage: {} }, home);
  await sweep("home-longname", { who: LONG }, home);
  await sweep("session", {}, (p) => session(p));
  await sweep("lesson", {}, (p) => lesson(p));
  await sweep("lesson-quickcheck-pass", { quiz: "pass" }, async (p) => {
    await lesson(p); await answerAll(p);
    await p.locator("article.lesson .quiz-bar .btn.primary").click();
    await p.locator(".quiz-bar .result").waitFor();
    await p.locator(".quiz-bar").evaluate((e) => e.scrollIntoView({ block: "center" }));
  });
  await sweep("lesson-quickcheck-miss", { quiz: "miss" }, async (p) => {
    await lesson(p);
    await p.locator("article.lesson .quiz-bar .btn.primary").click();
    await p.locator(".quiz-bar .result").waitFor();
    await p.locator(".feedback").first().evaluate((e) => e.scrollIntoView({ block: "center" }));
  });
  await sweep("check", { quiz: "miss" }, async (p) => {
    await session(p); await p.locator(".action-card", { hasText: "Session check" }).click(); await p.locator(".options").first().waitFor();
  });
  await sweep("check-graded", { quiz: "miss" }, async (p) => {
    await session(p); await p.locator(".action-card", { hasText: "Session check" }).click(); await p.locator(".options").first().waitFor();
    await answerAll(p); await p.locator(".quiz-bar .btn.primary").click(); await p.locator(".quiz-bar .result").waitFor();
    await p.locator(".quiz-bar").evaluate((e) => e.scrollIntoView({ block: "center" }));
  });
  await sweep("final", {}, async (p) => { await home(p); await p.locator(".session-card.final").first().click(); await p.locator("h1").first().waitFor(); });
  await sweep("exercise", {}, async (p) => {
    await session(p, "01"); await p.locator(".action-card", { hasText: "Exercise" }).click(); await p.locator("textarea.writeup").waitFor();
  });
  await sweep("exercise-practice", {}, async (p) => {
    await session(p, "01"); await p.locator(".action-card", { hasText: "Exercise" }).click(); await p.locator("textarea.writeup").waitFor();
    await p.locator(".practice .btn.primary").click(); await p.locator(".srail").waitFor(); await p.locator(".sidekick-pane").waitFor();
  }, { zoom: false });
  await sweep("coaching", {}, async (p) => {
    await lesson(p); await p.locator(".practice .btn.primary").click(); await p.locator(".sidekick-pane").waitFor();
    await p.locator(".pane-compose textarea").fill("How do I start?"); await p.locator(".pane-compose .btn.primary").click(); await p.waitForTimeout(400);
  }, { zoom: false });
  await sweep("certificate", {}, async (p) => { await home(p); await p.locator(".session-card.final", { hasText: "Your certificate" }).click(); await p.locator("ul.todo").waitFor(); });
  await sweep("certificate-earned", { who: LONG, extra: { get_certificate: { course: "Getting started with Brightline Notes", name: LONG.name, missing: [], final_score: 0.92, lessons_done: 6, lessons_total: 6, issued_on: "2026-10-28", id: "LMS-2026-0042" } } },
    async (p) => { await home(p); await p.locator(".session-card.final", { hasText: "Your certificate" }).click(); await p.locator(".cert-actions").waitFor(); });
  await sweep("settings", { role: "partner", who: LONG }, async (p) => { await home(p); await p.locator("button[aria-label=Settings]").click(); await p.locator("form.profile-form").waitFor(); },
    { after: async (p, label) => {
      // every section, one screenful at a time
      const n = await p.evaluate(() => document.querySelectorAll("#course-pane section, .settings section").length);
      for (let i = 1; i < n; i++) {
        await p.evaluate((i) => document.querySelectorAll("#course-pane section, .settings section")[i]?.scrollIntoView({ block: "start" }), i);
        await settle(p, 350);
      }
    } });
  await sweep("settings-bottom", { role: "partner", code: true }, async (p) => {
    await home(p); await p.locator("button[aria-label=Settings]").click(); await p.locator("form.profile-form").waitFor();
    await p.locator("button", { hasText: "Start over on this computer" }).scrollIntoViewIfNeeded();
  });
  await sweep("settings-access", { role: "partner", code: true }, async (p) => {
    await home(p); await p.locator("button[aria-label=Settings]").click(); await p.locator("form.profile-form").waitFor();
    const b = p.locator("form.unlock button", { hasText: "Remove access code" }); await b.evaluate((e) => e.closest("section").scrollIntoView({ block: "center" })); await b.click(); await p.locator(".confirm-inline").waitFor();
  }, { zoom: false });
  await sweep("start-over", {}, async (p) => {
    await home(p); await p.locator("button[aria-label=Settings]").click(); await p.locator("form.profile-form").waitFor();
    await p.locator("button", { hasText: "Start over on this computer" }).click(); await p.locator("section.confirm").waitFor();
  }, { zoom: false });
  await sweep("whats-new", { news: NEWS, storage: {} }, async (p) => { await p.locator("section.news").waitFor(); }, { zoom: false });
  // the course tour: only when content/courses/_shared/app-tour.yaml has one (version > 0)
  if (data.app_tour) await sweep("tour-first", { tour: "due" }, async (p) => { await p.locator("section.tour").waitFor(); await p.evaluate(() => document.querySelector("section.tour video")?.pause()); }, { zoom: false });
  if (data.app_tour) await sweep("tour-replay", {}, async (p) => { await home(p); await p.locator("button[aria-label='Course tour']").click(); await p.locator("section.tour").waitFor(); await p.evaluate(() => document.querySelector("section.tour video")?.pause()); }, { zoom: false });
  await sweep("sidekick-menu", {}, async (p) => { await home(p); await p.waitForTimeout(2500); await p.locator(".sidekick-tab").click(); await p.locator(".sidekick-where-toggle").click(); await p.locator(".sidekick-where .sidekick-chips").waitFor(); }, { zoom: false });
  await sweep("dock", {}, async (p) => { await lesson(p); await p.locator(".sidekick-tab").click(); await p.locator(".sidekick-side-pane").waitFor(); }, { zoom: false });
  await sweep("sidekick-settings", {}, async (p) => { await home(p); await p.locator("button[aria-label=Settings]").click(); await p.locator("form.profile-form").waitFor(); await p.locator(".sidekick-tab").click(); await p.locator(".sidekick-side-pane").waitFor(); }, { zoom: false });
  // error states: every load command refusing
  for (const cmd of ["get_courses", "get_course", "get_stats", "get_lesson", "get_check", "get_certificate", "get_settings", "get_exercise", "get_final"]) {
    const nav = { get_courses: home, get_course: home, get_stats: home, get_lesson: (p) => lesson(p), get_check: async (p) => { await session(p); await p.locator(".action-card", { hasText: "Session check" }).click(); },
      get_certificate: async (p) => { await home(p); await p.locator(".session-card.final", { hasText: "Your certificate" }).click(); },
      get_settings: async (p) => { await home(p).catch(() => {}); await p.locator("button[aria-label=Settings]").click(); },
      get_exercise: async (p) => { await session(p, "01"); await p.locator(".action-card", { hasText: "Exercise" }).click(); },
      get_final: async (p) => { await home(p); await p.locator(".session-card.final").first().click(); } }[cmd];
    await sweep(`error-${cmd}`, { fail: { [cmd]: `Couldn't read the course database: disk I/O error (${cmd}).` } }, async (p) => { await nav(p).catch(() => {}); await p.waitForTimeout(800); },
      { sizes: { phone: [390, 844], s1024: [1024, 768] }, themes: ["light"], zoom: false });
  }
} finally {
  await browser.close();
  server.close();
}
fs.writeFileSync(`${OUT}/issues.json`, JSON.stringify(issues, null, 1));
fs.writeFileSync(`${OUT}/bubbles.json`, JSON.stringify(bubbleIssues, null, 1));
// contact sheets: one per screen, every size and theme in a grid
const by = {};
for (const s of shots) (by[s.screen] ||= []).push(s.file);
const b2 = await chromium.launch();
const pg = await b2.newPage({ viewport: { width: 2400, height: 1200 } });
for (const [screen, files] of Object.entries(by)) {
  const html = `<body style="margin:0;background:#888;font:12px sans-serif;display:flex;flex-wrap:wrap;gap:6px;padding:6px;align-items:flex-start">${files.map((f) => `<figure style="margin:0;background:#fff"><img src="file://${OUT}/${f}" style="height:380px;display:block"><figcaption>${f}</figcaption></figure>`).join("")}</body>`;
  fs.writeFileSync(`${OUT}/sheet-${screen}.html`, html);
  await pg.goto(`file://${OUT}/sheet-${screen}.html`);
  await pg.waitForTimeout(300);
  await pg.screenshot({ path: `${OUT}/sheet-${screen}.png`, fullPage: true });
}
await b2.close();
const counts = {};
for (const i of issues) counts[i.check] = (counts[i.check] || 0) + 1;
console.log(`${shots.length} screenshots, ${issues.length} issues`, counts);
const bcounts = {};
for (const i of bubbleIssues) bcounts[i.check] = (bcounts[i.check] || 0) + 1;
console.log(`bubble pass: ${bubbleShows} bubbles shown, ${bubbleIssues.length} issues`, bcounts);
process.exitCode = issues.length || bubbleIssues.length ? 1 : 0;
