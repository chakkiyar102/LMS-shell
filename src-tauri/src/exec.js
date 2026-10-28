// The companion's hands inside the practice pane. The agent loop runs in Rust (screen_agent.rs); this script only
// reads the screen as an accessibility-style outline and performs one action at a time on a numbered
// element. It holds no keys, calls nothing outside the page, and the page gets no access to the app.
(() => {
  if (!window.__lmsPracticeGate || !window.__lmsPracticeGate.ok(location)) return; // never on a sign-in page or any other site (gate: practice.rs)
  if (window.top !== window || window.__lmsExec) return;

  const TAG_ROLE = { A: 'link', BUTTON: 'button', SELECT: 'combobox', TEXTAREA: 'textbox', SUMMARY: 'button' };
  const ACTION_ROLES = new Set(['button', 'link', 'tab', 'menuitem', 'menuitemcheckbox', 'menuitemradio', 'option',
    'checkbox', 'radio', 'switch', 'combobox', 'textbox', 'searchbox', 'treeitem', 'slider', 'spinbutton', 'gridcell', 'row']);
  const MAX_REFS = 300;
  const MAX_CHARS = 15000;
  let refs = [];

  const clean = (s) => (s || '').replace(/\s+/g, ' ').trim();
  const cut = (s, n) => (s.length > n ? s.slice(0, n - 1) + '…' : s);

  const isShown = (el) => {
    const r = el.getBoundingClientRect();
    if (r.width < 2 || r.height < 2) return false;
    if (r.bottom < 0 || r.top > innerHeight || r.right < 0 || r.left > innerWidth) return false;
    const s = getComputedStyle(el);
    return s.visibility !== 'hidden' && s.display !== 'none' && Number(s.opacity) > 0.05;
  };

  const roleOf = (el) => {
    const explicit = el.getAttribute('role');
    // a custom dropdown's trigger (custom selects, lookups): it pops a listbox
    if (/^(listbox|true|menu)$/.test(el.getAttribute('aria-haspopup') || '') && (!explicit || explicit === 'button')) return 'combobox';
    if (explicit) return explicit;
    if (el.tagName === 'INPUT') {
      const t = (el.type || 'text').toLowerCase();
      if (t === 'hidden') return '';
      if (t === 'checkbox' || t === 'radio') return t;
      if (['button', 'submit', 'reset', 'image'].includes(t)) return 'button';
      return 'textbox';
    }
    if (el.isContentEditable && !el.parentElement?.isContentEditable) return 'textbox';
    if (TAG_ROLE[el.tagName] && (el.tagName !== 'A' || el.hasAttribute('href'))) return TAG_ROLE[el.tagName];
    // React and Angular apps attach handlers in JS, so a pointer cursor is the reliable "clickable" signal
    if (getComputedStyle(el).cursor === 'pointer' && getComputedStyle(el.parentElement || el).cursor !== 'pointer') return 'button';
    return '';
  };

  // The label a form shows for a field: many apps put it in a sibling above the control, not a <label for>.
  const LABELISH = 'label, legend, [class*="label" i], [class*="title" i], [class*="caption" i]';
  const fieldLabel = (el) => {
    const byId = (ids) => clean(ids.split(/\s+/).map((id) => el.ownerDocument.getElementById(id)?.innerText).join(' '));
    if (el.getAttribute('aria-labelledby')) { const t = byId(el.getAttribute('aria-labelledby')); if (t) return t; }
    const lab = el.id && el.ownerDocument.querySelector(`label[for="${CSS.escape(el.id)}"]`);
    if (lab) return clean(lab.innerText);
    // otherwise the nearest short text before the control in its field: a label-ish element first,
    // else plain text that isn't itself a control
    const CONTROL = 'input, textarea, select, button, [contenteditable], [aria-haspopup], [role], [tabindex]';
    for (let p = el.parentElement, depth = 0; p && depth < 4; p = p.parentElement, depth++) {
      const before = [];
      for (const c of p.children) { if (c.contains(el)) break; before.push(c); }
      const text = (c) => clean(c.innerText).replace(/\s*\*$/, '');
      const ok = (c) => { const t = text(c); return t && t.length <= 60 && !c.matches(CONTROL) && !c.querySelector(CONTROL); };
      const labelish = before.filter((c) => c.matches(LABELISH) && ok(c)).pop();
      if (labelish) return text(labelish);
      const plain = before.filter(ok).pop();
      if (plain) return text(plain);
    }
    return '';
  };

  const nameOf = (el) => {
    const byId = (ids) => clean(ids.split(/\s+/).map((id) => el.ownerDocument.getElementById(id)?.innerText).join(' '));
    const label = el.id && el.ownerDocument.querySelector(`label[for="${CSS.escape(el.id)}"]`);
    const role = roleOf(el);
    if (role === 'textbox' || role === 'combobox' || role === 'searchbox') {
      // never the text inside: for a field that is the learner's value
      return cut(clean(el.getAttribute('aria-label')) || fieldLabel(el) || clean(el.getAttribute('placeholder')) || clean(el.getAttribute('title')), 80);
    }
    return cut(clean(el.getAttribute('aria-label'))
      || (el.getAttribute('aria-labelledby') && byId(el.getAttribute('aria-labelledby')))
      || clean(label?.innerText)
      || (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' ? clean(el.closest('label')?.innerText) : '')
      || clean(el.getAttribute('placeholder'))
      || clean(el.innerText)
      || clean(el.getAttribute('title'))
      || clean(el.getAttribute('alt'))
      || clean(el.querySelector('[aria-label]')?.getAttribute('aria-label'))
      || clean(el.querySelector('img[alt]')?.getAttribute('alt')), 80);
  };

  // What a field currently holds, as shown: an input's value, a native select's choice, a rich text
  // editor's text, or the text a custom dropdown shows minus its placeholder.
  const valueText = (el) => {
    if (el.tagName === 'SELECT') return el.selectedOptions[0]?.text || '';
    if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') return el.value || '';
    if (el.isContentEditable) return el.innerText || '';
    const inner = el.querySelector('input:not([type=hidden])');
    if (inner && inner.value) return inner.value;
    const ph = clean(el.getAttribute('placeholder') || el.querySelector('[class*="placeholder" i]')?.innerText);
    const t = clean(el.innerText);
    return t === ph ? '' : t;
  };

  // The visible options of an open dropdown: its aria-controls / aria-owns listbox, else any listbox on screen.
  const openOptions = (el) => {
    const doc = el?.ownerDocument || document;
    const ids = el && (el.getAttribute('aria-controls') || el.getAttribute('aria-owns'));
    const boxes = [];
    if (ids) for (const id of ids.split(/\s+/)) { const b = doc.getElementById(id); if (b) boxes.push(b); }
    if (!boxes.length) boxes.push(...doc.querySelectorAll('[role="listbox"], [role="menu"], [class*="dropdown-menu" i], [class*="options" i]'));
    const opts = [];
    for (const b of boxes) {
      // plain <li> items count only inside something that is clearly a dropdown list
      const sel = b.matches('[role="listbox"], [role="menu"], [class*="dropdown" i]') ? '[role="option"], [role="menuitem"], li' : '[role="option"], [role="menuitem"]';
      for (const o of b.querySelectorAll(sel)) if (isShown(o) && clean(o.innerText) && !opts.includes(o)) opts.push(o);
    }
    return opts;
  };

  const stateOf = (el, role) => {
    const bits = [];
    if (role === 'textbox' || role === 'searchbox' || role === 'combobox') {
      const v = valueText(el);
      if (el.type === 'password') bits.push('password');
      else if (clean(v)) bits.push(`value="${cut(clean(v), 40)}"`);
      else bits.push('empty');
      if (el.type === 'date') bits.push('date, YYYY-MM-DD');
      if (el.isContentEditable) bits.push('rich text');
    }
    if (el.checked || el.getAttribute('aria-checked') === 'true') bits.push('checked');
    if (el.getAttribute('aria-selected') === 'true') bits.push('selected');
    if (el.getAttribute('aria-expanded')) bits.push(el.getAttribute('aria-expanded') === 'true' ? 'expanded' : 'collapsed');
    if (el.disabled || el.getAttribute('aria-disabled') === 'true') bits.push('disabled');
    if (role === 'combobox' && el.getAttribute('aria-expanded') === 'true') {
      const opts = openOptions(el).map((o) => clean(o.innerText)).filter(Boolean);
      if (opts.length) bits.push(`options: ${cut(opts.join(' | '), 200)}`);
    }
    return bits.length ? ' (' + bits.join(', ') + ')' : '';
  };

  // Elements in open shadow roots and same-origin frames are part of the page the learner sees.
  function* everyElement(root) {
    for (const el of root.querySelectorAll('*')) {
      yield el;
      if (el.shadowRoot) yield* everyElement(el.shadowRoot);
      if (el.tagName === 'IFRAME') { try { if (el.contentDocument) yield* everyElement(el.contentDocument); } catch (_) {} }
    }
  }

  const snapshot = () => {
    refs = [];
    const lines = [`Page: ${clean(document.title)} (${location.pathname})`];
    const listed = new Set();
    let size = lines[0].length;
    for (const el of everyElement(document)) {
      if (refs.length >= MAX_REFS || size > MAX_CHARS) { lines.push('… more elements below; scroll to see them'); break; }
      if (/^H[1-4]$/.test(el.tagName) && isShown(el)) {
        const h = cut(clean(el.innerText), 100);
        if (h) { lines.push(`# ${h}`); size += h.length; }
        continue;
      }
      const role = roleOf(el);
      if (!ACTION_ROLES.has(role)) {
        // plain text the learner can read (field names, statuses, table cells): Sidekick needs it to answer and to check builds
        const own = clean([...el.childNodes].filter((n) => n.nodeType === 3).map((n) => n.textContent).join(' '));
        if (own.length > 1 && !el.closest('script,style,noscript') && isShown(el)) {
          let inside = false;
          for (let p = el; p && !inside; p = p.parentElement) inside = listed.has(p);
          if (!inside) { const t = `- ${cut(own, 100)}`; if (t !== lines[lines.length - 1]) { lines.push(t); size += t.length; } }
        }
        continue;
      }
      if (!isShown(el)) continue;
      // a button's inner <span> repeats its parent; list the outermost actionable element only
      let parent = el.parentElement, nested = false;
      while (parent) { if (listed.has(parent)) { nested = true; break; } parent = parent.parentElement; }
      if (nested) continue;
      const name = nameOf(el);
      if (!name && role !== 'textbox' && role !== 'combobox' && role !== 'checkbox' && role !== 'radio') continue;
      listed.add(el);
      const line = `[${refs.length}] ${role} "${name}"${stateOf(el, role)}`;
      refs.push(el);
      lines.push(line);
      size += line.length;
    }
    return lines.join('\n');
  };

  const target = (ref) => {
    const el = refs[ref];
    if (!el || !el.isConnected) throw new Error(`element ${ref} is no longer on screen; read the screen again`);
    return el;
  };

  // Show the learner what Sidekick touched.
  const flash = (el) => {
    const prev = el.style.outline;
    el.style.outline = '3px solid #2563eb';
    el.style.outlineOffset = '2px';
    setTimeout(() => { el.style.outline = prev; }, 1200);
  };

  // What Sidekick's safety policy (src-tauri/src/guard.rs) judges an action by, read from the page itself:
  // the element's label, aria-label and title, the dialog it sits in (title, then its first lines, so a
  // "This can't be undone" confirmation is seen), the URL path, a text field's type hints (never its
  // value) and how many rows are checked (a bulk selection).
  const DIALOG = '[role="dialog"], [role="alertdialog"], [aria-modal="true"], .modal';
  const dialogText = (el) => {
    const dlg = el.closest(DIALOG);
    if (!dlg) return '';
    const by = dlg.getAttribute('aria-labelledby');
    const title = clean((by && el.ownerDocument.getElementById(by)?.innerText) || dlg.getAttribute('aria-label')
      || dlg.querySelector('h1, h2, h3, h4, [class*="title" i], [class*="header" i]')?.innerText);
    // the dialog's own words, not what anyone typed into it
    const words = [];
    const walk = document.createTreeWalker(dlg, NodeFilter.SHOW_TEXT);
    for (let n = walk.nextNode(); n && words.join(' ').length < 400; n = walk.nextNode()) {
      if (n.parentElement && !n.parentElement.closest('textarea, [contenteditable="true"], script, style')) words.push(n.textContent);
    }
    return cut(`${title} | ${clean(words.join(' '))}`, 400);
  };
  const fieldHints = (el) => {
    if (!/^(INPUT|TEXTAREA)$/.test(el.tagName) && !el.isContentEditable) return '';
    return clean([el.type, el.getAttribute('autocomplete'), el.getAttribute('name'), el.id, el.getAttribute('placeholder'), fieldLabel(el)]
      .filter(Boolean).join(' ').replace(/[_-]/g, ' '));
  };
  const selectedRows = () => new Set([...document.querySelectorAll(
    '[role="row"] input[type="checkbox"]:checked, tr input[type="checkbox"]:checked, [role="row"] [role="checkbox"][aria-checked="true"], [role="row"][aria-selected="true"]')]
    .filter((c) => !c.closest('thead, [role="columnheader"]')).map((c) => c.closest('[role="row"], tr') || c)).size;

  const describeEl = (el) => ({
    role: roleOf(el), name: nameOf(el), title: clean(el.getAttribute('title')), aria: clean(el.getAttribute('aria-label')),
    dialog: dialogText(el), path: location.pathname + location.hash, field: fieldHints(el), selected: selectedRows(),
    page: cut(clean(document.querySelector('h1')?.innerText || document.title), 80),
  });
  const describe = (ref) => describeEl(target(ref));
  // Enter goes to the focused element: describe that one, so the policy judges what Enter would press
  const focused = () => {
    const el = document.activeElement;
    if (!el || el === document.body || el === document.documentElement) return describeEl(document.body);
    return describeEl(el);
  };
  // Points at an element Sidekick won't touch, so the learner can find it: an amber outline for a few seconds.
  const point = (ref) => {
    const el = target(ref);
    el.scrollIntoView({ block: 'center', inline: 'center' });
    const prev = [el.style.outline, el.style.outlineOffset];
    el.style.outline = '3px dashed #d97706';
    el.style.outlineOffset = '3px';
    setTimeout(() => { [el.style.outline, el.style.outlineOffset] = prev; }, 6000);
    return `pointed at "${nameOf(el)}"`;
  };

  const click = (ref) => {
    const el = target(ref);
    el.scrollIntoView({ block: 'center', inline: 'center' });
    const r = el.getBoundingClientRect();
    const at = { bubbles: true, cancelable: true, composed: true, button: 0, clientX: r.left + r.width / 2, clientY: r.top + r.height / 2, view: el.ownerDocument.defaultView };
    const pointer = { ...at, pointerId: 1, pointerType: 'mouse', isPrimary: true };
    flash(el);
    el.dispatchEvent(new PointerEvent('pointerover', pointer));
    el.dispatchEvent(new MouseEvent('mouseover', at));
    el.dispatchEvent(new PointerEvent('pointerdown', pointer));
    el.dispatchEvent(new MouseEvent('mousedown', at));
    el.focus?.({ preventScroll: true });
    el.dispatchEvent(new PointerEvent('pointerup', pointer));
    el.dispatchEvent(new MouseEvent('mouseup', at));
    el.dispatchEvent(new MouseEvent('click', at));
    return `clicked ${roleOf(el)} "${nameOf(el)}"`;
  };

  const key = (k, el = document.activeElement || document.body) => {
    const init = { key: k, code: k === 'Enter' ? 'Enter' : k, bubbles: true, cancelable: true, composed: true };
    el.dispatchEvent(new KeyboardEvent('keydown', init));
    el.dispatchEvent(new KeyboardEvent('keyup', init));
    if (k === 'Enter' && el.form && el.tagName === 'INPUT') el.form.requestSubmit?.();
    return `pressed ${k}`;
  };

  const type = (ref, text, pressEnter) => {
    const el = target(ref);
    // a second lock behind guard.rs: never a password, key or code field
    if (el.type === 'password' || /\b(password|passcode|secret|api key|apikey|access key|token|one time code|otp)\b/i.test(fieldHints(el))) {
      throw new Error('The companion never types into password or secret fields; the learner does that');
    }
    el.scrollIntoView({ block: 'center' });
    el.focus();
    flash(el);
    if (el.isContentEditable) {
      // rich text editor: replace its content the way typing would, so the editor's own model updates
      const sel = el.ownerDocument.getSelection();
      sel.selectAllChildren(el);
      if (!el.ownerDocument.execCommand('insertText', false, text)) { el.textContent = text; el.dispatchEvent(new InputEvent('input', { bubbles: true, composed: true, inputType: 'insertText', data: text })); }
    } else {
      if (el.type === 'date' && !/^\d{4}-\d{2}-\d{2}$/.test(text)) {
        const d = new Date(text);
        if (isNaN(d)) throw new Error(`a date field takes a date such as 2026-10-30, not "${text}"`);
        const two = (n) => String(n).padStart(2, '0');
        text = `${d.getFullYear()}-${two(d.getMonth() + 1)}-${two(d.getDate())}`;
      }
      // React tracks the value through the prototype setter; assigning el.value directly is ignored
      const W = el.ownerDocument.defaultView;
      const proto = el.tagName === 'TEXTAREA' ? W.HTMLTextAreaElement.prototype : W.HTMLInputElement.prototype;
      Object.getOwnPropertyDescriptor(proto, 'value').set.call(el, text);
      el.dispatchEvent(new Event('input', { bubbles: true, composed: true }));
      el.dispatchEvent(new Event('change', { bubbles: true, composed: true }));
    }
    if (pressEnter) key('Enter', el);
    return `typed "${cut(text, 60)}" into "${nameOf(el)}"`;
  };

  // Custom dropdowns, lookups and search-selects, in steps the agent loop paces (each returns at once):
  // open(ref) clicks the trigger; pick(text) clicks the visible option with that text, or types it into
  // the dropdown's search box when it isn't listed yet; valueOf(ref) reads back what the field shows.
  const open = (ref) => {
    const el = target(ref);
    if (el.tagName === 'SELECT') return 'native';
    lastOpened = el;
    click(ref);
    const opts = openOptions(el).map((o) => clean(o.innerText));
    return opts.length ? `open: ${cut(opts.join(' | '), 300)}` : 'opened';
  };

  let lastOpened = null;
  const pick = (option, ref) => {
    const el = ref === undefined ? lastOpened : target(ref);
    const want = clean(option).toLowerCase();
    const opts = openOptions(el);
    const match = opts.find((o) => clean(o.innerText).toLowerCase() === want) || opts.find((o) => clean(o.innerText).toLowerCase().includes(want));
    if (match) {
      const i = refs.push(match) - 1;
      click(i);
      return `picked "${clean(match.innerText)}"`;
    }
    // a lookup or search-select: type into its search box, then pick again once the list has filtered
    const doc = el?.ownerDocument || document;
    const search = [doc.activeElement, ...doc.querySelectorAll('[role="listbox"] input, [class*="dropdown" i] input, [class*="search" i] input, input[role="combobox"], input[type="search"]')]
      .find((i) => i && i.tagName === 'INPUT' && isShown(i) && i.type !== 'hidden' && i.type !== 'password');
    if (search && clean(search.value).toLowerCase() !== want) {
      const i = refs.push(search) - 1;
      type(i, option, false);
      return 'searching';
    }
    throw new Error(`no option "${option}"; the options are: ${opts.map((o) => clean(o.innerText)).join(', ') || 'none showing'}`);
  };

  const valueOf = (ref) => clean(valueText(target(ref)));

  const choose = (ref, option) => {
    const el = target(ref);
    if (el.tagName !== 'SELECT') throw new Error('not a native dropdown: use open and pick');
    const want = clean(option).toLowerCase();
    const match = [...el.options].find((o) => clean(o.text).toLowerCase() === want) || [...el.options].find((o) => clean(o.text).toLowerCase().includes(want));
    if (!match) throw new Error(`no option "${option}"; options are: ${[...el.options].map((o) => clean(o.text)).join(', ')}`);
    flash(el);
    el.value = match.value;
    el.dispatchEvent(new Event('change', { bubbles: true }));
    return `chose "${clean(match.text)}"`;
  };

  // many apps scroll an inner container, not the window, so scroll whatever scrollable box is under the centre.
  const scroll = (down) => {
    let el = document.elementFromPoint(innerWidth / 2, innerHeight / 2);
    while (el && !(el.scrollHeight > el.clientHeight + 4 && /auto|scroll/.test(getComputedStyle(el).overflowY))) el = el.parentElement;
    const box = el || document.scrollingElement;
    box.scrollBy({ top: (down ? 0.8 : -0.8) * (box.clientHeight || innerHeight) });
    return `scrolled ${down ? 'down' : 'up'}`;
  };

  // Signs that the learner is stuck, for Sidekick: a field error ("Due date: Value is required"), an error
  // toast, an open dialog's title. Labels and messages only, never what anyone typed.
  const shown = (sel) => [...document.querySelectorAll(sel)].filter(isShown);
  const signals = () => {
    let validation = '';
    for (const e of shown('[class*="error" i], [class*="invalid" i], [role="alert"]:not([class*="toast" i])')) {
      if (e.matches('input, textarea, select, [contenteditable]') || e.querySelector('input, textarea, select, [contenteditable]')) continue;
      const msg = clean(e.innerText);
      if (!msg || msg.length > 120) continue;
      const label = fieldLabel(e);
      validation = cut(label && label !== msg ? `${label}: ${msg}` : msg, 120);
      break;
    }
    const toast = cut(clean(shown('[class*="toast" i], [class*="snackbar" i], [class*="notification" i][role="alert"], [role="status"][class*="error" i]')
      .map((t) => t.innerText).find((t) => clean(t))), 120);
    const dlg = shown('[role="dialog"], [role="alertdialog"], [aria-modal="true"], [class*="modal" i][class*="open" i], .modal.show')[0];
    let modal = '';
    if (dlg) {
      const by = dlg.getAttribute('aria-labelledby');
      modal = cut(clean((by && document.getElementById(by)?.innerText) || dlg.getAttribute('aria-label')
        || dlg.querySelector('h1, h2, h3, h4, [class*="title" i], [class*="header" i]')?.innerText) || 'dialog', 120);
    }
    return { validation, toast, modal };
  };

  // frozen and not replaceable: page script can't swap describe() for one that hides a Delete button
  Object.defineProperty(window, '__lmsExec', {
    value: Object.freeze({ snapshot, describe, focused, point, click, type, choose, open, pick, valueOf, key, scroll, signals }),
    writable: false, configurable: false, enumerable: false,
  });
})();
