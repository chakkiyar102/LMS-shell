// The companion's executor (src-tauri/src/exec.js) against fixture pages that mimic a typical web app's
// form widgets (a fictional Brightline Notes "New note" form): a custom dropdown (aria-haspopup +
// listbox/option), a lookup with a search box, a date field, a rich text editor (contenteditable), a field
// error, an error toast and a modal. Served at an https address (routed, never fetched) that a frozen
// window.__lmsPracticeGate accepts, the way practice.rs executor_gate injects it before exec.js; a second
// page on another host checks that the executor doesn't arm there.
//   node tools/ui-check/exec-check.mjs [OUT_DIR]
import { chromium } from "playwright";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const EXEC = fs.readFileSync(path.join(ROOT, "src-tauri/src/exec.js"), "utf8");
const OUT = process.argv[2];
// the practice app the fixture pages pretend to be (fictional; routed, never fetched)
const HOST = "notes.brightline.example";
let fails = 0;
const ok = (name, cond, extra = "") => { console.log(`${cond ? "PASS" : "FAIL"} ${name}${extra ? " — " + extra : ""}`); if (!cond) fails++; };

// The widgets behave the way many web apps' do: the list renders on click, a pick writes the trigger's
// text, the lookup filters as you type, the editor is a contenteditable with no <label for>.
const FORM = `<!doctype html><html><head><title>New note</title><style>
  body { font: 14px system-ui; padding: 20px } .field { margin: 10px 0 } .lbl { display: block; font-weight: 600 }
  .trigger { border: 1px solid #999; padding: 6px; min-height: 18px; width: 240px; cursor: pointer }
  [role=listbox] { border: 1px solid #ccc; width: 240px } [role=option] { padding: 4px; cursor: pointer }
  .editor { border: 1px solid #999; min-height: 60px; width: 400px; padding: 6px } .error { color: #c00 }
  .modal { position: fixed; inset: 40px; background: #fff; border: 1px solid #333; padding: 16px; overflow: auto }
</style></head><body>
<div class="modal" role="dialog" aria-labelledby="mt"><h2 id="mt">New note</h2>
  <div class="field"><span class="lbl">Title *</span><input id="title" type="text"></div>
  <div class="field"><span class="lbl">Notebook *</span>
    <div id="nb" class="trigger" tabindex="0" aria-haspopup="listbox" aria-expanded="false" aria-controls="nb-list"><span class="placeholder">Select</span></div>
    <div class="error" id="nb-err">Value is required</div>
  </div>
  <div class="field"><span class="lbl">Assignee *</span>
    <div id="dept" class="trigger" tabindex="0" aria-haspopup="listbox" aria-expanded="false" aria-controls="dept-list"><span class="placeholder">Search</span></div>
  </div>
  <div class="field"><span class="lbl">Due date</span><input id="end" type="date"></div>
  <div class="field"><span class="lbl">Body</span><div class="editor" contenteditable="true"></div></div>
  <button id="submit">Submit</button>
</div>
<script>
  const OPTIONS = { nb: ["Personal", "Garden day", "Reading list"], dept: ["Alex Moreno", "Priya Shah", "Sam Lee", "Taylor Brooks"] };
  for (const id of ["nb", "dept"]) {
    const t = document.getElementById(id);
    t.addEventListener("click", () => {
      if (t.getAttribute("aria-expanded") === "true") return;
      t.setAttribute("aria-expanded", "true");
      const box = document.createElement("div"); box.id = id + "-list"; box.setAttribute("role", "listbox");
      if (id === "dept") { const s = document.createElement("input"); s.type = "text"; s.placeholder = "Search"; box.append(s);
        s.addEventListener("input", () => render(s.value)); setTimeout(() => s.focus(), 0); }
      const render = (q = "") => { box.querySelectorAll("[role=option]").forEach((o) => o.remove());
        // a lookup shows only its first two rows until you search
        const list = OPTIONS[id].filter((o) => o.toLowerCase().includes(q.toLowerCase())).slice(0, id === "dept" && !q ? 2 : 99);
        for (const o of list) { const d = document.createElement("div"); d.setAttribute("role", "option"); d.textContent = o;
          d.addEventListener("click", () => { t.innerHTML = "<span>" + o + "</span>"; t.setAttribute("aria-expanded", "false"); box.remove();
            if (id === "nb") document.getElementById("nb-err").remove(); }); box.append(d); } };
      render(); t.after(box);
    });
  }
  document.getElementById("submit").addEventListener("click", () => { const d = document.createElement("div"); d.className = "toast"; d.setAttribute("role", "alert"); d.textContent = "Could not submit: fill in the required fields"; document.body.append(d); });
</script></body></html>`;

// A note list with two rows checked, a password field, and a Delete that opens a confirmation.
const LIST = `<!doctype html><html><head><title>Garden day · Notes</title><style>
  body { font: 14px system-ui; padding: 20px } .row { display: flex; gap: 8px; padding: 4px }
  .modal { position: fixed; inset: 80px; background: #fff; border: 1px solid #333; padding: 16px }
</style></head><body>
<h1>Notes</h1>
<div role="grid">
  <div class="row" role="row"><input type="checkbox" aria-label="Select NOTE-1" checked><span>NOTE-1</span></div>
  <div class="row" role="row"><input type="checkbox" aria-label="Select NOTE-2" checked><span>NOTE-2</span></div>
  <div class="row" role="row"><input type="checkbox" aria-label="Select NOTE-3"><span>NOTE-3</span></div>
</div>
<button id="del">Delete record</button>
<label for="pw">Password</label><input id="pw" type="password" value="hunter2">
<script>
  document.getElementById("del").addEventListener("click", () => {
    const m = document.createElement("div"); m.className = "modal"; m.setAttribute("role", "alertdialog"); m.setAttribute("aria-labelledby", "dt");
    m.innerHTML = '<h2 id="dt">Delete record</h2><p>Are you sure you want to delete NOTE-1? This action cannot be undone.</p><button>Cancel</button> <button>Yes, delete</button>';
    document.body.append(m);
  });
</script></body></html>`;

const browser = await chromium.launch();
try {
  const ctx = await browser.newContext({ viewport: { width: 900, height: 900 } });
  // the gate practice.rs executor_gate defines before exec.js: frozen, accepts the practice host only
  await ctx.addInitScript(`Object.defineProperty(window, '__lmsPracticeGate', { value: Object.freeze({ hosts: ["${HOST}"], suffix: "",
    ok(l) { const h = String(l.hostname).toLowerCase().replace(/\\.$/, ''); return l.protocol === 'https:' && this.hosts.includes(h); } }) });`);
  await ctx.addInitScript(EXEC);
  await ctx.route(`https://${HOST}/**`, (r) => r.fulfill({ contentType: "text/html", body: r.request().url().includes("/my-items") ? LIST : FORM }));
  await ctx.route("https://accounts.example.com/**", (r) => r.fulfill({ contentType: "text/html", body: "<title>Sign in</title><input type=password>" }));
  const p = await ctx.newPage();
  await p.goto(`https://${HOST}/form`);
  const x = (call) => p.evaluate((c) => eval(`window.__lmsExec.${c}`), call);
  const refOf = async (re) => { const s = await x("snapshot()"); const m = s.split("\n").find((l) => /^\[/.test(l) && re.test(l)); return m ? Number(m.match(/^\[(\d+)\]/)[1]) : -1; };

  let s = await x("snapshot()");
  ok("E1 the custom dropdown is a labelled combobox, empty", /combobox "Notebook" \(empty, collapsed\)/.test(s), s.split("\n").find((l) => /Notebook/.test(l)));
  ok("E2 the lookup is a labelled combobox", /combobox "Assignee"/.test(s));
  ok("E3 the rich text editor is a labelled textbox", /textbox "Body" \(empty, rich text\)/.test(s), s.split("\n").find((l) => /rich|Justification/.test(l)));
  ok("E4 the date field says its format", /textbox "Due date" \(empty, date, YYYY-MM-DD\)/.test(s));
  ok("E5 signals: the field error with its label, and the dialog", JSON.stringify(await x("signals()")) === JSON.stringify({ validation: "Notebook: Value is required", toast: "", modal: "New note" }), JSON.stringify(await x("signals()")));

  // custom dropdown: open shows the options, pick by text, the value sticks and the error goes
  let nb = await refOf(/combobox "Notebook"/);
  const opened = await x(`open(${nb})`);
  ok("E6 open lists the options", /Personal \| Garden day \| Reading list/.test(opened), opened);
  s = await x("snapshot()");
  ok("E7 an open dropdown shows its options in the outline", /Notebook" \(empty, expanded, options: Personal \| Garden day \| Reading list\)/.test(s), s.split("\n").find((l) => /combobox "Notebook/.test(l)));
  nb = await refOf(/combobox "Notebook"/);
  ok("E8 pick by visible text", (await x(`pick("garden")`)) === 'picked "Garden day"');
  nb = await refOf(/combobox "Notebook"/);
  ok("E9 the value stuck", (await x(`valueOf(${nb})`)) === "Garden day");
  ok("E10 the field error cleared", (await x("signals()")).validation === "");

  // lookup: the option isn't in the first rows, so pick searches, then picks
  const dept = await refOf(/combobox "Assignee/);
  await x(`open(${dept})`);
  await p.waitForTimeout(50);
  ok("E11 a lookup option not yet listed is searched for", (await x(`pick("Taylor")`)) === "searching");
  ok("E12 then picked", (await x(`pick("Taylor")`)) === 'picked "Taylor Brooks"');
  ok("E13 and kept", (await x(`valueOf(${await refOf(/combobox "Assignee/)})`)) === "Taylor Brooks");
  let err = ""; try { await x(`open(${await refOf(/combobox "Notebook"/)})`); await x(`pick("Imaginary")`); } catch (e) { err = String(e); }
  ok("E14 a missing option says what is there", /no option "Imaginary".*Personal, Garden day, Reading list/.test(err), err.slice(0, 160));
  await p.keyboard.press("Escape");

  // date and rich text
  await x(`type(${await refOf(/textbox "Due date"/)}, "October 30, 2026", false)`);
  ok("E15 a date is typed in the field's format", (await p.inputValue("#end")) === "2026-10-30", await p.inputValue("#end"));
  await x(`type(${await refOf(/textbox "Body"/)}, "Bring gloves and seed trays.", false)`);
  ok("E16 the rich text editor takes the text", (await p.locator(".editor").innerText()).trim() === "Bring gloves and seed trays.");
  s = await x("snapshot()");
  ok("E17 the outline names fields by label, never by their content", /textbox "Body" \(value="Bring gloves/.test(s) && !/textbox "Bring gloves/.test(s));

  // toast; signals carry labels and messages, not values
  await x(`click(${await refOf(/button "Submit"/)})`);
  const sig = await x("signals()");
  ok("E18 an error toast is a signal", sig.toast === "Could not submit: fill in the required fields", JSON.stringify(sig));
  ok("E19 no signal carries what was typed", !/gloves|Taylor|Garden day/.test(JSON.stringify(sig)), JSON.stringify(sig));
  if (OUT) { fs.mkdirSync(OUT, { recursive: true }); await p.screenshot({ path: `${OUT}/exec-fixture.png` }); }

  // What the action policy (src-tauri/src/guard.rs) reads: a record list with two checked rows, a
  // password field, and a delete confirmation dialog. guard.rs's unit tests judge these shapes.
  const r = await ctx.newPage();
  await r.goto(`https://${HOST}/notebooks/garden-day/my-items#/list`);
  const y = (call) => r.evaluate((c) => eval(`window.__lmsExec.${c}`), call);
  const refIn = async (re) => { const s = await y("snapshot()"); const m = s.split("\n").find((l) => /^\[/.test(l) && re.test(l)); return m ? Number(m.match(/^\[(\d+)\]/)[1]) : -1; };
  let d = await y(`describe(${await refIn(/button "Delete record"/)})`);
  ok("E21 describe gives the label, the URL path and no dialog outside one", d.name === "Delete record" && d.path === "/notebooks/garden-day/my-items#/list" && d.dialog === "", JSON.stringify(d));
  ok("E22 describe counts the checked rows (a bulk selection)", d.selected === 2, String(d.selected));
  d = await y(`describe(${await refIn(/textbox "Password"/)})`);
  ok("E23 a password field says so in its hints, and never its value", /password/i.test(d.field) && !/hunter2/.test(JSON.stringify(d)), JSON.stringify(d));
  let typeErr = ""; try { await y(`type(${await refIn(/textbox "Password"/)}, "x", false)`); } catch (e) { typeErr = String(e); }
  ok("E24 the executor itself refuses to type into it", /never types into password/.test(typeErr), typeErr.slice(0, 120));
  await y(`click(${await refIn(/button "Delete record"/)})`);
  const yes = await refIn(/button "Yes, delete"/);
  d = await y(`describe(${yes})`);
  ok("E25 a button in the delete confirmation carries the dialog's title and warning", /^Delete record \| /.test(d.dialog) && /cannot be undone/.test(d.dialog), JSON.stringify(d.dialog));
  ok("E26 Cancel in the same dialog is described as Cancel", (await y(`describe(${await refIn(/button "Cancel"/)})`)).name === "Cancel");
  const swapped = await r.evaluate(() => { try { window.__lmsExec = { describe: () => ({ name: "Save" }) }; } catch (_) {} try { window.__lmsExec.describe = () => ({ name: "Save" }); } catch (_) {} return window.__lmsExec.describe(0).name; });
  ok("E27 page script can't replace the executor's describe()", swapped !== "Save", swapped);
  ok("E28 point() outlines a control without clicking it", /pointed at "Yes, delete"/.test(await y(`point(${yes})`)) && (await r.evaluate(() => document.querySelectorAll(".row").length)) === 3);
  if (OUT) await r.screenshot({ path: `${OUT}/exec-delete-dialog.png` });

  // the host gate: nothing on a sign-in page
  const q = await ctx.newPage();
  await q.goto("https://accounts.example.com/signin");
  ok("E20 the executor doesn't arm off the practice host", await q.evaluate(() => typeof window.__lmsExec) === "undefined");
  await ctx.close();
} finally {
  await browser.close();
}
console.log(fails ? `${fails} failed` : "all passed");
process.exit(fails ? 1 : 0);
