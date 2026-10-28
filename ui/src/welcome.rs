//! The welcome banner on the course home: a greeting by first name and one line that says where the
//! learner stands and what's next. The copy is pure and tested natively; the component follows.
pub use shared::{stage, Stage};
use shared::Stats;

/// The first word of the profile name, if there is one.
pub fn first_name(name: &str) -> Option<&str> { name.split_whitespace().next() }

#[derive(Clone, Debug, PartialEq)]
pub struct Welcome { pub eyebrow: &'static str, pub headline: String, pub line: String }

/// `next`: the first unfinished lesson's number, title and minutes.
pub fn welcome(name: &str, st: &Stats, next: Option<(&str, &str, u32)>) -> Welcome {
    let s = stage(st);
    let who = first_name(name);
    let greet = |plain: &str| match who { Some(n) => format!("{plain}, {n}"), None => plain.to_string() };
    let up_next = || next.map(|(n, t, m)| format!("Next up: {n} {t}, about {m} minutes.")).unwrap_or_default();
    let left = st.lessons_total.saturating_sub(st.lessons_done);
    let checks_left = st.checks_total.saturating_sub(st.checks_passed);
    let streak = if st.streak >= 2 { format!(" {}-day streak.", st.streak) } else { String::new() };
    let (headline, line) = match s {
        Stage::First => (greet("Glad you're here"), match next {
            Some((n, t, m)) => format!("{} lessons ahead. Start with {n} {t}, about {m} minutes.", st.lessons_total),
            None => format!("{} lessons ahead.", st.lessons_total),
        }),
        Stage::Ready => (greet("Ready to start"), match next {
            Some((n, t, m)) => format!("{} lessons ahead. Start with {n} {t}, about {m} minutes.", st.lessons_total),
            None => format!("{} lessons ahead.", st.lessons_total),
        }),
        Stage::Away => (greet("Ready when you are"), format!("It's been {} days. {}", st.days_since_active.unwrap_or(2), up_next())),
        Stage::Started => (greet("Good start"), format!("{} of {} lessons done.{streak} {}", st.lessons_done, st.lessons_total, up_next())),
        Stage::Midway => (greet("Keep going"), format!("{} of {} lessons done.{streak} {}", st.lessons_done, st.lessons_total, up_next())),
        Stage::Close if left > 0 => (greet("Almost there"), format!("{left} {} left, then the session checks and the final.", if left == 1 { "lesson" } else { "lessons" })),
        Stage::Close => (greet("Almost there"), match (checks_left, st.final_passed) {
            (0, _) => "Every lesson and session check is done. The final assessment is all that's left.".to_string(),
            (n, _) => format!("Every lesson is done. {n} session {} and the final stand between you and the certificate.", if n == 1 { "check" } else { "checks" }),
        }),
        Stage::Done => (greet("You did it"), "Every check and the final are passed. Your certificate is ready.".to_string()),
    };
    Welcome { eyebrow: if s == Stage::First { "Welcome" } else { "Welcome back" }, headline, line: line.trim().replace("  ", " ") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(done: u32, days: Option<u32>) -> Stats {
        Stats { streak: 0, best_streak: 0, active_today: days == Some(0), days_since_active: days, lessons_done: done, lessons_total: 20,
            checks_passed: 0, checks_total: 6, final_passed: false, activity: vec![] }
    }
    const NEXT: Option<(&str, &str, u32)> = Some(("2.3", "Expressions", 6));

    #[test]
    fn stages_follow_progress_and_activity() {
        assert_eq!(stage(&st(0, None)), Stage::First);
        assert_eq!(stage(&st(5, Some(3))), Stage::Away);
        assert_eq!(stage(&st(2, Some(0))), Stage::Started);
        assert_eq!(stage(&st(0, Some(1))), Stage::Ready, "0% with recent activity elsewhere: not started yet");
        assert_eq!(stage(&st(9, Some(1))), Stage::Midway);
        assert_eq!(stage(&st(16, Some(0))), Stage::Close);
        assert_eq!(stage(&Stats { checks_passed: 6, final_passed: true, ..st(20, Some(9)) }), Stage::Done);
        assert_eq!(stage(&Stats { lessons_total: 0, ..st(0, Some(0)) }), Stage::Ready, "an empty course doesn't divide by zero");
    }

    #[test]
    fn copy_greets_by_first_name_and_names_whats_next() {
        let w = welcome("Jordan Hale", &st(0, None), NEXT);
        assert_eq!((w.eyebrow, w.headline.as_str()), ("Welcome", "Glad you're here, Jordan"));
        assert_eq!(welcome("", &st(0, None), NEXT).headline, "Glad you're here");
        assert_eq!(w.line, "20 lessons ahead. Start with 2.3 Expressions, about 6 minutes.");
        let w = welcome("  ", &st(9, Some(0)), NEXT);
        assert_eq!((w.eyebrow, w.headline.as_str()), ("Welcome back", "Keep going"));
        assert_eq!(w.line, "9 of 20 lessons done. Next up: 2.3 Expressions, about 6 minutes.");
        let w = welcome("Jordan", &Stats { streak: 4, ..st(2, Some(0)) }, NEXT);
        assert_eq!(w.line, "2 of 20 lessons done. 4-day streak. Next up: 2.3 Expressions, about 6 minutes.");
        let w = welcome("Jordan Hale", &st(0, Some(0)), NEXT);
        assert_eq!((w.eyebrow, w.headline.as_str()), ("Welcome back", "Ready to start, Jordan"));
        assert_eq!(w.line, "20 lessons ahead. Start with 2.3 Expressions, about 6 minutes.");
        assert_eq!(shared::sidekick_mood(&st(0, Some(0))), shared::Mood::Curious, "banner and companion agree");
        assert_eq!(welcome("Jordan", &st(5, Some(3)), NEXT).headline, "Ready when you are, Jordan");
        assert!(welcome("Jordan", &st(19, Some(0)), NEXT).line.starts_with("1 lesson left"));
        assert!(welcome("Jordan", &Stats { checks_passed: 5, ..st(20, Some(0)) }, None).line.contains("1 session check and the final"));
        assert!(welcome("Jordan", &Stats { checks_passed: 6, final_passed: true, ..st(20, Some(0)) }, None).line.contains("certificate is ready"));
        for d in [None, Some(0), Some(5)] {
            for n in [0, 3, 10, 17, 20] {
                let w = welcome("Jordan Hale", &st(n, d), NEXT);
                for s in [w.headline.as_str(), w.line.as_str()] {
                    assert!(!s.contains('\u{2014}') && !s.contains('!') && !s.contains("  ") && !s.ends_with(' '), "{s:?}");
                }
            }
        }
    }
}
