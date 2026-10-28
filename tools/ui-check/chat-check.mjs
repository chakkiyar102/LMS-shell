// Sidekick's chat log scrolls like a chat app: anchored to the bottom, right above the composer; a short
// conversation sits at the bottom too; new replies and streamed screen-helper steps keep it pinned while
// the learner is at the bottom; scrolled up to read, it stays put and offers a "New messages" pill.
// Both surfaces (the session's Sidekick column and the sidekick), Coach and Ask Sidekick, in the real UI build
// with the Tauri bridge stubbed. Usage: node chat-check.mjs OUT_DIR URL [only-letter]
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
const calm = { sidekick_today: today, sidekick_coach_seen: JSON.stringify(MARKS), sidekick_coach_last: String(Date.now()),
  sidekick_snooze: JSON.stringify({ "c/first_launch": Date.now() + 1e9, "c/back_after_break": Date.now() + 1e9, "c/studied_today": Date.now() + 1e9 }) };
const page = (b, o = {}) => open(b, { S, URL, allDone: false, days: 0, today: true, viewport: { width: 1440, height: 860 }, ...o, storage: { ...calm, ...(o.storage || {}) } });
// replies the checks control: window.__fake[cmd](args) answers before the stub's fixed table
const fakes = (p) => p.evaluate(() => {
  const base = window.__TAURI__.core.invoke;
  window.__fake = {};
  window.__TAURI__.core.invoke = async (cmd, args) => (cmd in window.__fake ? (window.__calls.push([cmd, args]), window.__fake[cmd](args)) : base(cmd, args));
  const para = (i) => `Reply ${i}: open the **form builder**, add a field, then check its settings. Every field has a name, a type and a few options worth a look before you move on.`;
  window.__long = (n, from = 0) => Array.from({ length: n }, (_, i) => ({ role: i % 2 ? "user" : "assistant", content: i % 2 ? `Question ${from + i}` : para(from + i) }));
  window.__coach = (messages, stage = 0) => ({ title: "Coaching session", stages: ["Plan the form", "Build the fields", "Check it works"], stage, hints_used: 0, messages });
});
// the log's geometry: the last item's bottom against the visible log and the composer, and the pill
const geo = (p, root) => p.evaluate((root) => {
  const pane = document.querySelector(root), b = pane.querySelector(".pane-body");
  const items = [...b.querySelectorAll(".msg, .screen-step, .screen-check, .notice, .empty, .sidekick-chips")].filter((e) => e.getBoundingClientRect().height > 0);
  const first = items[0]?.getBoundingClientRect(), last = items[items.length - 1]?.getBoundingClientRect(), br = b.getBoundingClientRect();
  const pill = pane.querySelector(".pane-new");
  const pr = pill?.getBoundingClientRect(), comp = pane.querySelector(".pane-compose").getBoundingClientRect();
  return { top: Math.round(br.top), bottom: Math.round(br.bottom), firstTop: Math.round(first?.top ?? 0), lastBottom: Math.round(last?.bottom ?? 0), compTop: Math.round(comp.top),
    scrollTop: Math.round(b.scrollTop), gap: Math.round(b.scrollHeight - b.scrollTop - b.clientHeight), scrolls: b.scrollHeight > b.clientHeight + 1,
    pill: !!pill && pr.height > 0 && getComputedStyle(pill).visibility !== "hidden", pillBottom: Math.round(pr?.bottom ?? 0), n: items.length };
}, root);
// pinned: scrolled to the end, the newest item fully in view and right at the log's bottom edge (above the composer)
const pinned = (g) => g.gap <= 2 && g.lastBottom <= g.bottom + 1 && g.lastBottom >= g.bottom - 40 && g.bottom <= g.compTop + 1;
const settle = (p, ms = 350) => p.waitForTimeout(ms);
const session = async (p) => {
  await p.locator("[data-sidekick-anchor=continue]").click();
  await p.locator(".practice .btn.primary").click();
  await p.locator(".col-sidekick .pane-tools").waitFor({ timeout: 5000 });
  await settle(p, 800);
};
const send = async (p, root, text) => { await p.locator(`${root} .pane-compose textarea`).fill(text); await p.locator(`${root} .pane-compose .btn.primary`).click(); };
const shot = (p, name) => p.screenshot({ path: `${S}/chat-${name}.png` });
const browser = await chromium.launch();
try {
  for (const theme of ["light", "dark"]) {
    if (run("A")) {
      // A: the session's Sidekick column, Coach mode
      const R = ".col-sidekick";
      const { ctx, p } = await page(browser, { theme });
      await p.waitForTimeout(1000);
      await fakes(p);
      await p.evaluate(() => { window.__fake.coach_turn = () => window.__coach([{ role: "assistant", content: "Start by opening the form builder." }]); });
      await session(p);
      let g = await geo(p, R);
      ok(`A1 ${theme}: a short conversation sits at the bottom, above the composer, not at the top`, g.n > 0 && !g.scrolls && pinned(g) && g.firstTop > g.top + 60, JSON.stringify(g));
      await shot(p, `A-short-${theme}`);
      // a long conversation: the learner sends, Sidekick answers; their message and the reply land at the bottom
      await p.evaluate(() => { window.__hist = window.__long(14); window.__fake.coach_turn = (a) => new Promise((r) => setTimeout(() => {
        window.__hist = [...window.__hist, { role: "user", content: a.message || "" }, { role: "assistant", content: "Good. Now add a **Quantity** number field and set its minimum to 1. Then preview the form." }];
        r(window.__coach(window.__hist));
      }, 300)); });
      await send(p, R, "I added the form. What next?");
      await settle(p, 100);
      g = await geo(p, R);
      ok(`A2 ${theme}: while Sidekick thinks, the typing dots sit pinned at the bottom`, pinned(g), JSON.stringify(g));
      await settle(p, 600);
      g = await geo(p, R);
      ok(`A3 ${theme}: after sending, the newest message's bottom is in view, just above the composer`, g.scrolls && pinned(g), JSON.stringify(g));
      await shot(p, `A-long-${theme}`);
      // late growth (a reply that lays out after the first frame) keeps it pinned
      await p.evaluate(() => { const m = [...document.querySelectorAll(".col-sidekick .pane-body .msg")].pop(); m.style.paddingBottom = "160px"; });
      await settle(p);
      g = await geo(p, R);
      ok(`A4 ${theme}: content that grows after it renders keeps the log pinned`, pinned(g), JSON.stringify(g));
      await p.evaluate(() => { const m = [...document.querySelectorAll(".col-sidekick .pane-body .msg")].pop(); m.style.paddingBottom = ""; });
      await settle(p);
      // streamed screen-helper step lines, one at a time: each stays pinned
      let streamed = true, worst = null;
      for (let i = 0; i < 14; i++) {
        await p.evaluate((i) => window.__emit("screen-agent", { kind: "step", text: `Step ${i + 1}: clicked **Add field** and typed a name` }), i);
        await settle(p, 60);
        const s = await geo(p, R);
        if (!pinned(s)) { streamed = false; worst = s; }
      }
      ok(`A5 ${theme}: streamed step lines keep the log pinned`, streamed, JSON.stringify(worst));
      // scrolled up to read: new lines don't yank, a "New messages" pill shows above the composer
      ok(`A6 ${theme}: no pill while at the bottom`, !(await geo(p, R)).pill);
      await p.locator(`${R} .pane-body`).hover();
      await p.mouse.wheel(0, -500);
      await settle(p);
      const up = await geo(p, R);
      await p.evaluate(() => { for (let i = 0; i < 4; i++) window.__emit("screen-agent", { kind: "step", text: `Later step ${i + 1}` }); });
      await settle(p);
      g = await geo(p, R);
      ok(`A7 ${theme}: scrolled up, new lines don't move the reader`, up.gap > 100 && Math.abs(g.scrollTop - up.scrollTop) <= 2, JSON.stringify({ up, g }));
      ok(`A8 ${theme}: and a New messages pill shows just above the composer`, g.pill && g.pillBottom <= g.compTop && g.pillBottom >= g.bottom - 80, JSON.stringify(g));
      await shot(p, `A-pill-${theme}`);
      if (await p.locator(`${R} .pane-new`).count()) await p.locator(`${R} .pane-new`).click({ timeout: 2000 }).catch(() => {});
      await settle(p, 700);
      g = await geo(p, R);
      ok(`A9 ${theme}: the pill jumps to the latest and goes away`, pinned(g) && !g.pill, JSON.stringify(g));
      // a re-render of the whole list (a stage change, then Sidekick opening the stage) while scrolled up: no jump to the top
      await p.locator(`${R} .pane-body`).hover();
      await p.mouse.wheel(0, -300);
      await settle(p);
      const mid = await geo(p, R);
      await p.evaluate(() => {
        window.__hist = [...window.__hist, { role: "assistant", content: "On to stage 2: **Build the fields**." }];
        window.__fake.set_coach_stage = () => window.__coach(window.__hist, 1);
        window.__fake.coach_turn = () => new Promise((r) => setTimeout(() => r(window.__coach([...window.__hist, { role: "assistant", content: "Add the fields the brief lists, one at a time." }], 1)), 150));
      });
      await p.locator(`${R} .stages li:nth-child(2) button`).click();
      await settle(p, 700);
      g = await geo(p, R);
      ok(`A10 ${theme}: a re-render while scrolled up keeps the reader's place (no jump to the top)`, g.scrollTop > 50 && Math.abs(g.scrollTop - mid.scrollTop) <= 2 && g.pill, JSON.stringify({ mid, g }));
      // switching to Ask Sidekick and back lands on the newest message
      await p.evaluate(() => { window.__fake.get_messages = () => window.__long(12, 100); window.__fake.ask_tutor = () => new Promise((r) => setTimeout(() => r({ role: "assistant", content: "Short answer: yes, a **number** field takes a minimum and a maximum." }), 200)); });
      await p.locator(`${R} [data-help='coach.mode-ask']`).click();
      await settle(p, 600);
      g = await geo(p, R);
      ok(`A11 ${theme}: switching to Ask Sidekick lands on the newest message`, pinned(g) && !g.pill, JSON.stringify(g));
      await send(p, R, "Can a number field have a minimum?");
      await settle(p, 600);
      g = await geo(p, R);
      ok(`A12 ${theme}: Ask Sidekick: the question and its answer land at the bottom`, pinned(g) && /minimum and a maximum/.test(await p.locator(`${R} .pane-body .msg`).last().textContent()), JSON.stringify(g));
      await shot(p, `A-ask-${theme}`);
      await p.locator(`${R} [data-help='coach.mode-coach']`).click();
      await settle(p, 600);
      g = await geo(p, R);
      ok(`A13 ${theme}: and back to Coach lands on the newest message too`, pinned(g), JSON.stringify(g));
      await ctx.close();
    }
    if (run("B")) {
      // B: the sidekick on a lesson page, Ask Sidekick: a short then a long conversation
      const R = ".sidekick-side-pane";
      const { ctx, p } = await page(browser, { theme, storage: { sidekick_pane_mode: "ask" } });
      await p.waitForTimeout(1000);
      await fakes(p);
      await p.evaluate(() => { window.__fake.ask_tutor = () => new Promise((r) => setTimeout(() => r({ role: "assistant", content: "Here's the short answer: a form holds the fields, a process moves an item through steps." }), 200)); });
      await p.locator(".sidekick-tab").click();
      await p.locator(`${R} .pane-compose`).waitFor({ timeout: 3000 });
      await settle(p, 500);
      if (await p.locator(`${R} [data-help='coach.mode-ask'][aria-checked=false]`).count()) { await p.locator(`${R} [data-help='coach.mode-ask']`).click(); await settle(p, 400); }
      await send(p, R, "What's a form?");
      await settle(p, 600);
      let g = await geo(p, R);
      ok(`B1 ${theme}: sidekick, a short conversation sits at the bottom`, !g.scrolls && pinned(g) && g.firstTop > g.top + 60, JSON.stringify(g));
      await shot(p, `B-short-${theme}`);
      await p.evaluate(() => { window.__fake.ask_tutor = () => new Promise((r) => setTimeout(() => r({ role: "assistant", content: window.__long(1, 7)[0].content + "\n\n" + window.__long(1, 8)[0].content }), 200)); });
      for (let i = 0; i < 5; i++) { await send(p, R, `Follow-up question ${i + 1}?`); await settle(p, 450); }
      g = await geo(p, R);
      ok(`B2 ${theme}: sidekick, each answer lands at the bottom above the composer`, g.scrolls && pinned(g), JSON.stringify(g));
      await shot(p, `B-long-${theme}`);
      await p.locator(`${R} .pane-body`).hover();
      await p.mouse.wheel(0, -400);
      await settle(p);
      const up = await geo(p, R);
      // a chip or a slow answer arriving while the learner reads earlier answers
      await p.evaluate(() => { window.__fake.ask_tutor = () => new Promise((r) => setTimeout(() => r({ role: "assistant", content: "A late answer." }), 50)); });
      await p.evaluate(() => { const t = document.querySelector(".sidekick-side-pane .pane-compose textarea"); t.value = ""; });
      await send(p, R, "One more?");
      await settle(p, 500);
      g = await geo(p, R);
      ok(`B3 ${theme}: sending is the learner's own move, so it jumps to the latest even from above`, up.gap > 100 && pinned(g), JSON.stringify({ up, g }));
      await ctx.close();
    }
  }
} finally {
  await browser.close();
}
console.log(out.join("\n"));
const fails = out.filter((l) => l.startsWith("FAIL")).length;
console.log(`${out.length - fails}/${out.length} chat checks pass`);
if (fails) process.exit(1);
