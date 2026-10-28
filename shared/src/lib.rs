//! Types that cross the Tauri IPC boundary. Answer keys never appear here:
//! grading happens in the backend, the UI only ever sees prompts and options.
use crate::brand::COMPANION;
use serde::{Deserialize, Serialize};

pub mod brand;
pub mod email;
pub mod md;
pub mod tenant;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CourseSummary {
    pub id: String,
    pub title: String,
    pub short: String,
    /// Gated and not yet unlocked on this device.
    pub locked: bool,
}

/// Who the learner said they are on the onboarding screen.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role { Learner, Partner, Staff }

impl Role {
    /// The label shown for this role (see `brand::ROLES`).
    pub fn label(self) -> &'static str { brand::ROLES.iter().find(|(r, _, _)| *r == self).map(|(_, l, _)| *l).unwrap_or("Learner") }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Profile {
    pub name: String,
    pub email: String,
    pub role: Role,
}

/// The onboarding and Settings profile rules, checked in the UI and again by `save_profile`:
/// a name, a plausible work email (trimmed, lowercased, no trailing dot on the domain), an
/// address on `brand::STAFF_EMAIL_DOMAIN` for staff when one is set, and no disposable or mistyped webmail domain (see `email`).
pub fn clean_profile(p: Profile) -> Result<Profile, String> {
    let name = p.name.trim().to_string();
    if name.is_empty() { return Err("Enter your name.".into()) }
    let email = p.email.trim().to_lowercase();
    let (local, domain) = email.split_once('@').unwrap_or_default();
    let domain = email::normalise_domain(domain);
    if local.is_empty() || !domain.contains('.') || domain.contains('@') || email.contains(char::is_whitespace)
        || domain.split('.').any(str::is_empty) {
        return Err("Enter a valid work email.".into())
    }
    staff_rule(p.role, &domain, brand::STAFF_EMAIL_DOMAIN)?;
    if brand::CHECK_DISPOSABLE_EMAIL { email::check_domain(local, &domain)?; }
    Ok(Profile { name, email: format!("{local}@{domain}"), role: p.role })
}

/// The Staff role's email rule: with a staff domain set, a Staff email must be on it (or a subdomain).
pub fn staff_rule(role: Role, domain: &str, staff: Option<&str>) -> Result<(), String> {
    match (role, staff) {
        (Role::Staff, Some(staff)) if domain != staff && !domain.ends_with(&format!(".{staff}")) =>
            Err(format!("{} needs your @{staff} email.", Role::Staff.label())),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod profile_tests {
    use super::{clean_profile, Profile, Role};
    fn p(name: &str, email: &str, role: Role) -> Profile { Profile { name: name.into(), email: email.into(), role } }

    #[test]
    fn names_and_emails_are_cleaned() {
        let ok = clean_profile(p("  Jordan Hale ", " Jordan.Hale@Brightline.Example ", Role::Learner)).unwrap();
        assert_eq!((ok.name.as_str(), ok.email.as_str()), ("Jordan Hale", "jordan.hale@brightline.example"));
        for role in [Role::Learner, Role::Partner, Role::Staff] {
            assert!(clean_profile(p("Jordan Hale", "jordan@brightline.example", role)).is_ok());
        }
        assert_eq!(clean_profile(p(" ", "jordan@brightline.example", Role::Staff)).unwrap_err(), "Enter your name.");
        assert_eq!(clean_profile(p("Jordan", "jordan@brightline", Role::Partner)).unwrap_err(), "Enter a valid work email.");
        assert_eq!(clean_profile(p("Jordan", "jordan@example..com", Role::Partner)).unwrap_err(), "Enter a valid work email.");
    }

    #[test]
    fn staff_need_the_staff_domain_when_one_is_set() {
        use super::staff_rule;
        let staff = Some("northwind-labs.example");
        assert!(staff_rule(Role::Staff, "northwind-labs.example", staff).is_ok());
        assert!(staff_rule(Role::Staff, "mail.northwind-labs.example", staff).is_ok());
        for bad in ["brightline.example", "northwind-labs.example.evil.example", "notnorthwind-labs.example"] {
            let e = staff_rule(Role::Staff, bad, staff).unwrap_err();
            assert!(e.contains("@northwind-labs.example"), "{bad}: {e}");
        }
        assert!(staff_rule(Role::Learner, "brightline.example", staff).is_ok(), "only Staff is bound");
        assert!(staff_rule(Role::Staff, "brightline.example", None).is_ok(), "no staff domain: any email");
    }

    #[test]
    fn disposable_domains_are_refused() {
        for bad in ["jordan@mailinator.com", "jordan@sub.mailinator.com", "jordan@10minutemail.com", "jordan@guerrillamail.com",
                    "jordan@temp-mail.org", "jordan@yopmail.com", "Jordan@MAILINATOR.COM.", " jordan@YopMail.com "] {
            for role in [Role::Partner, Role::Learner] {
                assert_eq!(clean_profile(p("Jordan Hale", bad, role)).unwrap_err(),
                    "Use your work email. Temporary or disposable addresses aren't accepted.", "{bad}");
            }
        }
    }

    #[test]
    fn real_domains_and_free_webmail_are_accepted() {
        for ok in ["jordan@northwind-labs.example", "jordan@mail.brightline-logistics.co.uk", "jordan@gmail.com", "jordan@outlook.com"] {
            assert_eq!(clean_profile(p("Jordan Hale", ok, Role::Learner)).unwrap().email, ok);
        }
        let ok = clean_profile(p("Jordan Hale", " Jordan.Hale@Northwind-Labs.EXAMPLE. ", Role::Staff)).unwrap();
        assert_eq!(ok.email, "jordan.hale@northwind-labs.example");
    }

    #[test]
    fn provider_typos_suggest_the_fix() {
        for (typo, fix) in [("gmial.com", "gmail.com"), ("gmai.com", "gmail.com"), ("hotmial.com", "hotmail.com"), ("yaho.com", "yahoo.com"), ("outlok.com", "outlook.com")] {
            let e = clean_profile(p("Jordan Hale", &format!("Jordan@{}", typo.to_uppercase()), Role::Learner)).unwrap_err();
            assert_eq!(e, format!("Did you mean jordan@{fix}?"));
        }
    }

    #[test]
    fn non_ascii_domains_are_refused_plainly() {
        let e = clean_profile(p("Jordan Hale", "jordan@bücher.example", Role::Learner)).unwrap_err();
        assert_eq!(e, super::email::NON_ASCII_ERROR);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CourseView {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub final_minutes: u32,
    pub final_blurb: String,
    pub sessions: Vec<SessionView>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SessionView {
    pub id: String,
    pub phase: Option<String>,
    pub title: String,
    pub outcome: String,
    pub lessons: Vec<LessonSummary>,
    pub check: String,
    pub exercise: String,
    pub check_best: Option<f32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LessonSummary {
    pub id: String,
    pub number: String,
    pub title: String,
    pub duration_min: u32,
    pub has_video: bool,
    pub completed: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Chapter {
    pub t: f64,
    pub title: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VideoView {
    pub url: String,
    pub poster: Option<String>,
    pub captions: Option<String>,
    pub chapters: Vec<Chapter>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LessonView {
    pub id: String,
    pub number: String,
    pub title: String,
    pub objective: String,
    pub duration_min: u32,
    pub body_html: String,
    pub key_points: Vec<String>,
    pub gotchas: Vec<String>,
    pub video: Option<VideoView>,
    pub resume_at: f64,
    pub quick_check: Vec<QuestionView>,
    /// "Read more" links for this lesson (https, under brand::HELP_URL_PREFIX).
    #[serde(default)]
    pub help: Vec<HelpLink>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QuestionView {
    Single { prompt: String, options: Vec<String> },
    Multi { prompt: String, options: Vec<String> },
    Match { prompt: String, left: Vec<String>, right: Vec<String> },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct QuizView {
    pub id: String,
    pub title: String,
    pub pass_mark: f32,
    pub minutes: Option<u32>,
    pub intro_html: String,
    pub sections: Vec<QuizSection>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct QuizSection {
    pub title: String,
    pub questions: Vec<QuestionView>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Graded {
    pub score: f32,
    pub passed: bool,
    pub results: Vec<QuestionResult>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct QuestionResult {
    pub score: f32,
    /// Feedback for each option the learner picked, or for each match row.
    pub feedback: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ExerciseView {
    pub id: String,
    pub title: String,
    pub minutes: u32,
    pub brief_html: String,
    pub setup: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    /// Only present once the learner has submitted.
    pub walkthrough_html: Option<String>,
    pub last_submission: Option<String>,
    pub last_review: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Msg {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Provider {
    /// Local Ollama, e.g. http://127.0.0.1:11434
    Ollama { host: String, model: String },
    /// Any OpenAI-compatible endpoint: OpenAI, OpenRouter, Groq, vLLM, LM Studio, a company gateway.
    OpenAi { base_url: String, model: String },
    Anthropic { model: String },
    /// The learner's ChatGPT Plus or Pro plan, through OpenAI's Codex CLI signed in with ChatGPT.
    /// Text only: the screen helper needs the OpenAI API.
    ChatGpt { model: String },
}

impl Default for Provider {
    fn default() -> Self {
        // a phone has no local Ollama; start on OpenAI so the first prompt asks for a key
        if cfg!(target_os = "android") {
            return Provider::OpenAi { base_url: OPENAI_BASE_URL.into(), model: "gpt-4o-mini".into() };
        }
        Provider::Ollama { host: "http://127.0.0.1:11434".into(), model: "gemma2:9b".into() }
    }
}

pub const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Settings {
    pub provider: Provider,
    pub has_api_key: bool,
    /// The learner's practice app, e.g. https://notes.brightline.example
    pub practice_url: String,
    /// An optional Development (staging) address of the same app. Empty: none.
    #[serde(default)]
    pub practice_dev_url: String,
}

/// Today's AI use on this computer, against the learner's soft daily limit (0: no limit).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct AiUsage { pub calls: u32, pub tokens: u64, pub cap: u32 }

/// One line of Sidekick's activity log: a screen-helper action or a guardrail decision. Never holds what
/// was typed or chosen, only the control's label.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct SidekickActivity {
    /// Local time, "2026-10-28 14:03:11"
    pub at: String,
    /// look | act | check | chat
    pub mode: String,
    /// click, type_text, choose_option, press_key, run, ...
    pub action: String,
    pub label: String,
    /// Production | Development
    pub env: String,
    /// allow | confirm | allowed | declined | block | stop | limit
    pub decision: String,
    /// The policy class or the reason ("delete", "write cap", ...)
    pub reason: String,
}

/// One unmet certificate requirement, with where to go to meet it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Todo {
    pub label: String,
    /// The session check to open; None means the final assessment.
    pub check: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CertificateView {
    pub course: String,
    pub name: String,
    /// Empty when everything needed is passed.
    pub missing: Vec<Todo>,
    pub final_score: Option<f32>,
    /// Lessons completed / total in this course, shared lessons included (same count as the course home page).
    pub lessons_done: usize,
    pub lessons_total: usize,
    pub issued_on: Option<String>,
    pub id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Stats {
    /// Consecutive active days ending today (or yesterday, if today has no activity yet).
    pub streak: u32,
    pub best_streak: u32,
    pub active_today: bool,
    /// None when the learner has never done anything.
    pub days_since_active: Option<u32>,
    pub lessons_done: u32,
    pub lessons_total: u32,
    pub checks_passed: u32,
    pub checks_total: u32,
    pub final_passed: bool,
    /// Events per day for the last `activity.len()` days, oldest first, ending today.
    pub activity: Vec<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WhatsNew {
    pub version: String,
    pub title: String,
    #[serde(default)]
    pub new: Vec<String>,
    #[serde(default)]
    pub fixed: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Rect { pub x: f64, pub y: f64, pub w: f64, pub h: f64 }

/// A coaching session: one stage per acceptance criterion (exercise) or key point (lesson).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct CoachState {
    pub title: String,
    pub stages: Vec<String>,
    pub stage: usize,
    pub hints_used: u32,
    pub messages: Vec<Msg>,
}

/// The kind of course page the in-app guide is on, for `guide_allows`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GuidePage { Home, Session, Lesson, Check, Final, Exercise, Certificate, Settings }

/// Something that just happened in the course UI, for Sidekick's nudge rules.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SidekickEvent {
    /// The course outline and stats have loaded (app start or course switch).
    CourseLoaded,
    PageChanged { page: GuidePage },
    LessonDone { lesson: String },
    /// A session check ("check") or the final ("final") was graded.
    QuizGraded { quiz: String, id: String, passed: bool },
}

/// The nudge triggers, in the order they are documented in content/sidekick-nudges.yaml.
pub const TRIGGERS: [&str; 8] = [
    "first_launch", "lesson_done_next", "session_lessons_done", "check_failed",
    "all_lessons_done_todo", "final_passed", "back_after_break", "studied_today",
];

/// App destinations a nudge (or a destination chip) can open.
pub const NUDGE_ACTIONS: [&str; 7] = ["next_lesson", "next_check", "session_check", "session", "final", "certificate", "settings"];

/// Everything the nudge rules look at. Caps and snoozes are decided by the caller's stored state.
pub struct NudgeCtx<'a> {
    pub event: &'a SidekickEvent,
    pub stats: &'a Stats,
    pub outline: &'a CourseView,
    /// The certificate's unmet requirements (`get_certificate().missing`); empty when unknown.
    pub missing: &'a [Todo],
    /// Triggers that must stay quiet ("Not now" for 7 days, or ✕ earlier in this run).
    pub quiet: &'a [String],
    /// Unprompted nudges shown in the last 60 minutes.
    pub shown_last_hour: usize,
    /// A video plays, a check/final/exercise is open, the learner is typing, or a dialog is open.
    pub blocked: bool,
    /// The "studied today" nudge has already been shown today.
    pub studied_today_shown: bool,
}

/// The chosen trigger and the course items its copy and action refer to.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NudgePick {
    pub trigger: String,
    pub lesson: Option<String>,
    pub session: Option<String>,
    /// A session check id; None with trigger `all_lessons_done_todo` means the final.
    pub check: Option<String>,
}

pub const NUDGES_PER_HOUR: usize = 3;
const HOUR_MS: f64 = 3_600_000.0;

/// Keeps only the times (ms since epoch) inside the rolling last hour.
pub fn recent(shown: &[f64], now_ms: f64) -> Vec<f64> {
    shown.iter().copied().filter(|t| *t <= now_ms && now_ms - *t < HOUR_MS).collect()
}

fn pick(trigger: &str) -> NudgePick { NudgePick { trigger: trigger.into(), lesson: None, session: None, check: None } }

/// First unfinished lesson in course order, counting `also_done` as finished.
pub fn next_lesson(cv: &CourseView, also_done: Option<&str>) -> Option<String> {
    cv.sessions.iter().flat_map(|s| &s.lessons).find(|l| !l.completed && Some(l.id.as_str()) != also_done).map(|l| l.id.clone())
}

/// The next session check still to pass: the first check in the certificate's to-do list, which
/// lists checks in session order before the final. The to-do nudge, the Next check chip and the
/// guide's open_check all go here, so the check Sidekick names is the check Sidekick opens.
pub fn next_check(missing: &[Todo]) -> Option<&str> {
    missing.iter().find_map(|t| t.check.as_deref())
}

/// Sidekick's nudge rules: which authored nudge (if any) fits this moment. Pure, so every trigger,
/// cap and snooze is unit-tested; the copy itself lives in content/sidekick-nudges.yaml.
pub fn pick_nudge(x: &NudgeCtx) -> Option<NudgePick> {
    if x.blocked || x.shown_last_hour >= NUDGES_PER_HOUR { return None; }
    let cv = x.outline;
    let all_lessons = |also: Option<&str>| cv.sessions.iter().flat_map(|s| &s.lessons).all(|l| l.completed || Some(l.id.as_str()) == also);
    // the next check to pass, or (every check passed) the final: check None
    let todo = || (!x.missing.is_empty()).then(|| NudgePick { check: next_check(x.missing).map(str::to_string), ..pick("all_lessons_done_todo") });
    let studied = || (x.stats.active_today && !x.studied_today_shown).then(|| pick("studied_today"));
    let mut cands: Vec<Option<NudgePick>> = Vec::new();
    match x.event {
        SidekickEvent::CourseLoaded => match x.stats.days_since_active {
            None => cands.push(Some(NudgePick { lesson: next_lesson(cv, None), ..pick("first_launch") })),
            Some(d) if d >= 2 => cands.push(Some(NudgePick { lesson: next_lesson(cv, None), ..pick("back_after_break") })),
            _ => { if all_lessons(None) { cands.push(todo()) } cands.push(studied()) }
        },
        SidekickEvent::PageChanged { page: GuidePage::Home } => { if all_lessons(None) { cands.push(todo()) } cands.push(studied()) }
        SidekickEvent::PageChanged { .. } => {}
        SidekickEvent::LessonDone { lesson } => {
            if all_lessons(Some(lesson)) { cands.push(todo()) }
            if let Some(s) = cv.sessions.iter().find(|s| s.lessons.iter().any(|l| &l.id == lesson)) {
                let last = s.lessons.last().is_some_and(|l| &l.id == lesson);
                let session_done = s.lessons.iter().all(|l| l.completed || &l.id == lesson);
                if last && session_done && s.check_best.is_none() {
                    cands.push(Some(NudgePick { session: Some(s.id.clone()), check: Some(s.check.clone()), ..pick("session_lessons_done") }));
                }
            }
            if let Some(n) = next_lesson(cv, Some(lesson)) { cands.push(Some(NudgePick { lesson: Some(n), ..pick("lesson_done_next") })) }
        }
        SidekickEvent::QuizGraded { quiz, id, passed } => match (quiz.as_str(), passed) {
            ("check", false) => {
                let s = cv.sessions.iter().find(|s| &s.check == id);
                cands.push(Some(NudgePick { session: s.map(|s| s.id.clone()), check: Some(id.clone()), ..pick("check_failed") }));
            }
            ("check", true) => { if all_lessons(None) { cands.push(todo()) } }
            ("final", true) => cands.push(Some(if x.missing.is_empty() { pick("final_passed") } else { todo()? })),
            _ => {}
        },
    }
    cands.into_iter().flatten().find(|p| !x.quiet.iter().any(|q| q == &p.trigger))
}

/// Fills `{lesson}`, `{session}` and `{check}` in nudge copy from the outline.
pub fn fill_nudge(text: &str, p: &NudgePick, cv: &CourseView) -> String {
    let lesson = p.lesson.as_ref().and_then(|id| cv.sessions.iter().flat_map(|s| &s.lessons).find(|l| &l.id == id))
        .map(|l| format!("{} {}", l.number, l.title)).unwrap_or_else(|| "the next lesson".into());
    let ses = cv.sessions.iter().enumerate().find(|(_, s)| Some(&s.id) == p.session.as_ref() || Some(&s.check) == p.check.as_ref());
    let session = ses.map(|(_, s)| s.title.clone()).unwrap_or_else(|| "this session".into());
    let check = match (&p.check, ses) {
        (Some(_), Some((i, _))) => format!("the session {} check", i + 1),
        (None, _) if p.trigger == "all_lessons_done_todo" => "the final assessment".into(),
        _ => "the session check".into(),
    };
    text.replace("{lesson}", &lesson).replace("{session}", &session).replace("{check}", &check)
}

#[cfg(test)]
mod nudge_tests {
    use super::*;

    fn lesson(id: &str, done: bool) -> LessonSummary {
        LessonSummary { id: id.into(), number: id.into(), title: format!("T{id}"), duration_min: 5, has_video: false, completed: done }
    }
    fn session(n: u32, lessons: Vec<LessonSummary>, best: Option<f32>) -> SessionView {
        SessionView { id: format!("s{n}"), phase: None, title: format!("Session {n}"), outcome: String::new(), lessons, check: format!("s{n}-check"), exercise: format!("s{n}-ex"), check_best: best }
    }
    fn course(s1: [bool; 2], s2: [bool; 2]) -> CourseView {
        CourseView { id: "c".into(), title: "C".into(), summary: String::new(), final_minutes: 60, final_blurb: String::new(), sessions: vec![
            session(1, vec![lesson("1.1", s1[0]), lesson("1.2", s1[1])], None),
            session(2, vec![lesson("2.1", s2[0]), lesson("2.2", s2[1])], None),
        ] }
    }
    fn stats(days: Option<u32>, today: bool) -> Stats { Stats { days_since_active: days, active_today: today, ..Default::default() } }
    fn run(ev: SidekickEvent, st: &Stats, cv: &CourseView, missing: &[Todo], quiet: &[String], shown: usize, blocked: bool) -> Option<NudgePick> {
        pick_nudge(&NudgeCtx { event: &ev, stats: st, outline: cv, missing, quiet, shown_last_hour: shown, blocked, studied_today_shown: false })
    }
    fn trig(p: Option<NudgePick>) -> Option<String> { p.map(|p| p.trigger) }
    fn todo(check: Option<&str>) -> Todo { Todo { label: "x".into(), check: check.map(Into::into) } }

    #[test]
    fn first_launch_points_at_the_first_lesson() {
        let p = run(SidekickEvent::CourseLoaded, &stats(None, false), &course([false; 2], [false; 2]), &[], &[], 0, false).unwrap();
        assert_eq!((p.trigger.as_str(), p.lesson.as_deref()), ("first_launch", Some("1.1")));
    }

    #[test]
    fn back_after_two_days() {
        let p = run(SidekickEvent::CourseLoaded, &stats(Some(3), false), &course([true, false], [false; 2]), &[], &[], 0, false).unwrap();
        assert_eq!((p.trigger.as_str(), p.lesson.as_deref()), ("back_after_break", Some("1.2")));
        assert_eq!(trig(run(SidekickEvent::CourseLoaded, &stats(Some(1), false), &course([true, false], [false; 2]), &[], &[], 0, false)), None);
    }

    #[test]
    fn lesson_done_points_to_the_next_lesson() {
        let p = run(SidekickEvent::LessonDone { lesson: "1.1".into() }, &stats(Some(0), true), &course([false; 2], [false; 2]), &[], &[], 0, false).unwrap();
        assert_eq!((p.trigger.as_str(), p.lesson.as_deref()), ("lesson_done_next", Some("1.2")));
    }

    #[test]
    fn last_lesson_of_a_session_points_to_its_check() {
        // the outline may not have caught up yet: the lesson just finished counts as done
        let p = run(SidekickEvent::LessonDone { lesson: "1.2".into() }, &stats(Some(0), true), &course([true, false], [false; 2]), &[], &[], 0, false).unwrap();
        assert_eq!((p.trigger.as_str(), p.check.as_deref(), p.session.as_deref()), ("session_lessons_done", Some("s1-check"), Some("s1")));
    }

    #[test]
    fn check_failed_names_the_session() {
        let p = run(SidekickEvent::QuizGraded { quiz: "check".into(), id: "s2-check".into(), passed: false }, &stats(Some(0), true), &course([true; 2], [true; 2]), &[], &[], 0, false).unwrap();
        assert_eq!((p.trigger.as_str(), p.session.as_deref()), ("check_failed", Some("s2")));
    }

    #[test]
    fn all_lessons_done_with_checks_missing_opens_the_first_missing_check() {
        let cv = course([true; 2], [true; 2]);
        let missing = [todo(Some("s1-check")), todo(None)];
        for ev in [SidekickEvent::CourseLoaded, SidekickEvent::PageChanged { page: GuidePage::Home }] {
            let p = run(ev, &stats(Some(0), false), &cv, &missing, &[], 0, false).unwrap();
            assert_eq!((p.trigger.as_str(), p.check.as_deref()), ("all_lessons_done_todo", Some("s1-check")));
        }
        // the last lesson finished: the to-do list wins over the session check
        let p = run(SidekickEvent::LessonDone { lesson: "2.2".into() }, &stats(Some(0), true), &course([true; 2], [true, false]), &missing, &[], 0, false).unwrap();
        assert_eq!(p.trigger, "all_lessons_done_todo");
        // only the final left
        let p = run(SidekickEvent::PageChanged { page: GuidePage::Home }, &stats(Some(0), false), &cv, &[todo(None)], &[], 0, false).unwrap();
        assert_eq!((p.trigger.as_str(), p.check), ("all_lessons_done_todo".into(), None));
    }

    #[test]
    fn final_passed_with_nothing_missing() {
        let ev = SidekickEvent::QuizGraded { quiz: "final".into(), id: "f".into(), passed: true };
        assert_eq!(trig(run(ev.clone(), &stats(Some(0), true), &course([true; 2], [true; 2]), &[], &[], 0, false)).as_deref(), Some("final_passed"));
        assert_eq!(trig(run(ev, &stats(Some(0), true), &course([true; 2], [true; 2]), &[todo(Some("s2-check"))], &[], 0, false)).as_deref(), Some("all_lessons_done_todo"));
    }

    #[test]
    fn studied_today_once() {
        let cv = course([true, false], [false; 2]);
        let st = stats(Some(0), true);
        assert_eq!(trig(run(SidekickEvent::PageChanged { page: GuidePage::Home }, &st, &cv, &[], &[], 0, false)).as_deref(), Some("studied_today"));
        let ev = SidekickEvent::PageChanged { page: GuidePage::Home };
        assert_eq!(pick_nudge(&NudgeCtx { event: &ev, stats: &st, outline: &cv, missing: &[], quiet: &[], shown_last_hour: 0, blocked: false, studied_today_shown: true }), None);
        assert_eq!(trig(run(SidekickEvent::PageChanged { page: GuidePage::Lesson }, &st, &cv, &[], &[], 0, false)), None);
    }

    #[test]
    fn caps_snooze_and_blocking() {
        let cv = course([false; 2], [false; 2]);
        let ev = || SidekickEvent::LessonDone { lesson: "1.1".into() };
        let st = stats(Some(0), true);
        assert!(run(ev(), &st, &cv, &[], &[], 2, false).is_some());
        assert_eq!(run(ev(), &st, &cv, &[], &[], 3, false), None, "three in the last hour");
        assert_eq!(run(ev(), &st, &cv, &[], &[], 0, true), None, "blocked");
        assert_eq!(run(ev(), &st, &cv, &[], &["lesson_done_next".into()], 0, false), None, "snoozed");
    }

    #[test]
    fn rolling_hour() {
        let now = 10.0 * HOUR_MS;
        assert_eq!(recent(&[now - HOUR_MS - 60_000.0, now - 59.0 * 60_000.0, now], now), vec![now - 59.0 * 60_000.0, now]);
    }

    #[test]
    fn fill_placeholders() {
        let cv = course([true, false], [false; 2]);
        let p = NudgePick { trigger: "session_lessons_done".into(), lesson: Some("1.2".into()), session: Some("s1".into()), check: Some("s1-check".into()) };
        assert_eq!(fill_nudge("{lesson} in {session}: try {check}.", &p, &cv), "1.2 T1.2 in Session 1: try the session 1 check.");
        let f = NudgePick { trigger: "all_lessons_done_todo".into(), lesson: None, session: None, check: None };
        assert_eq!(fill_nudge("Next: {check}.", &f, &cv), "Next: the final assessment.");
    }
}

/// A "Read more" link from a lesson or a nudge.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HelpLink {
    pub title: String,
    pub url: String,
}

pub use brand::HELP_URL_PREFIX;

/// True for an https link under `brand::HELP_URL_PREFIX` with no spaces, quotes or angle brackets
/// (same rule as tools/validate.py).
pub fn help_url_ok(url: &str) -> bool { help_url_ok_with(url, HELP_URL_PREFIX) }

pub fn help_url_ok_with(url: &str, prefix: &str) -> bool {
    url.starts_with("https://") && url.strip_prefix(prefix).is_some_and(|rest| {
        !rest.is_empty() && url.len() > "https://".len() + 3
            && !url.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '<' | '>' | '\\'))
    })
}

#[cfg(test)]
mod help_tests {
    use super::help_url_ok_with;

    #[test]
    fn only_https_links_under_the_prefix() {
        assert!(help_url_ok_with("https://docs.brightline.example/notes/tags", "https://"));
        assert!(help_url_ok_with("https://docs.brightline.example/notes/tags", "https://docs.brightline.example/"));
        for bad in ["http://docs.brightline.example/x", "javascript:alert(1)", "https://", "https://a b.example/", "https://x.example/\"q"] {
            assert!(!help_url_ok_with(bad, "https://"), "{bad}");
        }
        assert!(!help_url_ok_with("https://other.example/x", "https://docs.brightline.example/"));
        assert!(!help_url_ok_with("https://docs.brightline.example.evil.example/x", "https://docs.brightline.example/"));
    }
}

/// One authored nudge from content/sidekick-nudges.yaml.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Nudge {
    pub id: String,
    pub trigger: String,
    pub text: String,
    #[serde(default)]
    pub action: Option<String>,
    /// The action button's text.
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub help: Option<HelpLink>,
    pub pose: String,
}

/// One hover-help entry from content/sidekick-help.yaml, keyed by a control's `data-help` attribute.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct HelpEntry {
    pub key: String,
    /// The tooltip, and the title of Sidekick's explanation.
    pub label: String,
    /// Shown as a tooltip (icon-only and terse controls).
    #[serde(default)]
    pub tip: bool,
    /// What Sidekick explains after a 5 s dwell.
    pub text: String,
}

/// What's wrong with the help file: duplicate keys, an unknown area prefix, copy too long, dashes.
pub fn help_problems(list: &[HelpEntry]) -> Vec<String> {
    let mut out = vec![];
    let mut seen = std::collections::BTreeSet::new();
    for h in list {
        if !seen.insert(h.key.as_str()) { out.push(format!("{}: duplicate key", h.key)) }
        let area = h.key.split('.').next().unwrap_or_default();
        if !["top", "nav", "text", "side", "course", "lesson", "quiz", "coach", "pane", "rail", "settings"].contains(&area) || !h.key.contains('.') {
            out.push(format!("{}: unknown area", h.key))
        }
        if h.label.trim().is_empty() || h.label.len() > 40 { out.push(format!("{}: label must be 1-40 chars", h.key)) }
        if h.text.trim().is_empty() || h.text.len() > 260 { out.push(format!("{}: text must be 1-260 chars", h.key)) }
        if [&h.label, &h.text].iter().any(|s| s.contains('\u{2014}') || s.contains('\u{2013}')) { out.push(format!("{}: no em or en dashes", h.key)) }
    }
    out
}

/// Sidekick's poses: the eight shipped first, then the companion set (each falls back to an older pose
/// until its art ships; see `pose_fallback`).
pub const POSES: [&str; 20] = [
    "idle", "wave", "thumbsup", "celebrate", "sleepy", "reading", "thinking", "pointing",
    "walk", "hop", "point-left", "point-right", "peek", "highfive", "map", "flag", "trophy", "encourage", "idea", "goodbye",
];

/// The existing pose a companion pose falls back to when its image fails to load.
pub fn pose_fallback(pose: &str) -> &'static str {
    match pose {
        "walk" | "hop" | "peek" => "idle",
        "point-left" | "point-right" | "map" => "pointing",
        "highfive" | "encourage" => "thumbsup",
        "flag" | "trophy" => "celebrate",
        "idea" => "thinking",
        "mood-low" => "sleepy",
        "goodbye" => "wave",
        _ => "idle",
    }
}

/// How Sidekick feels about the learner's progress, shown beside the welcome banner and when Sidekick is clicked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mood { Low, Curious, Determined, Excited, Proud }

impl Mood {
    /// The mascot picture (ui/assets/mascot/<pose>.png). Existing poses where they fit; `mood-low` is its own.
    pub fn pose(self) -> &'static str {
        match self { Mood::Low => "mood-low", Mood::Curious => "peek", Mood::Determined => "encourage", Mood::Excited => "hop", Mood::Proud => "trophy" }
    }
    pub fn label(self) -> &'static str {
        match self { Mood::Low => "feeling low", Mood::Curious => "curious", Mood::Determined => "determined", Mood::Excited => "excited", Mood::Proud => "proud" }
    }
}

/// Where the learner stands in a course. The one source both the welcome banner's copy and Sidekick's mood
/// read, so the two can't disagree.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    /// Never active anywhere: a first visit.
    First,
    /// Two or more days since the last activity (a broken streak).
    Away,
    /// Active recently, but no lesson of this course done yet.
    Ready,
    /// Under a quarter of the lessons done.
    Started,
    /// A quarter or more done.
    Midway,
    /// Four fifths or more of the lessons done, or all done with checks or the final left.
    Close,
    /// Every session check and the final passed.
    Done,
}

impl Stage {
    /// How Sidekick feels at this stage: low only after two or more days away; curious on a first visit
    /// or before the first lesson; determined from a quarter; excited from four fifths; proud when done.
    pub fn mood(self) -> Mood {
        match self {
            Stage::Done => Mood::Proud,
            Stage::Away => Mood::Low,
            Stage::First | Stage::Ready | Stage::Started => Mood::Curious,
            Stage::Midway => Mood::Determined,
            Stage::Close => Mood::Excited,
        }
    }
}

pub fn stage(st: &Stats) -> Stage {
    if st.final_passed && st.checks_passed >= st.checks_total { return Stage::Done }
    let Some(days) = st.days_since_active else { return Stage::First };
    if days >= 2 { return Stage::Away }
    if st.lessons_done == 0 { return Stage::Ready }
    match st.lessons_done * 100 / st.lessons_total.max(1) {
        p if p >= 80 => Stage::Close,
        p if p >= 25 => Stage::Midway,
        _ => Stage::Started,
    }
}

/// Sidekick's mood: whatever the learner's `stage` says (see `Stage::mood`).
pub fn sidekick_mood(st: &Stats) -> Mood { stage(st).mood() }

/// Sidekick's line on the progress card: how he feels, then one real number from the learner's stats.
/// (headline, detail), e.g. ("Sidekick is curious", "18 lessons to go.").
pub fn mood_line(st: &Stats) -> (String, String) {
    let lessons = |n: u32| if n == 1 { "1 lesson".to_string() } else { format!("{n} lessons") };
    let left = st.lessons_total.saturating_sub(st.lessons_done);
    let mood = sidekick_mood(st);
    let detail = match mood {
        Mood::Low if left > 0 => format!("One short lesson today starts a new streak. {} to go.", lessons(left)),
        Mood::Low => "One short visit today starts a new streak.".to_string(),
        Mood::Curious if st.lessons_done == 0 => format!("{} to explore, starting with the first.", lessons(st.lessons_total)),
        Mood::Curious => format!("{} to go.", lessons(left)),
        Mood::Determined => format!("{} of {} lessons done. Keep the pace.", st.lessons_done, st.lessons_total),
        Mood::Excited if left > 0 => format!("Only {} to go.", lessons(left)),
        Mood::Excited if st.checks_passed < st.checks_total => format!("{} of {} session checks passed.", st.checks_passed, st.checks_total),
        Mood::Excited => "Only the final assessment is left.".to_string(),
        Mood::Proud => "Every check and the final passed.".to_string(),
    };
    (format!("{COMPANION} is {}", mood.label()), detail)
}

#[cfg(test)]
mod mood_tests {
    use super::{sidekick_mood, stage, Mood, Stage, Stats};
    fn st(done: u32, days: Option<u32>) -> Stats {
        Stats { streak: 1, best_streak: 1, active_today: days == Some(0), days_since_active: days, lessons_done: done, lessons_total: 20,
            checks_passed: 0, checks_total: 6, final_passed: false, activity: vec![] }
    }

    #[test]
    fn the_card_line_names_the_mood_and_a_real_number() {
        use super::mood_line;
        assert_eq!(mood_line(&st(0, None)), (concat!(crate::companion_name!(), " is curious").into(), "20 lessons to explore, starting with the first.".into()));
        assert_eq!(mood_line(&st(2, Some(0))).1, "18 lessons to go.");
        assert_eq!(mood_line(&st(12, Some(4))), (concat!(crate::companion_name!(), " is feeling low").into(), "One short lesson today starts a new streak. 8 lessons to go.".into()));
        assert_eq!(mood_line(&st(10, Some(0))), (concat!(crate::companion_name!(), " is determined").into(), "10 of 20 lessons done. Keep the pace.".into()));
        assert_eq!(mood_line(&st(19, Some(0))).1, "Only 1 lesson to go.");
        assert_eq!(mood_line(&Stats { checks_passed: 3, ..st(20, Some(0)) }).1, "3 of 6 session checks passed.");
        let (h, d) = mood_line(&Stats { checks_passed: 6, final_passed: true, ..st(20, Some(0)) });
        assert_eq!(h, concat!(crate::companion_name!(), " is proud"));
        for x in [h, d] { assert!(!x.contains('\u{2014}') && !x.contains('\u{2013}'), "no em or en dashes") }
    }

    #[test]
    fn mood_and_banner_read_the_same_stage() {
        assert_eq!(stage(&st(0, None)), Stage::First);
        assert_eq!(stage(&st(0, Some(0))), Stage::Ready);
        assert_eq!(stage(&st(0, Some(1))), Stage::Ready);
        assert_eq!(stage(&st(0, Some(2))), Stage::Away);
        assert_eq!(stage(&st(2, Some(0))), Stage::Started);
        for d in [None, Some(0), Some(1), Some(2), Some(9)] {
            for n in [0, 1, 5, 16, 20] {
                let s = st(n, d);
                assert_eq!(sidekick_mood(&s), stage(&s).mood(), "{n} done, {d:?} days");
                assert_eq!(sidekick_mood(&s) == Mood::Low, stage(&s) == Stage::Away, "low only when away");
            }
        }
    }

    #[test]
    fn mood_follows_progress_and_activity() {
        assert_eq!(sidekick_mood(&st(0, None)), Mood::Curious, "a first visit");
        assert_eq!(sidekick_mood(&st(0, Some(0))), Mood::Curious, "0% with recent activity elsewhere is not low");
        assert_eq!(sidekick_mood(&st(0, Some(1))), Mood::Curious);
        assert_eq!(sidekick_mood(&st(0, Some(2))), Mood::Low, "two days away");
        assert_eq!(sidekick_mood(&st(12, Some(4))), Mood::Low, "a broken streak");
        assert_eq!(sidekick_mood(&st(3, Some(1))), Mood::Curious);
        assert_eq!(sidekick_mood(&st(5, Some(0))), Mood::Determined);
        assert_eq!(sidekick_mood(&st(16, Some(0))), Mood::Excited);
        assert_eq!(sidekick_mood(&st(20, Some(0))), Mood::Excited);
        assert_eq!(sidekick_mood(&Stats { checks_passed: 6, final_passed: true, ..st(20, Some(30)) }), Mood::Proud);
        assert_eq!(sidekick_mood(&Stats { lessons_total: 0, ..st(0, Some(0)) }), Mood::Curious);
        for m in [Mood::Low, Mood::Curious, Mood::Determined, Mood::Excited, Mood::Proud] {
            let f = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../ui/assets/mascot/{}.png", m.pose()));
            assert!(f.is_file(), "{} missing", f.display());
        }
    }
}

/// Problems with the nudge copy (same rules as tools/validate.py `check_nudges`).
pub fn nudge_problems(list: &[Nudge]) -> Vec<String> {
    let mut out = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for n in list {
        if !ids.insert(n.id.as_str()) { out.push(format!("{}: duplicate id", n.id)) }
        if !TRIGGERS.contains(&n.trigger.as_str()) { out.push(format!("{}: unknown trigger {}", n.id, n.trigger)) }
        let len = n.text.chars().count();
        if !(1..=140).contains(&len) { out.push(format!("{}: text is {len} chars, want 1-140", n.id)) }
        if n.text.contains("!!") { out.push(format!("{}: no \"!!\"", n.id)) }
        if let Some(a) = &n.action { if !NUDGE_ACTIONS.contains(&a.as_str()) { out.push(format!("{}: unknown action {a}", n.id)) } }
        if n.action.is_some() != n.label.is_some() { out.push(format!("{}: action and label go together", n.id)) }
        if let Some(h) = &n.help { if !help_url_ok(&h.url) || h.title.trim().is_empty() { out.push(format!("{}: bad help link", n.id)) } }
        if !POSES.contains(&n.pose.as_str()) { out.push(format!("{}: unknown pose {}", n.id, n.pose)) }
    }
    for t in TRIGGERS { if !list.iter().any(|n| n.trigger == t) { out.push(format!("no nudge for trigger {t}")) } }
    out
}

/// The first-run app tour (content/courses/_shared/app-tour.yaml) with its media resolved to URLs the
/// webview can load, plus the version this device has already watched to the end.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AppTour {
    pub version: u32,
    pub title: String,
    pub src: String,
    pub poster: Option<String>,
    pub captions: Option<String>,
    pub duration_s: f64,
    #[serde(default)]
    pub chapters: Vec<Chapter>,
    /// `app_tour_seen_version` from the app's settings db; 0 when never seen.
    #[serde(default)]
    pub seen_version: u32,
}

/// Whether the tour opens by itself on this launch: a real tour (version 1 or later) newer than the one seen.
pub fn tour_due(t: Option<&AppTour>) -> bool { t.is_some_and(|t| t.version > 0 && t.seen_version < t.version) }

/// The seen version to store: never lower than what is stored, so replaying an older tour can't re-arm a newer one.
pub fn tour_seen_after(stored: u32, watched: u32) -> u32 { stored.max(watched) }

/// Seconds before the end that count as watched (a learner who scrubs to the last frame is done).
pub const TOUR_END_SLACK_S: f64 = 2.0;

/// First run: Continue opens once the video ended, played to within `TOUR_END_SLACK_S` of its end, or
/// failed (an error must never trap the learner). `duration` is the element's when known, else the yaml's.
pub fn tour_can_continue(ended: bool, failed: bool, current_s: f64, duration_s: f64) -> bool {
    failed || ended || (duration_s.is_finite() && duration_s > 0.0 && current_s.is_finite() && current_s >= duration_s - TOUR_END_SLACK_S)
}

#[cfg(test)]
mod tour_tests {
    use super::*;
    fn tour(version: u32, seen: u32) -> AppTour {
        AppTour { version, title: "t".into(), src: "x".into(), poster: None, captions: None, duration_s: 90.0, chapters: vec![], seen_version: seen }
    }

    #[test]
    fn tour_opens_only_when_newer_than_seen() {
        assert!(!tour_due(None), "no tour file");
        assert!(!tour_due(Some(&tour(0, 0))), "a version-0 stub never opens");
        assert!(tour_due(Some(&tour(1, 0))), "never seen");
        assert!(!tour_due(Some(&tour(1, 1))), "seen");
        assert!(tour_due(Some(&tour(2, 1))), "a new cut opens again");
        assert!(!tour_due(Some(&tour(1, 3))), "a rolled-back tour stays closed");
        assert_eq!(tour_seen_after(0, 1), 1);
        assert_eq!(tour_seen_after(3, 1), 3, "replaying an older tour never lowers it");
    }

    #[test]
    fn continue_waits_for_the_end_unless_the_video_fails() {
        assert!(!tour_can_continue(false, false, 0.0, 90.0));
        assert!(!tour_can_continue(false, false, 87.9, 90.0));
        assert!(tour_can_continue(false, false, 88.0, 90.0), "within 2 s of the end");
        assert!(tour_can_continue(true, false, 0.0, 90.0), "ended");
        assert!(tour_can_continue(false, true, 0.0, f64::NAN), "an error opens it at once");
        assert!(!tour_can_continue(false, false, 0.0, f64::NAN), "unknown duration: wait for ended");
        assert!(!tour_can_continue(false, false, 0.0, 0.0));
        assert!(!tour_can_continue(false, false, 1.0, f64::INFINITY));
    }
}
