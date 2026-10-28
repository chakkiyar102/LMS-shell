//! Practice pane: the learner's practice web app (any https site, see `shared::brand`) as webviews inside the main window. The
//! course UI owns the layout: it computes the pane's rectangle with `widths()` (the same numbers as the
//! CSS grid) and sends it, so split / full / hidden are just different rectangles.
//!
//! Tabs: a practice-app page that opens a new window (an editor, a builder, target=_blank)
//! gets a new tab in the pane, a sibling child webview sharing the same cookies. One tab is on screen at
//! a time; the others are parked. The toolbar and the screen helper act on the active tab.
//!
//! Sidekick's screen helper (screen_agent.rs) drives the pane from outside: it injects exec.js, a small
//! script that reads the screen and performs one action at a time, and calls it with `exec`.
//! Safety rails:
//! - only https URLs open as the pane's home (and only under `brand::PRACTICE_HOST_SUFFIX` when it is set)
//! - exec.js only arms itself on the practice hosts (the saved URL's host and the optional
//!   Development host, or any host under the suffix), and `exec` refuses any other page, so nothing
//!   of the companion's ever runs on a sign-in page (Google, Microsoft, Okta, ...)
//! - the remote page has no IPC access at all: the agent loop and the API key stay in Rust
//!
//! Lifecycle: the backend owns the pane. It closes (every tab and sign-in window) when the session
//! ends and when the main webview (re)loads (the UI always starts with no session, e.g. after a dev
//! hot-reload), and an open that finishes after a close (End clicked while the app was still loading)
//! closes itself.
use shared::Rect;
use tauri::{AppHandle, Url};
#[cfg(desktop)]
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[cfg(desktop)]
use std::sync::{Arc, Mutex};
#[cfg(desktop)]
use tauri::{Emitter, LogicalPosition, LogicalSize, Manager, WebviewBuilder, WebviewUrl};

#[cfg(desktop)]
const EXECUTOR_JS: &str = include_str!("exec.js");
/// Tab webviews are labelled `practice-tab-<id>`.
pub const TAB_PREFIX: &str = "practice-tab-";
/// Sign-in popups from the pane open as small windows with this label prefix.
pub const AUTH_PREFIX: &str = "practice-auth-";
pub const MAX_TABS: usize = 6;

/// Many web apps show an "unsupported browser" banner for the bare WKWebView user agent.
#[cfg(desktop)]
#[cfg(target_os = "macos")]
const USER_AGENT: Option<&str> =
    Some("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Safari/605.1.15");
#[cfg(desktop)]
#[cfg(not(target_os = "macos"))]
const USER_AGENT: Option<&str> = None; // WebView2 already reports as Edge

pub const NO_URL: &str = "Add your practice app's address in Settings, under Practice app, to open it here.";
pub const NO_DEV: &str = "Add a Development address in Settings, under Practice app, to open it here.";
pub const OFF_PRACTICE: &str = concat!(shared::companion_name!(), " only works on your practice app's pages. Finish signing in first, then ask again.");

/// A learner-entered practice address; a bare host such as `app.example.com` gets https:// added.
/// The rule is `shared::tenant::tenant_url` (the form uses it too, so both give the same answer).
pub fn validate_url(raw: &str) -> Result<Url, String> {
    let url = shared::tenant::tenant_url(raw).map_err(String::from)?;
    url.parse().map_err(|_| "That address isn't a valid URL.".to_string())
}

/// The practice hosts: the saved URL's host and the Development host, set when the pane opens.
static HOSTS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn set_hosts(prod: &Url, dev: Option<&Url>) {
    let hosts = [Some(prod), dev].into_iter().flatten().filter_map(|u| u.host_str()).map(|h| h.to_ascii_lowercase()).collect();
    *HOSTS.lock().unwrap_or_else(|e| e.into_inner()) = hosts;
}

/// The one host gate: https, and either one of the practice hosts or (when the fork sets
/// `brand::PRACTICE_HOST_SUFFIX`) any host under that suffix. The companion's executor and screen
/// helper only act where this holds, so they never run on an identity provider's sign-in page.
pub fn is_practice(url: &Url) -> bool {
    let hosts = HOSTS.lock().unwrap_or_else(|e| e.into_inner()).clone();
    is_practice_with(url, shared::brand::PRACTICE_HOST_SUFFIX, &hosts)
}

pub fn is_practice_with(url: &Url, suffix: Option<&str>, hosts: &[String]) -> bool {
    let host = url.host_str().unwrap_or_default().trim_end_matches('.').to_ascii_lowercase();
    if url.scheme() != "https" || host.is_empty() { return false; }
    hosts.iter().any(|h| *h == host) || suffix.is_some_and(|s| shared::tenant::under_suffix(&host, s))
}

/// The host check exec.js runs first: the same hosts and suffix as `is_practice`, defined before any
/// page script runs and frozen, so the page can't widen it.
fn executor_gate(hosts: &[String], suffix: Option<&str>) -> String {
    let hosts = serde_json::to_string(hosts).unwrap_or_else(|_| "[]".into());
    let suffix = serde_json::to_string(&suffix.unwrap_or_default().trim_matches('.').to_ascii_lowercase()).unwrap_or_else(|_| "\"\"".into());
    format!("Object.defineProperty(window, '__lmsPracticeGate', {{ value: Object.freeze({{ hosts: {hosts}, suffix: {suffix}, \
ok(l) {{ const h = String(l.hostname).toLowerCase().replace(/\\.$/, ''); \
return l.protocol === 'https:' && (this.hosts.includes(h) || (!!this.suffix && (h === this.suffix || h.endsWith('.' + this.suffix)))); }} }}) }});\n")
}

/// Debug builds only: a loopback http page, for testing popups, tabs and redirects without a tenant.
fn is_test_loopback(url: &Url) -> bool {
    cfg!(debug_assertions) && url.scheme() == "http" && matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
}

/// Which navigations the pane (and its sign-in windows) may make, in any frame. Top-level moves to an
/// identity provider are fine (SSO redirects need them); non-web schemes (mailto:, custom app links,
/// file:) and plain http are refused.
pub fn nav_allowed(url: &Url) -> bool {
    match url.scheme() {
        "https" => true,
        "about" | "blob" | "data" => true, // subframes and generated documents
        "http" => is_test_loopback(url),
        _ => false,
    }
}

/// An identity provider refusing embedded browsers ("This browser or app may not be secure"). Judged
/// from the URL alone: nothing reads the sign-in page.
pub fn embedded_refused(url: &Url) -> bool {
    let s = url.as_str();
    s.contains("disallowed_useragent")
        || (url.host_str() == Some("accounts.google.com") && url.path().contains("/signin/rejected"))
}

/// What a `window.open` / target=_blank from the pane becomes.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Popup {
    /// A small window sharing the pane's cookies (sign-in popups), closed once it is back on the practice app.
    Auth,
    /// A new tab in the pane (a practice-app page opened "in a new tab").
    Tab,
    /// The system browser (an ordinary outside link: help, docs, someone's website).
    Browser,
    Deny,
}

/// `sized`: the opener asked for a width/height, which is how sign-in popups open.
pub fn popup_action(url: &Url, sized: bool) -> Popup {
    if url.scheme() == "about" { return Popup::Auth; } // window.open('') then assign: sign-in libraries do this
    if !(url.scheme() == "https" || is_test_loopback(url)) { return Popup::Deny; }
    if sized { return Popup::Auth; }
    if is_practice(url) || is_test_loopback(url) { Popup::Tab } else { Popup::Browser }
}

/// Which environment a page is in: the saved practice URL is Production; the optional second
/// address in Settings is Development (a staging or sandbox copy of the same app).
#[derive(serde::Serialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Env { Prod, Dev, #[default] Other }

impl Env {
    pub fn label(self) -> &'static str {
        match self { Env::Prod => "Production", Env::Dev => "Development", Env::Other => "outside your practice app" }
    }
}

/// Production, Development, or neither, for a page the pane is on.
pub fn env_of(url: &Url, prod: Option<&Url>, dev: Option<&Url>) -> Env {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let same = |u: Option<&Url>| u.and_then(|u| u.host_str()).is_some_and(|h| h.eq_ignore_ascii_case(&host));
    if url.scheme() != "https" { return Env::Other; }
    if same(prod) { return Env::Prod; }
    if same(dev) { return Env::Dev; }
    Env::Other
}

/// One tab, as the toolbar shows it.
#[derive(serde::Serialize, Clone, Debug, PartialEq, Default)]
pub struct Tab {
    pub id: u64,
    /// The page title, e.g. "Brightline · Editor" (the host until the page names itself).
    pub title: String,
    pub host: String,
    /// On a practice-app page (the companion can help); false on a sign-in page or anything else.
    pub practice: bool,
    /// The identity provider refused the embedded browser.
    pub refused: bool,
    pub env: Env,
}

impl Tab {
    fn at(id: u64, url: &Url) -> Tab {
        let host = url.host_str().unwrap_or_default().to_string();
        Tab { id, title: host.clone(), host, practice: is_practice(url), refused: embedded_refused(url), env: Env::Other }
    }
}

/// The pane's tabs, oldest first. Pure state; the webviews follow it.
#[derive(Debug, Default, Clone)]
pub struct Tabs { pub list: Vec<Tab>, pub active: u64, next: u64 }

impl Tabs {
    /// Adds a tab for `url` and makes it active. At MAX_TABS the oldest tab that isn't active makes
    /// room; its id comes back so its webview can close.
    pub fn add(&mut self, url: &Url) -> (u64, Option<u64>) {
        let evicted = if self.list.len() >= MAX_TABS {
            let i = self.list.iter().position(|t| t.id != self.active).unwrap_or(0);
            Some(self.list.remove(i).id)
        } else { None };
        let id = self.next;
        self.next += 1;
        self.list.push(Tab::at(id, url));
        self.active = id;
        (id, evicted)
    }
    pub fn activate(&mut self, id: u64) -> bool {
        let ok = self.list.iter().any(|t| t.id == id);
        if ok { self.active = id }
        ok
    }
    /// Closes a tab. The last tab stays (closing it is closing the pane). An active tab hands over to
    /// its left neighbour, or the right one when it was first.
    pub fn remove(&mut self, id: u64) -> bool {
        let Some(i) = self.list.iter().position(|t| t.id == id) else { return false };
        if self.list.len() == 1 { return false; }
        self.list.remove(i);
        if self.active == id { self.active = self.list[i.saturating_sub(1).min(self.list.len() - 1)].id }
        true
    }
    pub fn get_mut(&mut self, id: u64) -> Option<&mut Tab> { self.list.iter_mut().find(|t| t.id == id) }
    pub fn active_tab(&self) -> Option<&Tab> { self.list.iter().find(|t| t.id == self.active) }
    /// The tab after the active one, wrapping (Ctrl+Tab).
    pub fn next_id(&self) -> Option<u64> {
        let i = self.list.iter().position(|t| t.id == self.active)?;
        self.list.get((i + 1) % self.list.len()).map(|t| t.id)
    }
}

/// What the toolbar shows: the active tab's page (host / practice / refused, which drive the notice)
/// and every tab for the strip.
#[derive(serde::Serialize, Clone, Debug, PartialEq, Default)]
pub struct PaneNav {
    pub host: String,
    pub practice: bool,
    pub refused: bool,
    pub tabs: Vec<Tab>,
    pub active: u64,
    /// The Development environment is known, so "+ Development" can open it.
    pub dev: bool,
}

pub fn pane_nav(tabs: &Tabs, dev: bool) -> PaneNav {
    let a = tabs.active_tab().cloned().unwrap_or_default();
    PaneNav { host: a.host, practice: a.practice, refused: a.refused, tabs: tabs.list.clone(), active: tabs.active, dev }
}

pub fn tab_label(id: u64) -> String { format!("{TAB_PREFIX}{id}") }

/// Bumped by every open and close: an open that finishes after a close drops its pane.
#[cfg(desktop)]
static GEN: AtomicU64 = AtomicU64::new(0);
/// Bumped whenever a different tab comes to the front, so the screen helper knows to re-read.
#[cfg(desktop)]
static EPOCH: AtomicU64 = AtomicU64::new(0);
#[cfg(desktop)]
static AUTH_N: AtomicU64 = AtomicU64::new(0);
#[cfg(desktop)]
static TABS: Mutex<Tabs> = Mutex::new(Tabs { list: Vec::new(), active: 0, next: 0 });
/// The pane's home (the saved practice URL), for the toolbar's Home and Open in browser.
#[cfg(desktop)]
static HOME: Mutex<Option<Url>> = Mutex::new(None);
/// The Development environment's address, from Settings (optional).
#[cfg(desktop)]
static DEV: Mutex<Option<Url>> = Mutex::new(None);
/// The rectangle the UI last asked for; every tab switch puts the new active tab there.
#[cfg(desktop)]
static RECT: Mutex<Rect> = Mutex::new(Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 });

#[cfg(desktop)]
fn tabs() -> std::sync::MutexGuard<'static, Tabs> { TABS.lock().unwrap_or_else(|e| e.into_inner()) }

#[cfg(desktop)]
fn emit_nav(app: &AppHandle) {
    let dev = DEV.lock().unwrap().is_some();
    let nav = pane_nav(&tabs(), dev);
    let _ = app.emit_to("main", "practice-nav", nav);
}

#[cfg(desktop)]
/// The tab on screen, if the pane is open.
fn active_webview(app: &AppHandle) -> Option<tauri::Webview> {
    let id = { let t = tabs(); t.active_tab()?.id };
    app.get_webview(&tab_label(id))
}

#[cfg(desktop)]
/// The current tab-switch count; compare before and after an action to see whether a new tab came up.
pub fn epoch() -> u64 { EPOCH.load(Ordering::SeqCst) }

#[cfg(desktop)]
/// A tab webview: the executor injected on every page load (it arms itself only on practice-app pages),
/// the navigation policy, popups routed through `new_window`, and the toolbar kept current.
fn tab_builder(app: &AppHandle, id: u64, url: Url) -> WebviewBuilder<tauri::Wry> {
    let (a1, a2, a3) = (app.clone(), app.clone(), app.clone());
    let gate = executor_gate(&HOSTS.lock().unwrap_or_else(|e| e.into_inner()), shared::brand::PRACTICE_HOST_SUFFIX);
    let mut builder = WebviewBuilder::new(tab_label(id), WebviewUrl::External(url))
        .initialization_script(format!("{gate}{EXECUTOR_JS}"))
        .on_navigation(|u| {
            let ok = nav_allowed(u);
            if !ok { eprintln!("[practice] blocked navigation to {}", u.scheme()); }
            ok
        })
        .on_new_window(move |u, features| new_window(&a1, u, features))
        .on_page_load(move |_, p| {
            let u = p.url();
            if let Some(t) = tabs().get_mut(id) {
                // the page title (set by on_document_title_changed) stays unless the tab went to another site
                let fresh = Tab::at(id, u);
                let title = if fresh.host == t.host { std::mem::take(&mut t.title) } else { fresh.title.clone() };
                let (home, dev) = (HOME.lock().unwrap().clone(), DEV.lock().unwrap().clone());
                let env = env_of(u, home.as_ref(), dev.as_ref());
                *t = Tab { title, env, ..fresh };
            }
            emit_nav(&a2);
        })
        .on_document_title_changed(move |_, title| {
            let title = title.trim().to_string();
            if title.is_empty() { return; }
            if let Some(t) = tabs().get_mut(id) { t.title = title; }
            emit_nav(&a3);
        });
    if let Some(ua) = USER_AGENT { builder = builder.user_agent(ua); }
    builder
}

#[cfg(desktop)]
/// Opens the practice pane on its home page, as its first tab.
pub fn open(app: &AppHandle, url: Url, dev: Option<Url>, rect: Rect) -> Result<(), String> {
    // opens run one at a time, so the only thing that can bump GEN mid-build is a close
    static OPENING: Mutex<()> = Mutex::new(());
    let _one = OPENING.lock().unwrap_or_else(|e| e.into_inner());
    let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    close_webviews(app);
    set_hosts(&url, dev.as_ref());
    *HOME.lock().unwrap() = Some(url.clone());
    *DEV.lock().unwrap() = dev;
    *RECT.lock().unwrap() = rect;
    let (id, _) = { let mut t = tabs(); *t = Tabs::default(); t.add(&url) };
    EPOCH.fetch_add(1, Ordering::SeqCst);
    let window = app.get_window("main").ok_or("main window missing")?;
    window
        .add_child(tab_builder(app, id, url), LogicalPosition::new(-20000.0, 0.0), LogicalSize::new(rect.w.max(10.0), rect.h.max(10.0)))
        .map_err(|e| e.to_string())?;
    emit_nav(app);
    watch_signals(app.clone(), gen);
    if GEN.load(Ordering::SeqCst) != gen {
        // the session ended while this one was being built
        eprintln!("[practice] session closed while the pane was opening; closing it");
        close_webviews(app);
    }
    Ok(())
}

#[cfg(desktop)]
/// A new tab for `url`, in front. Built off the webview's own callback (creating a webview from inside
/// a WebKit delegate call isn't safe).
fn open_tab(app: &AppHandle, url: Url) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let gen = GEN.load(Ordering::SeqCst);
        let Some(window) = app.get_window("main") else { return };
        if active_webview(&app).is_none() { return; } // the pane closed meanwhile
        let (id, evicted) = tabs().add(&url);
        if let Some(old) = evicted {
            eprintln!("[practice] {MAX_TABS} tabs open; closing the oldest");
            if let Some(w) = app.get_webview(&tab_label(old)) { let _ = w.close(); }
        }
        let rect = *RECT.lock().unwrap();
        if let Err(e) = window.add_child(tab_builder(&app, id, url), LogicalPosition::new(-20000.0, 0.0), LogicalSize::new(rect.w.max(10.0), rect.h.max(10.0))) {
            eprintln!("[practice] new tab failed: {e}");
            let mut t = tabs();
            t.remove(id);
            return;
        }
        if GEN.load(Ordering::SeqCst) != gen { close_webviews(&app); return; }
        EPOCH.fetch_add(1, Ordering::SeqCst);
        let _ = place(&app, rect);
        emit_nav(&app);
    });
}

#[cfg(desktop)]
fn new_window(app: &AppHandle, url: Url, features: tauri::webview::NewWindowFeatures) -> tauri::webview::NewWindowResponse<tauri::Wry> {
    use tauri::webview::NewWindowResponse;
    let action = popup_action(&url, features.size().is_some());
    eprintln!("[practice] new window request ({action:?}) to {}", url.host_str().unwrap_or(url.scheme()));
    match action {
        Popup::Deny => NewWindowResponse::Deny,
        Popup::Browser => {
            use tauri_plugin_opener::OpenerExt;
            let _ = app.opener().open_url(url.as_str(), None::<&str>);
            NewWindowResponse::Deny
        }
        Popup::Tab => { open_tab(app, url); NewWindowResponse::Deny }
        Popup::Auth => match auth_window(app, features) {
            Ok(window) => NewWindowResponse::Create { window },
            Err(e) => { eprintln!("[practice] sign-in window failed: {e}"); NewWindowResponse::Deny }
        },
    }
}

#[cfg(desktop)]
/// A sign-in popup: a plain window sharing the pane's web data (window_features carries the opener's
/// configuration), no companion script, closed once its navigation comes back to the practice app.
fn auth_window(app: &AppHandle, features: tauri::webview::NewWindowFeatures) -> tauri::Result<tauri::WebviewWindow> {
    let label = format!("{AUTH_PREFIX}{}", AUTH_N.fetch_add(1, Ordering::SeqCst));
    let left = Arc::new(AtomicBool::new(false));
    let sized = features.size().is_some();
    let a = app.clone();
    let mut b = tauri::WebviewWindowBuilder::new(app, &label, WebviewUrl::External("about:blank".parse().unwrap()))
        .title("Sign in")
        .window_features(features)
        .on_navigation(nav_allowed)
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny)
        .on_page_load(move |w, p| {
            let u = p.url();
            if u.scheme() == "about" { return; }
            if !is_practice(u) { left.store(true, Ordering::SeqCst); }
            if embedded_refused(u) {
                // say so in the pane's toolbar, on the tab that opened it
                { let mut g = tabs(); let id = g.active; if let Some(t) = g.get_mut(id) { t.refused = true } }
                emit_nav(&a);
            }
            if p.event() == tauri::webview::PageLoadEvent::Finished && is_practice(u) && left.load(Ordering::SeqCst) {
                eprintln!("[practice] sign-in window back on the practice app; closing it");
                let _ = w.close();
                // the pane picks up the new session cookie
                if let Some(pane) = active_webview(&a) {
                    if pane.url().is_ok_and(|pu| is_practice(&pu)) { let _ = pane.reload(); }
                }
            }
        });
    if !sized { b = b.inner_size(520.0, 680.0); }
    if let Some(ua) = USER_AGENT { b = b.user_agent(ua); }
    b.build()
}

#[cfg(desktop)]
/// Inner (content) height of the main window in logical px.
fn content_height(app: &AppHandle) -> Option<f64> {
    let w = app.get_window("main")?;
    Some(w.inner_size().ok()?.to_logical::<f64>(w.scale_factor().ok()?).height)
}

#[cfg(desktop)]
const PARKED: (LogicalPosition<f64>, LogicalSize<f64>) = (LogicalPosition { x: -20000.0, y: 0.0 }, LogicalSize { width: 10.0, height: 10.0 });

#[cfg(desktop)]
/// Puts the active tab in `rect` and parks every other tab off-screen; a zero-size rect parks them
/// all. (Parking, not hide(): a hidden child view on macOS can still sit over the course and swallow
/// clicks and drags.)
pub fn place(app: &AppHandle, rect: Rect) -> Result<(), String> {
    *RECT.lock().unwrap() = rect;
    let (ids, active) = { let t = tabs(); (t.list.iter().map(|t| t.id).collect::<Vec<_>>(), t.active) };
    let on = if rect.w < 2.0 || rect.h < 2.0 { PARKED } else {
        // Child views are placed in the window's own coordinate space, which on macOS includes the
        // title bar; the UI measures from below it. The UI's viewport ends at rect.y + rect.h, so the
        // difference from the native content height is the title-bar offset (0 where they agree).
        // Log evidence: native content 777 vs UI viewport 745 -> 32px title bar.
        let offset = content_height(app).map(|h| (h - (rect.y + rect.h)).max(0.0)).unwrap_or(0.0);
        (LogicalPosition::new(rect.x, rect.y + offset), LogicalSize::new(rect.w, rect.h))
    };
    eprintln!("[practice] place {rect:?} -> tab {active} at {:?} {:?}", on.0, on.1);
    for id in ids {
        let Some(w) = app.get_webview(&tab_label(id)) else { continue };
        let (pos, size) = if id == active { on } else { PARKED };
        w.set_position(pos).map_err(|e| e.to_string())?;
        w.set_size(size).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(desktop)]
/// The tab strip: `switch` brings a tab to the front, `close` closes it (never the last one), `next`
/// moves to the next tab, `new` opens one on Production, `dev` one on Development.
pub fn tab(app: &AppHandle, action: &str, id: u64) -> Result<(), String> {
    match action {
        "new" | "dev" => {
            let url = if action == "dev" { DEV.lock().unwrap().clone().ok_or(NO_DEV)? } else { HOME.lock().unwrap().clone().ok_or("The practice pane isn't open.")? };
            if active_webview(app).is_none() { return Err("The practice pane isn't open.".into()); }
            open_tab(app, url);
            return Ok(());
        }
        "next" => {
            let Some(next) = tabs().next_id() else { return Ok(()) };
            return tab(app, "switch", next);
        }
        "switch" => {
            if !tabs().activate(id) { return Err("That tab is gone.".into()); }
            EPOCH.fetch_add(1, Ordering::SeqCst);
        }
        "close" => {
            let before = tabs().active;
            if !tabs().remove(id) { return Err("The last tab closes with the pane: use Close.".into()); }
            if let Some(w) = app.get_webview(&tab_label(id)) { let _ = w.close(); }
            if tabs().active != before { EPOCH.fetch_add(1, Ordering::SeqCst); }
        }
        _ => return Err(format!("unknown tab action {action}")),
    }
    let rect = *RECT.lock().unwrap();
    place(app, rect)?;
    emit_nav(app);
    Ok(())
}

#[cfg(desktop)]
/// The active tab's environment and host, for the screen helper ("Development (staging.example.com)").
pub fn active_env() -> String {
    tabs().active_tab().map(|t| format!("{} ({})", t.env.label(), t.host)).unwrap_or_default()
}

#[cfg(desktop)]
/// The active tab's environment; `Other` when there is no tab or the page isn't known to be either.
pub fn active_env_kind() -> Env { tabs().active_tab().map(|t| t.env).unwrap_or_default() }

/// One "stuck" signal for Sidekick: `validation` (a field error), `toast` (an error toast) or `modal` (a
/// dialog's title; "" when it closes).
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct Signal { pub kind: &'static str, pub text: String }

/// What changed between two reads of the page's signals. New field errors and toasts are reported
/// (not their clearing); a dialog both opening and closing.
pub fn signal_changes(last: &serde_json::Value, now: &serde_json::Value) -> Vec<Signal> {
    let s = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().chars().take(120).collect::<String>();
    let mut out = vec![];
    for kind in ["validation", "toast"] {
        let t = s(now, kind);
        if !t.is_empty() && t != s(last, kind) { out.push(Signal { kind, text: t }) }
    }
    if s(now, "modal") != s(last, "modal") { out.push(Signal { kind: "modal", text: s(now, "modal") }) }
    out
}

#[cfg(desktop)]
/// Reads the active tab's signals about once a second while this pane is open, and tells Sidekick's UI
/// what changed (`practice-signal`). Only on practice-app pages (exec refuses others), and the page
/// can't push anything: the app asks.
fn watch_signals(app: AppHandle, gen: u64) {
    tauri::async_runtime::spawn(async move {
        let mut last = serde_json::json!({});
        while GEN.load(Ordering::SeqCst) == gen {
            let _ = tauri::async_runtime::spawn_blocking(|| std::thread::sleep(std::time::Duration::from_millis(1000))).await;
            let Ok(now) = exec(&app, "signals()").await else { continue };
            for sig in signal_changes(&last, &now) { let _ = app.emit_to("main", "practice-signal", sig); }
            last = now;
        }
    });
}

#[cfg(desktop)]
/// Whether the pane is open (it has a tab).
pub fn is_open() -> bool { !tabs().list.is_empty() }

#[cfg(desktop)]
/// The tab on screen.
pub fn active_id() -> u64 { tabs().active }

#[cfg(desktop)]
/// The pane's tabs, for the screen helper's list_tabs tool.
pub fn tab_list() -> PaneNav { pane_nav(&tabs(), DEV.lock().unwrap().is_some()) }

#[cfg(desktop)]
/// The toolbar: back | forward | reload | home | browser, on the active tab. Back/forward run one
/// history call in the page (they read nothing); the rest are native.
pub fn nav(app: &AppHandle, action: &str) -> Result<(), String> {
    let w = active_webview(app).ok_or("The practice pane isn't open.")?;
    let home = HOME.lock().unwrap().clone();
    match action {
        "back" => w.eval("history.back()").map_err(|e| e.to_string()),
        "forward" => w.eval("history.forward()").map_err(|e| e.to_string()),
        "reload" => w.reload().map_err(|e| e.to_string()),
        "home" => w.navigate(home.ok_or("No practice URL saved.")?).map_err(|e| e.to_string()),
        "browser" => {
            use tauri_plugin_opener::OpenerExt;
            // the practice page you're on, or the saved home from a sign-in page
            let url = w.url().ok().filter(is_practice).or(home).ok_or("No practice URL saved.")?;
            app.opener().open_url(url.as_str(), None::<&str>).map_err(|e| e.to_string())
        }
        _ => Err(format!("unknown pane action {action}")),
    }
}

#[cfg(desktop)]
/// Runs one `__lmsExec` call in the active tab and returns its value. The executor throws on bad input,
/// which comes back as Err so the agent can read the reason and recover. Refuses any page that isn't
/// a practice host (see `is_practice`): a sign-in page is never read or touched.
pub async fn exec(app: &AppHandle, call: &str) -> Result<serde_json::Value, String> {
    let w = active_webview(app).ok_or("Open a coaching session first, so the practice pane is there.")?;
    if !w.url().is_ok_and(|u| is_practice(&u)) { return Err(OFF_PRACTICE.into()); }
    let js = format!("(() => {{ if (!window.__lmsExec) return {{ err: '{LOADING}' }}; \
        try {{ return {{ ok: window.__lmsExec.{call} }}; }} catch (e) {{ return {{ err: String((e && e.message) || e) }}; }} }})()");
    let (tx, rx) = std::sync::mpsc::channel();
    w.eval_with_callback(js, move |r| { let _ = tx.send(r); }).map_err(|e| e.to_string())?;
    let raw = tauri::async_runtime::spawn_blocking(move || rx.recv_timeout(std::time::Duration::from_secs(8)))
        .await.map_err(|e| e.to_string())?
        .map_err(|_| "The page didn't respond in time. Is it still loading?".to_string())?;
    let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    match v.get("err").and_then(|e| e.as_str()) {
        Some(err) => Err(err.to_string()),
        None => Ok(v.get("ok").cloned().unwrap_or_default()),
    }
}

pub const LOADING: &str = "The page is still loading";

#[cfg(desktop)]
/// Reads the active tab once it has loaded (a new tab needs a moment): snapshot() retried for up to
/// `secs` while the page is still coming up.
pub async fn snapshot_when_ready(app: &AppHandle, secs: u64) -> Result<serde_json::Value, String> {
    let start = std::time::Instant::now();
    loop {
        match exec(app, "snapshot()").await {
            Err(e) if start.elapsed().as_secs() < secs && (e == LOADING || e == OFF_PRACTICE || e.contains("in time")) => {
                let _ = tauri::async_runtime::spawn_blocking(|| std::thread::sleep(std::time::Duration::from_millis(400))).await;
            }
            r => return r,
        }
    }
}

#[cfg(desktop)]
/// Whether the pane is on screen (it is parked off-screen when hidden).
pub fn visible(app: &AppHandle) -> bool {
    active_webview(app).and_then(|w| w.position().ok()).is_some_and(|p| p.x > -1000)
}

#[cfg(desktop)]
/// Whether the active tab is on a practice-app page (false on a sign-in page).
pub fn on_practice(app: &AppHandle) -> bool {
    active_webview(app).and_then(|w| w.url().ok()).is_some_and(|u| is_practice(&u))
}

#[cfg(desktop)]
fn close_webviews(app: &AppHandle) {
    for w in app.webviews().into_values() {
        if w.label().starts_with(TAB_PREFIX) { let _ = w.close(); }
    }
    for (label, w) in app.webview_windows() {
        if label.starts_with(AUTH_PREFIX) { let _ = w.close(); }
    }
    *tabs() = Tabs::default();
}

#[cfg(desktop)]
/// Closes the pane (every tab) and any sign-in windows, and cancels an open still in flight.
pub fn close(app: &AppHandle) {
    GEN.fetch_add(1, Ordering::SeqCst);
    close_webviews(app);
}


// Android: one webview per app, so the tenant opens in the browser (open_practice) and there is
// no pane to place or close.
#[cfg(mobile)]
pub fn open(_: &AppHandle, _: Url, _: Option<Url>, _: Rect) -> Result<(), String> { Ok(()) }
#[cfg(mobile)]
pub fn place(_: &AppHandle, _: Rect) -> Result<(), String> { Ok(()) }
#[cfg(mobile)]
pub fn close(_: &AppHandle) {}
#[cfg(mobile)]
pub fn nav(_: &AppHandle, _: &str) -> Result<(), String> { Ok(()) }
#[cfg(mobile)]
pub fn tab(_: &AppHandle, _: &str, _: u64) -> Result<(), String> { Ok(()) }

#[cfg(test)]
mod tests {
    use super::*;
    fn u(s: &str) -> Url { s.parse().unwrap() }

    #[test]
    fn bare_host_gets_https_and_others_are_refused() {
        assert_eq!(validate_url(" app.brightline.example ").unwrap().as_str(), "https://app.brightline.example/");
        assert!(validate_url("https://notes.brightline.example").is_ok());
        assert!(validate_url("http://notes.brightline.example").is_err());
        assert!(validate_url("javascript:alert(1)").is_err());
    }

    #[test]
    fn host_gate_is_https_on_the_practice_hosts_only() {
        let hosts = vec!["notes.brightline.example".to_string(), "staging.brightline.example".to_string()];
        for ok in ["https://notes.brightline.example/x", "https://NOTES.brightline.example/", "https://staging.brightline.example/"] {
            assert!(is_practice_with(&u(ok), None, &hosts), "{ok}");
        }
        for bad in ["http://notes.brightline.example/", "https://accounts.google.com/o/oauth2", "https://login.microsoftonline.com/",
            "https://brightline.okta.com/", "https://notes.brightline.example.evil.io/", "https://other.brightline.example/",
            "about:blank", "data:text/html,hi"] {
            assert!(!is_practice_with(&u(bad), None, &hosts), "{bad}");
        }
        // with a suffix configured, every host under it counts, and nothing else
        let s = Some("brightline.example");
        for ok in ["https://brightline.example/", "https://other.brightline.example/x", "https://a.b.BRIGHTLINE.example/"] {
            assert!(is_practice_with(&u(ok), s, &[]), "{ok}");
        }
        for bad in ["https://brightline.example.evil.io/", "https://evilbrightline.example/", "http://other.brightline.example/"] {
            assert!(!is_practice_with(&u(bad), s, &[]), "{bad}");
        }
    }

    #[test]
    fn the_executor_gate_carries_the_hosts_and_suffix() {
        let g = executor_gate(&["notes.brightline.example".into()], Some("Brightline.example"));
        assert!(g.starts_with("Object.defineProperty(window, '__lmsPracticeGate', { value: Object.freeze({ hosts: [\"notes.brightline.example\"], suffix: \"brightline.example\", "), "{g}");
        assert!(g.contains("l.protocol === 'https:'"));
        assert!(executor_gate(&[], None).contains("suffix: \"\""));
    }

    #[test]
    fn navigation_policy_allows_sso_and_refuses_other_schemes() {
        assert!(nav_allowed(&u("https://accounts.google.com/signin")));
        assert!(nav_allowed(&u("https://notes.brightline.example/")));
        assert!(nav_allowed(&u("about:blank")));
        assert!(!nav_allowed(&u("http://example.com/")));
        assert!(!nav_allowed(&u("mailto:a@b.c")));
        assert!(!nav_allowed(&u("file:///etc/passwd")));
        assert!(!nav_allowed(&u("zoommtg://join")));
        // loopback http only in debug builds, for the local popup test page
        assert_eq!(nav_allowed(&u("http://127.0.0.1:8899/")), cfg!(debug_assertions));
    }

    #[test]
    fn popups_become_signin_windows_tabs_or_the_browser() {
        assert_eq!(popup_action(&u("https://accounts.google.com/o/oauth2/auth"), true), Popup::Auth);
        assert_eq!(popup_action(&u("about:blank"), false), Popup::Auth);
        set_hosts(&u("https://notes.brightline.example/"), None);
        assert_eq!(popup_action(&u("https://notes.brightline.example/view/1"), false), Popup::Tab, "a page opened in a new tab");
        assert_eq!(popup_action(&u("https://notes.brightline.example/login"), true), Popup::Auth, "a sized popup is a sign-in, even on the practice host");
        assert_eq!(popup_action(&u("https://help.example.com/"), false), Popup::Browser);
        assert_eq!(popup_action(&u("mailto:a@b.c"), false), Popup::Deny);
        assert_eq!(popup_action(&u("http://example.com/"), true), Popup::Deny);
    }

    #[test]
    fn embedded_browser_refusal_is_seen_from_the_url() {
        assert!(embedded_refused(&u("https://accounts.google.com/v3/signin/rejected?continue=x")));
        assert!(embedded_refused(&u("https://accounts.google.com/o/oauth2/auth?error=disallowed_useragent")));
        assert!(!embedded_refused(&u("https://accounts.google.com/v3/signin/identifier")));
        let mut t = Tabs::default();
        t.add(&u("https://accounts.google.com/v3/signin/identifier"));
        let n = pane_nav(&t, false);
        assert_eq!((n.host.as_str(), n.practice, n.refused), ("accounts.google.com", false, false));
    }

    #[test]
    fn a_new_tab_comes_to_the_front_and_switching_moves_it() {
        let mut t = Tabs::default();
        let (home, _) = t.add(&u("https://notes.brightline.example/"));
        let (builder, _) = t.add(&u("https://notes.brightline.example/editor"));
        assert_eq!(t.active, builder, "Edit app's tab is the one on screen");
        assert_eq!(pane_nav(&t, false).tabs.len(), 2);
        assert!(t.activate(home));
        assert_eq!(t.active, home);
        assert!(!t.activate(99), "no such tab");
        assert_eq!(t.active, home);
    }

    #[test]
    fn closing_tabs_hands_over_and_keeps_the_last_one() {
        let mut t = Tabs::default();
        let ids: Vec<u64> = (0..3).map(|i| t.add(&u(&format!("https://notes.brightline.example/{i}"))).0).collect();
        assert!(t.remove(ids[2]), "close the active, rightmost tab");
        assert_eq!(t.active, ids[1], "its left neighbour takes over");
        t.activate(ids[0]);
        assert!(t.remove(ids[0]));
        assert_eq!(t.active, ids[1], "the first tab hands over to the right");
        assert!(!t.remove(ids[1]), "the last tab closes with the pane, not on its own");
        assert!(!t.remove(42));
    }

    #[test]
    fn at_six_tabs_the_oldest_inactive_one_makes_room() {
        let mut t = Tabs::default();
        let first = t.add(&u("https://notes.brightline.example/0")).0;
        for i in 1..MAX_TABS { t.add(&u(&format!("https://notes.brightline.example/{i}"))); }
        assert_eq!(t.list.len(), MAX_TABS);
        // the learner went back to the first tab, then opened one more: the second-oldest goes
        t.activate(first);
        let (new, evicted) = t.add(&u("https://notes.brightline.example/new"));
        assert_eq!(evicted, Some(1));
        assert_eq!(t.list.len(), MAX_TABS);
        assert_eq!(t.active, new);
        assert!(t.list.iter().any(|x| x.id == first), "the tab the learner was on survives");
    }

    #[test]
    fn environments_come_from_the_two_saved_addresses() {
        let (prod, dev) = (u("https://notes.brightline.example/"), u("https://staging.brightline.example/"));
        assert_eq!(env_of(&u("https://notes.brightline.example/a"), Some(&prod), None), Env::Prod);
        assert_eq!(env_of(&u("https://staging.brightline.example/a"), Some(&prod), Some(&dev)), Env::Dev);
        assert_eq!(env_of(&u("https://staging.brightline.example/a"), Some(&prod), None), Env::Other, "no Development address saved");
        assert_eq!(env_of(&u("https://accounts.google.com/"), Some(&prod), None), Env::Other);
        let mut t = Tabs::default();
        let a = t.add(&prod).0; let b = t.add(&dev).0;
        assert_eq!(t.next_id(), Some(a), "Ctrl+Tab wraps");
        t.activate(a);
        assert_eq!(t.next_id(), Some(b));
    }

    #[test]
    fn signals_report_new_errors_and_dialogs_opening_and_closing() {
        use serde_json::json;
        let none = json!({ "validation": "", "toast": "", "modal": "" });
        let err = json!({ "validation": "Due date: Value is required", "toast": "", "modal": "New note" });
        let kinds = |v: Vec<Signal>| v.into_iter().map(|s| format!("{}={}", s.kind, s.text)).collect::<Vec<_>>();
        assert_eq!(kinds(signal_changes(&none, &err)), ["validation=Due date: Value is required", "modal=New note"]);
        assert!(signal_changes(&err, &err).is_empty(), "on change only");
        assert_eq!(kinds(signal_changes(&err, &none)), ["modal="], "a closed dialog is reported, a cleared error isn't");
        let long = json!({ "toast": "x".repeat(300) });
        assert_eq!(signal_changes(&none, &long)[0].text.len(), 120);
    }

    #[test]
    fn executor_arms_itself_only_on_practice_pages() {
        let js = include_str!("exec.js");
        let gate = "if (!window.__lmsPracticeGate || !window.__lmsPracticeGate.ok(location)) return;";
        let body = js.find("(() => {").expect("iife");
        assert!(js[body..].trim_start_matches("(() => {").trim_start().starts_with(gate), "the host gate must be the executor's first statement");
    }
}
