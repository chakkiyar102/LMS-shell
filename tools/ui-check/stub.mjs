// The real UI build in Chromium with the Tauri bridge stubbed: shared by the companion checks.
import fs from "node:fs";
const L = (id, done) => ({ id, number: id, title: `Lesson ${id}`, duration_min: 6, has_video: false, completed: done });
function course(allDone) {
  return { id: "c", title: "Getting started with Brightline Notes", summary: "S", final_minutes: 60, final_blurb: "F", sessions: [
    { id: "s1", phase: null, title: "Your first notes", outcome: "o", lessons: [L("1.1", true), L("1.2", allDone)], check: "s1-check", exercise: "s1-ex", check_best: null },
    { id: "s2", phase: null, title: "Find, share and keep", outcome: "o", lessons: [L("2.1", allDone), L("2.2", allDone)], check: "s2-check", exercise: "s2-ex", check_best: null },
  ] };
}
export async function page(browser, { S, URL, viewport = { width: 1280, height: 840 }, storage: extraStorage = {}, allDone, days, today, reduce = false, theme = "light", clock = false, settings = { provider: { kind: "ollama", host: "http://127.0.0.1:11434", model: "gemma2:9b" }, has_api_key: false, practice_url: "" }, extra = {}, stats = {} }) {
  const storage = { sidekick_coach_last: String(Date.now()), ...extraStorage };
  const nudges = JSON.parse(fs.readFileSync(`${S}/nudges.json`, "utf8"));
  const help = JSON.parse(fs.readFileSync(`${S}/help.json`, "utf8"));
  const ctx = await browser.newContext({ viewport, colorScheme: theme, reducedMotion: reduce ? "reduce" : "no-preference" });
  const p = await ctx.newPage();
  p.on("pageerror", (e) => console.error("pageerror:", e.message));
  // debug builds warn when a signal is read outside a reactive context (a get() that should be get_untracked())
  p.on("console", (m) => { if (m.type() === "warning" && /you access/.test(m.text())) console.error("reactive read outside a reactive context:", m.text().slice(0, 160)); });
  const data = { help, course: course(allDone), stats: { streak: 1, best_streak: 1, active_today: today, days_since_active: days, lessons_done: allDone ? 4 : 1, lessons_total: 4, checks_passed: 0, checks_total: 2, final_passed: false, activity: [], ...stats }, nudges, settings, extra, storage,
    cert: { course: "C", name: "", missing: [{ label: "Session 1 check: not attempted", check: "s1-check" }, { label: "Final assessment: not attempted", check: null }], final_score: null, lessons_done: 4, lessons_total: 4, issued_on: null, id: null } };
  await p.addInitScript((d) => {
    window.__calls = [];
    const r = { get_profile: { name: "Jordan Hale", email: "jordan.hale@brightline.example", role: "learner" },
      get_courses: [{ id: "c", title: d.course.title, short: "Brightline basics", locked: false }], get_course: d.course, get_stats: d.stats,
      get_sidekick_nudges: d.nudges, get_sidekick_help: d.help, get_certificate: d.cert, get_whats_new: null, check_update: null, get_settings: d.settings,
      get_check: { id: "s1-check", title: "Session 1 check", pass_mark: 0.7, minutes: null, intro_html: "", sections: [] },
      get_lesson: { id: "1.2", number: "1.2", title: "Lesson 1.2", objective: "o", duration_min: 6, body_html: "<p>Body</p>", key_points: ["a"], gotchas: [], video: null, resume_at: 0, quick_check: [{ kind: "single", prompt: "Q?", options: ["Yes", "No"] }], help: [] },
      ...d.extra };
    for (const [k, v] of Object.entries(d.storage)) if (localStorage.getItem(k) === null) localStorage.setItem(k, v);
    const coach = { title: "Coaching session", stages: ["Plan the notebook", "Write the notes", "Share it"], stage: 0, hints_used: 0, messages: [] };
    r.get_coach ??= coach;
    r.coach_turn ??= { ...coach, messages: [{ role: "assistant", content: "Start by creating a notebook called Garden day." }] };
    r.set_coach_stage ??= { ...coach, stage: 1, messages: [{ role: "assistant", content: "Now write the first note." }] };
    r.get_messages ??= [];
    r.ask_tutor ??= { role: "assistant", content: "Here's the short answer." };
    // listeners are kept so a check can play a backend event: window.__emit("practice-signal", {...})
    window.__listeners = {};
    window.__emit = (name, payload) => (window.__listeners[name] || []).forEach((cb) => cb({ event: name, payload }));
    window.__TAURI__ = { core: { invoke: async (cmd, args) => { window.__calls.push([cmd, args]); if (cmd in r) return r[cmd]; return null; } },
      event: { listen: async (name, cb) => { (window.__listeners[name] ||= []).push(cb); return () => {}; } } };
    // every picture the companion sprite shows, in order
    window.__srcs = [];
    new MutationObserver(() => {
      const s = document.querySelector(".sidekick-companion img")?.getAttribute("src");
      if (s && window.__srcs[window.__srcs.length - 1] !== s) window.__srcs.push(s);
    }).observe(document, { subtree: true, childList: true, attributes: true, attributeFilter: ["src"] });
  }, data);
  if (clock) await p.clock.install();
  await p.goto(URL);
  return { ctx, p };
}
