//! Coach marks: a spotlight ring on one element and a short bubble beside it, the first time a
//! learner meets something. Each shows once per device (`sidekick_coach_seen`), and an unprompted one
//! waits at least `GAP_MS` after the last (`sidekick_coach_last`). Pure rules here; sidekick.rs renders.
use std::collections::BTreeSet;

/// What "Show me" does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShowMe {
    /// Scroll the target into view, focus the first control in it, and pulse the ring.
    Focus,
    /// Open Sidekick's Where to? menu.
    Menu,
}

/// Where a mark may appear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum On { Home, Lesson, Exercise, Check, Certificate, Settings, Any, Coach }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mark {
    pub id: &'static str,
    pub on: On,
    /// CSS selector of the element the ring goes round; the first visible match wins.
    pub target: &'static str,
    pub title: &'static str,
    pub text: &'static str,
    pub show_me: ShowMe,
}

/// In priority order: the first eligible one on the page is the one that shows.
pub const MARKS: &[Mark] = &[
    Mark { id: "where-to", on: On::Home, target: ".sidekick-tab", title: "I live here",
        text: "Click me any time to ask about the course, get coached through a lesson, or jump to your next step.", show_me: ShowMe::Menu },
    Mark { id: "continue", on: On::Home, target: "[data-sidekick-anchor=\"continue\"]", title: "Pick up where you left off",
        text: "This card always opens the next lesson you haven't finished.", show_me: ShowMe::Focus },
    Mark { id: "first-lesson", on: On::Lesson, target: "[data-sidekick-anchor=\"complete\"]", title: "Finished reading?",
        text: "Mark the lesson complete here. It saves your progress and tells me what's next.", show_me: ShowMe::Focus },
    Mark { id: "quick-check", on: On::Lesson, target: ".block .quiz", title: "A quick check",
        text: "A few questions to lock it in. They don't count toward your certificate, so relax.", show_me: ShowMe::Focus },
    Mark { id: "practice", on: On::Lesson, target: "section.practice", title: "Try it for real",
        text: "Start a coaching session to open your practice app beside the lesson, with me alongside.", show_me: ShowMe::Focus },
    Mark { id: "first-exercise", on: On::Exercise, target: "section.criteria", title: "Your first exercise",
        text: "Build it in your practice app, then describe how you met each of these criteria.", show_me: ShowMe::Focus },
    Mark { id: "first-check", on: On::Check, target: ".quiz", title: "A session check",
        text: "This one counts toward your certificate. Take your time, and you can retry.", show_me: ShowMe::Focus },
    Mark { id: "ai-provider", on: On::Settings, target: "[data-sidekick-anchor=\"ai-provider\"]", title: "Set up an AI provider",
        text: {concat!("Ask ", shared::companion_name!(), ", the guide and reviews need one. Pick a provider here, then test it.")}, show_me: ShowMe::Focus },
    Mark { id: "streak", on: On::Home, target: ".streak-pill", title: "Your streak",
        text: "The flame counts days in a row with some study. One lesson a day keeps it lit.", show_me: ShowMe::Focus },
    // the coaching column: shown when the pointer lingers near one the learner hasn't used yet
    Mark { id: "coach-look", on: On::Coach, target: "[data-help=\"coach.look\"]", title: "Look at my screen",
        text: "Ask about your practice screen and I'll tell you where to click. I read the page; I don't change it.", show_me: ShowMe::Focus },
    Mark { id: "coach-act", on: On::Coach, target: "[data-help=\"coach.act\"]", title: "Do it for me",
        text: "I do the step in your practice app while you watch. Risky ones like Submit ask first. Needs an OpenAI API key.", show_me: ShowMe::Focus },
    Mark { id: "coach-hint", on: On::Coach, target: "[data-help=\"coach.hint\"]", title: "Show a hint",
        text: "Stuck? Hints come one small step at a time, so you can stop as soon as it clicks.", show_me: ShowMe::Focus },
    Mark { id: "coach-next", on: On::Coach, target: "[data-help=\"coach.next-stage\"]", title: "Next stage",
        text: "Done with this part? Move on. You can click any stage number to go back.", show_me: ShowMe::Focus },
    Mark { id: "coach-send", on: On::Coach, target: "[data-help=\"coach.send\"]", title: "Talk to me here",
        text: {concat!("Tell me what you did or where you're stuck. Coach mode guides you; Ask ", shared::companion_name!(), " answers straight.")}, show_me: ShowMe::Focus },
    Mark { id: "certificate", on: On::Certificate, target: ".cert-actions", title: "You earned it",
        text: "Save your certificate as a PDF or print it from here.", show_me: ShowMe::Focus },
];

pub fn mark(id: &str) -> Option<&'static Mark> { MARKS.iter().find(|m| m.id == id) }

/// The old fixed gap between unprompted coach marks; the gap now follows familiarity (sidekick/familiar.rs),
/// from 45 s for someone new to this at the middle, then longer.
#[cfg(test)]
pub const GAP_MS: f64 = 180_000.0;
/// A mark up this long has been seen, even if the learner then scrolls or moves on without acting on it.
pub const SEEN_AFTER_MS: u64 = 3_000;

pub fn parse_seen(v: Option<&str>) -> BTreeSet<String> {
    v.and_then(|s| serde_json::from_str::<Vec<String>>(s).ok()).unwrap_or_default().into_iter().collect()
}

pub fn seen_str(s: &BTreeSet<String>) -> String { serde_json::to_string(&s.iter().collect::<Vec<_>>()).unwrap_or_else(|_| "[]".into()) }

/// Bad or missing → never shown (0).
pub fn parse_last(v: Option<&str>) -> f64 { v.and_then(|s| s.parse::<f64>().ok()).filter(|x| x.is_finite()).unwrap_or(0.0) }

/// Whether mark `id` may show now. Prompted marks (the learner asked, e.g. an AI feature that needs a
/// provider) skip the rate limit but still respect "once" unless `force`.
pub fn may_show(id: &str, seen: &BTreeSet<String>, last_ms: f64, now_ms: f64, prompted: bool, gap: f64) -> bool {
    !seen.contains(id) && (prompted || now_ms - last_ms >= gap || now_ms < last_ms)
}

/// The first mark for `on` that may show and whose condition `ready(mark)` holds (its target is on
/// screen, its data says it applies).
pub fn next(on: On, seen: &BTreeSet<String>, last_ms: f64, now_ms: f64, gap: f64, ready: impl Fn(&Mark) -> bool) -> Option<&'static Mark> {
    if !may_show("", &BTreeSet::new(), last_ms, now_ms, false, gap) { return None; }
    MARKS.iter().filter(|m| m.on == on).find(|m| may_show(m.id, seen, last_ms, now_ms, false, gap) && ready(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_mark_shows_once() {
        let mut seen = BTreeSet::new();
        let now = 10.0 * GAP_MS;
        let m = next(On::Lesson, &seen, 0.0, now, GAP_MS, |_| true).unwrap();
        assert_eq!(m.id, "first-lesson");
        seen.insert(m.id.to_string());
        let back = parse_seen(Some(&seen_str(&seen)));
        assert_eq!(back, seen);
        assert_eq!(next(On::Lesson, &back, 0.0, now, GAP_MS, |_| true).unwrap().id, "quick-check", "the next one, never the same again");
        assert!(!may_show("first-lesson", &back, 0.0, now, true, GAP_MS), "even prompted, once is once");
        for bad in [None, Some(""), Some("{}"), Some("[1]")] { assert!(parse_seen(bad).is_empty()) }
    }

    #[test]
    fn one_unprompted_mark_per_three_minutes() {
        let seen = BTreeSet::new();
        let last = 1_000_000.0;
        assert!(next(On::Home, &seen, last, last + 60_000.0, GAP_MS, |_| true).is_none());
        assert!(next(On::Home, &seen, last, last + GAP_MS - 1.0, GAP_MS, |_| true).is_none());
        assert!(next(On::Home, &seen, last, last + GAP_MS, GAP_MS, |_| true).is_some());
        // someone new to the app hears from Sidekick sooner (familiarity 0: 45 s)
        let new = crate::sidekick::familiar::gap_ms(0.0);
        assert!(next(On::Home, &seen, last, last + 44_000.0, new, |_| true).is_none());
        assert!(next(On::Home, &seen, last, last + 45_000.0, new, |_| true).is_some());
        assert!(may_show("ai-provider", &seen, last, last + 1.0, true, GAP_MS), "a prompted mark skips the wait");
        assert!(next(On::Home, &seen, last, last - 5.0, GAP_MS, |_| true).is_some(), "a clock that went backwards doesn't lock marks out");
        assert_eq!(parse_last(Some("nope")), 0.0);
    }

    #[test]
    fn marks_are_short_kind_and_unique() {
        let ids: BTreeSet<_> = MARKS.iter().map(|m| m.id).collect();
        assert_eq!(ids.len(), MARKS.len());
        for m in MARKS {
            assert!(m.text.len() <= 120 && m.title.len() <= 40, "{}", m.id);
            for s in [m.text, m.title] { assert!(!s.contains('\u{2014}') && !s.contains('\u{2013}'), "{s}") }
        }
        for on in [On::Home, On::Lesson, On::Exercise, On::Check, On::Certificate, On::Settings, On::Coach] {
            assert!(MARKS.iter().any(|m| m.on == on), "{on:?}");
        }
        assert!(next(On::Home, &BTreeSet::new(), 0.0, GAP_MS, GAP_MS, |m| m.id == "streak").is_some_and(|m| m.id == "streak"), "conditions are honoured");
    }
}
