use shared::brand::COMPANION;
mod api;
mod sidekick;
mod md;
mod welcome;

use leptos::{prelude::*, task::spawn_local};
use serde::de::DeserializeOwned;
use serde_json::json;
use shared::*;
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::JsCast;

#[derive(Clone, PartialEq)]
enum Page {
    Course,
    Session(String),
    Lesson(String),
    Check(String),
    Exercise(String),
    Final,
    Certificate,
    Settings,
}

#[derive(Clone, Copy, PartialEq)]
enum Pmode { Off, Split }

/// Each session column is open, collapsed to a slim tab, or closed (hidden; the rail reopens it).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Ps { Open, Collapsed, Closed }

#[derive(Clone, Copy, PartialEq, Debug)]
struct Panels { course: Ps, practice: Ps, sidekick: Ps }

impl Panels {
    const ALL: Panels = Panels { course: Ps::Open, practice: Ps::Open, sidekick: Ps::Open };
    fn get(&self, i: u8) -> Ps { match i { 0 => self.course, 1 => self.practice, _ => self.sidekick } }
    fn set(&mut self, i: u8, s: Ps) { match i { 0 => self.course = s, 1 => self.practice = s, _ => self.sidekick = s } }
    /// Never leave the learner looking at nothing: if every column is hidden, the course comes back.
    fn keep_one(mut self) -> Self { if (0..3).all(|j| self.get(j) != Ps::Open) { self.course = Ps::Open } self }
}

const RAIL_W: f64 = 60.0;
const TAB_W: f64 = 40.0;
const SIDEKICK_W: f64 = 360.0;
/// Narrower than these, an open column is squeezed past use (an icon overflows, the course scrolls
/// sideways): fit() folds it to its tab instead.
const MIN_COURSE: f64 = 320.0;
const MIN_PRACTICE: f64 = 360.0;
const MIN_SIDEKICK: f64 = 300.0;
const HEAD_H: f64 = 40.0;
/// The pane toolbar's notice row (sign-in page / embedded browser refused), between the head and the pane.
const NOTICE_H: f64 = 56.0;
/// The pane's tab strip (Production, Development, and pages the app opens in a new tab).
const TABS_H: f64 = 34.0;

/// Closes the practice pane (toolbar Close, the shortcut, the macOS menu item).
#[derive(Clone, Copy)]
struct PaneClose(Callback<()>);

/// Where the practice pane is, as the backend reports it (`practice-nav`).
#[derive(serde::Deserialize, Clone, Default, PartialEq, Debug)]
struct PaneNav { host: String, practice: bool, refused: bool, #[serde(default)] tabs: Vec<PaneTab>, #[serde(default)] active: u64, #[serde(default)] dev: bool }

#[derive(serde::Deserialize, Clone, Default, PartialEq, Debug)]
struct PaneTab { id: u64, title: String, host: String, #[serde(default)] env: String }

impl PaneNav {
    /// Height of the rows between the column head and the pane: the tab strip and the notice.
    fn extra_h(&self) -> f64 {
        (if self.tabs.is_empty() { 0.0 } else { TABS_H }) + if self.notice().is_some() { NOTICE_H } else { 0.0 }
    }
    /// The toolbar notice for this page, if any. A fresh pane (no host yet) says nothing.
    fn notice(&self) -> Option<&'static str> {
        if self.refused { return Some("Your company's sign-in doesn't allow embedded browsers. Sign in once with Open in browser, or use a password sign-in for your training account.") }
        (!self.practice && !self.host.is_empty()).then_some("Signing in with your company's sign-in page. Use Back or Close if you get stuck.")
    }
}

/// Column widths (course, practice, sidekick) in CSS px. The SAME numbers size the grid and the native
/// practice pane, so the two can't disagree. They always add up to the window less the rail.
fn widths(p: Panels, share: f64, win_w: f64) -> (f64, f64, f64) {
    let fixed = |s: Ps| match s { Ps::Closed => Some(0.0), Ps::Collapsed => Some(TAB_W), Ps::Open => None };
    let (c, k, z) = (fixed(p.course), fixed(p.practice), fixed(p.sidekick));
    let free = (win_w - RAIL_W - c.unwrap_or(0.0) - k.unwrap_or(0.0) - z.unwrap_or(0.0)).max(0.0);
    // Sidekick keeps his width beside an open course or practice column (giving up to 60 px so they reach their
    // minimum), and takes the rest when he's alone
    let need = if c.is_none() { MIN_COURSE } else { 0.0 } + if k.is_none() { MIN_PRACTICE } else { 0.0 };
    let zw = z.unwrap_or(if c.is_none() || k.is_none() { SIDEKICK_W.min((free - need).max(MIN_SIDEKICK)).min(free) } else { free });
    let avail = if z.is_none() { free - zw } else { free };
    match (c, k) {
        (None, None) => {
            let cw = (avail * share.clamp(0.2, 0.8)).round();
            // the split keeps both at their minimum while there's room for both (fit() folds one when there isn't)
            let cw = if avail >= MIN_COURSE + MIN_PRACTICE { cw.clamp(MIN_COURSE, avail - MIN_PRACTICE) } else { cw };
            (cw, avail - cw, zw)
        }
        (None, Some(k)) => (avail, k, zw),
        (Some(c), None) => (c, avail, zw),
        (Some(c), Some(k)) => (c, k, zw),
    }
}

/// The columns as shown: the learner's choice, with an open column folded to its tab whenever the
/// window can't give every open column its minimum width (the course first, then practice, then Sidekick;
/// `keep`, the column the learner opened last, folds last). One open column always stays.
fn fit(p: Panels, share: f64, win_w: f64, keep: u8) -> Panels {
    let mut p = p.keep_one();
    let order = [0u8, 1, 2].into_iter().filter(|&i| i != keep).chain([keep]);
    for i in order {
        let w = widths(p, share, win_w);
        let width = |j: u8| [w.0, w.1, w.2][j as usize];
        let short = (0..3).any(|j| p.get(j) == Ps::Open && width(j) < [MIN_COURSE, MIN_PRACTICE, MIN_SIDEKICK][j as usize]);
        if !short || (0..3).filter(|&j| p.get(j) == Ps::Open).count() <= 1 { break; }
        if p.get(i) == Ps::Open { p.set(i, Ps::Collapsed) }
    }
    p
}

/// App-wide state: where we are, which course, and what the floating tutor should be grounded in.
#[derive(Clone, Copy)]
struct Ctx {
    page: RwSignal<Page>,
    course: RwSignal<String>,
    /// (lesson id, lesson title) while a lesson is open; None means the course-wide thread.
    chat_lesson: RwSignal<Option<(String, String)>>,
    chat_open: RwSignal<bool>,
    /// Practice pane layout; Off means no practice webview exists.
    practice: RwSignal<Pmode>,
    /// (kind "lesson"|"exercise", id, label) of the open coaching session.
    session: RwSignal<Option<(String, String, String)>>,
    coach: RwSignal<Option<CoachState>>,
    /// Session columns: open / collapsed / closed, as the learner chose them (fit() decides what fits).
    panels: RwSignal<Panels>,
    /// The column opened last (0 course, 1 practice, 2 Sidekick): it keeps its room when the window is too
    /// narrow for every open column.
    keep: RwSignal<u8>,
    /// Course column share in Split (0.2–0.8), dragged by the learner, remembered on this machine.
    share: RwSignal<f64>,
    dragging: RwSignal<bool>,
    stats: RwSignal<Option<Stats>>,
    /// The open course's outline, for breadcrumbs.
    outline: RwSignal<Option<CourseView>>,
    /// Pages visited, newest last, for the back button.
    history: RwSignal<Vec<Page>>,
    /// Sidekick's screen helper: its running log, whether a run is going, and a pending Allow/Don't question.
    screen: RwSignal<Vec<ScreenLine>>,
    screen_busy: RwSignal<bool>,
    screen_ask: RwSignal<Option<String>>,
    /// Settings → Sidekick companion (On / Quiet / Off), stored per device as `sidekick_mode`.
    sidekick_mode: RwSignal<sidekick::Mode>,
    /// The latest thing that happened that Sidekick's companion may react to (sidekick::Companion).
    sidekick_event: RwSignal<Option<SidekickEvent>>,
    /// The open lesson's "Read more" links.
    lesson_help: RwSignal<Vec<HelpLink>>,
    /// Bumped when the access lock may have changed (a role change in Settings): the course list reloads.
    courses_rev: RwSignal<u32>,
    /// The app tour, when there is a playable one (get_app_tour); None hides the Course tour button.
    tour: RwSignal<Option<AppTour>>,
    /// The tour player while it is open: on its own on first run, or replayed from the top bar or Settings.
    tour_open: RwSignal<Option<TourMode>>,
}

/// First run gates Continue on watching to the end; a replay closes freely.
#[derive(Clone, Copy, PartialEq, Debug)]
enum TourMode { First, Replay }

/// One line of the screen helper's log, as sent by screen_agent.rs (`kind`: you | step | ask | check | done | error | stopped).
#[derive(serde::Deserialize, Clone)]
struct ScreenLine { kind: String, text: String }

fn refresh_stats(c: Ctx) {
    let course = c.course.get_untracked();
    if course.is_empty() { return; }
    spawn_local(async move {
        // a locked course errs: clear the last course's stats rather than show them under this tab
        c.stats.set(api::call::<Stats>("get_stats", json!({ "course_id": course })).await.ok());
    });
}

#[component]
fn Sidekick(#[prop(into)] pose: Signal<&'static str>, #[prop(optional)] size: Option<u32>) -> impl IntoView {
    let px = size.unwrap_or(160);
    view! {
        <img class="sidekick" src=move || format!("assets/mascot/{}.png", pose.get()) alt="" width=px height=px
            on:error=move |ev| { let el: web_sys::HtmlImageElement = event_target(&ev); el.set_src("assets/icon.png"); }/>
    }
}

const ZOOMS: [u32; 7] = [80, 90, 100, 110, 120, 130, 140];

fn apply_zoom(pct: u32) {
    if let Some(el) = document().document_element() {
        let _ = el.unchecked_ref::<web_sys::HtmlElement>().style().set_property("font-size", &format!("{}px", 16 * pct / 100));
    }
    if let Some(st) = storage() { let _ = st.set_item("zoom", &pct.to_string()); }
}

/// Whether the top bar can't hold the brand, the course tabs and its actions on one row (a narrow window,
/// large text, a long streak): then the tabs take a row of their own instead of scrolling out of sight.
/// Measured from the items' natural widths, so it holds whichever way the bar is laid out now.
fn bar_overflows(bar: &web_sys::Element) -> bool {
    let q = |s: &str| bar.query_selector(s).ok().flatten();
    let (Some(brand), Some(tabs), Some(actions)) = (q(".brand"), q(".tabs"), q(".top-actions")) else { return false };
    let px = |e: &web_sys::Element, prop: &str| window().get_computed_style(e).ok().flatten()
        .and_then(|st| st.get_property_value(prop).ok()).and_then(|v| v.trim_end_matches("px").parse::<f64>().ok()).unwrap_or(0.0);
    let w = |e: &web_sys::Element| e.get_bounding_client_rect().width();
    let list = tabs.query_selector_all("button").ok();
    let buttons: Vec<web_sys::Element> = list.map(|l| (0..l.length()).filter_map(|i| l.item(i)).filter_map(|n| n.dyn_into().ok()).collect()).unwrap_or_default();
    let tabs_w = buttons.iter().map(w).sum::<f64>() + px(&tabs, "column-gap") * (buttons.len().max(1) - 1) as f64
        + px(&tabs, "padding-left") + px(&tabs, "padding-right");
    let actions_w = actions.scroll_width() as f64;
    let room = bar.client_width() as f64 - px(bar, "padding-left") - px(bar, "padding-right");
    w(&brand) + tabs_w + actions_w + 2.0 * px(bar, "column-gap") > room + 0.5
}

fn saved_zoom() -> u32 {
    storage().and_then(|s| s.get_item("zoom").ok().flatten()).and_then(|v| v.parse().ok()).filter(|z| ZOOMS.contains(z)).unwrap_or(100)
}

fn ctx() -> Ctx { use_context::<Ctx>().expect("app context") }

/// Fire-and-store: runs a command once and exposes its result as a signal.
fn load<T: DeserializeOwned + Send + Sync + 'static>(cmd: &'static str, args: serde_json::Value) -> ReadSignal<Option<Result<T, String>>> {
    let (r, w) = signal(None);
    spawn_local(async move { w.set(Some(api::call(cmd, args).await)) });
    r
}

/// Loading skeleton / error / ready around a loaded value.
fn ready<T: Clone + Send + Sync + 'static, V: IntoView + 'static>(
    data: ReadSignal<Option<Result<T, String>>>,
    f: impl Fn(T) -> V + Send + Sync + 'static,
) -> impl IntoView {
    move || match data.get() {
        None => view! { <Skeleton/> }.into_any(),
        Some(Err(e)) => view! { <p class="notice error" role="alert">{e}</p> }.into_any(),
        Some(Ok(v)) => f(v).into_any(),
    }
}

#[component]
fn Skeleton() -> impl IntoView {
    view! {
        <div class="skeleton" aria-busy="true" aria-label="Loading">
            <span class="sk sk-title"></span><span class="sk"></span><span class="sk"></span><span class="sk sk-short"></span>
        </div>
    }
}

// Icons: one stroke family, 1.75 weight, 24px grid.
#[component]
fn Icon(name: &'static str) -> impl IntoView {
    let d = match name {
        "check" => "M5 12.5l4.2 4.2L19 7",
        "play" => "M8 5.5v13l10.5-6.5z",
        "left" => "M14.5 6l-6 6 6 6",
        "right" => "M9.5 6l6 6-6 6",
        "down" => "M6 9.5l6 6 6-6",
        "close" => "M6 6l12 12M18 6L6 18",
        "sun" => "M12 4v1.5M12 18.5V20M4 12h1.5M18.5 12H20M6.3 6.3l1.1 1.1M16.6 16.6l1.1 1.1M6.3 17.7l1.1-1.1M16.6 7.4l1.1-1.1M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7z",
        "moon" => "M19.5 14.5A7.5 7.5 0 0 1 9.5 4.5a7.5 7.5 0 1 0 10 10z",
        "gear" => "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2zM15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0z", // lucide "settings" (ISC)

        "send" => "M5 12h13M13 6l6 6-6 6",
        "steps" => "M5 6h3M5 12h7M5 18h11M19 4v16",
        "chat" => "M5 5h14a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-7l-4.5 3.5V16H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z",
        "minus" => "M6 12h12",
        "hide" => "M15 6l-6 6 6 6M20 4v16",
        "reload" => "M19.5 12a7.5 7.5 0 1 1-2.2-5.3M19.5 4v4h-4",
        "home" => "M4 11l8-6.5 8 6.5M6.5 9.5V19h11V9.5",
        "external" => "M14 4h6v6M20 4l-9 9M18 14v5H5V6h5",
        "maximize" => "M9 4H4v5M15 4h5v5M9 20H4v-5M15 20h5v-5",
        "restore" => "M4 9h5V4M20 9h-5V4M4 15h5v5M20 15h-5v5",
        "book" => "M4 5.5A2.5 2.5 0 0 1 6.5 3H20v15H6.5A2.5 2.5 0 0 0 4 20.5zM4 20.5A2.5 2.5 0 0 0 6.5 23H20v-5",
        "split" => "M4 4h16v16H4zM12 4v16",
        "more" => "M5 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0M11 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0M17 12a1 1 0 1 0 2 0a1 1 0 1 0-2 0",
        "lock" => "M7 11V8a5 5 0 0 1 10 0v3M6 11h12v9H6z",
        "tour" => "M12 3a9 9 0 1 0 0 18 9 9 0 1 0 0-18zM10.25 8.75v6.5l5-3.25z",
        "flame" => "M12 3c.5 3.2 4.5 5 4.5 9.3A4.5 4.5 0 0 1 12 17a4.5 4.5 0 0 1-4.5-4.7c0-1.8.8-3 1.8-4 .2 1.6 1 2.6 2.2 3C11 9 11 6 12 3zM12 21a6 6 0 0 0 6-6",
        _ => "",
    };
    view! {
        <svg class="icon" viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor"
            stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d=d/></svg>
    }
}

// ---------- theme ----------

fn storage() -> Option<web_sys::Storage> { window().local_storage().ok().flatten() }

/// Android build: one webview per app (the practice app opens in the browser), no self-update, no print.
fn android() -> bool {
    static A: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *A.get_or_init(|| window().navigator().user_agent().is_ok_and(|ua| ua.contains("Android")))
}

/// Phone-width windows show one session column at a time.
fn narrow() -> bool { window().inner_width().ok().and_then(|v| v.as_f64()).is_some_and(|w| w < 900.0) }

/// Where a session starts: everything on desktop; on Android no practice column, and on a phone the course alone.
fn session_start() -> Panels {
    if !android() { return Panels::ALL }
    Panels { course: Ps::Open, practice: Ps::Closed, sidekick: if narrow() { Ps::Collapsed } else { Ps::Open } }
}

/// "light" | "dark" | "system". Stored per machine; "system" follows macOS appearance.
fn apply_theme(t: &str) {
    if let Some(el) = document().document_element() {
        let _ = if t == "system" { el.remove_attribute("data-theme") } else { el.set_attribute("data-theme", t) };
    }
    if let Some(s) = storage() { let _ = s.set_item("theme", t); }
}

fn saved_theme() -> String {
    storage().and_then(|s| s.get_item("theme").ok().flatten()).unwrap_or_else(|| "system".into())
}

/// The course to open: the one picked last (stored under `course`) while it is still listed, else the first.
fn pick_course(saved: Option<&str>, ids: &[&str]) -> Option<String> {
    saved.filter(|s| ids.contains(s)).or_else(|| ids.first().copied()).map(str::to_string)
}

/// After something that can lock courses (a role change, Remove access code): if the course being shown is
/// now locked, switch to the first open one (and remember it); then reload the course list.
async fn leave_locked_course(c: Ctx) {
    if let Ok(list) = api::call::<Vec<CourseSummary>>("get_courses", json!({})).await {
        let cur = c.course.get_untracked();
        if list.iter().any(|co| co.id == cur && co.locked) {
            if let Some(co) = list.iter().find(|co| !co.locked) { save_course(&co.id); c.course.set(co.id.clone()) }
        }
    }
    c.courses_rev.update(|n| *n += 1);
}

fn save_course(id: &str) {
    if let Some(s) = storage() { let _ = s.set_item("course", id); }
}

fn system_is_dark() -> bool {
    window().match_media("(prefers-color-scheme: dark)").ok().flatten().is_some_and(|m| m.matches())
}

fn main() {
    console_error_panic_hook::set_once();
    apply_theme(&saved_theme());
    mount_to_body(Root);
}

/// Onboarding until a profile is saved on this device, then the app.
#[component]
fn Root() -> impl IntoView {
    let profile = load::<Option<Profile>>("get_profile", json!({}));
    move || match profile.get() {
        None => view! { <Skeleton/> }.into_any(),
        Some(Ok(Some(_))) => view! { <App/> }.into_any(),
        Some(Ok(None)) => view! { <Onboarding/> }.into_any(),
        Some(Err(e)) => view! { <p class="notice error" role="alert">{e}</p> }.into_any(),
    }
}

/// The profile as typed, checked with the rules `save_profile` applies again in Rust.
fn typed_profile(name: &str, email: &str, role: &str) -> Result<Profile, String> {
    let role: Role = serde_json::from_value(json!(role)).map_err(|_| "Choose your role.".to_string())?;
    clean_profile(Profile { name: name.into(), email: email.into(), role })
}

/// The id a role is stored under ("learner", "partner", "staff").
fn role_id(r: Role) -> String { serde_json::to_value(r).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default() }

/// The inline line under the email field once a Staff address can't be right (only when the fork
/// sets brand::STAFF_EMAIL_DOMAIN).
fn staff_email_error(email: &str, role: &str) -> Option<String> {
    let e = email.trim().to_lowercase();
    let (_, domain) = e.split_once('@')?;
    if role != role_id(Role::Staff) || domain.is_empty() { return None; }
    shared::staff_rule(Role::Staff, domain.trim_end_matches('.'), shared::brand::STAFF_EMAIL_DOMAIN).err()
}

/// The role select's options as (value, label, disabled, selected): a disabled "Choose your role"
/// placeholder first, selected while the stored role is empty or unknown. Each option carries its own
/// selectedness, so the select never falls back to the first enabled option when its value is set
/// before the options exist.
fn role_options(role: &str) -> Vec<(String, &'static str, bool, bool)> {
    let roles: Vec<(String, &str)> = shared::brand::ROLES.iter().map(|(r, l, _)| (role_id(*r), *l)).collect();
    let known = roles.iter().any(|(v, _)| v == role);
    let mut out = vec![(String::new(), "Choose your role", true, !known)];
    for (v, l) in roles { let sel = v == role; out.push((v, l, false, sel)) }
    out
}

/// Whether picking this role offers the access-code fields during onboarding (brand::ROLES_ASKING_CODE).
fn role_asks_code(role: &str) -> bool { shared::brand::ROLES_ASKING_CODE.iter().any(|r| role_id(*r) == role) }

/// Name, work email and role: the onboarding form and the Settings profile row.
#[component]
fn ProfileFields(name: RwSignal<String>, email: RwSignal<String>, role: RwSignal<String>) -> impl IntoView {
    let n = role_options("").len();
    view! {
        <label class="field" data-help="settings.name"><span>"Full name"</span>
            <input prop:value=move || name.get() on:input=move |ev| name.set(event_target_value(&ev)) autocomplete="name" required/>
        </label>
        <label class="field" data-help="settings.email"><span>"Work email"</span>
            <input type="email" prop:value=move || email.get() on:input=move |ev| email.set(event_target_value(&ev)) autocomplete="email" required
                aria-invalid=move || staff_email_error(&email.get(), &role.get()).is_some().to_string()/>
            {move || staff_email_error(&email.get(), &role.get()).map(|m| view! { <small class="field-error" role="alert">{m}</small> })}
        </label>
        <label class="field" data-help="settings.role"><span>"I am a"</span>
            <select on:change=move |ev| role.set(event_target_value(&ev)) required>
                {(0..n).map(|i| {
                    let (value, label, disabled, _) = role_options("")[i].clone();
                    view! { <option value=value disabled=disabled prop:selected=move || role_options(&role.get())[i].3>{label}</option> }
                }).collect_view()}
            </select>
        </label>
        <p class="meta" aria-live="polite">{move || role_note(&role.get())}</p>
    }
}

/// What each role opens, under the role select (the hints in brand::ROLES). No role opens a gated
/// course by itself: it takes an access code.
fn role_note(role: &str) -> &'static str {
    shared::brand::ROLES.iter().find(|(r, _, _)| role_id(*r) == role).map(|(_, _, hint)| *hint).unwrap_or("")
}

/// Username and access code (or the shared login) for gated courses: onboarding, Settings and the locked course.
#[component]
fn AccessFields(user: RwSignal<String>, code: RwSignal<String>, #[prop(optional)] required: bool) -> impl IntoView {
    view! {
        <label class="field"><span>"Username"</span>
            <input prop:value=move || user.get() on:input=move |ev| user.set(event_target_value(&ev)) autocomplete="username" required=required/>
        </label>
        <label class="field"><span>"Access code"</span>
            <input type="password" prop:value=move || code.get() on:input=move |ev| code.set(event_target_value(&ev)) autocomplete="current-password" required=required/>
        </label>
    }
}

/// Checks a username and access code in the backend and stores it on this device.
async fn unlock_with(user: String, code: String) -> Result<(), String> {
    if user.trim().is_empty() || code.trim().is_empty() { return Err("Enter the username and access code you were given.".into()) }
    api::call::<()>("unlock_courses", json!({ "username": user, "code": code })).await
}

#[component]
fn Onboarding() -> impl IntoView {
    let name = RwSignal::new(String::new());
    let email = RwSignal::new(String::new());
    let role = RwSignal::new(String::new());
    let (user, code) = (RwSignal::new(String::new()), RwSignal::new(String::new()));
    // optional: where they'll practise; saved to Settings → Practice app (empty is fine)
    let account = RwSignal::new(String::new());
    let account_error = move || { let a = account.get(); (!a.trim().is_empty()).then(|| shared::tenant::tenant_url(&a).err()).flatten() };
    let err = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    // Roles in brand::ROLES_ASKING_CODE enter their access code here; "I'll add it later" saves the profile without one
    // (Settings → Access, or the locked course, takes it afterwards).
    let finish = move |with_code: bool| {
        let p = match typed_profile(&name.get_untracked(), &email.get_untracked(), &role.get_untracked()) {
            Ok(p) => p,
            Err(e) => { err.set(Some(e)); return }
        };
        if let Some(e) = account_error() { err.set(Some(e)); return }
        let account = account.get_untracked().trim().to_string();
        err.set(None);
        busy.set(true);
        spawn_local(async move {
            if with_code {
                if let Err(e) = unlock_with(user.get_untracked(), code.get_untracked()).await { err.set(Some(e)); busy.set(false); return }
            }
            if let Err(e) = api::call::<()>("save_profile", json!({ "profile": p })).await { err.set(Some(e)); busy.set(false); return }
            if !account.is_empty() {
                // the backend checks it again (practice::validate_url) and stores it with https://
                let saved = match api::call::<Settings>("get_settings", json!({})).await {
                    Ok(st) => api::call::<()>("save_settings", json!({ "provider": st.provider, "practice_url": account })).await,
                    Err(e) => Err(e),
                };
                if let Err(e) = saved { err.set(Some(format!("Your profile is saved, but not your practice app address: {e} Fix it here, or leave it empty and add it later in Settings."))); busy.set(false); return }
            }
            let _ = window().location().reload();
        });
    };
    let partner = move || role_asks_code(&role.get());
    let submit = move |ev: leptos::ev::SubmitEvent| { ev.prevent_default(); finish(partner()) };
    view! {
        <div class="onboard">
            <section class="onboard-intro">
                <img class="logo logo-light" src="assets/logo.svg" alt=shared::brand::APP_NAME width="132" height="28"/>
                <img class="logo logo-dark" src="assets/logo-white.svg" alt=shared::brand::APP_NAME width="132" height="28"/>
                <Sidekick pose="wave" size=200/>
                <h1>{shared::brand::TAGLINE}</h1>
                <p class="standfirst">"Short lessons, hands-on practice in your own account, and a certificate when you finish."</p>
            </section>
            <form class="onboard-form" on:submit=submit>
                <h2>"Tell us about you"</h2>
                <ProfileFields name email role/>
                <label class="field"><span>{format!("Your {}", shared::brand::PRACTICE_NOUN)}</span>
                    <input id="onboard-account" prop:value=move || account.get() on:input=move |ev| account.set(event_target_value(&ev))
                        placeholder=format!("e.g. {}", shared::brand::PRACTICE_EXAMPLE) spellcheck="false" autocomplete="url" inputmode="url"
                        aria-describedby="onboard-account-help" aria-invalid=move || account_error().is_some().to_string()/>
                    {move || account_error().map(|m| view! { <small class="field-error" role="alert">{m}</small> })}
                    <small id="onboard-account-help" class="meta">"Optional. This is where you'll practise; you can add it later in Settings."</small>
                </label>
                <Show when=partner>
                    <AccessFields user code/>
                </Show>
                {move || err.get().map(|e| view! { <p class="notice error" role="alert">{e}</p> })}
                <div class="onboard-actions">
                    <button class="btn primary" type="submit" disabled=move || busy.get()>{move || match (busy.get(), partner()) {
                        (true, _) => "Saving…",
                        (false, true) => "Unlock and start",
                        (false, false) => "Start learning",
                    }}</button>
                    <Show when=partner>
                        <button class="btn" type="button" disabled=move || busy.get() on:click=move |_| finish(false)>"I'll add it later"</button>
                    </Show>
                </div>
                <p class="meta">"Saved on this computer only. Your name goes on your certificate."</p>
            </form>
        </div>
    }
}

#[component]
fn App() -> impl IntoView {
    let c = Ctx {
        page: RwSignal::new(Page::Course),
        course: RwSignal::new(String::new()),
        chat_lesson: RwSignal::new(None),
        chat_open: RwSignal::new(false),
        practice: RwSignal::new(Pmode::Off),
        session: RwSignal::new(None),
        coach: RwSignal::new(None),
        panels: RwSignal::new(Panels::ALL),
        keep: RwSignal::new(2),
        share: RwSignal::new(storage().and_then(|s| s.get_item("split_share").ok().flatten()).and_then(|v| v.parse().ok()).unwrap_or(0.42)),
        dragging: RwSignal::new(false),
        stats: RwSignal::new(None),
        outline: RwSignal::new(None),
        history: RwSignal::new(Vec::new()),
        screen: RwSignal::new(Vec::new()),
        screen_busy: RwSignal::new(false),
        screen_ask: RwSignal::new(None),
        sidekick_mode: RwSignal::new(sidekick::parse_mode(storage().and_then(|s| s.get_item("sidekick_mode").ok().flatten()).as_deref())),
        sidekick_event: RwSignal::new(None),
        lesson_help: RwSignal::new(Vec::new()),
        courses_rev: RwSignal::new(0),
        tour: RwSignal::new(None),
        tour_open: RwSignal::new(None),
    };
    provide_context(c);
    // one Sidekick for the sidekick and the session column: mode, pose, what it says
    provide_context(sidekick::Side::new());
    // one app-lifetime listener, so the log survives Sidekick's column being closed and reopened
    spawn_local(async move {
        api::listen("screen-agent", move |l: ScreenLine| {
            match l.kind.as_str() {
                "ask" => c.screen_ask.set(Some(l.text.clone())),
                "done" | "error" | "stopped" => { c.screen_busy.set(false); c.screen_ask.set(None) }
                _ => {}
            }
            c.screen.update(|v| v.push(l));
        }).await;
    });
    // the app tour: opens by itself after onboarding, and on any launch with a newer cut than the one watched
    spawn_local(async move {
        if let Ok(t) = api::call::<Option<AppTour>>("get_app_tour", json!({})).await {
            let due = tour_due(t.as_ref());
            c.tour.set(t);
            if due { c.tour_open.set(Some(TourMode::First)) }
        }
    });
    let theme = RwSignal::new(saved_theme());
    provide_context(theme);
    let zoom = RwSignal::new(saved_zoom());
    apply_zoom(zoom.get_untracked());
    // reloads when Settings changes the role, so the access lock and tab follow without a restart
    let courses = RwSignal::new(None::<Result<Vec<CourseSummary>, String>>);
    Effect::new(move |_| {
        c.courses_rev.track();
        spawn_local(async move { courses.set(Some(api::call::<Vec<CourseSummary>>("get_courses", json!({})).await)) });
    });
    Effect::new(move |_| {
        if let Some(Ok(list)) = courses.get() {
            if c.course.get_untracked().is_empty() {
                let saved = storage().and_then(|s| s.get_item("course").ok().flatten());
                let ids: Vec<&str> = list.iter().map(|co| co.id.as_str()).collect();
                if let Some(id) = pick_course(saved.as_deref(), &ids) { c.course.set(id) }
            }
        }
    });
    // a memo, so a course-list reload (Settings → Profile) doesn't remount the page being shown
    let show_unlock = Memo::new(move |_| c.page.get() != Page::Settings
        && courses.get().and_then(|r| r.ok()).is_some_and(|l| l.iter().any(|co| co.id == c.course.get() && co.locked)));
    // a memo, so switching course (Settings → Remove access code) doesn't remount the page either
    let no_course = Memo::new(move |_| c.course.with(String::is_empty));
    // remember where we came from (the back button pops this; a pop doesn't push)
    let going_back = StoredValue::new(false);
    Effect::new(move |prev: Option<Page>| {
        let cur = c.page.get();
        if let Some(p) = prev {
            if p != cur && !going_back.get_value() { c.history.update(|h| { h.push(p); if h.len() > 50 { h.remove(0); } }) }
        }
        going_back.set_value(false);
        cur
    });
    provide_context(going_back);
    // Back stays within the course: switching course (a tab, a lock) forgets the other course's pages
    Effect::new(move |prev: Option<String>| {
        let cur = c.course.get();
        if prev.is_some_and(|p| !p.is_empty() && p != cur) { c.history.set(Vec::new()) }
        cur
    });
    Effect::new(move |_| {
        c.courses_rev.track();
        let course = c.course.get();
        if course.is_empty() { return; }
        spawn_local(async move {
            c.outline.set(api::call::<CourseView>("get_course", json!({ "course_id": course })).await.ok());
        });
    });
    // stats follow the course and every navigation (cheap: one query)
    Effect::new(move |_| { c.course.track(); c.page.track(); refresh_stats(c); });
    // every page starts at the top; non-lesson pages talk to the course-wide tutor thread
    Effect::new(move |_| {
        let p = c.page.get();
        if let Some(el) = document().get_element_by_id("course-pane") { el.set_scroll_top(0) }
        window().scroll_to_with_x_and_y(0.0, 0.0);
        if !matches!(p, Page::Lesson(_)) { c.chat_lesson.set(None); c.lesson_help.set(Vec::new()) }
    });

    // ---- coaching session layout: one formula sizes the grid columns AND the practice pane ----
    let win_size = || (
        window().inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1280.0),
        window().inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(840.0),
    );
    let win = RwSignal::new(win_size());
    let _resize = window_event_listener(leptos::ev::resize, move |_| win.set(win_size()));
    let keep = c.keep;
    let shown = Memo::new(move |_| fit(c.panels.get(), c.share.get(), win.get().0, keep.get()));
    let cols = Memo::new(move |_| widths(shown.get(), c.share.get(), win.get().0));
    let pane = RwSignal::new(PaneNav::default());
    provide_context(pane);
    let pane_rect = move || -> Rect {
        let none = Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 };
        // the practice app steps aside while dragging, or it would swallow the pointer
        if c.dragging.get_untracked() || shown.get_untracked().practice != Ps::Open { return none; }
        let (cw, kw, _) = cols.get_untracked();
        // the toolbar (and its tab strip and notice, when there are any) stays above the pane, in the app's own UI
        let top = HEAD_H + pane.with_untracked(PaneNav::extra_h);
        Rect { x: RAIL_W + cw, y: top, w: kw, h: (win.get_untracked().1 - top).max(0.0) }
    };
    Effect::new(move |_| {
        cols.track(); c.dragging.track(); shown.track(); win.track(); pane.track();
        if c.practice.get_untracked() == Pmode::Off || android() { return; }
        let rect = pane_rect();
        spawn_local(async move { let _ = api::call::<()>("place_practice", json!({ "rect": rect })).await; });
    });
    // on a phone, opening one column folds the others
    let only = |p: &mut Panels, i: u8| if narrow() { for j in 0..3 { if j != i && p.get(j) == Ps::Open { p.set(j, Ps::Collapsed) } } };
    // both act on the columns as shown: a column folded for room opens again, and keeps its room
    let set_panel = move |i: u8, st: Ps| {
        if st == Ps::Open { keep.set(i) }
        c.panels.update(|p| { p.set(i, st); if st == Ps::Open { only(p, i) } *p = p.keep_one() })
    };
    let toggle = move |i: u8| {
        let s = if shown.get_untracked().get(i) == Ps::Open { Ps::Collapsed } else { Ps::Open };
        set_panel(i, s)
    };
    let practice_error = RwSignal::new(None::<String>);
    let open_practice = move |sess: (String, String, String)| {
        practice_error.set(None);
        let (kind, id, _) = sess.clone();
        c.session.set(Some(sess));
        c.coach.set(None);
        c.chat_open.set(false);
        c.panels.set(session_start());
        c.keep.set(2);
        c.practice.set(Pmode::Split);
        let course = c.course.get_untracked();
        // coaching state first (fast, local), then the practice app pane
        {
            let (course, kind, id) = (course.clone(), kind.clone(), id.clone());
            spawn_local(async move {
                if let Ok(st) = api::call::<CoachState>("get_coach", json!({ "course_id": course, "kind": kind, "id": id })).await { c.coach.set(Some(st)) }
            });
        }
        if android() { return; } // the practice app opens in the browser from the rail, on request
        let lesson_id = (kind == "lesson").then_some(id);
        let rect = pane_rect();
        spawn_local(async move {
            let r = api::call::<()>("open_practice", json!({ "course_id": course, "lesson_id": lesson_id, "rect": rect })).await;
            match r {
                Err(e) => practice_error.set(Some(e)),
                // the columns may have moved while the practice app was loading: place it where they are now
                Ok(()) if c.practice.get_untracked() != Pmode::Off => {
                    let rect = pane_rect();
                    let _ = api::call::<()>("place_practice", json!({ "rect": rect })).await;
                }
                Ok(()) => {}
            }
        });
    };
    // The backend pane exists only during a session. Whenever the UI has none (on every load, which also
    // covers a reload that kept an old pane alive, and on every way out of a session), close it.
    Effect::new(move |_| {
        if c.practice.get() == Pmode::Off && !android() {
            pane.set(PaneNav::default());
            spawn_local(async move { let _ = api::call::<()>("close_practice", json!({})).await; });
        }
    });
    // Close: hide the practice column (it keeps its page; Home starts over). With no session it closes
    // whatever pane is left. From the toolbar, Shift-Cmd/Ctrl-W in the app, or the macOS menu shortcut,
    // which works with focus inside the pane.
    let close_pane = move || {
        if c.practice.get_untracked() == Pmode::Off { spawn_local(async move { let _ = api::call::<()>("close_practice", json!({})).await; }); }
        else { set_panel(1, Ps::Closed) }
    };
    provide_context(PaneClose(Callback::new(move |_: ()| close_pane())));
    spawn_local(async move { api::listen::<PaneNav>("practice-nav", move |n| pane.set(n)).await });
    spawn_local(async move { api::listen::<()>("practice-close", move |_| close_pane()).await });
    let _close_key = window_event_listener(leptos::ev::keydown, move |ev| {
        if (ev.meta_key() || ev.ctrl_key()) && ev.shift_key() && ev.key().eq_ignore_ascii_case("w") {
            ev.prevent_default();
            close_pane();
            return;
        }
        // tabs: on macOS the Practice menu owns Cmd-T / Cmd-W / Ctrl-Tab (they work in the pane too)
        let mac = window().navigator().platform().is_ok_and(|p| p.starts_with("Mac"));
        if mac || c.practice.get_untracked() == Pmode::Off || pane.with_untracked(|p| p.tabs.is_empty()) { return; }
        let key = ev.key().to_ascii_lowercase();
        let action = match (ev.ctrl_key() || ev.meta_key(), ev.shift_key(), key.as_str()) {
            (true, false, "t") => "new",
            (true, false, "w") => "close",
            (true, false, "tab") if ev.ctrl_key() => "next",
            _ => return,
        };
        ev.prevent_default();
        let id = pane.with_untracked(|p| p.active);
        spawn_local(async move {
            // the last tab closes the practice column, as in a browser
            if api::call::<()>("practice_tab", json!({ "action": action, "id": id })).await.is_err() && action == "close" { close_pane() }
        });
    });
    provide_context(Callback::new(open_practice));
    // Android: the practice app lives in the browser; the backend opens it there
    let open_practice_app = move || {
        let Some((kind, id, _)) = c.session.get_untracked() else { return };
        let lesson_id = (kind == "lesson").then_some(id);
        let args = json!({ "course_id": c.course.get_untracked(), "lesson_id": lesson_id, "rect": Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 } });
        spawn_local(async move { if let Err(e) = api::call::<()>("open_practice", args).await { practice_error.set(Some(e)) } });
    };
    provide_context(practice_error);
    let close_practice = move |_| {
        c.practice.set(Pmode::Off);
        c.session.set(None);
        c.coach.set(None);
        spawn_local(async move { let _ = api::call::<()>("close_practice", json!({})).await; });
    };
    #[allow(unused)]
    let goto_stage = move |n: usize| {
        let Some((kind, id, _)) = c.session.get_untracked() else { return };
        let course = c.course.get_untracked();
        spawn_local(async move {
            if let Ok(st) = api::call::<CoachState>("set_coach_stage", json!({ "course_id": course, "kind": kind, "id": id, "stage": n })).await { c.coach.set(Some(st)) }
        });
    };

    let drag_move = window_event_listener(leptos::ev::pointermove, move |ev| {
        if !c.dragging.get_untracked() { return; }
        let (cw, kw, _) = cols.get_untracked();
        c.share.set(((ev.client_x() as f64 - RAIL_W) / (cw + kw).max(1.0)).clamp(0.2, 0.8));
    });
    let drag_end = window_event_listener(leptos::ev::pointerup, move |_| {
        if !c.dragging.get_untracked() { return; }
        if let Some(st) = storage() { let _ = st.set_item("split_share", &format!("{:.3}", c.share.get_untracked())); }
        c.dragging.set(false);
    });
    on_cleanup(move || { drag_move.remove(); drag_end.remove(); });

    // ---- updates: always-visible pill, like INDEX 0 ----
    #[derive(Clone, PartialEq)]
    enum Up { Checking, Current, Available(String), Downloading(u64), Ready(String), Failed(String) }
    let up = RwSignal::new(Up::Checking);
    let check = move || {
        up.set(Up::Checking);
        spawn_local(async move {
            up.set(match api::call::<Option<UpdateInfo>>("check_update", json!({})).await {
                Ok(Some(u)) => Up::Available(u.version),
                Ok(None) => Up::Current,
                Err(e) => Up::Failed(e),
            });
        });
    };
    if !android() { check(); }
    spawn_local(async move { api::listen::<u64>("update-progress", move |p| up.set(Up::Downloading(p))).await });
    // re-check every 30 minutes while the app is open
    let _poll = set_interval_with_handle(move || { if matches!(up.get_untracked(), Up::Current | Up::Failed(_)) { check() } }, std::time::Duration::from_secs(1800));
    let on_pill = move |_| match up.get_untracked() {
        Up::Current | Up::Failed(_) => check(),
        Up::Available(_) => {
            up.set(Up::Downloading(0));
            spawn_local(async move {
                up.set(match api::call::<String>("download_update", json!({})).await { Ok(v) => Up::Ready(v), Err(e) => Up::Failed(e) });
            });
        }
        Up::Ready(_) => spawn_local(async move { if let Err(e) = api::call::<()>("apply_update", json!({})).await { up.set(Up::Failed(e)) } }),
        _ => {}
    };
    let whats_new = RwSignal::new(None::<WhatsNew>);
    // release notes describe the desktop app (updates, panes); Android installs start clean
    if !android() { spawn_local(async move { if let Ok(w) = api::call::<Option<WhatsNew>>("get_whats_new", json!({})).await { whats_new.set(w) } }); }
    let dismiss_news = move |_| {
        whats_new.set(None);
        spawn_local(async move { let _ = api::call::<()>("mark_whats_new_seen", json!({})).await; });
    };

    let dark_now = move || match theme.get().as_str() { "dark" => true, "light" => false, _ => system_is_dark() };
    let toggle_theme = move |_| {
        let next = if dark_now() { "light" } else { "dark" };
        apply_theme(next);
        theme.set(next.into());
    };
    let step_zoom = move |dir: i32| {
        let i = ZOOMS.iter().position(|z| *z == zoom.get_untracked()).unwrap_or(2) as i32;
        let next = ZOOMS[(i + dir).clamp(0, ZOOMS.len() as i32 - 1) as usize];
        zoom.set(next);
        apply_zoom(next);
    };
    // the top bar wraps when its contents don't fit; re-measured whenever it or anything in it changes size
    let bar = NodeRef::<leptos::html::Header>::new();
    let bar_wrapped = RwSignal::new(false);
    Effect::new(move |_| {
        let Some(el) = bar.get() else { return };
        let el: web_sys::Element = el.into();
        let cb = wasm_bindgen::closure::Closure::<dyn Fn()>::new({ let el = el.clone(); move || bar_wrapped.set(bar_overflows(&el)) });
        let Ok(ro) = web_sys::ResizeObserver::new(cb.as_ref().unchecked_ref()) else { return };
        cb.forget(); // app-lifetime observer on the app-lifetime bar
        ro.observe(&el);
        for sel in [".tabs", ".top-actions", ".brand"] { if let Ok(Some(x)) = el.query_selector(sel) { ro.observe(&x) } }
    });
    // the tabs' own box can be squeezed, so a new course or a text-size step doesn't always resize it
    Effect::new(move |_| {
        courses.track(); zoom.track();
        request_animation_frame(move || if let Some(el) = bar.get_untracked() { bar_wrapped.set(bar_overflows(&el.into())) });
    });
    view! {
        <header class="top" node_ref=bar class:wrapped=move || bar_wrapped.get() class:hidden=move || c.practice.get() != Pmode::Off>
            <button class="brand" data-help="top.home" on:click=move |_| c.page.set(Page::Course) aria-label={concat!(shared::app_name!(), " home")}>
                <img class="logo logo-light" src="assets/logo.svg" alt=shared::brand::APP_NAME width="132" height="28"/>
                <img class="logo logo-dark" src="assets/logo-white.svg" alt="" width="132" height="28"/>
                {shared::brand::BRAND_SUFFIX.map(|w| view! { <span>{w}</span> })}
            </button>
            <nav class="tabs" aria-label="Courses">
                {move || courses.get().and_then(|r| r.ok()).unwrap_or_default().into_iter().map(|co| {
                    let id = co.id.clone();
                    let id2 = co.id.clone();
                    view! {
                        <button data-help="top.course-tab" class:active=move || c.course.get() == id && !matches!(c.page.get(), Page::Settings)
                            on:click=move |_| { save_course(&id2); c.course.set(id2.clone()); c.page.set(Page::Course) }>
                            {co.locked.then(|| view! { <span class="tab-lock" data-help="top.locked" title="Needs an access code"><Icon name="lock"/></span> })}{co.short}</button>
                    }
                }).collect_view()}
            </nav>
            <div class="top-actions">
                {move || c.stats.get().filter(|s| s.streak > 0).map(|s| view! {
                    <span class="streak-pill" data-help="top.streak" class:cold=!s.active_today title=if s.active_today { "Studied today" } else { "Study today to keep it going" }>
                        <Icon name="flame"/>{format!("{}-day streak", s.streak)}
                    </span>
                })}
                <button class="update-pill" data-help="top.update" on:click=on_pill class:hidden=android
                    class:busy=move || matches!(up.get(), Up::Checking | Up::Downloading(_))
                    class:ready=move || matches!(up.get(), Up::Available(_) | Up::Ready(_))
                    class:failed=move || matches!(up.get(), Up::Failed(_))
                    title=move || match up.get() { Up::Failed(e) => format!("Couldn't check for updates: {e}. Click to retry."), Up::Current => "Click to check again".into(), _ => String::new() }>
                    <span class="dot"></span>
                    {move || match up.get() {
                        Up::Checking => "Checking…".to_string(),
                        Up::Current => "Up to date".to_string(),
                        Up::Available(v) => format!("Update to {v}"),
                        Up::Downloading(p) => format!("Downloading… {p}%"),
                        Up::Ready(_) => "Restart to update".to_string(),
                        Up::Failed(_) => "Update check failed".to_string(),
                    }}
                </button>
                {move || c.tour.with(|t| t.is_some()).then(|| view! {
                    <button class="icon-btn" data-help="top.tour" on:click=move |_| c.tour_open.set(Some(TourMode::Replay)) aria-label="Course tour" title="Course tour"
                        class:active=move || c.tour_open.get().is_some()>
                        <Icon name="tour"/>
                    </button>
                })}
                <button class="icon-btn" data-help="top.theme" on:click=toggle_theme aria-label=move || if dark_now() { "Switch to light theme" } else { "Switch to dark theme" }>
                    {move || if dark_now() { view! { <Icon name="sun"/> }.into_any() } else { view! { <Icon name="moon"/> }.into_any() }}
                </button>
                <button class="icon-btn" data-help="top.settings" class:active=move || c.page.get() == Page::Settings on:click=move |_| c.page.set(Page::Settings) aria-label="Settings">
                    <Icon name="gear"/>
                </button>
            </div>
        </header>
        {move || (c.practice.get() != Pmode::Off).then(|| view! {
            <nav class="srail" aria-label="Session">
                <img class="srail-logo" src="assets/icon.png" alt={concat!(shared::app_name!())} width="28" height="28"/>
                <button class="srail-btn" data-help="rail.course" class:on=move || shown.get().course == Ps::Open on:click=move |_| toggle(0) title="Show or hide the course" aria-label="Course">
                    <Icon name="book"/><span>"Course"</span></button>
                <button class="srail-btn" data-help="rail.practice" class:on=move || !android() && shown.get().practice == Ps::Open
                    on:click=move |_| if android() { open_practice_app() } else { toggle(1) }
                    title=if android() { "Open your practice app in the browser" } else { "Show or hide the practice app" } aria-label="Practice">
                    <img src="assets/icon.png" alt="" width="20" height="20"/><span>{if android() { "Practice ↗" } else { "Practice" }}</span></button>
                <button class="srail-btn sidekick-btn" data-help="rail.sidekick" class:on=move || shown.get().sidekick == Ps::Open on:click=move |_| toggle(2) title={concat!("Show or hide ", shared::companion_name!())} aria-label={concat!(shared::companion_name!())}>
                    <img src="assets/mascot/idle.png" alt="" width="30" height="30"
                        on:error=move |ev| { let el: web_sys::HtmlImageElement = event_target(&ev); el.set_src("assets/icon.png"); }/><span>{concat!(shared::companion_name!())}</span></button>
                <span class="srail-rule"></span>
                <button class="srail-btn" data-help="rail.all" class:hidden=android class:on=move || shown.get() == Panels::ALL on:click=move |_| c.panels.set(Panels::ALL) title="Show all three" aria-label="Show all">
                    <Icon name="split"/><span>"All"</span></button>
                <span class="srail-spacer"></span>
                <button class="srail-btn tiny" data-help="text.bigger" on:click=move |_| step_zoom(1) disabled=move || zoom.get() == ZOOMS[ZOOMS.len() - 1] title="Larger text" aria-label="Larger text"><b>"A+"</b></button>
                <span class="srail-zoom">{move || format!("{}%", zoom.get())}</span>
                <button class="srail-btn tiny" data-help="text.smaller" on:click=move |_| step_zoom(-1) disabled=move || zoom.get() == ZOOMS[0] title="Smaller text" aria-label="Smaller text"><b>"A−"</b></button>
                <span class="srail-rule"></span>
                <button class="srail-btn end" data-help="rail.end" on:click=close_practice title="End session" aria-label="End session"><Icon name="close"/><span>"End"</span></button>
            </nav>
        })}
        <div class="workspace" class:session-on=move || c.practice.get() != Pmode::Off class:dragging=move || c.dragging.get()
            style=move || if c.practice.get() == Pmode::Off { String::new() } else {
                let (cw, kw, zw) = cols.get();
                format!("grid-template-columns: {cw}px {kw}px {zw}px")
            }>
            <section class="col col-course" class:collapsed=move || c.practice.get() != Pmode::Off && shown.get().course == Ps::Collapsed
                class:closed=move || c.practice.get() != Pmode::Off && shown.get().course == Ps::Closed>
                {move || (c.practice.get() != Pmode::Off).then(|| view! { <ColHead i=0 name="Course" set_panel/> })}
                <main id="course-pane">
                    {move || (c.page.get() != Page::Course).then(|| view! { <Crumbs/> })}
                    {move || {
                        if no_course.get() && c.page.get() != Page::Settings { return view! { <Skeleton/> }.into_any(); }
                        if show_unlock.get() { return view! { <Unlock/> }.into_any(); }
                        match c.page.get() {
                            Page::Course => view! { <CourseHome/> }.into_any(),
                            Page::Session(id) => view! { <SessionPage id/> }.into_any(),
                            Page::Lesson(id) => view! { <Lesson id/> }.into_any(),
                            Page::Check(id) => view! { <Check id/> }.into_any(),
                            Page::Exercise(id) => view! { <Exercise id/> }.into_any(),
                            Page::Final => view! { <Final/> }.into_any(),
                            Page::Certificate => view! { <CertificatePage/> }.into_any(),
                            Page::Settings => view! { <SettingsPage/> }.into_any(),
                        }
                    }}
                </main>
            </section>
            {move || (c.practice.get() != Pmode::Off).then(|| view! {
                <section class="col col-practice" class:collapsed=move || shown.get().practice == Ps::Collapsed class:closed=move || shown.get().practice == Ps::Closed>
                    <PaneBar set_panel/>
                    <div class="practice-slot" aria-hidden="true">
                        {move || match practice_error.get() {
                            Some(e) => view! { <p class="notice error">{e}</p> }.into_any(),
                            None => view! { <p class="meta">{move || if c.dragging.get() { "Release to resize" } else { "Loading your practice app…" }}</p> }.into_any(),
                        }}
                    </div>
                </section>
                <section class="col col-sidekick" class:collapsed=move || shown.get().sidekick == Ps::Collapsed class:closed=move || shown.get().sidekick == Ps::Closed>
                    <ColHead i=2 name={concat!(shared::companion_name!())} set_panel/>
                    <sidekick::pane::SidekickPane/>
                </section>
                {move || (shown.get().course == Ps::Open && shown.get().practice == Ps::Open).then(|| view! {
                    <div class="splitter" role="separator" aria-orientation="vertical" aria-label="Drag to resize"
                        title="Drag to resize · double-click to reset" style=move || format!("left: {}px", cols.get().0 - 14.0)
                        on:pointerdown=move |ev| { ev.prevent_default(); c.dragging.set(true) }
                        on:dblclick=move |_| { c.share.set(0.42); if let Some(st) = storage() { let _ = st.remove_item("split_share"); } }></div>
                })}
            })}
        </div>
        <div class="zoom" class:hidden=move || c.practice.get() != Pmode::Off role="group" aria-label="Text size">
            <button data-help="text.smaller" on:click=move |_| step_zoom(-1) disabled=move || zoom.get() == ZOOMS[0] aria-label="Smaller text">"A−"</button>
            <span>{move || format!("{}%", zoom.get())}</span>
            <button data-help="text.bigger" on:click=move |_| step_zoom(1) disabled=move || zoom.get() == ZOOMS[ZOOMS.len() - 1] aria-label="Larger text">"A+"</button>
        </div>
        <sidekick::Companion modal_open=Signal::derive(move || whats_new.get().is_some() || c.tour_open.get().is_some())/>
        {move || c.tour_open.get().and_then(|mode| c.tour.get().map(|tour| view! { <TourDialog tour mode/> }))}
        // the first-run tour goes first; release notes wait until it closes
        {move || whats_new.get().filter(|_| c.tour_open.get().is_none()).map(|w| view! { <NewsDialog w dismiss=Callback::new(dismiss_news)/> })}
    }
}

/// Release notes after an update: a modal like Start over's: Got it focused, Tab kept inside, Escape or
/// the backdrop dismisses, focus goes back to where it was.
#[component]
fn NewsDialog(w: WhatsNew, dismiss: Callback<()>) -> impl IntoView {
    let dialog_ref = NodeRef::<leptos::html::Section>::new();
    let ok_ref = NodeRef::<leptos::html::Button>::new();
    let keys = window_event_listener(leptos::ev::keydown, move |ev| match ev.key().as_str() {
        "Escape" => { ev.prevent_default(); ev.stop_immediate_propagation(); dismiss.run(()) }
        // one button: Tab and Shift-Tab stay on it
        "Tab" => { ev.prevent_default(); if let Some(b) = ok_ref.get_untracked() { let _ = b.focus(); } }
        _ => {}
    });
    let opener = document().active_element().and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());
    request_animation_frame(move || { if let Some(b) = ok_ref.get_untracked() { let _ = b.focus(); } });
    on_cleanup(move || {
        keys.remove();
        if let Some(o) = opener { let _ = o.focus(); }
    });
    view! {
        <div class="scrim" on:click=move |_| dismiss.run(())></div>
        <section class="news" node_ref=dialog_ref role="dialog" aria-modal="true" aria-labelledby="news-title">
            <div class="news-head">
                <img src="assets/mascot/celebrate.png" alt="" width="64" height="64"
                    on:error=move |ev| { let el: web_sys::HtmlImageElement = event_target(&ev); el.set_src("assets/icon.png"); }/>
                <div>
                    <p class="meta">{format!("Updated to {}", w.version)}</p>
                    <h2 id="news-title">{w.title.clone()}</h2>
                </div>
            </div>
            {(!w.new.is_empty()).then(|| view! { <h3>"New"</h3><ul class="news-list">{w.new.iter().map(|x| view! { <li>{x.clone()}</li> }).collect_view()}</ul> })}
            {(!w.fixed.is_empty()).then(|| view! { <h3>"Fixed"</h3><ul class="news-list fixed">{w.fixed.iter().map(|x| view! { <li>{x.clone()}</li> }).collect_view()}</ul> })}
            <button class="btn primary wide" node_ref=ok_ref on:click=move |_| dismiss.run(())>"Got it"</button>
        </section>
    }
}

fn course_args(extra: serde_json::Value) -> serde_json::Value { with_course(ctx().course, extra) }

fn with_course(course: RwSignal<String>, extra: serde_json::Value) -> serde_json::Value {
    let mut v = extra;
    v["course_id"] = json!(course.get_untracked());
    v
}

fn pad2(i: usize) -> String { format!("{:02}", i) }

/// Header for a session column: one hide button. A hidden column becomes a slim tab that expands it again.
#[component]
fn ColHead(i: u8, name: &'static str, set_panel: impl Fn(u8, Ps) + Copy + Send + Sync + 'static) -> impl IntoView {
    view! {
        <div class="col-head">
            <span class="col-name">{name}</span>
            <button class="icon-btn small" data-help="pane.col-hide" on:click=move |_| set_panel(i, Ps::Collapsed) title=format!("Hide {name}") aria-label=format!("Hide {name}")><Icon name="hide"/></button>
        </div>
        <button class="col-tab" data-help="pane.col-show" on:click=move |_| set_panel(i, Ps::Open) title=format!("Show {name}") aria-label=format!("Show {name}")>
            <Icon name="right"/><span>{name}</span>
        </button>
    }
}

/// The practice column's head: the pane's toolbar, always in the app's own UI (never inside the pane),
/// so there is always a way back and a way out. A notice row appears under it on a page outside the practice app.
#[component]
fn PaneBar(set_panel: impl Fn(u8, Ps) + Copy + Send + Sync + 'static) -> impl IntoView {
    let pane = use_context::<RwSignal<PaneNav>>().expect("pane nav");
    let PaneClose(close) = use_context::<PaneClose>().expect("pane close");
    // in a narrow column (style.css, @container practice) Forward, Reload, Home and Open in browser fold behind More,
    // which swaps them into the toolbar's own row: a menu dropping below it would sit under the native pane
    let more = RwSignal::new(false);
    let nav = move |action: &'static str| move |_| { more.set(false); spawn_local(async move { let _ = api::call::<()>("practice_nav", json!({ "action": action })).await; }); };
    view! {
        <div class="col-head pane-bar" class:more=move || more.get() role="toolbar" aria-label="Practice pane">
            <span class="col-name" title=move || pane.with(|p| p.host.clone())>"Practice"</span>
            <button class="icon-btn small pane-main" data-help="pane.back" on:click=nav("back") title="Back" aria-label="Back"><Icon name="left"/></button>
            <button class="icon-btn small pane-rare" data-help="pane.forward" on:click=nav("forward") title="Forward" aria-label="Forward"><Icon name="right"/></button>
            <button class="icon-btn small pane-rare" data-help="pane.reload" on:click=nav("reload") title="Reload" aria-label="Reload"><Icon name="reload"/></button>
            <button class="icon-btn small pane-rare" data-help="pane.home" on:click=nav("home") title="Home: your practice account" aria-label="Home"><Icon name="home"/></button>
            <button class="icon-btn small pane-rare" data-help="pane.browser" on:click=nav("browser") title="Open in browser" aria-label="Open in browser"><Icon name="external"/></button>
            <button class="icon-btn small pane-more" data-help="pane.more" on:click=move |_| more.update(|m| *m = !*m) aria-expanded=move || more.get().to_string()
                title=move || if more.get() { "Fewer buttons" } else { "More: Forward, Reload, Home, Open in browser" } aria-label="More pane buttons"><Icon name="more"/></button>
            <button class="icon-btn small pane-main" data-help="pane.hide" on:click=move |_| set_panel(1, Ps::Collapsed) title="Hide the practice app" aria-label="Hide the practice app"><Icon name="hide"/></button>
            <button class="icon-btn small pane-close pane-main" data-help="pane.close" on:click=move |_| close.run(()) title="Close the practice app (Shift-Cmd-W)" aria-label="Close the practice app"><Icon name="close"/></button>
        </div>
        <div class="pane-extra" style=move || format!("height: {}px", pane.with(PaneNav::extra_h))>
            {move || pane.with(|p| !p.tabs.is_empty()).then(|| view! { <PaneTabs/> })}
            {move || pane.with(|p| p.notice()).map(|t| view! { <p class="pane-notice" class:refused=move || pane.with(|p| p.refused) role="status" title=t>{t}</p> })}
        </div>
        <button class="col-tab" data-help="pane.col-show" on:click=move |_| set_panel(1, Ps::Open) title="Show the practice app" aria-label="Show the practice app">
            <Icon name="right"/><span>"Practice"</span>
        </button>
    }
}

/// The pane's tabs, like a browser's: each says whether it is Production or Development (amber, like
/// the app's own badge). + opens Production or Development; x closes a tab (the last one closes with
/// the pane). Cmd/Ctrl+T, Cmd/Ctrl+W and Ctrl+Tab do the same.
#[component]
fn PaneTabs() -> impl IntoView {
    let pane = use_context::<RwSignal<PaneNav>>().expect("pane nav");
    let menu = RwSignal::new(false);
    let note = RwSignal::new(None::<String>);
    // the strip scrolls once its tabs reach their minimum width: bring the + choices into view as they open
    Effect::new(move |_| if menu.get() { request_animation_frame(|| {
        if let Some(m) = document().query_selector(".pane-tab-menu").ok().flatten() {
            let o = web_sys::ScrollIntoViewOptions::new();
            o.set_block(web_sys::ScrollLogicalPosition::Nearest);
            o.set_inline(web_sys::ScrollLogicalPosition::Nearest);
            m.scroll_into_view_with_scroll_into_view_options(&o);
        }
    }) });
    let tab = move |action: &'static str, id: u64| spawn_local(async move {
        note.set(api::call::<()>("practice_tab", json!({ "action": action, "id": id })).await.err());
    });
    view! {
        <div class="pane-tabs" role="tablist" aria-label="Practice tabs">
            <For each=move || pane.with(|p| p.tabs.clone()) key=|t| (t.id, t.title.clone(), t.env.clone()) let:t>
                {
                    let id = t.id;
                    let on = move || pane.with(|p| p.active == id);
                    let (pill, env_name) = match t.env.as_str() { "prod" => ("Prod", "Production"), "dev" => ("Dev", "Development"), _ => ("", "") };
                    view! {
                        <div class="pane-tab" class:on=on class:dev=t.env == "dev">
                            <button role="tab" data-help="pane.tab" aria-selected=move || on().to_string() title=format!("{} · {}{}", t.title, t.host, if env_name.is_empty() { String::new() } else { format!(" · {env_name}") })
                                on:click=move |_| tab("switch", id)>
                                {(!pill.is_empty()).then(|| view! { <span class="env-pill" aria-label=env_name>{pill}</span> })}
                                <span class="pane-tab-title">{t.title.clone()}</span>
                            </button>
                            <button class="pane-tab-x" data-help="pane.tab-close" title="Close tab" aria-label=format!("Close tab {}", t.title) on:click=move |_| tab("close", id)><Icon name="close"/></button>
                        </div>
                    }
                }
            </For>
            <div class="pane-tab-new">
                <button class="pane-tab-plus" title="New tab (Cmd/Ctrl+T)" aria-label="New tab" aria-haspopup="menu" aria-expanded=move || menu.get().to_string()
                    on:click=move |_| { note.set(None); menu.update(|m| *m = !*m) }>"+"</button>
                {move || menu.get().then(|| view! {
                    <div class="pane-tab-menu" role="menu">
                        <button role="menuitem" title="New tab (Production)" on:click=move |_| { menu.set(false); tab("new", 0) }><span class="env-pill">"Prod"</span>"Production"</button>
                        <button role="menuitem" on:click=move |_| { menu.set(false); tab("dev", 0) }>
                            <span class="env-pill dev">"Dev"</span>"Development"</button>
                    </div>
                })}
            </div>
            {move || note.get().map(|n| view! { <span class="pane-tab-note" role="status" title=n.clone()>{n.clone()}</span> })}
        </div>
    }
}

/// Breadcrumb trail for the current page, plus a back button to the previous page.
#[component]
fn Crumbs() -> impl IntoView {
    let c = ctx();
    let going_back = use_context::<StoredValue<bool>>().expect("history flag");
    let back = move |_| {
        if let Some(p) = c.history.try_update(|h| h.pop()).flatten() {
            going_back.set_value(true);
            c.page.set(p);
        } else {
            c.page.set(Page::Course);
        }
    };
    let trail = move || -> Vec<(String, Option<Page>)> {
        let page = c.page.get();
        let Some(cv) = c.outline.get() else { return vec![] };
        let home = (cv.title.clone(), Some(Page::Course));
        let session_of = |pred: &dyn Fn(&SessionView) -> bool| cv.sessions.iter().enumerate().find(|(_, s)| pred(s))
            .map(|(i, s)| (format!("{} · {}", i + 1, s.title), Some(Page::Session(s.id.clone()))));
        let mut t = vec![home];
        match &page {
            Page::Session(id) => { if let Some((label, _)) = session_of(&|s| &s.id == id) { t.push((label, None)) } }
            Page::Lesson(id) => {
                if let Some(x) = session_of(&|s| s.lessons.iter().any(|l| &l.id == id)) { t.push(x) }
                if let Some(l) = cv.sessions.iter().flat_map(|s| &s.lessons).find(|l| &l.id == id) { t.push((format!("{} {}", l.number, l.title), None)) }
            }
            Page::Check(id) => { if let Some(x) = session_of(&|s| &s.check == id) { t.push(x) } t.push(("Session check".into(), None)) }
            Page::Exercise(id) => { if let Some(x) = session_of(&|s| &s.exercise == id) { t.push(x) } t.push(("Exercise".into(), None)) }
            Page::Final => t.push(("Final assessment".into(), None)),
            Page::Certificate => t.push(("Certificate".into(), None)),
            Page::Settings => t = vec![(cv.title.clone(), Some(Page::Course)), ("Settings".into(), None)],
            Page::Course => {}
        }
        t
    };
    view! {
        <nav class="crumbs" aria-label="Breadcrumb">
            <button class="crumb-back" data-help="nav.back" on:click=back title="Back to the previous page" aria-label="Back"><Icon name="left"/></button>
            <ol>
                {move || {
                    let t = trail();
                    let n = t.len();
                    t.into_iter().enumerate().map(|(i, (label, target))| {
                        let last = i + 1 == n;
                        view! {
                            <li>
                                {match target.filter(|_| !last) {
                                    Some(p) => view! { <button class="crumb" on:click=move |_| c.page.set(p.clone()) title=label.clone()>{label.clone()}</button> }.into_any(),
                                    None => view! { <span class="crumb current" aria-current="page" title=label.clone()>{label.clone()}</span> }.into_any(),
                                }}
                            </li>
                        }
                    }).collect_view()
                }}
            </ol>
        </nav>
    }
}

/// Course home, above the next lesson and the progress card: a greeting by first name, one line on
/// where the learner stands and what's next, and Sidekick beside it (copy in welcome.rs).
#[component]
fn WelcomeBanner(next: Option<(String, String, u32)>) -> impl IntoView {
    let c = ctx();
    let name = RwSignal::new(String::new());
    spawn_local(async move { if let Ok(Some(p)) = api::call::<Option<Profile>>("get_profile", json!({})).await { name.set(p.name) } });
    let copy = move || c.stats.get().map(|st| welcome::welcome(&name.get(), &st, next.as_ref().map(|(n, t, m)| (n.as_str(), t.as_str(), *m))));
    view! {
        <section class="welcome" aria-label="Welcome">
            <div class="welcome-text">
                {move || copy().map(|w| view! {
                    <p class="welcome-eyebrow">{w.eyebrow}</p>
                    <h2 class="welcome-title">{w.headline}</h2>
                    <p class="welcome-line">{w.line}</p>
                })}
            </div>
            <figure class="hero-sidekick">
                // a hello: his mood (from real progress) is on the progress card below (CardSidekick), not repeated here
                <SidekickTilt><Sidekick pose=Signal::derive(|| "wave") size=116/></SidekickTilt>
                <figcaption class="meta">{concat!(shared::companion_name!(), " waves hello")}</figcaption>
            </figure>
        </section>
    }
}

#[component]
fn CourseHome() -> impl IntoView {
    let c = ctx();
    let course = load::<CourseView>("get_course", course_args(json!({})));
    ready(course, move |cv| {
        let total: usize = cv.sessions.iter().map(|s| s.lessons.len()).sum();
        let done: usize = cv.sessions.iter().flat_map(|s| &s.lessons).filter(|l| l.completed).count();
        let pct = if total > 0 { done * 100 / total } else { 0 };
        // first unfinished lesson, with its session title
        let next = cv.sessions.iter().find_map(|s| s.lessons.iter().find(|l| !l.completed).map(|l| (l.clone(), s.title.clone())));
        let started = done > 0;
        // group sessions by phase, keeping course order
        let mut phases: Vec<(String, Vec<(usize, SessionView)>)> = Vec::new();
        for (i, s) in cv.sessions.iter().cloned().enumerate() {
            let ph = s.phase.clone().unwrap_or_else(|| "Sessions".into());
            match phases.last_mut() { Some((p, v)) if *p == ph => v.push((i, s)), _ => phases.push((ph, vec![(i, s)])) }
        }
        let lessons_total = total;
        view! {
            <div class="wide">
                <section class="hero">
                    <div class="hero-text">
                        <h1>{cv.title.clone()}</h1>
                        <p class="standfirst">{cv.summary.clone()}</p>
                        <div class="pills">
                            <span class="pill">{format!("{} sessions", cv.sessions.len())}</span>
                            <span class="pill warm">{format!("{lessons_total} lessons")}</span>
                        </div>
                    </div>
                </section>
                <div class="home-lead">
                <WelcomeBanner next=next.as_ref().map(|(l, _)| (l.number.clone(), l.title.clone(), l.duration_min))/>
                {next.map(|(l, ses)| {
                    let id = l.id.clone();
                    view! {
                        <button class="card continue" data-help="course.continue" data-sidekick-anchor="continue" on:click=move |_| c.page.set(Page::Lesson(id.clone()))>
                            <span class="card-kicker"><img src="assets/mascot/reading.png" alt="" width="36" height="36"
                                on:error=move |ev| { let el: web_sys::HtmlImageElement = event_target(&ev); el.set_src("assets/icon.png"); }/>
                                {if started { {concat!(shared::companion_name!(), " suggests picking up here")} } else { {concat!(shared::companion_name!(), " suggests starting here")} }}</span>
                            <span class="continue-title">{format!("{} {}", l.number, l.title)}</span>
                            <span class="continue-sub">{ses}</span>
                            <span class="continue-go"><Icon name="right"/></span>
                        </button>
                    }
                })}
                </div>

                <section class="card overview" data-help="course.progress" aria-label="Your progress" data-sidekick-anchor="ring">
                    <div class="ring-wrap">
                        <svg class="ring" viewBox="0 0 120 120" aria-hidden="true">
                            <circle cx="60" cy="60" r="52" class="ring-track"/>
                            <circle cx="60" cy="60" r="52" class="ring-fill" pathLength="100"
                                stroke-dasharray=format!("{} 100", pct) transform="rotate(-90 60 60)"/>
                        </svg>
                        <span class="ring-label"><strong>{format!("{pct}%")}</strong><span>"of the course"</span></span>
                    </div>
                    <dl class="stat-strip">
                        <div><dt>"Lessons done"</dt><dd>{format!("{done} / {total}")}</dd></div>
                        {move || c.stats.get().map(|st| view! {
                            <div><dt>"Day streak"</dt><dd class="flame-num">{st.streak}</dd></div>
                            <div><dt>"Session checks passed"</dt><dd>{format!("{} / {}", st.checks_passed, st.checks_total)}</dd></div>
                            <div><dt>"Best streak"</dt><dd>{st.best_streak}</dd></div>
                        })}
                    </dl>
                    {move || c.stats.get().map(|st| view! { <ActivityStrip days=st.activity/> })}
                    <CardSidekick/>
                </section>

                {phases.into_iter().map(|(phase, sessions)| view! {
                    <section class="phase">
                        <h2 class="phase-title"><span>{phase}</span></h2>
                        <div class="card-grid">
                            {sessions.into_iter().map(|(i, s)| {
                                let sid = s.id.clone();
                                let n = s.lessons.len();
                                let d = s.lessons.iter().filter(|l| l.completed).count();
                                view! {
                                    <button data-help="course.session" class=format!("card session-card p{}", i % 4) on:click=move |_| c.page.set(Page::Session(sid.clone()))>
                                        <span class="card-top">
                                            <span class="big-num">{pad2(i + 1)}</span>
                                            <span class="pill">{format!("{n} lessons")}</span>
                                        </span>
                                        <span class="card-title">{s.title.clone()}</span>
                                        <span class="card-desc">{s.outcome.clone()}</span>
                                        <span class="card-foot">
                                            <span class="mini-bar"><span style=format!("transform: scaleX({:.3})", if n > 0 { d as f64 / n as f64 } else { 0.0 })></span></span>
                                            <span class="meta">{format!("{d} of {n} done")}{s.check_best.map(|b| format!(" · check {:.0}%", b * 100.0))}</span>
                                        </span>
                                    </button>
                                }
                            }).collect_view()}
                        </div>
                    </section>
                }).collect_view()}

                <section class="phase">
                    <h2 class="phase-title"><span>"Finish"</span></h2>
                    <div class="card-grid">
                        <button class="card session-card final" data-help="course.final" on:click=move |_| c.page.set(Page::Final)>
                            <span class="card-top"><span class="big-num">{cv.final_minutes}</span><span class="pill">"minutes"</span></span>
                            <span class="card-title">"Final assessment"</span>
                            <span class="card-desc">{cv.final_blurb.clone()}</span>
                        </button>
                        <button class="card session-card final" data-help="course.certificate" on:click=move |_| c.page.set(Page::Certificate)>
                            <span class="card-top"><span class="big-num">"✦"</span><span class="pill warm">"certificate"</span></span>
                            <span class="card-title">"Your certificate"</span>
                            <span class="card-desc">"Issued once every session check and the final assessment are passed."</span>
                        </button>
                    </div>
                </section>
            </div>
        }
    })
}

/// One square per day, oldest first, in weeks (columns) of seven; intensity buckets from real counts.
#[component]
fn ActivityStrip(days: Vec<u32>) -> impl IntoView {
    let active = days.iter().filter(|d| **d > 0).count();
    let level = |n: u32| match n { 0 => 0, 1..=2 => 1, 3..=5 => 2, 6..=9 => 3, _ => 4 };
    let weeks: Vec<Vec<u32>> = days.chunks(7).map(|w| w.to_vec()).collect();
    view! {
        <div class="activity">
            <div class="activity-grid" role="img" aria-label=format!("Active on {active} of the last {} days", days.len())>
                {weeks.into_iter().map(|w| view! {
                    <div class="week">{w.into_iter().map(|n| view! { <span class=format!("day l{}", level(n)) title=format!("{n} activities")></span> }).collect_view()}</div>
                }).collect_view()}
            </div>
            <p class="meta">{if active == 0 { "Your activity will show here as you study.".to_string() } else { format!("Active on {active} of the last {} days", days.len()) }}</p>
        </div>
    }
}

/// Sidekick on the progress card: the pose for the learner's mood and a line with a real number.
/// He bobs gently and waves when the mood changes; a tap hops and opens Ask Sidekick, like the tab.
/// Off in Settings keeps him here (this is progress, not a nudge) but still; reduced motion stills him too.
#[component]
fn CardSidekick() -> impl IntoView {
    let c = ctx();
    let hi = RwSignal::new(false);
    let wave = move || {
        hi.set(false);
        request_animation_frame(move || hi.set(true));
        set_timeout(move || hi.set(false), std::time::Duration::from_millis(900));
    };
    Effect::new(move |prev: Option<Option<Mood>>| {
        let m = c.stats.with(|s| s.as_ref().map(sidekick_mood));
        if prev.is_some_and(|p| p.is_some() && p != m) { wave() }
        m
    });
    move || c.stats.get().map(|st| {
        let mood = sidekick_mood(&st);
        let (head, line) = mood_line(&st);
        let label = format!("{head}. {line} Ask {COMPANION}");
        view! {
            <button class="card-sidekick" class:still=move || c.sidekick_mode.get() == sidekick::Mode::Off class:hi=move || hi.get()
                data-mood=mood.label() aria-label=label title={concat!("Ask ", shared::companion_name!())}
                on:click=move |_| { wave(); set_timeout(move || c.chat_open.set(true), std::time::Duration::from_millis(260)) }>
                <span class="card-sidekick-say"><strong>{head}</strong><span>{line}</span></span>
                <span class="card-sidekick-bob"><Sidekick pose=Signal::derive(move || mood.pose()) size=150/></span>
            </button>
        }
    })
}

/// Sidekick leans toward the cursor: CSS 3D perspective tilt, eased, with a counter-moving shadow.
#[component]
fn SidekickTilt(children: Children) -> impl IntoView {
    let stage = NodeRef::<leptos::html::Div>::new();
    let handle = window_event_listener(leptos::ev::mousemove, move |ev| {
        let Some(el) = stage.get_untracked() else { return };
        let r = el.get_bounding_client_rect();
        let (cx, cy) = (r.left() + r.width() / 2.0, r.top() + r.height() / 2.0);
        let nx = ((ev.client_x() as f64 - cx) / 500.0).clamp(-1.0, 1.0);
        let ny = ((ev.client_y() as f64 - cy) / 400.0).clamp(-1.0, 1.0);
        let st = web_sys::HtmlElement::style(&el);
        let _ = st.set_property("--ry", &format!("{:.2}deg", nx * 18.0));
        let _ = st.set_property("--rx", &format!("{:.2}deg", -ny * 12.0));
        let _ = st.set_property("--tx", &format!("{:.1}px", nx * 10.0));
        let _ = st.set_property("--ty", &format!("{:.1}px", ny * 6.0));
    });
    on_cleanup(move || handle.remove());
    view! {
        <div class="sidekick-stage" node_ref=stage>
            <div class="sidekick-float"><div class="sidekick-tilt">{children()}</div></div>
            <span class="sidekick-shadow"></span>
        </div>
    }
}

#[component]
fn SessionPage(id: String) -> impl IntoView {
    let c = ctx();
    let course = load::<CourseView>("get_course", course_args(json!({})));
    ready(course, move |cv| {
        let Some((i, s)) = cv.sessions.into_iter().enumerate().find(|(_, s)| s.id == id) else {
            return view! { <p class="notice error">"Session not found."</p> }.into_any();
        };
        let (check, exercise) = (s.check.clone(), s.exercise.clone());
        view! {
            <div class=format!("page session-page p{}", i % 4)>
                <div class="session-head">
                    <span class="big-num">{pad2(i + 1)}</span>
                    <div>
                        <h1>{s.title}</h1>
                        <p class="standfirst">{s.outcome}</p>
                    </div>
                </div>
                <ol class="lessons">
                    {s.lessons.into_iter().map(|l| {
                        let lid = l.id.clone();
                        view! {
                            <li>
                                <button class="lesson-row" class:done=l.completed on:click=move |_| c.page.set(Page::Lesson(lid.clone()))>
                                    <span class="n">{l.number}</span>
                                    <span class="t">{l.title}</span>
                                    <span class="d">{l.has_video.then(|| view! { <Icon name="play"/> })}{format!("{} min", l.duration_min)}</span>
                                    <span class="state">{l.completed.then(|| view! { <Icon name="check"/> })}</span>
                                </button>
                            </li>
                        }
                    }).collect_view()}
                </ol>
                <div class="card-grid two" data-sidekick-anchor="session-actions">
                    <button class="card action-card" on:click=move |_| c.page.set(Page::Exercise(exercise.clone()))>
                        <span class="card-title">"Exercise"</span>
                        <span class="card-desc">"Build it in your practice app against published acceptance criteria."</span>
                    </button>
                    <button class="card action-card" on:click=move |_| c.page.set(Page::Check(check.clone()))>
                        <span class="card-title">"Session check"</span>
                        <span class="card-desc">{match s.check_best { Some(b) => format!("Best score {:.0}%. Ten questions, unlimited retries.", b * 100.0), None => "Ten questions, unlimited retries, feedback on every answer.".into() }}</span>
                    </button>
                </div>
            </div>
        }.into_any()
    })
}

#[derive(Clone)]
struct Neighbour { id: String, label: String }

#[component]
fn Lesson(id: String) -> impl IntoView {
    let c = ctx();
    let lesson = load::<LessonView>("get_lesson", course_args(json!({ "lesson_id": id })));
    let course = load::<CourseView>("get_course", course_args(json!({})));
    let rail_id = id.clone();
    let rail = move || course.get().and_then(|r| r.ok()).map(|cv| {
        let s = cv.sessions.iter().position(|s| s.lessons.iter().any(|l| l.id == rail_id)).unwrap_or(0);
        let ses = cv.sessions[s].clone();
        let sid = ses.id.clone();
        let current = rail_id.clone();
        view! {
            <nav class=format!("rail p{}", s % 4) data-help="lesson.rail" aria-label="Lessons in this session">
                <button class="rail-title" on:click=move |_| c.page.set(Page::Session(sid.clone()))><span class="rail-num">{pad2(s + 1)}</span>{ses.title}</button>
                <ol>
                    {ses.lessons.into_iter().map(|l| {
                        let lid = l.id.clone();
                        let here = l.id == current;
                        view! {
                            <li><button class:here=here class:done=l.completed aria-current=here.then_some("page")
                                on:click=move |_| c.page.set(Page::Lesson(lid.clone()))>
                                <span class="n">{l.number}</span><span>{l.title}</span>
                            </button></li>
                        }
                    }).collect_view()}
                </ol>
            </nav>
        }
    });
    let pn_id = id.clone();
    let prev_next = move || course.get().and_then(|r| r.ok()).map(|cv| {
        let all: Vec<Neighbour> = cv.sessions.iter().flat_map(|s| &s.lessons)
            .map(|l| Neighbour { id: l.id.clone(), label: format!("{} {}", l.number, l.title) }).collect();
        let i = all.iter().position(|n| n.id == pn_id).unwrap_or(0);
        let prev = i.checked_sub(1).and_then(|j| all.get(j).cloned());
        let next = all.get(i + 1).cloned();
        view! {
            <nav class="pager" data-help="lesson.pager" aria-label="Previous and next lesson">
                {prev.map(|p| view! { <button class="pager-btn" on:click=move |_| c.page.set(Page::Lesson(p.id.clone()))>
                    <Icon name="left"/><span><small>"Previous"</small>{p.label.clone()}</span></button> })}
                {next.map(|n| view! { <button class="pager-btn next" on:click=move |_| c.page.set(Page::Lesson(n.id.clone()))>
                    <span><small>"Next"</small>{n.label.clone()}</span><Icon name="right"/></button> })}
            </nav>
        }
    });
    view! {
        <div class="lesson-grid">
            <aside class="rail-wrap">{rail}</aside>
            <div class="lesson-main">
                {ready(lesson, move |l| view! { <LessonBody l/> })}
                {prev_next}
            </div>
        </div>
    }
}

/// The app tour in a full-screen modal player (the lesson player's markup: native controls, captions on,
/// chapter chips). First run: Escape and the backdrop do nothing, and Continue opens once the video has
/// played to the end (`ended`, or within 2 s of it) or can't play at all. A replay closes any way.
/// Focus stays inside while it is open and goes back where it was when it closes.
#[component]
fn TourDialog(tour: AppTour, mode: TourMode) -> impl IntoView {
    let c = ctx();
    let first = mode == TourMode::First;
    let video_ref = NodeRef::<leptos::html::Video>::new();
    let dialog_ref = NodeRef::<leptos::html::Section>::new();
    let failed = RwSignal::new(false);
    let watched = RwSignal::new(false);
    let can_continue = move || !first || watched.get() || failed.get();
    let version = tour.version;
    let yaml_duration = tour.duration_s;
    let check_end = move || {
        if watched.get_untracked() { return; }
        let Some(v) = video_ref.get_untracked() else { return };
        let d = if v.duration().is_finite() && v.duration() > 0.0 { v.duration() } else { yaml_duration };
        if tour_can_continue(v.ended(), false, v.current_time(), d) { watched.set(true) }
    };
    // Pause first, then unmount a tick later: closing drops the dialog's handlers (and the key listener that
    // may be asking), and a video removed while playing still fires timeupdate/pause at them.
    let close = move || {
        if let Some(v) = video_ref.get_untracked() { let _ = v.pause(); }
        set_timeout(move || c.tour_open.set(None), std::time::Duration::from_millis(30));
    };
    let finish = move || {
        if !can_continue() { return; }
        c.tour.update(|t| if let Some(t) = t { t.seen_version = tour_seen_after(t.seen_version, version) });
        spawn_local(async move { let _ = api::call::<()>("mark_app_tour_seen", json!({ "version": version })).await; });
        close();
    };
    // never trap the learner: a video that hasn't even loaded its metadata after 10 s counts as failed
    set_timeout(move || {
        if let Some(v) = video_ref.get_untracked() { if v.ready_state() == 0 { failed.set(true) } }
    }, std::time::Duration::from_secs(10));
    let focusables = move || -> Vec<web_sys::HtmlElement> {
        let Some(d) = dialog_ref.get_untracked() else { return vec![] };
        let Ok(list) = d.query_selector_all("button:not([disabled]), video[controls], a[href], [tabindex]:not([tabindex='-1'])") else { return vec![] };
        (0..list.length()).filter_map(|i| list.item(i)).filter_map(|n| n.dyn_into::<web_sys::HtmlElement>().ok()).collect()
    };
    let keys = window_event_listener(leptos::ev::keydown, move |ev| {
        match ev.key().as_str() {
            "Escape" => {
                ev.prevent_default();
                ev.stop_immediate_propagation();
                if !first { close() }
            }
            "Tab" => {
                let els = focusables();
                let (Some(head), Some(tail)) = (els.first(), els.last()) else { return };
                let active = document().active_element();
                let inside = active.as_ref().zip(dialog_ref.get_untracked()).is_some_and(|(a, d)| d.contains(Some(a)));
                let at = |el: &web_sys::HtmlElement| active.as_ref().is_some_and(|a| a == el.unchecked_ref::<web_sys::Element>());
                if !inside || (ev.shift_key() && at(head)) { ev.prevent_default(); let _ = if ev.shift_key() { tail.focus() } else { head.focus() }; }
                else if !ev.shift_key() && at(tail) { ev.prevent_default(); let _ = head.focus(); }
            }
            _ => {}
        }
    });
    // focus the player on open (Space plays); give focus back on close
    let opener = document().active_element().and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());
    request_animation_frame(move || { if let Some(v) = video_ref.get_untracked() { let _ = v.focus(); } });
    on_cleanup(move || {
        keys.remove();
        if let Some(o) = opener { let _ = o.focus(); }
    });
    let gate = move || match (first, failed.get(), watched.get()) {
        (_, true, _) => "The tour video can't play right now. Continue to the course and watch it later from Course tour in the top bar.",
        (false, _, _) => "Pause, rewind or jump to a chapter whenever you like. Close this any time.",
        (true, _, false) => "Continue unlocks when the tour ends. Pause, rewind or jump to a chapter whenever you like.",
        (true, _, true) => "All set. You can watch this again from Course tour in the top bar.",
    };
    view! {
        <div class="scrim tour-scrim" on:click=move |_| if !first { close() }></div>
        <section class="tour" class:replay=!first node_ref=dialog_ref role="dialog" aria-modal="true" aria-labelledby="tour-title" aria-describedby="tour-gate">
            <header class="tour-head">
                <div>
                    <p class="meta">{if first { "Before you start" } else { "Course tour" }}</p>
                    <h2 id="tour-title">{tour.title.clone()}</h2>
                </div>
                {(!first).then(|| view! {
                    <button class="icon-btn" on:click=move |_| close() aria-label="Close course tour" title="Close"><Icon name="close"/></button>
                })}
            </header>
            <figure class="video tour-video">
                <video node_ref=video_ref controls playsinline preload="auto" src=tour.src.clone() poster=tour.poster.clone()
                    on:loadedmetadata=move |_| {
                        // try to start on its own; a refused autoplay just leaves the play button to the learner
                        if let Some(Ok(p)) = video_ref.get_untracked().map(|v| v.play()) {
                            spawn_local(async move { let _ = wasm_bindgen_futures::JsFuture::from(p).await; });
                        }
                    }
                    on:timeupdate=move |_| check_end()
                    on:seeked=move |_| check_end()
                    on:ended=move |_| watched.set(true)
                    on:error=move |_| failed.set(true)>
                    {tour.captions.clone().map(|cap| view! { <track kind="captions" src=cap srclang="en" label="English" default/> })}
                </video>
                <Chapters video=video_ref chapters=tour.chapters.clone()/>
            </figure>
            <footer class="tour-foot">
                <p class="meta" id="tour-gate" aria-live="polite">{gate}</p>
                <button class="btn primary" aria-disabled=move || (!can_continue()).to_string() class:locked=move || !can_continue()
                    on:click=move |_| finish()>"Continue to the course"</button>
            </footer>
        </section>
    }
}

/// A video's chapter chips: each seeks the player to its start and plays. Lessons and the app tour share it.
#[component]
fn Chapters(video: NodeRef<leptos::html::Video>, chapters: Vec<Chapter>) -> impl IntoView {
    (!chapters.is_empty()).then(|| view! {
        <figcaption class="chapters" data-help="lesson.chapters">
            {chapters.into_iter().map(|ch| {
                let stamp = format!("{}:{:02}", (ch.t as u32) / 60, (ch.t as u32) % 60);
                view! {
                    <button class="chapter" on:click=move |_| { if let Some(el) = video.get_untracked() { el.set_current_time(ch.t); let _ = el.play(); } }>
                        <span class="ts">{stamp}</span>{ch.title}
                    </button>
                }
            }).collect_view()}
        </figcaption>
    })
}

#[component]
fn LessonBody(l: LessonView) -> impl IntoView {
    let c = ctx();
    c.chat_lesson.set(Some((l.id.clone(), format!("{} {}", l.number, l.title))));
    c.lesson_help.set(l.help.clone());
    let video_ref = NodeRef::<leptos::html::Video>::new();
    let completed = RwSignal::new(false);
    let lid = l.id.clone();
    let save = move |done: bool| {
        let pos = video_ref.get_untracked().map(|v| v.current_time()).unwrap_or(0.0);
        let args = with_course(c.course, json!({ "lesson_id": lid, "position_s": pos, "completed": done }));
        if done { completed.set(true) }
        let lesson = lid.clone();
        spawn_local(async move {
            let saved = api::call::<()>("save_progress", args).await.is_ok();
            if done && saved { c.sidekick_event.set(Some(SidekickEvent::LessonDone { lesson })) }
        });
    };
    let save_tick = save.clone();
    let last_saved = Rc::new(Cell::new(0.0f64));
    let save_end = save.clone();
    let resume = l.resume_at;
    // The video's listeners are ours to remove: a video still loading when the lesson goes (a course switch, Back)
    // fires loadedmetadata / timeupdate on the detached element, and a dropped `on:` handler throws
    // "closure invoked ... after being dropped". So they're added here and removed on cleanup, before they drop.
    type Listener = (&'static str, wasm_bindgen::closure::Closure<dyn FnMut()>);
    let listeners = StoredValue::new_local(None::<(web_sys::HtmlVideoElement, Vec<Listener>)>);
    video_ref.on_load(move |el| {
        let el: web_sys::HtmlVideoElement = el;
        let meta = { let el = el.clone(); move || {
            el.set_current_time(resume);
            // match the box to the video's own ratio (CSS default 16/10 reserves height until now)
            let (w, h) = (el.video_width(), el.video_height());
            if w > 0 && h > 0 { let _ = el.set_attribute("style", &format!("aspect-ratio: {w} / {h}")); }
        } };
        let tick = { let el = el.clone(); let (last_saved, save_tick) = (last_saved.clone(), save_tick.clone()); move || {
            // save every 10s of playback; good enough for resume
            let t = el.current_time();
            if (t - last_saved.get()).abs() >= 10.0 { last_saved.set(t); save_tick(false) }
        } };
        let end = { let s = save_end.clone(); move || s(true) };
        // metadata may already be in (a cached video) by the time this runs: resume now, not never
        if el.ready_state() >= 1 { meta() }
        let list: Vec<Listener> = vec![
            ("loadedmetadata", wasm_bindgen::closure::Closure::new(meta)),
            ("timeupdate", wasm_bindgen::closure::Closure::new(tick)),
            ("ended", wasm_bindgen::closure::Closure::new(end)),
        ];
        for (ev, cb) in &list { let _ = el.add_event_listener_with_callback(ev, cb.as_ref().unchecked_ref()); }
        listeners.set_value(Some((el, list)));
    });
    on_cleanup(move || {
        if let Some((el, list)) = listeners.try_update_value(Option::take).flatten() {
            for (ev, cb) in &list { let _ = el.remove_event_listener_with_callback(ev, cb.as_ref().unchecked_ref()); }
            let _ = el.pause();
        }
    });
    view! {
        <article class="lesson">
            <h1>{l.title.clone()}</h1>
            <p class="meta">{format!("Lesson {} · {} min", l.number, l.duration_min)}</p>
            <p class="standfirst">{l.objective.clone()}</p>
            {l.video.clone().map(|v| view! {
                <figure class="video" data-help="lesson.video">
                    <video node_ref=video_ref controls preload="metadata" src=v.url poster=v.poster>
                        {v.captions.map(|c| view! { <track kind="captions" src=c srclang="en" label="English" default/> })}
                    </video>
                    <Chapters video=video_ref chapters=v.chapters/>
                </figure>
            })}
            <div class="prose" inner_html=l.body_html.clone()></div>
            <section class="keypoints" aria-labelledby="kp">
                <h3 id="kp">"Key points"</h3>
                <ul>{l.key_points.iter().map(|k| view! { <li>{k.clone()}</li> }).collect_view()}</ul>
                {(!l.gotchas.is_empty()).then(|| view! {
                    <h3>"Gotchas"</h3>
                    <ul class="gotchas">{l.gotchas.iter().map(|g| view! { <li>{g.clone()}</li> }).collect_view()}</ul>
                })}
            </section>
            {(!l.help.is_empty()).then(|| view! {
                <section class="help-links" data-help="lesson.help" aria-labelledby="help-h">
                    <h3 id="help-h">{format!("Read more in {}", shared::brand::HELP_SITE_NAME)}</h3>
                    <ul>{l.help.iter().map(|h| {
                        let url = h.url.clone();
                        view! { <li><button class="help-link" title="Opens in your browser" on:click=move |_| {
                            let url = url.clone();
                            spawn_local(async move { let _ = api::call::<()>("open_help", json!({ "url": url })).await; });
                        }>{h.title.clone()}<span aria-hidden="true">" ↗"</span></button></li> }
                    }).collect_view()}</ul>
                </section>
            })}
            <section class="block" data-help="lesson.quick-check">
                <h2>"Quick check"</h2>
                <Quiz kind="lesson" id=l.id.clone() sections=vec![QuizSection { title: String::new(), questions: l.quick_check.clone() }]/>
            </section>
            <div class="complete-row" data-sidekick-anchor="complete">
                <button class="btn" data-help="lesson.complete" class:done=move || completed.get() on:click={ let s = save.clone(); move |_| s(true) }>
                    {move || if completed.get() { view! { <Icon name="check"/>"Marked complete" }.into_any() } else { view! { "Mark lesson complete" }.into_any() }}
                </button>
                <button class="btn quiet" data-help="lesson.ask" on:click=move |_| { if let Some(sk) = use_context::<sidekick::Side>() { sk.ask.set(true) } c.chat_open.set(true) }>{concat!("Ask ", shared::companion_name!(), " about this lesson")}</button>
            </div>
            <Practice kind="lesson" id=l.id.clone() label=format!("{} {}", l.number, l.title)/>
        </article>
    }
}

#[component]
fn Quiz(#[prop(into)] kind: String, id: String, sections: Vec<QuizSection>) -> impl IntoView {
    let blank: Vec<Vec<usize>> = sections.iter().flat_map(|s| &s.questions).map(|q| match q {
        QuestionView::Match { left, .. } => vec![usize::MAX; left.len()],
        _ => vec![],
    }).collect();
    let total = blank.len();
    let answers = RwSignal::new(blank);
    let graded = RwSignal::new(None::<Graded>);
    let error = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let course = ctx().course;
    let sidekick_event = ctx().sidekick_event;
    let answered = move || answers.with(|a| a.iter().filter(|x| !x.is_empty() && !x.contains(&usize::MAX)).count());
    let (quiz_kind, quiz_id) = (kind.clone(), id.clone());
    let submit = move |_| {
        let (quiz_kind, quiz_id) = (quiz_kind.clone(), quiz_id.clone());
        let args = json!({ "course_id": course.get_untracked(), "kind": kind, "id": id, "answers": answers.get_untracked() });
        busy.set(true);
        spawn_local(async move {
            match api::call::<Graded>("submit_quiz", args).await {
                Ok(g) => {
                    if quiz_kind == "check" || quiz_kind == "final" {
                        sidekick_event.set(Some(SidekickEvent::QuizGraded { quiz: quiz_kind.clone(), id: quiz_id.clone(), passed: g.passed }))
                    }
                    graded.set(Some(g)); error.set(None)
                }
                Err(e) => error.set(Some(e)),
            }
            busy.set(false);
        });
    };
    let mut idx = 0usize;
    view! {
        <div class="quiz" data-sidekick-protected="">
            {sections.into_iter().map(|s| {
                let qs = s.questions.into_iter().map(|q| { let i = idx; idx += 1; (i, q) }).collect::<Vec<_>>();
                view! {
                    {(!s.title.is_empty()).then(|| view! { <h3 class="quiz-section">{s.title.clone()}</h3> })}
                    <ol class="questions">
                        {qs.into_iter().map(|(i, q)| view! { <QuestionItem i q answers graded/> }).collect_view()}
                    </ol>
                }
            }).collect_view()}
            <div class="quiz-bar">
                <button class="btn primary" data-help="quiz.submit" on:click=submit disabled=move || busy.get()>
                    {move || if graded.get().is_some() { "Resubmit" } else { "Submit answers" }}
                </button>
                <span class="meta">{move || format!("{} of {total} answered", answered())}</span>
                {move || graded.get().map(|g| view! {
                    <span class=format!("result {}", if g.passed { "pass" } else { "fail" }) role="status">
                        <strong>{format!("{:.0}%", g.score * 100.0)}</strong>
                        {if g.passed { " Passed" } else { " Not yet. Read the feedback and try again." }}
                    </span>
                })}
                {move || error.get().map(|e| view! { <span class="notice error" role="alert">{e}</span> })}
            </div>
        </div>
    }
}

#[component]
fn QuestionItem(i: usize, q: QuestionView, answers: RwSignal<Vec<Vec<usize>>>, graded: RwSignal<Option<Graded>>) -> impl IntoView {
    let set = move |f: Box<dyn FnOnce(&mut Vec<usize>)>| answers.update(|a| f(&mut a[i]));
    let body = match q {
        QuestionView::Single { prompt, options } => view! {
            <p class="prompt">{prompt}</p>
            <div class="options" role="radiogroup">
                {options.into_iter().enumerate().map(|(o, text)| view! {
                    <label class="opt"><input type="radio" name=format!("q{i}") on:change=move |_| set(Box::new(move |a| *a = vec![o]))/><span>{text}</span></label>
                }).collect_view()}
            </div>
        }.into_any(),
        QuestionView::Multi { prompt, options } => view! {
            <p class="prompt">{prompt}<span class="hint">"Select all that apply"</span></p>
            <div class="options">
                {options.into_iter().enumerate().map(|(o, text)| view! {
                    <label class="opt"><input type="checkbox" on:change=move |ev| {
                        let on = event_target_checked(&ev);
                        set(Box::new(move |a| { a.retain(|x| *x != o); if on { a.push(o) } }))
                    }/><span>{text}</span></label>
                }).collect_view()}
            </div>
        }.into_any(),
        QuestionView::Match { prompt, left, right } => view! {
            <p class="prompt">{prompt}</p>
            <div class="match">
                {left.into_iter().enumerate().map(|(row, l)| {
                    let right = right.clone();
                    let label = format!("Match for: {l}");
                    view! {
                        <div class="match-row">
                            <span class="match-left">{l}</span>
                            <select aria-label=label on:change=move |ev| {
                                let v = event_target_value(&ev).parse().unwrap_or(usize::MAX);
                                set(Box::new(move |a| a[row] = v))
                            }>
                                <option value="">"Choose…"</option>
                                {right.into_iter().enumerate().map(|(r, t)| view! { <option value=r.to_string()>{t}</option> }).collect_view()}
                            </select>
                        </div>
                    }
                }).collect_view()}
            </div>
        }.into_any(),
    };
    view! {
        <li class="question">
            {body}
            {move || graded.get().and_then(|g| g.results.get(i).cloned()).map(|r| {
                let full = r.score >= 1.0;
                view! {
                    <div class=if full { "feedback ok" } else { "feedback" }>
                        <p class="feedback-score">{if full { "Correct".to_string() } else if r.score > 0.0 { format!("Partly right · {:.0}%", r.score * 100.0) } else { "Not quite".to_string() }}</p>
                        <ul>{r.feedback.into_iter().map(|f| view! { <li>{f}</li> }).collect_view()}</ul>
                    </div>
                }
            })}
        </li>
    }
}

#[component]
fn Practice(#[prop(into)] kind: String, id: String, label: String) -> impl IntoView {
    let open = use_context::<Callback<(String, String, String)>>().expect("session opener");
    let c = ctx();
    let busy = move || c.practice.get() != Pmode::Off;
    view! {
        <section class="practice">
            <img src="assets/mascot/pointing.png" alt="" width="96" height="96" class="sidekick practice-sidekick"
                on:error=move |ev| { let el: web_sys::HtmlImageElement = event_target(&ev); el.set_src("assets/icon.png"); }/>
            <div>
                <h3>{concat!("Coaching session with ", shared::companion_name!())}</h3>
                <p>{if android() {
                    {concat!(shared::companion_name!(), " coaches you here, stage by stage with hints, or answers anything straight in Ask ", shared::companion_name!(), ". Tap Practice ↗ in the session rail to open your practice app in the browser, then switch back and forth or use split screen.")}
                } else {
                    {concat!("Your practice app opens beside this page, with ", shared::companion_name!(), " in a column on the right. Coach walks you through it stage by stage with hints; Ask ", shared::companion_name!(), " answers anything straight.")}
                }}</p>
            </div>
            <button class="btn primary" data-help="lesson.practice" on:click=move |_| open.run((kind.clone(), id.clone(), label.clone())) disabled=busy>
                {move || if busy() { "Session open" } else { "Start coaching session" }}
                {move || (!busy()).then(|| view! { <span class="btn-orb" aria-hidden="true"><Icon name="right"/></span> })}
            </button>
            <PracticeError/>
        </section>
    }
}

/// Why the practice app didn't open (no URL set, a bad one, a webview error), where the learner is
/// looking: the course column's coaching block and Sidekick's column. When the fix is in Settings, a button
/// ends the session and goes there (the top bar is hidden during a session), to the Practice app field.
#[component]
fn PracticeError() -> impl IntoView {
    let c = ctx();
    let err = use_context::<RwSignal<Option<String>>>().expect("practice error");
    let to_settings = move |_| {
        c.practice.set(Pmode::Off);
        c.session.set(None);
        c.coach.set(None);
        err.set(None);
        spawn_local(async move { let _ = api::call::<()>("close_practice", json!({})).await; });
        c.page.set(Page::Settings);
        // once Settings has drawn, bring the URL field into view and focus it
        set_timeout(|| if let Some(el) = document().get_element_by_id("practice-url") {
            el.scroll_into_view_with_bool(false);
            if let Ok(h) = el.dyn_into::<web_sys::HtmlElement>() { let _ = h.focus(); }
        }, std::time::Duration::from_millis(400));
    };
    // on Android the practice app opens from the session rail, wherever the lesson is scrolled: bring it into view
    Effect::new(move |_| if err.with(Option::is_some) {
        request_animation_frame(|| if let Ok(Some(el)) = document().query_selector(".practice .practice-error") {
            let o = web_sys::ScrollIntoViewOptions::new();
            o.set_block(web_sys::ScrollLogicalPosition::Center);
            el.scroll_into_view_with_scroll_into_view_options(&o);
        });
    });
    move || err.get().map(|e| {
        let settings = e.contains("Settings");
        view! {
            <div class="notice error practice-error" role="alert">
                <p>{e}</p>
                {settings.then(|| view! { <button class="btn small" on:click=to_settings>"Open Settings"</button> })}
            </div>
        }
    })
}

/// Sidekick's column in a coaching session: guided Coach mode (stages + hint ladder) or free Ask mode.
/// The screen helper's log inside Sidekick's column: the learner's requests, each step Sidekick takes, Allow/Don't
/// questions, Check my build verdicts and the final answer.
#[component]
fn ScreenLog(answer: impl Fn(bool) + Copy + Send + Sync + 'static) -> impl IntoView {
    let c = ctx();
    let line = move |l: ScreenLine, last: bool| match l.kind.as_str() {
        "you" => view! { <p class="msg user">{l.text}</p> }.into_any(),
        "step" => view! { <p class="screen-step">{md::line(&l.text)}</p> }.into_any(),
        "ask" => view! {
            <div class="notice warn screen-ask" role="alertdialog">
                <p>{md::line(&l.text)}</p>
                {(last && c.screen_ask.get().is_some()).then(|| view! {
                    <div class="row"><button class="btn small primary" on:click=move |_| answer(true)>"Allow"</button>
                    <button class="btn small" on:click=move |_| answer(false)>"Not now"</button></div>
                })}
            </div> }.into_any(),
        "check" => {
            let v: serde_json::Value = serde_json::from_str(&l.text).unwrap_or_default();
            let (mark, cls) = match v["verdict"].as_str() { Some("met") => ("✓", "met"), Some("not_met") => ("✗", "not-met"), _ => ("?", "unsure") };
            view! { <p class=format!("screen-check {cls}")><b>{format!("{mark} Criterion {}", v["criterion"])}</b>" "{v["evidence"].as_str().unwrap_or_default().to_string()}</p> }.into_any()
        }
        "error" => view! { <p class="notice error" role="alert">{l.text}</p> }.into_any(),
        "blocked" => view! { <div class="notice screen-blocked" role="status"><p>{md::line(&l.text)}</p></div> }.into_any(),
        "stopped" => view! { <p class="notice">{l.text}</p> }.into_any(),
        _ => view! { <div class="msg assistant md">{md::reply(&l.text)}</div> }.into_any(),
    };
    view! {
        {move || {
            let all = c.screen.get();
            let newest_ask = all.iter().rposition(|l| l.kind == "ask");
            all.into_iter().enumerate().map(|(i, l)| line(l, Some(i) == newest_ask)).collect_view()
        }}
        {move || c.screen_busy.get().then(|| view! { <p class="screen-step working">{concat!(shared::companion_name!(), " is working in the practice app…")}</p> })}
    }
}

#[component]
fn Check(id: String) -> impl IntoView {
    let quiz = load::<QuizView>("get_check", course_args(json!({ "check_id": id })));
    view! {
        <div class="page">
            {ready(quiz, |q| view! {
                <h1>{q.title}</h1>
                <p class="meta">{format!("Ten questions · pass mark {:.0}% · unlimited retries, feedback on every answer", q.pass_mark * 100.0)}</p>
                <Quiz kind="check" id=q.id sections=q.sections/>
            })}
        </div>
    }
}

#[component]
fn Final() -> impl IntoView {
    let c = ctx();
    let quiz = load::<QuizView>("get_final", course_args(json!({})));
    view! {
        <div class="page">
            {ready(quiz, move |q| view! {
                <h1>{q.title}</h1>
                <p class="meta">{format!("{} minutes · pass mark {:.0}% · graded automatically", q.minutes.unwrap_or(60), q.pass_mark * 100.0)}</p>
                <details class="fixture" open>
                    <summary>"The brief and the supplied app"</summary>
                    <div class="prose" inner_html=q.intro_html></div>
                </details>
                <Quiz kind="final" id=q.id sections=q.sections/>
                <div class="cert-link">
                    <p>"Passed the assessment and every session check? Your certificate is ready."</p>
                    <button class="btn" on:click=move |_| c.page.set(Page::Certificate)>"View certificate"</button>
                </div>
            })}
        </div>
    }
}

#[component]
fn Exercise(id: String) -> impl IntoView {
    let (data, set_data) = signal(None::<Result<ExerciseView, String>>);
    {
        let args = course_args(json!({ "exercise_id": id }));
        spawn_local(async move { set_data.set(Some(api::call("get_exercise", args).await)) });
    }
    let draft = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let course = ctx().course;
    let submit = move |_| {
        let text = draft.get_untracked();
        if text.trim().is_empty() { return; }
        let args = json!({ "course_id": course.get_untracked(), "exercise_id": id, "submission": text });
        busy.set(true);
        spawn_local(async move {
            set_data.set(Some(api::call("submit_exercise", args).await));
            busy.set(false);
        });
    };
    view! {
        <div class="page">
            {ready(data, |x| view! {
                <h1>{x.title.clone()}</h1>
                <p class="meta">{format!("{} minutes in your practice app", x.minutes)}</p>
                <div class="prose" inner_html=x.brief_html></div>
                {(!x.setup.is_empty()).then(|| view! { <h3>"Before you start"</h3><ul class="plain">{x.setup.into_iter().map(|s| view! { <li>{md::line(&s)}</li> }).collect_view()}</ul> })}
                <Practice kind="exercise" id=x.id.clone() label=x.title.clone()/>
                <section class="criteria">
                    <h3>"Acceptance criteria"</h3>
                    <ol>{x.acceptance_criteria.into_iter().map(|c| view! { <li>{md::line(&c)}</li> }).collect_view()}</ol>
                </section>
                {x.last_review.map(|r| view! { <section class="block"><h2>"AI review"</h2><p class="meta">"Advisory only. It never affects your score."</p><div class="review md">{md::reply(&r)}</div></section> })}
                {x.walkthrough_html.map(|w| view! { <section class="block"><h2>"Reference walkthrough"</h2><div class="prose" inner_html=w></div></section> })}
            })}
            <section class="block" data-sidekick-protected="">
                <h2>"Your write-up"</h2>
                <textarea class="writeup" aria-label="Your write-up" prop:value=move || draft.get() on:input=move |ev| draft.set(event_target_value(&ev))
                    placeholder="Describe what you built and how it meets each criterion. Submitting unlocks the reference walkthrough."></textarea>
                <div class="quiz-bar">
                    <button class="btn primary" on:click=submit disabled=move || busy.get()>{move || if busy.get() { "Reviewing…" } else { "Submit write-up" }}</button>
                    <span class="meta">"Saved on this computer only, and sent to your AI provider for an advisory review. It never affects your score."</span>
                </div>
            </section>
        </div>
    }
}

/// A chat message: the learner's words as typed, a model reply formatted (md.rs: elements, never HTML).
fn msg_view(m: Msg) -> AnyView {
    if m.role == "user" { view! { <p class="msg user">{m.content}</p> }.into_any() }
    else { view! { <div class=format!("msg md {}", m.role)>{md::reply(&m.content)}</div> }.into_any() }
}

/// Shown instead of a gated course until a username and access code are entered.
#[component]
fn Unlock() -> impl IntoView {
    let user = RwSignal::new(String::new());
    let code = RwSignal::new(String::new());
    let err = RwSignal::new(None::<String>);
    let busy = RwSignal::new(false);
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        spawn_local(async move {
            match unlock_with(user.get_untracked(), code.get_untracked()).await {
                Ok(()) => { let _ = window().location().reload(); }
                Err(e) => { err.set(Some(e)); busy.set(false) }
            }
        });
    };
    view! {
        <div class="page">
            <h1>"This course needs an access code"</h1>
            <p class="standfirst">"Enter the username and access code you were given."</p>
            <form class="unlock" on:submit=submit>
                <AccessFields user code required=true/>
                <button class="btn primary" type="submit" disabled=move || busy.get()>{move || if busy.get() { "Checking…" } else { "Unlock course" }}</button>
                {move || err.get().map(|e| view! { <p class="notice error" role="alert">{e}</p> })}
            </form>
            <p class="meta">"No code yet? Ask the person who runs this course for one."</p>
        </div>
    }
}

#[component]
fn CertificatePage() -> impl IntoView {
    let course = ctx().course;
    let page = ctx().page;
    let (data, set_data) = signal(None::<Result<CertificateView, String>>);
    let fetch = move |name: Option<String>| {
        let args = json!({ "course_id": course.get_untracked(), "name": name });
        spawn_local(async move { set_data.set(Some(api::call("get_certificate", args).await)) });
    };
    fetch(None);
    let name = RwSignal::new(String::new());
    let print_err = RwSignal::new(None::<String>);
    let print = move |_| spawn_local(async move {
        if let Err(e) = api::call::<()>("print_certificate", json!({ "course_id": course.get_untracked() })).await { print_err.set(Some(e)) }
    });
    view! {
        <div class="page">
            {ready(data, move |c| {
                if !c.missing.is_empty() {
                    return view! {
                        <h1>"Your certificate"</h1>
                        <p class="standfirst">"Pass the final assessment and every session check to earn it."</p>
                        <p class="meta">{format!("Lessons done: {} / {}", c.lessons_done, c.lessons_total)}</p>
                        {(c.lessons_total > 0 && c.lessons_done == c.lessons_total).then(|| view! {
                            <p>"All lessons done. Take each session check and the final assessment to earn the certificate."</p>
                        })}
                        <section class="criteria" data-sidekick-anchor="todo">
                            <h3>"Still to do"</h3>
                            <ul class="todo">{c.missing.into_iter().map(|m| {
                                let to = m.check.map(Page::Check).unwrap_or(Page::Final);
                                let cta = if to == Page::Final { "Take the final" } else { "Take the check" };
                                view! { <li>{m.label}" "<button class="btn" on:click=move |_| page.set(to.clone())>{cta}</button></li> }
                            }).collect_view()}</ul>
                        </section>
                    }.into_any();
                }
                if c.id.is_none() {
                    return view! {
                        <h1>"You passed"</h1>
                        <p class="standfirst">"Every check and the final assessment are done. Enter your name as it should appear on the certificate."</p>
                        <label class="field" data-help="settings.name"><span>"Full name"</span>
                            <div class="inline">
                                <input prop:value=move || name.get() on:input=move |ev| name.set(event_target_value(&ev)) autocomplete="name"/>
                                <button class="btn primary" on:click=move |_| { let n = name.get_untracked(); if !n.trim().is_empty() { fetch(Some(n)) } }>"Issue certificate"</button>
                            </div>
                        </label>
                    }.into_any();
                }
                let score = c.final_score.map(|s| format!("{:.0}%", s * 100.0)).unwrap_or_default();
                view! {
                    <div class="cert-actions no-print">
                        <button class="btn primary" on:click=print>"Save as PDF or print"</button>
                        {move || print_err.get().map(|e| view! { <span class="notice error">{e}</span> })}
                    </div>
                    <article class="certificate" aria-label="Certificate of completion">
                        <img class="cert-logo" src="assets/logo.svg" alt=shared::brand::APP_NAME/>
                        <h1 class="cert-title">"Certificate of Completion"</h1>
                        <p class="cert-line">"This certifies that"</p>
                        <p class="cert-name">{c.name}</p>
                        <p class="cert-line">"has completed"</p>
                        <p class="cert-course">{c.course}</p>
                        <div class="petals" aria-hidden="true"><span class="p0"></span><span class="p1"></span><span class="p2"></span><span class="p3"></span></div>
                        <dl class="cert-meta">
                            <div><dt>"Issued"</dt><dd>{c.issued_on.unwrap_or_default()}</dd></div>
                            <div><dt>"Final assessment"</dt><dd>{score}</dd></div>
                            <div><dt>"Certificate ID"</dt><dd>{c.id.unwrap_or_default()}</dd></div>
                        </dl>
                    </article>
                }.into_any()
            })}
        </div>
    }
}

const PROVIDERS: [(&str, &str, &str); 5] = [
    ("chatgpt", "ChatGPT plan", "Use your ChatGPT Plus or Pro plan, no API key. Sign in once in OpenAI's Codex app. Not the screen helper."),
    ("openai", "OpenAI", "Paste your OpenAI API key, then pick a model."),
    ("anthropic", "Anthropic", "Paste your Anthropic API key. Tutor and reviews only, not practice mode."),
    ("ollama", "Ollama", "Runs on this machine. No key, nothing leaves it."),
    ("custom", "Custom API", "Any OpenAI-compatible endpoint: OpenRouter, Groq, vLLM, a company gateway."),
];

/// What each provider can drive, in PROVIDERS order. Mirrors llm.rs: the screen helper needs
/// OpenAI's Responses API.
const CAPS: [(&str, [bool; 5]); 3] = [
    ({concat!("Ask ", shared::companion_name!(), " and coaching")}, [true; 5]),
    ("Reviews of your exercises", [true; 5]),
    ("Screen helper: looks at, works in and checks your practice app", [false, true, false, false, true]),
];

fn provider_for(kind: &str, endpoint: &str, model: &str) -> Provider {
    let model = model.trim().to_string();
    match kind {
        "openai" => Provider::OpenAi { base_url: OPENAI_BASE_URL.into(), model },
        "custom" => Provider::OpenAi { base_url: endpoint.trim().into(), model },
        "anthropic" => Provider::Anthropic { model },
        "chatgpt" => Provider::ChatGpt { model },
        _ => Provider::Ollama { host: endpoint.trim().into(), model },
    }
}

#[component]
fn SettingsPage() -> impl IntoView {
    let theme = use_context::<RwSignal<String>>().expect("theme");
    let sidekick_mode = ctx().sidekick_mode;
    let cx = ctx();
    let kind = RwSignal::new("openai".to_string());
    let endpoint = RwSignal::new(String::new());
    let model = RwSignal::new(String::new());
    let key = RwSignal::new(String::new());
    let has_key = RwSignal::new(false);
    let practice = RwSignal::new(String::new());
    let practice_dev = RwSignal::new(String::new());
    let models = RwSignal::new(None::<Result<Vec<String>, String>>);
    let loading = RwSignal::new(false);
    let status = RwSignal::new(None::<Result<String, String>>);
    let busy = RwSignal::new(false);

    let current = move || provider_for(&kind.get_untracked(), &endpoint.get_untracked(), &model.get_untracked());
    let needs_key = move || !matches!(kind.get().as_str(), "ollama" | "chatgpt");
    let load_models = move || {
        loading.set(true);
        models.set(None);
        let p = current();
        spawn_local(async move {
            let r = api::call::<Vec<String>>("list_models", json!({ "provider": p })).await;
            if let Ok(list) = &r {
                // keep the saved choice if it's still offered, else preselect the first
                if !list.contains(&model.get_untracked()) { model.set(list.first().cloned().unwrap_or_default()) }
            }
            models.set(Some(r));
            loading.set(false);
        });
    };

    spawn_local(async move {
        if let Ok(s) = api::call::<Settings>("get_settings", json!({})).await {
            let (k, ep, m) = match s.provider {
                Provider::Ollama { host, model } => ("ollama", host, model),
                Provider::OpenAi { base_url, model } if base_url.trim_end_matches('/') == OPENAI_BASE_URL => ("openai", base_url, model),
                Provider::OpenAi { base_url, model } => ("custom", base_url, model),
                Provider::Anthropic { model } => ("anthropic", String::new(), model),
                Provider::ChatGpt { model } => ("chatgpt", String::new(), model),
            };
            kind.set(k.into()); endpoint.set(ep); model.set(m); has_key.set(s.has_api_key); practice.set(s.practice_url.clone());
            // Development: an optional second address (a staging or sandbox copy of the same app)
            practice_dev.set(s.practice_dev_url.clone());
            if k == "ollama" || k == "chatgpt" || s.has_api_key { load_models() }
        }
    });

    let choose = move |k: &'static str| {
        if kind.get_untracked() == k { return; }
        kind.set(k.into());
        endpoint.set(if k == "ollama" { "http://127.0.0.1:11434".into() } else { String::new() });
        model.set(String::new());
        key.set(String::new());
        status.set(None);
        models.set(None);
        // each provider keeps its own key: ask the backend whether this one already has one
        spawn_local(async move {
            let p = provider_for(k, &endpoint.get_untracked(), "");
            let r = api::call::<Vec<String>>("list_models", json!({ "provider": p })).await;
            has_key.set(!matches!(k, "ollama" | "chatgpt") && r.is_ok());
            if r.is_ok() || k == "ollama" || k == "chatgpt" {
                if let Ok(list) = &r { model.set(list.first().cloned().unwrap_or_default()) }
                models.set(Some(r));
            }
        });
    };

    let connect = move |_| {
        let k = key.get_untracked();
        if k.trim().is_empty() && needs_key() && !has_key.get_untracked() {
            status.set(Some(Err("Paste your API key first.".into())));
            return;
        }
        status.set(None);
        let p = current();
        spawn_local(async move {
            if !k.trim().is_empty() {
                if let Err(e) = api::call::<()>("set_api_key", json!({ "provider": p, "key": k })).await {
                    status.set(Some(Err(e)));
                    return;
                }
                key.set(String::new());
                has_key.set(true);
            }
            load_models();
        });
    };

    let save = move |test: bool| {
        if model.get_untracked().trim().is_empty() {
            status.set(Some(Err("Pick a model first.".into())));
            return;
        }
        busy.set(true);
        status.set(None);
        let p = current();
        spawn_local(async move {
            let r = async {
                api::call::<()>("save_settings", json!({ "provider": p, "practice_url": practice.get_untracked(), "practice_dev_url": practice_dev.get_untracked() })).await?;
                if test {
                    let reply = api::call::<String>("test_provider", json!({})).await?;
                    Ok(format!("Connected. The model replied: “{}”", reply.trim()))
                } else {
                    Ok("Settings saved.".to_string())
                }
            }.await;
            status.set(Some(r));
            busy.set(false);
        });
    };

    view! {
        <div class="page settings">
            <h1>"Settings"</h1>
            <ProfileSettings/>
            <AccessSettings/>
            <section class="block">
                <h2>"Appearance"</h2>
                <div class="segmented" data-help="settings.theme" role="radiogroup" aria-label="Theme">
                    {[("light", "Light"), ("dark", "Dark"), ("system", "Match system")].into_iter().map(|(v, label)| view! {
                        <label class:on=move || theme.get() == v>
                            <input type="radio" name="theme" prop:checked=move || theme.get() == v on:change=move |_| { apply_theme(v); theme.set(v.into()) }/>
                            {label}
                        </label>
                    }).collect_view()}
                </div>
            </section>
            {move || cx.tour.with(|t| t.is_some()).then(|| view! {
                <section class="block">
                    <h2>"Course tour"</h2>
                    <p class="meta">"A short video of where everything is in the app."</p>
                    <button class="link-btn" data-help="settings.tour" on:click=move |_| cx.tour_open.set(Some(TourMode::Replay))>
                        <Icon name="tour"/>"Watch the course tour again"
                    </button>
                </section>
            })}
            <section class="block">
                <h2>{concat!(shared::companion_name!(), " companion")}</h2>
                <p class="meta">{concat!(shared::companion_name!(), " lives in the tab on the right edge: it explains controls, nudges when you seem stuck, and coaches you. Quiet keeps ", shared::companion_name!(), " in the tab and silent until you click it. Off leaves just the tab, for asking.")}</p>
                <div class="segmented" data-help="settings.sidekick-mode" role="radiogroup" aria-label={concat!(shared::companion_name!(), " companion")}>
                    {sidekick::MODES.into_iter().map(|(m, _, label)| view! {
                        <label class:on=move || sidekick_mode.get() == m>
                            <input type="radio" name="sidekick-mode" prop:checked=move || sidekick_mode.get() == m on:change=move |_| {
                                if let Some(s) = storage() { let _ = s.set_item("sidekick_mode", sidekick::mode_str(m)); }
                                sidekick_mode.set(m)
                            }/>
                            {label}
                        </label>
                    }).collect_view()}
                </div>
            </section>
            <section class="block" data-sidekick-anchor="ai-provider">
                <h2>"AI provider"</h2>
                <p class="meta">{concat!("Powers ", shared::companion_name!(), ", exercise reviews and ", shared::companion_name!(), "'s screen helper.")}</p>
                <div class="choices" data-help="settings.provider" role="radiogroup" aria-label="AI provider">
                    {PROVIDERS.iter().filter(|(k, _, _)| !(android() && *k == "chatgpt")).map(|(k, name, desc)| view! {
                        <label class="choice" class:on=move || kind.get() == *k>
                            <input type="radio" name="provider" prop:checked=move || kind.get() == *k on:change=move |_| choose(k)/>
                            <span class="choice-name">{*name}</span>
                            <span class="choice-desc">{*desc}</span>
                        </label>
                    }).collect_view()}
                </div>
                <details class="caps" data-help="settings.caps">
                    <summary>"Which one should I pick?"</summary>
                    <p class="meta">{concat!("Lessons, checks and your certificate never need AI. A provider only adds ", shared::companion_name!(), "'s help, and not every provider can do everything:")}</p>
                    <div class="caps-scroll">
                        <table>
                            <thead><tr>
                                <th scope="col"><span class="sr-only">"Feature"</span></th>
                                {PROVIDERS.iter().filter(|(k, _, _)| !(android() && *k == "chatgpt")).map(|(k, name, _)| view! {
                                    <th scope="col" class:on=move || kind.get() == *k>{*name}</th>
                                }).collect_view()}
                            </tr></thead>
                            <tbody>
                                {CAPS.iter().map(|(feature, ok)| view! {
                                    <tr>
                                        <th scope="row">{*feature}</th>
                                        {PROVIDERS.iter().enumerate().filter(|(_, (k, _, _))| !(android() && *k == "chatgpt")).map(|(i, (k, _, _))| {
                                            let yes = ok[i];
                                            view! {
                                                <td class:on=move || kind.get() == *k class:yes=yes>
                                                    <span aria-hidden="true">{if yes { "✓" } else { "✕" }}</span>
                                                    <span class="sr-only">{if yes { "Yes" } else { "No" }}</span>
                                                </td>
                                            }
                                        }).collect_view()}
                                    </tr>
                                }).collect_view()}
                            </tbody>
                        </table>
                    </div>
                    <p class="meta">{concat!("Just want to chat with ", shared::companion_name!(), "? Your ChatGPT plan is enough. Want ", shared::companion_name!(), " to look at or work in your practice app? Use an OpenAI API key. A custom endpoint runs the screen helper only if it supports OpenAI's Responses API.")}</p>
                </details>

                {move || matches!(kind.get().as_str(), "ollama" | "custom").then(|| view! {
                    <label class="field" data-help="settings.endpoint"><span>{move || if kind.get() == "ollama" { "Ollama address" } else { "Base URL" }}</span>
                        <input prop:value=move || endpoint.get() on:input=move |ev| endpoint.set(event_target_value(&ev))
                            placeholder=move || if kind.get() == "ollama" { "http://127.0.0.1:11434" } else { "https://openrouter.ai/api/v1" } spellcheck="false"/>
                    </label>
                })}

                {move || needs_key().then(|| view! {
                    <label class="field" data-help="settings.api-key"><span>{move || if has_key.get() { "API key · saved in your Keychain" } else { "API key" }}</span>
                        <div class="inline">
                            <input type="password" autocomplete="off" spellcheck="false" prop:value=move || key.get()
                                on:input=move |ev| key.set(event_target_value(&ev))
                                placeholder=move || if has_key.get() { "Paste a new key to replace it" } else if kind.get() == "openai" { "sk-…" } else { "Paste your key" }/>
                            <button class="btn" on:click=connect disabled=move || loading.get()>
                                {move || if has_key.get() && key.get().is_empty() { "Reload models" } else { "Save key" }}
                            </button>
                        </div>
                    </label>
                })}
                {move || (kind.get() == "ollama").then(|| view! {
                    <div class="quiz-bar flat"><button class="btn" data-help="settings.find-models" on:click=connect disabled=move || loading.get()>"Find installed models"</button></div>
                })}
                {move || (kind.get() == "chatgpt").then(|| view! {
                    <p class="meta">"Install OpenAI's Codex CLI ("<code>"npm install -g @openai/codex"</code>"), run "<code>"codex login"</code>" and choose Sign in with ChatGPT. Replies count against your ChatGPT plan's limits and take a few seconds longer."</p>
                    <div class="quiz-bar flat"><button class="btn" data-help="settings.find-models" on:click=connect disabled=move || loading.get()>"Check ChatGPT sign-in"</button></div>
                })}

                <label class="field" data-help="settings.model"><span>"Model"</span>
                    {move || match (loading.get(), models.get()) {
                        (true, _) => view! { <select disabled=true><option>"Loading models…"</option></select> }.into_any(),
                        (_, Some(Ok(list))) if !list.is_empty() => view! {
                            <select on:change=move |ev| model.set(event_target_value(&ev))>
                                {list.into_iter().map(|m| {
                                    let sel = m == model.get_untracked();
                                    view! { <option value=m.clone() selected=sel>{m.clone()}</option> }
                                }).collect_view()}
                            </select>
                        }.into_any(),
                        (_, Some(Ok(_))) => view! { <p class="notice warn">{move || if kind.get() == "ollama" { "No models installed. Run `ollama pull gemma2:9b`, then try again." } else { "This key has no chat models available." }}</p> }.into_any(),
                        (_, Some(Err(e))) => view! { <p class="notice error" role="alert">{e}</p> }.into_any(),
                        (_, None) => view! { <select disabled=true><option>{move || match kind.get().as_str() {
                            "chatgpt" => "Check your ChatGPT sign-in first", "ollama" => "Find installed models first", _ => "Save your API key to see models" }}</option></select> }.into_any(),
                    }}
                </label>

                {move || (kind.get() != "ollama").then(|| view! {
                    <p class="notice warn" data-help="settings.data-note">"Lesson text and your questions go to this provider. Choose Ollama for anything confidential."</p>
                })}
            </section>
            <section class="block">
                <h2>"Practice app"</h2>
                <label class="field" data-help="settings.practice-url"><span>"Practice app address"</span>
                    <input id="practice-url" prop:value=move || practice.get() on:input=move |ev| practice.set(event_target_value(&ev)) placeholder=format!("e.g. {}", shared::brand::PRACTICE_EXAMPLE) spellcheck="false"/>
                </label>
                <label class="field"><span>"Development URL (optional)"</span>
                    <input id="practice-dev-url" prop:value=move || practice_dev.get() on:input=move |ev| practice_dev.set(event_target_value(&ev)) placeholder="e.g. staging.example.com (optional)" spellcheck="false"/>
                </label>
                <p class="meta">"Use a training or sandbox account, never production data you can't lose: exercises create real records. The screen helper reads the page you're on and sends it to your AI provider. The Development address (a staging copy) opens from + in the practice pane's tabs."</p>
            </section>
            <div class="quiz-bar">
                <button class="btn primary" data-help="settings.save" on:click=move |_| save(false) disabled=move || busy.get()>"Save settings"</button>
                <button class="btn" data-help="settings.save-test" on:click=move |_| save(true) disabled=move || busy.get()>"Save and test"</button>
                {move || status.get().map(|s| match s {
                    Ok(m) => view! { <span class="result pass" role="status">{m}</span> }.into_any(),
                    Err(e) => view! { <span class="notice error" role="alert">{e}</span> }.into_any(),
                })}
            </div>
            <SidekickSafety/>
            <StartOver/>
        </div>
    }
}

/// Settings → Profile: the onboarding fields, the same checks, and the access lock re-read on save.
#[component]
fn ProfileSettings() -> impl IntoView {
    let c = ctx();
    let (name, email, role) = (RwSignal::new(String::new()), RwSignal::new(String::new()), RwSignal::new(String::new()));
    let status = RwSignal::new(None::<Result<String, String>>);
    let busy = RwSignal::new(false);
    spawn_local(async move {
        if let Ok(Some(p)) = api::call::<Option<Profile>>("get_profile", json!({})).await {
            name.set(p.name);
            email.set(p.email);
            role.set(serde_json::to_value(p.role).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default());
        }
    });
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let p = match typed_profile(&name.get_untracked(), &email.get_untracked(), &role.get_untracked()) {
            Ok(p) => p,
            Err(e) => { status.set(Some(Err(e))); return }
        };
        busy.set(true);
        status.set(None);
        spawn_local(async move {
            let r = api::call::<()>("save_profile", json!({ "profile": p })).await;
            busy.set(false);
            // only the access code locks or opens a course, not the role; moving off a locked one stays as a safety net
            if r.is_ok() { leave_locked_course(c).await }
            status.set(Some(r.map(|()| "Profile saved.".to_string())));
        });
    };
    view! {
        <section class="block">
            <h2>"Profile"</h2>
            <form class="profile-form" on:submit=save>
                <ProfileFields name email role/>
                <div class="quiz-bar">
                    <button class="btn" type="submit" data-help="settings.save-profile" disabled=move || busy.get()>"Save profile"</button>
                    {move || status.get().map(|s| match s {
                        Ok(m) => view! { <span class="result pass" role="status">{m}</span> }.into_any(),
                        Err(e) => view! { <span class="notice error" role="alert">{e}</span> }.into_any(),
                    })}
                </div>
            </form>
        </section>
    }
}

/// Settings → Access: enter an access code (or the shared login) after onboarding, for any role.
#[component]
fn AccessSettings() -> impl IntoView {
    let c = ctx();
    let has = RwSignal::new(false);
    spawn_local(async move { has.set(api::call::<bool>("has_access_code", json!({})).await.unwrap_or(false)) });
    let (user, code) = (RwSignal::new(String::new()), RwSignal::new(String::new()));
    let status = RwSignal::new(None::<Result<String, String>>);
    let busy = RwSignal::new(false);
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        status.set(None);
        spawn_local(async move {
            let r = unlock_with(user.get_untracked(), code.get_untracked()).await;
            busy.set(false);
            if r.is_ok() { has.set(true); code.set(String::new()); c.courses_rev.update(|n| *n += 1) }
            status.set(Some(r.map(|()| "Gated courses are unlocked.".to_string())));
        });
    };
    // Remove access code: an inline confirm, then the course list reloads (lock icons) and, if the open
    // course just locked, the first open course takes its place.
    let confirming = RwSignal::new(false);
    let remove_ref = NodeRef::<leptos::html::Button>::new();
    let keep = move || {
        confirming.set(false);
        request_animation_frame(move || { if let Some(b) = remove_ref.get_untracked() { let _ = b.focus(); } });
    };
    let remove = move |_| {
        busy.set(true);
        status.set(None);
        spawn_local(async move {
            match api::call::<()>("remove_access_code", json!({})).await {
                Ok(()) => {
                    has.set(false);
                    leave_locked_course(c).await;
                    status.set(Some(Ok("Access code removed. Gated courses are locked on this computer.".to_string())));
                }
                Err(e) => status.set(Some(Err(e))),
            }
            confirming.set(false);
            busy.set(false);
        });
    };
    view! {
        <section class="block">
            <h2 data-help="settings.access">"Access"</h2>
            <p class="meta">{move || if has.get() {
                "An access code is saved on this computer, so gated courses stay open whatever role you pick. Enter a new one to replace it."
            } else {
                "Gated courses open with the username and access code you were given."
            }}</p>
            <form class="unlock" on:submit=save>
                <AccessFields user code required=true/>
                <div class="quiz-bar">
                    <button class="btn" type="submit" disabled=move || busy.get()>{move || if busy.get() { "Checking…" } else { "Unlock course" }}</button>
                    {move || (has.get() && !confirming.get()).then(|| view! {
                        <button class="btn quiet danger" type="button" data-help="settings.remove-code" node_ref=remove_ref disabled=move || busy.get()
                            on:click=move |_| confirming.set(true)>"Remove access code"</button>
                    })}
                    {move || status.get().map(|s| match s {
                        Ok(m) => view! { <span class="result pass" role="status">{m}</span> }.into_any(),
                        Err(e) => view! { <span class="notice error" role="alert">{e}</span> }.into_any(),
                    })}
                </div>
            </form>
            {move || (has.get() && confirming.get()).then(|| {
                let keep_ref = NodeRef::<leptos::html::Button>::new();
                request_animation_frame(move || { if let Some(b) = keep_ref.get_untracked() { let _ = b.focus(); } });
                view! {
                    <div class="confirm-inline" role="group" aria-labelledby="remove-code-q"
                        on:keydown=move |ev| if ev.key() == "Escape" { ev.prevent_default(); keep() }>
                        <p id="remove-code-q">"Remove the saved access code? Gated courses lock again on this computer."</p>
                        <div class="confirm-actions">
                            <button class="btn" type="button" node_ref=keep_ref on:click=move |_| keep()>"Keep it"</button>
                            <button class="btn danger" type="button" on:click=remove disabled=move || busy.get()>
                                {move || if busy.get() { "Removing…" } else { "Remove access code" }}
                            </button>
                        </div>
                    </div>
                }
            })}
        </section>
    }
}

/// "about 18.2k tokens"
fn tokens_text(t: u64) -> String { if t >= 1000 { format!("about {:.1}k tokens", t as f64 / 1000.0) } else { format!("about {t} tokens") } }

/// Settings → Sidekick safety and AI usage: today's AI requests against the learner's daily limit, and
/// Sidekick's activity log (screen-helper actions and safety decisions), with an export.
#[component]
fn SidekickSafety() -> impl IntoView {
    let usage = RwSignal::new(None::<AiUsage>);
    let cap = RwSignal::new(String::new());
    let log = RwSignal::new(Vec::<SidekickActivity>::new());
    let status = RwSignal::new(None::<Result<String, String>>);
    spawn_local(async move {
        if let Ok(u) = api::call::<AiUsage>("get_ai_usage", json!({})).await { cap.set(u.cap.to_string()); usage.set(Some(u)); }
        log.set(api::call::<Vec<SidekickActivity>>("get_sidekick_activity", json!({ "limit": 50 })).await.unwrap_or_default());
    });
    let save_cap = move |_| match cap.get_untracked().trim().parse::<u32>() {
        Err(_) => status.set(Some(Err("Enter a whole number. 0 means no limit.".into()))),
        Ok(n) => spawn_local(async move {
            match api::call::<AiUsage>("set_ai_daily_cap", json!({ "cap": n })).await {
                Ok(u) => { cap.set(u.cap.to_string()); usage.set(Some(u)); status.set(Some(Ok("Daily limit saved.".into()))) }
                Err(e) => status.set(Some(Err(e))),
            }
        }),
    };
    let export = move |_| spawn_local(async move {
        match api::call::<bool>("export_sidekick_activity", json!({})).await {
            Ok(true) => status.set(Some(Ok("Activity log saved.".into()))),
            Ok(false) => {}
            Err(e) => status.set(Some(Err(e))),
        }
    });
    view! {
        <section class="block" aria-labelledby="sidekick-safety-t">
            <h2 id="sidekick-safety-t">{concat!(shared::companion_name!(), " safety and AI usage")}</h2>
            <p class="meta">{concat!(shared::companion_name!(), " never deletes, archives, deploys or changes users, roles, keys or account settings in your practice app, even when asked, and asks you before it submits or saves anything.")}</p>
            <p class="ai-usage">{move || usage.get().map(|u| format!("AI usage today: {} request{}, {}{}.", u.calls, if u.calls == 1 { "" } else { "s" }, tokens_text(u.tokens),
                if u.cap > 0 { format!(" Daily limit: {}", u.cap) } else { " No daily limit".into() }))}</p>
            <div class="ai-usage-cap">
                <label for="ai-cap">"Daily limit (requests, 0 for none)"</label>
                <input id="ai-cap" inputmode="numeric" prop:value=move || cap.get() on:input=move |ev| cap.set(event_target_value(&ev))/>
                <button class="btn small" on:click=save_cap>"Save limit"</button>
            </div>
            <details class="caps">
                <summary>{concat!(shared::companion_name!(), " activity log")}</summary>
                <p class="meta">{concat!("What ", shared::companion_name!(), " did in your practice pane and what its safety rules decided. Kept on this computer only, without anything you typed; Start over erases it.")}</p>
                {move || if log.with(|l| l.is_empty()) {
                    view! { <p class="meta">"Nothing yet."</p> }.into_any()
                } else {
                    view! { <ul class="sidekick-log">{log.get().into_iter().map(|a| view! {
                        <li>{format!("{} · {} · {} ", a.at, a.mode, a.action)}
                            {(!a.label.is_empty()).then(|| format!("\"{}\" · ", a.label))}
                            {(!a.env.is_empty()).then(|| format!("{} · ", a.env))}
                            <span class=format!("dec {}", a.decision)>{a.decision.clone()}</span>
                            {(!a.reason.is_empty()).then(|| format!(" ({})", a.reason))}
                        </li>
                    }).collect_view()}</ul> }.into_any()
                }}
                <button class="btn small" on:click=export>"Export log"</button>
            </details>
            {move || status.get().map(|s| match s {
                Ok(m) => view! { <span class="result pass" role="status">{m}</span> }.into_any(),
                Err(e) => view! { <span class="notice error" role="alert">{e}</span> }.into_any(),
            })}
        </section>
    }
}

/// Settings → Start over: erases everything this app keeps for the learner on this computer (reset_app),
/// then clears the webview's storage and reloads to onboarding, where the course tour plays again after.
#[component]
fn StartOver() -> impl IntoView {
    let open = RwSignal::new(false);
    view! {
        <section class="block danger-zone" aria-labelledby="start-over-t">
            <h2 id="start-over-t">"Start over"</h2>
            <p class="meta">"Erase everything this app keeps about you on this computer and go back to the welcome screen. The courses stay installed."</p>
            <button class="btn danger-outline" type="button" data-help="settings.start-over" on:click=move |_| open.set(true)>"Start over on this computer"</button>
            {move || open.get().then(|| view! { <StartOverDialog open/> })}
        </section>
    }
}

#[component]
fn StartOverDialog(open: RwSignal<bool>) -> impl IntoView {
    let dialog_ref = NodeRef::<leptos::html::Section>::new();
    let cancel_ref = NodeRef::<leptos::html::Button>::new();
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let cancel = move || if !busy.get_untracked() { open.set(false) };
    let erase = move |_| {
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            match api::call::<()>("reset_app", json!({})).await {
                Ok(()) => {
                    if let Some(s) = storage() { let _ = s.clear(); }
                    if let Ok(Some(s)) = window().session_storage() { let _ = s.clear(); }
                    let _ = window().location().reload();
                }
                Err(e) => { error.set(Some(e)); busy.set(false) }
            }
        });
    };
    let keys = window_event_listener(leptos::ev::keydown, move |ev| {
        match ev.key().as_str() {
            "Escape" => { ev.prevent_default(); ev.stop_immediate_propagation(); cancel() }
            "Tab" => {
                let Some(d) = dialog_ref.get_untracked() else { return };
                let Ok(list) = d.query_selector_all("button:not([disabled])") else { return };
                let els: Vec<web_sys::HtmlElement> = (0..list.length()).filter_map(|i| list.item(i)).filter_map(|n| n.dyn_into().ok()).collect();
                let (Some(head), Some(tail)) = (els.first(), els.last()) else { ev.prevent_default(); return };
                let active = document().active_element();
                let inside = active.as_ref().is_some_and(|a| d.contains(Some(a)));
                let at = |el: &web_sys::HtmlElement| active.as_ref().is_some_and(|a| a == el.unchecked_ref::<web_sys::Element>());
                if !inside || (ev.shift_key() && at(head)) { ev.prevent_default(); let _ = if ev.shift_key() { tail.focus() } else { head.focus() }; }
                else if !ev.shift_key() && at(tail) { ev.prevent_default(); let _ = head.focus(); }
            }
            _ => {}
        }
    });
    let opener = document().active_element().and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok());
    request_animation_frame(move || { if let Some(b) = cancel_ref.get_untracked() { let _ = b.focus(); } });
    on_cleanup(move || {
        keys.remove();
        if let Some(o) = opener { let _ = o.focus(); }
    });
    view! {
        <div class="scrim" on:click=move |_| cancel()></div>
        <section class="news confirm" node_ref=dialog_ref role="alertdialog" aria-modal="true" aria-labelledby="start-over-q" aria-describedby="start-over-d">
            <h2 id="start-over-q">"Start over on this computer?"</h2>
            <p id="start-over-d">"This erases your name, progress, certificates, saved access code and AI keys from this computer. It can't be undone."</p>
            {move || error.get().map(|e| view! { <p class="notice error" role="alert">{e}</p> })}
            <div class="confirm-actions">
                <button class="btn" type="button" node_ref=cancel_ref on:click=move |_| cancel() disabled=move || busy.get()>"Cancel"</button>
                <button class="btn danger" type="button" on:click=erase disabled=move || busy.get()>
                    {move || if busy.get() { "Erasing…" } else { "Erase and start over" }}
                </button>
            </div>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::pick_course;

    #[test]
    fn session_columns_fold_rather_than_squeeze() {
        use super::{fit, widths, Panels, Ps, MIN_COURSE, MIN_PRACTICE, MIN_SIDEKICK, RAIL_W, TAB_W};
        for win in [390.0, 640.0, 800.0, 1024.0, 1280.0, 1440.0, 1920.0] {
            for share in [0.2, 0.42, 0.8] {
                for keep in 0..3u8 {
                    for p in [Panels::ALL, Panels { course: Ps::Open, practice: Ps::Collapsed, sidekick: Ps::Open }, Panels { course: Ps::Collapsed, practice: Ps::Open, sidekick: Ps::Open },
                        Panels { course: Ps::Open, practice: Ps::Closed, sidekick: Ps::Collapsed }, Panels { course: Ps::Collapsed, practice: Ps::Collapsed, sidekick: Ps::Collapsed }] {
                        let f = fit(p, share, win, keep);
                        let (c, k, z) = widths(f, share, win);
                        // every pixel is used, nothing runs past the window
                        assert!((c + k + z + RAIL_W - win).abs() < 0.5, "{win} {p:?} -> {f:?}: {c}+{k}+{z}");
                        let open: Vec<u8> = (0..3).filter(|&i| f.get(i) == Ps::Open).collect();
                        assert!(!open.is_empty(), "{win} {p:?}: nothing open");
                        // what the learner hid stays hidden; fit only folds
                        let base = p.keep_one();
                        for i in 0..3 { if base.get(i) != Ps::Open { assert_eq!(f.get(i), base.get(i)) } }
                        // two or more open columns each get their minimum
                        if open.len() > 1 {
                            for (i, w, m) in [(0, c, MIN_COURSE), (1, k, MIN_PRACTICE), (2, z, MIN_SIDEKICK)] {
                                if f.get(i) == Ps::Open { assert!(w >= m, "{win} share {share} {p:?} -> {f:?}: column {i} only {w}") }
                            }
                        }
                        // a folded column is exactly its tab
                        for (i, w) in [(0u8, c), (1, k), (2, z)] { if f.get(i) == Ps::Collapsed { assert_eq!(w, TAB_W) } }
                        // the column opened last never folds
                        if base.get(keep) == Ps::Open { assert_eq!(f.get(keep), Ps::Open, "{win} keep {keep}: {p:?} -> {f:?}") }
                    }
                }
            }
        }
        // the reported case: at 1024 the course column was 254 px; now it folds and the practice app gets the room
        let f = fit(Panels::ALL, 0.42, 1024.0, 2);
        assert_eq!((f.course, f.practice, f.sidekick), (Ps::Collapsed, Ps::Open, Ps::Open));
        assert_eq!(widths(f, 0.42, 1024.0), (TAB_W, 1024.0 - RAIL_W - TAB_W - 360.0, 360.0));
        // at 800 the companion gives up a little so the practice app still fits beside it
        let f = fit(Panels::ALL, 0.42, 800.0, 2);
        assert_eq!((f.course, f.practice, f.sidekick), (Ps::Collapsed, Ps::Open, Ps::Open));
        assert_eq!(widths(f, 0.42, 800.0), (TAB_W, MIN_PRACTICE, 800.0 - RAIL_W - TAB_W - MIN_PRACTICE));
        // opened last, the course keeps its room and the practice app folds
        assert_eq!(fit(Panels::ALL, 0.42, 1024.0, 0).practice, Ps::Collapsed);
        // wide enough: all three, the split clamped so neither side is squeezed
        let (c, k, _) = widths(fit(Panels::ALL, 0.2, 1440.0, 2), 0.2, 1440.0);
        assert!(c >= MIN_COURSE && k >= MIN_PRACTICE, "{c} {k}");
    }

    #[test]
    fn every_role_has_a_note_and_the_code_roles_get_the_form() {
        use super::{role_asks_code, role_note};
        for role in ["learner", "partner", "staff"] {
            assert!(role_note(role).contains("access code"), "{role}");
        }
        assert_eq!(role_note(""), "");
        assert!(role_asks_code("partner") && role_asks_code("staff") && !role_asks_code("learner") && !role_asks_code(""));
    }

    #[test]
    fn profile_form_checks_the_profile() {
        use super::{staff_email_error, typed_profile};
        assert!(staff_email_error("jordan", "staff").is_none(), "no error before the @ is typed");
        assert!(staff_email_error("jordan@brightline.example", "learner").is_none());
        assert_eq!(staff_email_error("jordan@brightline.example", "staff").is_some(), shared::brand::STAFF_EMAIL_DOMAIN.is_some());
        assert!(typed_profile("Jordan Hale", "jordan@brightline.example", "").is_err());
        assert!(typed_profile("Jordan Hale", "jordan@mailinator.com", "learner").is_err());
        assert_eq!(typed_profile("Jordan Hale", "Jordan@Brightline.example", "learner").unwrap().email, "jordan@brightline.example");
    }

    #[test]
    fn role_select_shows_the_stored_role_or_the_placeholder() {
        use super::role_options;
        let picked = |role: &str| role_options(role).iter().filter(|o| o.3).map(|o| o.0.clone()).collect::<Vec<_>>();
        assert_eq!(picked(""), [""], "nothing stored: the placeholder, never the first role");
        assert_eq!(picked("bogus"), [""]);
        assert_eq!(picked("partner"), ["partner"], "Settings shows the saved role");
        assert_eq!(picked("learner"), ["learner"]);
        let first = role_options("")[0].clone();
        assert_eq!((first.0.as_str(), first.1, first.2), ("", "Choose your role", true), "the placeholder is value \"\" and disabled");
        assert!(role_options("staff")[1..].iter().all(|o| !o.2 && !o.0.is_empty()));
    }

    #[test]
    fn saved_course_wins_when_still_listed() {
        assert_eq!(pick_course(Some("advanced-notes"), &["brightline-basics", "team-notes", "advanced-notes"]).as_deref(), Some("advanced-notes"));
    }

    #[test]
    fn stale_or_missing_choice_falls_back_to_first() {
        let ids = ["brightline-basics", "team-notes"];
        assert_eq!(pick_course(Some("gone"), &ids).as_deref(), Some("brightline-basics"));
        assert_eq!(pick_course(None, &ids).as_deref(), Some("brightline-basics"));
        assert_eq!(pick_course(Some(""), &ids).as_deref(), Some("brightline-basics"));
    }

    #[test]
    fn no_courses_no_pick() {
        assert_eq!(pick_course(Some("advanced-notes"), &[]), None);
    }
}
