// The tour harness's server and stubbed bridge, shared by tour.mjs and sweep.mjs.
// serve(DATA, DIST, PORT) serves the UI build and lesson media; open(browser, opts) gives a page whose
// invoke() answers from the tour_dump output (see tour.mjs for how to make it).
import fs from "node:fs";
import { execFileSync } from "node:child_process";
// The companion's display name (shared/src/brand.rs companion_name!()), put in for {companion} the way lib.rs named() does
const BRAND = fs.readFileSync(new URL("../../shared/src/brand.rs", import.meta.url), "utf8");
export const COMPANION = /macro_rules! companion_name \{ \(\) => \{ "([^"]*)" \} \}/.exec(BRAND)?.[1] || "Guide";
// the companion's hover help and tooltips (content/sidekick-help.yaml), as get_sidekick_help returns them
const HELP = JSON.parse(execFileSync("/usr/bin/python3", ["-c", "import yaml,json,sys;print(json.dumps(yaml.safe_load(open(sys.argv[1]).read().replace('{companion}',sys.argv[2]))))",
  new URL("../../content/sidekick-help.yaml", import.meta.url).pathname, COMPANION], { encoding: "utf8" }));
import http from "node:http";
import path from "node:path";

export async function serve(DATA, DIST, PORT) {
const BASE = `http://127.0.0.1:${PORT}/`;
const raw = JSON.parse(fs.readFileSync(DATA, "utf8"));
const MEDIA_ROOT = raw.content_root;
// asset://localhost/<encoded abs path> → this server's /media/<encoded path under content/courses>
const data = JSON.parse(JSON.stringify(raw).split(`asset://localhost/${encodeURIComponent(MEDIA_ROOT)}`).join(`${BASE}media`));
delete data.content_root;
// The course tour as get_app_tour returns it: null while content/courses/_shared/app-tour.yaml is version 0
// (no tour), and then the shots that need a tour video are skipped.
const TOUR_NOTE = "";

const TYPES = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".wasm": "application/wasm", ".css": "text/css",
  ".png": "image/png", ".jpg": "image/jpeg", ".svg": "image/svg+xml", ".mp4": "video/mp4", ".vtt": "text/vtt", ".woff2": "font/woff2", ".json": "application/json", ".gif": "image/gif" };
const server = http.createServer((req, res) => {
  const u = decodeURIComponent(new URL(req.url, BASE).pathname);
  const file = u.startsWith("/media/") ? path.join(MEDIA_ROOT, u.slice(7)) : path.join(DIST, u === "/" ? "index.html" : u);
  const root = u.startsWith("/media/") ? MEDIA_ROOT : DIST;
  if (!file.startsWith(root) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) { res.writeHead(404); return res.end(); }
  const size = fs.statSync(file).size;
  const type = TYPES[path.extname(file)] || "application/octet-stream";
  const m = /bytes=(\d*)-(\d*)/.exec(req.headers.range || "");
  if (m) {
    const start = m[1] ? Number(m[1]) : size - Number(m[2]);
    const end = m[1] && m[2] ? Math.min(Number(m[2]), size - 1) : size - 1;
    res.writeHead(206, { "Content-Type": type, "Accept-Ranges": "bytes", "Content-Range": `bytes ${start}-${end}/${size}`, "Content-Length": end - start + 1 });
    return fs.createReadStream(file, { start, end }).pipe(res);
  }
  res.writeHead(200, { "Content-Type": type, "Accept-Ranges": "bytes", "Content-Length": size });
  fs.createReadStream(file).pipe(res);
});
await new Promise((r) => server.listen(PORT, "127.0.0.1", r));

const JORDAN = { name: "Jordan Hale", email: "jordan.hale@brightline.example" };
const COURSE = Object.keys(data.main.by_course)[0];
// shared/data lists, as shared::email embeds them
const domainList = (f) => fs.readFileSync(new URL(`../../shared/data/${f}`, import.meta.url), "utf8")
  .split("\n").map((l) => l.trim()).filter((l) => l && !l.startsWith("#"));
const DOMAINS = { disposable: domainList("disposable_domains.txt"), allowed: domainList("allowed_domains.txt") };

/** A page whose invoke() answers from the dump. scenario: "main" | "fresh"; profile null = onboarding. */
/** tour: "seen" (default: the tour button is there, nothing opens) | "due" (first run: the player opens) | "none" (no tour file). */
/** code: an access code is saved on this device (has_access_code), so gated courses are open; without one they are locked for every role. */
/** role: "learner" | "partner" | "staff" (Jordan is a learner by default). course: defaults to the first course in the dump. */
/** viewport: {width,height}; who: a profile to use instead of Jordan's; fail: {command: error text} for commands that refuse; */
/** wait: ms to settle after load; quiz: "pass" | "miss" | "mixed" (how submit_quiz grades); news: a WhatsNew to show; extra: {command: value} overrides. */
async function open(browser, { scenario = "main", role = "learner", profile = true, course = COURSE, theme = "light", storage = {}, dpr = 1, tour = "seen", code = false, viewport = { width: 1440, height: 900 }, who = null, fail = {}, wait = 1500, quiz = "pass", news = null, extra = {} } = {}) {
  const ctx = await browser.newContext({ viewport, colorScheme: theme, deviceScaleFactor: dpr });
  const p = await ctx.newPage();
  p.on("pageerror", (e) => console.error("pageerror:", e.message));
  const sc = data[scenario];
  const appTour = tour === "none" || !data.app_tour ? null : { ...data.app_tour, seen_version: tour === "due" ? 0 : data.app_tour.version };
  const init = { domains: DOMAINS, code, appTour, sc, nudges: data.nudges, help: HELP, unlockError: data.unlock_error, settings: data.settings, role, profile: profile ? { ...(who || JORDAN), role } : null, fail, quiz, news, extra,
    storage: { theme, course, sidekick_coach_last: String(Date.now()), ...storage } };
  await p.addInitScript((d) => {
    for (const [k, v] of Object.entries(d.storage)) localStorage.setItem(k, v);
    window.__calls = [];
    let profile = d.profile;
    let hasCode = d.code;
    // same rule as lib.rs partner_open: a stored access code, whatever the role
    const open = () => hasCode;
    const locked = (id) => !open() && d.sc.courses_locked.find((c) => c.id === id)?.locked;
    const course = (a) => {
      if (locked(a.course_id)) throw `${d.sc.by_course[a.course_id].course.title} needs an access code.`;
      const c = d.sc.by_course[a.course_id];
      if (!c) throw `no course ${a.course_id}`;
      return c;
    };
    // same rules as shared::clean_profile and shared::email::check_domain (no DNS: the tour has no backend)
    const disposable = new Set(d.domains.disposable), allowed = new Set(d.domains.allowed);
    const TYPOS = { "gmial.com": "gmail.com", "gmai.com": "gmail.com", "gmal.com": "gmail.com", "gamil.com": "gmail.com", "gnail.com": "gmail.com",
      "gmaill.com": "gmail.com", "gmail.con": "gmail.com", "gmail.cm": "gmail.com", "gmail.cmo": "gmail.com", "hotmial.com": "hotmail.com",
      "hotmal.com": "hotmail.com", "hotmai.com": "hotmail.com", "hotamil.com": "hotmail.com", "hotmail.con": "hotmail.com", "yaho.com": "yahoo.com",
      "yahooo.com": "yahoo.com", "yhoo.com": "yahoo.com", "yahoo.con": "yahoo.com", "outlok.com": "outlook.com", "outllok.com": "outlook.com",
      "outloo.com": "outlook.com", "outlook.con": "outlook.com", "iclod.com": "icloud.com", "icoud.com": "icloud.com" };
    const parents = (dom) => { const out = []; for (let x = dom; x.includes("."); x = x.slice(x.indexOf(".") + 1)) out.push(x); return out; };
    const clean = (p) => {
      const name = p.name.trim(), email = p.email.trim().toLowerCase();
      if (!name) throw "Enter your name.";
      let [local, domain = ""] = email.split(/@(.*)/s);
      domain = domain.trim().replace(/\.+$/, "");
      if (!local || !domain.includes(".") || domain.includes("@") || /\s/.test(email) || domain.split(".").some((l) => !l)) throw "Enter a valid work email.";
      // shared::staff_rule: brand::STAFF_EMAIL_DOMAIN is None in the shell, so no role has a domain rule
      if (/[^\x00-\x7f]/.test(domain)) throw "Use the plain-letter form of your email domain. Domains with accented or non-Latin letters aren't supported.";
      if (!parents(domain).some((x) => allowed.has(x))) {
        if (TYPOS[domain]) throw `Did you mean ${local}@${TYPOS[domain]}?`;
        if (parents(domain).some((x) => disposable.has(x))) throw "Use your work email. Temporary or disposable addresses aren't accepted.";
      }
      return { name, email: `${local}@${domain}`, role: p.role };
    };
    const h = {
      get_profile: () => profile,
      save_profile: (a) => { profile = clean(a.profile); return null; },
      get_courses: () => (open() ? d.sc.courses_open : d.sc.courses_locked),
      has_access_code: () => hasCode,
      // lib.rs forget_code: drop the code; gated courses lock for every role
      remove_access_code: () => { hasCode = false; return null; },
      // lib.rs reset_app: the learner's data and keys are gone (the UI then clears storage and reloads)
      reset_app: () => { profile = null; hasCode = false; return null; },
      get_course: (a) => course(a).course,
      get_stats: (a) => course(a).stats,
      get_lesson: (a) => course(a).lessons[a.lesson_id] ?? (() => { throw `no lesson ${a.lesson_id}`; })(),
      get_check: (a) => course(a).checks[a.check_id] ?? (() => { throw `no check ${a.check_id}`; })(),
      get_final: (a) => course(a).final,
      get_exercise: (a) => course(a).exercises[a.exercise_id],
      get_certificate: (a) => course(a).certificate,
      get_settings: () => d.settings,
      get_sidekick_nudges: () => d.nudges,
      get_sidekick_help: () => d.help,
      get_messages: () => [],
      get_whats_new: () => d.news,
      mark_whats_new_seen: () => null,
      get_app_tour: () => d.appTour,
      mark_app_tour_seen: () => null,
      check_update: () => null,
      app_version: () => "tour",
      save_progress: () => null,
      // graded by the quiz option: every answer right, or every answer wrong with feedback
      // "mixed": every third answer wrong (the second, fifth, ...), the rest right, passed at 70%
      submit_quiz: (a) => {
        const right = (a.answers || []).map((_, i) => d.quiz === "pass" || (d.quiz === "mixed" && i % 3 !== 1));
        const score = right.length ? right.filter(Boolean).length / right.length : 0;
        return { score, passed: score >= 0.7, results: right.map((ok) => ({ score: ok ? 1 : 0, feedback: [ok ? "Right: that's the one." : "Not this one. Look again at what the lesson said about it."] })) };
      },
      ask_tutor: (a) => ({ role: "assistant", content: `Here's the short answer to "${a.question ?? ""}".` }),
      get_coach: () => ({ title: "Coaching session", stages: ["Plan the notebook", "Write the notes", "Share it"], stage: 0, hints_used: 0, messages: [] }),
      coach_turn: () => ({ title: "Coaching session", stages: ["Plan the notebook", "Write the notes", "Share it"], stage: 0, hints_used: 1, messages: [{ role: "user", content: "How do I start?" }, { role: "assistant", content: "Create a notebook called Garden day, then add a note titled Volunteer list." }] }),
      submit_exercise: (a) => ({ ...course(a).exercises[a.exercise_id], last_submission: a.submission ?? a.text ?? "", last_review: "Advisory: your write-up covers the required fields.", walkthrough_html: "<p>Reference walkthrough.</p>" }),
      open_practice: () => null, place_practice: () => null, close_practice: () => null,
      list_models: () => ["gemma2:9b", "llama3.1:8b"],
      // no real code is used here: refuse with the text access::check gives a wrong code
      unlock_courses: () => { throw d.unlockError; },
    };
    window.__TAURI__ = {
      core: { invoke: async (cmd, args) => { window.__calls.push([cmd, args]); if (cmd in d.fail) throw d.fail[cmd]; if (cmd in d.extra) return d.extra[cmd]; return cmd in h ? h[cmd](args || {}) : null; } },
      // listeners are kept so a check can play a backend event: window.__emit("practice-nav", {...})
      event: { listen: async (name, cb) => { (window.__listeners[name] ||= []).push(cb); return () => {}; } },
    };
    window.__listeners = {};
    window.__emit = (name, payload) => (window.__listeners[name] || []).forEach((cb) => cb({ event: name, payload }));
    window.__srcs = [];
    new MutationObserver(() => {
      const s = document.querySelector(".sidekick-companion img")?.getAttribute("src");
      if (s && window.__srcs[window.__srcs.length - 1] !== s) window.__srcs.push(s);
    }).observe(document, { subtree: true, childList: true, attributes: true, attributeFilter: ["src"] });
  }, init);
  await p.goto(BASE);
  await p.waitForTimeout(wait);
  return { ctx, p };
}
return { data, BASE, TOUR_NOTE, COURSE, server, open };
}
