//! Dumps what the Tauri commands return for the real corpus, as JSON, for the browser tour
//! (tools/ui-check/tour.mjs). No Tauri runtime: the content and db modules are compiled in as-is,
//! and each command body below mirrors its counterpart in src/lib.rs.
//!
//! LMS_CONTENT_DIR=content cargo run -p lms-shell --example tour_dump -- OUT.json
//!
//! Two scenarios, each from its own temp SQLite db:
//!   "fresh": nothing done, never active (first launch);
//!   "main": a 4-day streak and the first course part done (session 1 finished, session 2 started).
#![allow(dead_code)] // the modules carry items only the app uses

#[path = "../src/content.rs"]
mod content;
#[path = "../src/db.rs"]
mod db;
#[path = "../src/access.rs"]
mod access;

use content::{md, Content, Question};
use serde_json::{json, Map, Value};
use shared::*;

fn course_view(c: &Content, db: &rusqlite::Connection) -> CourseView {
    CourseView {
        id: c.course.id.clone(),
        title: c.course.title.clone(),
        summary: c.course.summary.clone(),
        final_minutes: c.assessment.minutes,
        final_blurb: c.assessment.blurb.clone(),
        sessions: c.course.sessions.iter().map(|ses| SessionView {
            id: ses.id.clone(),
            phase: ses.phase.clone(),
            title: ses.title.clone(),
            outcome: ses.outcome.clone(),
            lessons: ses.lessons.iter().filter_map(|id| c.lessons.get(id)).map(|l| LessonSummary {
                id: l.id.clone(),
                number: l.number.clone(),
                title: l.title.clone(),
                duration_min: l.duration_min,
                has_video: l.video.is_some(),
                completed: db::progress(db, &c.key(&l.id)).1,
            }).collect(),
            check: ses.check.clone(),
            exercise: ses.exercise.clone(),
            check_best: db::best_score(db, &c.key(&ses.check)),
        }).collect(),
    }
}

fn lesson_view(c: &Content, l: &content::Lesson, db: &rusqlite::Connection) -> LessonView {
    LessonView {
        id: l.id.clone(),
        number: l.number.clone(),
        title: l.title.clone(),
        objective: l.objective.clone(),
        duration_min: l.duration_min,
        body_html: md(&l.body),
        key_points: l.key_points.clone(),
        gotchas: l.gotchas.clone(),
        video: c.video(l),
        resume_at: db::progress(db, &c.key(&l.id)).0,
        quick_check: l.quick_check.iter().map(Question::view).collect(),
        help: l.help.clone(),
    }
}

fn check_view(ch: &content::Check) -> QuizView {
    QuizView {
        id: ch.id.clone(),
        title: ch.title.clone(),
        pass_mark: ch.pass_mark,
        minutes: None,
        intro_html: String::new(),
        sections: vec![QuizSection { title: String::new(), questions: ch.questions.iter().map(Question::view).collect() }],
    }
}

fn final_view(a: &content::Assessment) -> QuizView {
    QuizView {
        id: a.id.clone(),
        title: a.title.clone(),
        pass_mark: a.pass_mark,
        minutes: Some(a.minutes),
        intro_html: md(&a.fixture),
        sections: a.tasks.iter().map(|t| QuizSection {
            title: format!("{} · {} min", t.id.replace('-', " "), t.minutes),
            questions: t.questions.iter().map(Question::view).collect(),
        }).collect(),
    }
}

fn exercise_view(c: &Content, x: &content::Exercise, db: &rusqlite::Connection) -> ExerciseView {
    let last = db::last_submission(db, &c.key(&x.id));
    ExerciseView {
        id: x.id.clone(),
        title: x.title.clone(),
        minutes: x.minutes,
        brief_html: md(&x.brief),
        setup: x.setup.clone(),
        acceptance_criteria: x.acceptance_criteria.clone(),
        walkthrough_html: last.as_ref().map(|_| md(&x.reference_walkthrough)),
        last_submission: last.as_ref().map(|l| l.0.clone()),
        last_review: last.and_then(|l| l.1),
    }
}

fn lesson_counts(c: &Content, db: &rusqlite::Connection) -> (usize, usize) {
    let lessons: Vec<_> = c.course.sessions.iter().flat_map(|s| &s.lessons).filter_map(|id| c.lessons.get(id)).collect();
    (lessons.iter().filter(|l| db::progress(db, &c.key(&l.id)).1).count(), lessons.len())
}

/// get_certificate without the `name` write and without issuing (none of these scenarios has earned one).
fn certificate(c: &Content, db: &rusqlite::Connection, name: &str) -> CertificateView {
    let mut missing = Vec::new();
    for (i, ses) in c.course.sessions.iter().enumerate() {
        let need = c.checks.get(&ses.check).map(|ch| ch.pass_mark).unwrap_or(1.0);
        match db::best_score(db, &c.key(&ses.check)) {
            Some(b) if b >= need => {}
            Some(b) => missing.push(Todo { label: format!("Session {} check: best {:.0}%, needs {:.0}%", i + 1, b * 100.0, need * 100.0), check: Some(ses.check.clone()) }),
            None => missing.push(Todo { label: format!("Session {} check: not attempted", i + 1), check: Some(ses.check.clone()) }),
        }
    }
    let final_score = db::best_score(db, &c.key(&c.assessment.id));
    match final_score {
        Some(b) if b >= c.assessment.pass_mark => {}
        Some(b) => missing.push(Todo { label: format!("Final assessment: best {:.0}%, needs {:.0}%", b * 100.0, c.assessment.pass_mark * 100.0), check: None }),
        None => missing.push(Todo { label: "Final assessment: not attempted".into(), check: None }),
    }
    let (lessons_done, lessons_total) = lesson_counts(c, db);
    CertificateView { course: c.course.title.clone(), name: name.into(), missing, final_score, lessons_done, lessons_total, issued_on: None, id: None }
}

fn stats(c: &Content, db: &rusqlite::Connection) -> Stats {
    const DAYS: i64 = 140;
    let days = db::activity_days(db);
    let today = db::today_jd(db);
    let active: std::collections::HashSet<i64> = days.iter().map(|d| d.0).collect();
    let run_from = |mut d: i64| { let mut n = 0; while active.contains(&d) { n += 1; d -= 1; } n };
    let streak = if active.contains(&today) { run_from(today) } else { run_from(today - 1) };
    let (mut best, mut cur, mut prev) = (0u32, 0u32, i64::MIN);
    for (d, _) in &days { cur = if *d == prev + 1 { cur + 1 } else { 1 }; best = best.max(cur); prev = *d; }
    let counts: std::collections::HashMap<i64, i64> = days.iter().cloned().collect();
    let activity = (0..DAYS).map(|i| *counts.get(&(today - DAYS + 1 + i)).unwrap_or(&0) as u32).collect();
    let lessons: Vec<&String> = c.course.sessions.iter().flat_map(|x| &x.lessons).collect();
    let lessons_done = lessons.iter().filter(|l| db::progress(db, &c.key(l)).1).count() as u32;
    let checks_passed = c.course.sessions.iter().filter(|x| {
        let pass = c.checks.get(&x.check).map(|ch| ch.pass_mark).unwrap_or(1.0);
        db::best_score(db, &c.key(&x.check)).is_some_and(|b| b >= pass)
    }).count() as u32;
    Stats {
        streak,
        best_streak: best.max(streak),
        active_today: active.contains(&today),
        days_since_active: days.last().map(|d| (today - d.0).max(0) as u32),
        lessons_done,
        lessons_total: lessons.len() as u32,
        checks_passed,
        checks_total: c.course.sessions.len() as u32,
        final_passed: db::best_score(db, &c.key(&c.assessment.id)).is_some_and(|b| b >= c.assessment.pass_mark),
        activity,
    }
}

fn attempt(db: &rusqlite::Connection, c: &Content, id: &str, score: f32, pass: f32) {
    let g = Graded { score, passed: score >= pass, results: vec![] };
    db::record_attempt(db, "check", &c.key(id), "[]", &g).unwrap();
}

/// The "main" learner: a 4-day streak plus scattered earlier days, the first course part done.
fn seed_main(all: &[Content], db: &rusqlite::Connection) {
    for (ago, n) in [(0, 3), (1, 2), (2, 4), (3, 1), (6, 2), (7, 3), (9, 1), (13, 2), (14, 1), (20, 2), (27, 1)] {
        db.execute("INSERT INTO activity (day, kind, count) VALUES (date('now', 'localtime', ?1), 'lesson', ?2)",
            rusqlite::params![format!("-{ago} day"), n]).unwrap();
    }
    let cd = &all[0];
    let done: Vec<&String> = cd.course.sessions[0].lessons.iter().chain(cd.course.sessions[1].lessons.iter().take(1)).collect();
    for l in done { db::save_progress(db, &cd.key(l), 0.0, true).unwrap(); }
    if let Some(l) = cd.course.sessions[1].lessons.get(1) { db::save_progress(db, &cd.key(l), 41.0, false).unwrap(); }
    let pass = |i: usize| cd.checks.get(&cd.course.sessions[i].check).map(|c| c.pass_mark).unwrap_or(0.7);
    attempt(db, cd, &cd.course.sessions[0].check, 0.9, pass(0));
}

fn scenario(all: &[Content], seed: impl Fn(&[Content], &rusqlite::Connection), name: &str) -> Value {
    let path = std::env::temp_dir().join(format!("lms-shell-tour-{}.db", uuid::Uuid::new_v4()));
    let db = db::open(&path).unwrap();
    seed(all, &db);
    let summaries = |locked_gated: bool| -> Vec<CourseSummary> {
        all.iter().map(|c| CourseSummary {
            id: c.course.id.clone(),
            title: c.course.title.clone(),
            short: c.course.short.clone().unwrap_or_else(|| c.course.title.clone()),
            locked: c.course.gated && locked_gated,
        }).collect()
    };
    let mut courses = Map::new();
    for c in all {
        let lessons: Map<String, Value> = c.lessons.values().map(|l| (l.id.clone(), json!(lesson_view(c, l, &db)))).collect();
        let checks: Map<String, Value> = c.checks.values().map(|ch| (ch.id.clone(), json!(check_view(ch)))).collect();
        let exercises: Map<String, Value> = c.exercises.values().map(|x| (x.id.clone(), json!(exercise_view(c, x, &db)))).collect();
        courses.insert(c.course.id.clone(), json!({
            "course": course_view(c, &db),
            "stats": stats(c, &db),
            "certificate": certificate(c, &db, name),
            "final": final_view(&c.assessment),
            "lessons": lessons,
            "checks": checks,
            "exercises": exercises,
        }));
    }
    drop(db);
    let _ = std::fs::remove_file(&path);
    json!({
        "courses_open": summaries(false),
        "courses_locked": summaries(true),
        "by_course": courses,
    })
}

fn main() {
    let out = std::env::args().nth(1).expect("usage: tour_dump OUT.json");
    let root = content::content_root(std::path::Path::new(&std::env::var("LMS_CONTENT_DIR").unwrap_or_else(|_| "content".into())));
    let all = content::load_all(&root).expect("content loads");
    let nudges: Vec<Nudge> = serde_yaml::from_str(&std::fs::read_to_string(root.join("sidekick-nudges.yaml")).unwrap().replace("{companion}", shared::brand::COMPANION)).unwrap();
    let name = "Jordan Hale";
    let v = json!({
        "content_root": root.join("courses").to_string_lossy(),
        "nudges": nudges,
        // null until a tour film is added (the stub is version 0; see docs/course-tour.md)
        "app_tour": content::app_tour(&root),
        // unlock_courses with a made-up code: the real refusal text
        "unlock_error": access::check("casey.mercer", "20500.WRONG-CODE", access::today()).unwrap_err(),
        "settings": Settings { provider: Provider::default(), has_api_key: false, practice_url: "https://notes.brightline.example".into(), practice_dev_url: String::new() },
        "fresh": scenario(&all, |_, _| {}, name),
        "main": scenario(&all, seed_main, name),
    });
    std::fs::write(&out, serde_json::to_string(&v).unwrap()).unwrap();
    eprintln!("wrote {out}");
}
