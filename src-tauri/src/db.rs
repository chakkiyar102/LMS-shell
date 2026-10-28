use rusqlite::{params, Connection, OptionalExtension};

pub fn open(path: &std::path::Path) -> rusqlite::Result<Connection> {
    let c = Connection::open(path)?;
    c.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        CREATE TABLE IF NOT EXISTS lesson_progress (lesson_id TEXT PRIMARY KEY, position_s REAL NOT NULL DEFAULT 0,
          completed INTEGER NOT NULL DEFAULT 0, updated_at TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS attempts (id TEXT PRIMARY KEY, kind TEXT NOT NULL, item_id TEXT NOT NULL,
          answers TEXT NOT NULL, score REAL NOT NULL, passed INTEGER NOT NULL, created_at TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS attempts_item ON attempts(item_id, created_at);
        CREATE TABLE IF NOT EXISTS exercise_submissions (id TEXT PRIMARY KEY, exercise_id TEXT NOT NULL,
          submission TEXT NOT NULL, review TEXT, created_at TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS messages (id TEXT PRIMARY KEY, lesson_id TEXT NOT NULL, role TEXT NOT NULL,
          content TEXT NOT NULL, created_at TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS messages_lesson ON messages(lesson_id, created_at);
        CREATE TABLE IF NOT EXISTS certificates (course_id TEXT PRIMARY KEY, id TEXT NOT NULL, issued_on TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS activity (day TEXT NOT NULL, kind TEXT NOT NULL, count INTEGER NOT NULL DEFAULT 0, PRIMARY KEY (day, kind));
        CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS sidekick_audit (at TEXT NOT NULL, mode TEXT NOT NULL, action TEXT NOT NULL, label TEXT NOT NULL,
          env TEXT NOT NULL, decision TEXT NOT NULL, reason TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS ai_usage (day TEXT PRIMARY KEY, calls INTEGER NOT NULL DEFAULT 0, tokens INTEGER NOT NULL DEFAULT 0);
        "#,
    )?;
    Ok(c)
}

/// Settings → Start over: empties every table in one transaction (the schema stays), then compacts
/// the file and the WAL so the erased rows don't linger on disk. Leaves the db as `open` makes a new one.
pub fn wipe(c: &mut Connection) -> rusqlite::Result<()> {
    let tx = c.transaction()?;
    let tables: Vec<String> = {
        let mut st = tx.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")?;
        let rows = st.query_map([], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for t in &tables { tx.execute(&format!("DELETE FROM \"{}\"", t.replace('"', "\"\"")), [])?; }
    tx.commit()?;
    // best effort: the rows are already gone; these only reclaim the pages that held them
    let _ = c.execute_batch("VACUUM; PRAGMA wal_checkpoint(TRUNCATE);");
    Ok(())
}

fn id() -> String { uuid::Uuid::new_v4().to_string() }

pub fn get_setting(c: &Connection, key: &str) -> Option<String> {
    c.query_row("SELECT value FROM app_settings WHERE key = ?1", [key], |r| r.get(0)).optional().ok().flatten()
}

pub fn set_setting(c: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    c.execute("INSERT INTO app_settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value", [key, value])?;
    Ok(())
}

pub fn progress(c: &Connection, lesson_id: &str) -> (f64, bool) {
    c.query_row("SELECT position_s, completed FROM lesson_progress WHERE lesson_id = ?1", [lesson_id], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0)))
        .optional().ok().flatten().unwrap_or((0.0, false))
}

/// `completed` only ever flips on: rewatching a finished lesson doesn't un-finish it.
pub fn save_progress(c: &Connection, lesson_id: &str, position_s: f64, completed: bool) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO lesson_progress (lesson_id, position_s, completed, updated_at) VALUES (?1, ?2, ?3, datetime('now'))
         ON CONFLICT(lesson_id) DO UPDATE SET position_s = excluded.position_s,
           completed = MAX(completed, excluded.completed), updated_at = excluded.updated_at",
        params![lesson_id, position_s, completed as i64],
    )?;
    Ok(())
}

pub fn record_attempt(c: &Connection, kind: &str, item_id: &str, answers: &str, g: &shared::Graded) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO attempts (id, kind, item_id, answers, score, passed, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
        params![id(), kind, item_id, answers, g.score, g.passed as i64],
    )?;
    Ok(())
}

pub fn best_score(c: &Connection, item_id: &str) -> Option<f32> {
    c.query_row("SELECT MAX(score) FROM attempts WHERE item_id = ?1", [item_id], |r| r.get(0)).ok().flatten()
}

pub fn last_submission(c: &Connection, exercise_id: &str) -> Option<(String, Option<String>)> {
    c.query_row(
        "SELECT submission, review FROM exercise_submissions WHERE exercise_id = ?1 ORDER BY created_at DESC LIMIT 1",
        [exercise_id], |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional().ok().flatten()
}

pub fn add_submission(c: &Connection, exercise_id: &str, submission: &str, review: Option<&str>) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO exercise_submissions (id, exercise_id, submission, review, created_at) VALUES (?1, ?2, ?3, ?4, datetime('now'))",
        params![id(), exercise_id, submission, review],
    )?;
    Ok(())
}

pub fn messages(c: &Connection, lesson_id: &str) -> Vec<shared::Msg> {
    let Ok(mut st) = c.prepare("SELECT role, content FROM messages WHERE lesson_id = ?1 ORDER BY created_at, rowid") else { return vec![] };
    st.query_map([lesson_id], |r| Ok(shared::Msg { role: r.get(0)?, content: r.get(1)? }))
        .map(|rows| rows.flatten().collect()).unwrap_or_default()
}

pub fn add_message(c: &Connection, lesson_id: &str, m: &shared::Msg) -> rusqlite::Result<()> {
    c.execute(
        "INSERT INTO messages (id, lesson_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, datetime('now'))",
        params![id(), lesson_id, m.role, m.content],
    )?;
    Ok(())
}

/// Issued once, on the first visit after everything is passed; later visits return the same record.
pub fn issue_certificate(c: &Connection, course_id: &str) -> rusqlite::Result<(String, String)> {
    let short: String = id().chars().filter(|c| c.is_ascii_hexdigit()).take(12).collect::<String>().to_uppercase();
    c.execute(
        "INSERT OR IGNORE INTO certificates (course_id, id, issued_on) VALUES (?1, ?2, date('now'))",
        params![course_id, format!("CERT-{}-{}", &short[..4], &short[4..12])],
    )?;
    c.query_row("SELECT id, issued_on FROM certificates WHERE course_id = ?1", [course_id], |r| Ok((r.get(0)?, r.get(1)?)))
}

/// One row per local day and kind; feeds the streak and the activity strip.
pub fn record_activity(c: &Connection, kind: &str) {
    let _ = c.execute(
        "INSERT INTO activity (day, kind, count) VALUES (date('now', 'localtime'), ?1, 1)
         ON CONFLICT(day, kind) DO UPDATE SET count = count + 1",
        [kind],
    );
}

/// (julian day number, total events) for every active day, oldest first.
pub fn activity_days(c: &Connection) -> Vec<(i64, i64)> {
    let Ok(mut st) = c.prepare("SELECT CAST(julianday(day) AS INTEGER), SUM(count) FROM activity GROUP BY day ORDER BY day") else { return vec![] };
    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map(|rows| rows.flatten().collect()).unwrap_or_default()
}

pub fn today_jd(c: &Connection) -> i64 {
    c.query_row("SELECT CAST(julianday(date('now', 'localtime')) AS INTEGER)", [], |r| r.get(0)).unwrap_or_default()
}

// ---- Sidekick's activity log (screen-helper actions and guardrail decisions) ----

/// Rows kept; older ones are dropped as new ones arrive.
const AUDIT_KEEP: i64 = 2000;

/// Appends one decision. Labels are cut to 80 characters; typed text and chosen values never get here.
pub fn audit_add(c: &Connection, a: &shared::SidekickActivity) {
    let label: String = a.label.chars().take(80).collect();
    let _ = c.execute(
        "INSERT INTO sidekick_audit (at, mode, action, label, env, decision, reason) VALUES (datetime('now', 'localtime'), ?1, ?2, ?3, ?4, ?5, ?6)",
        params![a.mode, a.action, label, a.env, a.decision, a.reason],
    );
    let _ = c.execute("DELETE FROM sidekick_audit WHERE rowid <= (SELECT MAX(rowid) FROM sidekick_audit) - ?1", [AUDIT_KEEP]);
}

/// Newest first.
pub fn audit_list(c: &Connection, limit: u32) -> Vec<shared::SidekickActivity> {
    let Ok(mut st) = c.prepare("SELECT at, mode, action, label, env, decision, reason FROM sidekick_audit ORDER BY rowid DESC LIMIT ?1") else { return vec![] };
    st.query_map([limit], |r| Ok(shared::SidekickActivity { at: r.get(0)?, mode: r.get(1)?, action: r.get(2)?, label: r.get(3)?, env: r.get(4)?, decision: r.get(5)?, reason: r.get(6)? }))
        .map(|rows| rows.flatten().collect()).unwrap_or_default()
}

/// The log as CSV, oldest first, for Settings → Export.
pub fn audit_csv(c: &Connection) -> String {
    let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let mut rows = audit_list(c, AUDIT_KEEP as u32);
    rows.reverse();
    let mut out = String::from("time,mode,action,label,environment,decision,reason\n");
    for a in rows { out.push_str(&[q(&a.at), q(&a.mode), q(&a.action), q(&a.label), q(&a.env), q(&a.decision), q(&a.reason)].join(",")); out.push('\n'); }
    out
}

// ---- AI usage per local day (the soft daily limit) ----

/// The default soft limit on AI requests per day; the learner can change it in Settings (0: none).
pub const DEFAULT_DAILY_CAP: u32 = 300;

pub fn usage_today(c: &Connection) -> (u32, u64) {
    c.query_row("SELECT calls, tokens FROM ai_usage WHERE day = date('now', 'localtime')", [], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional().ok().flatten().unwrap_or((0, 0))
}

pub fn usage_add(c: &Connection, tokens: u64) {
    let _ = c.execute(
        "INSERT INTO ai_usage (day, calls, tokens) VALUES (date('now', 'localtime'), 1, ?1)
         ON CONFLICT(day) DO UPDATE SET calls = calls + 1, tokens = tokens + excluded.tokens",
        [tokens as i64],
    );
}

pub fn daily_cap(c: &Connection) -> u32 {
    get_setting(c, "ai_daily_cap").and_then(|v| v.parse().ok()).unwrap_or(DEFAULT_DAILY_CAP)
}

pub const LIMIT_NOTICE: &str = concat!(shared::companion_name!(), " has reached today's AI limit on this computer, so it's taking a break until tomorrow. \
    You can raise the limit in Settings, under ", shared::companion_name!(), " safety and AI usage. Lessons, checks and your certificate don't need AI.");

/// Ok while today's requests are under the learner's limit.
pub fn usage_check(c: &Connection) -> Result<(), String> {
    let cap = daily_cap(c);
    if cap > 0 && usage_today(c).0 >= cap { Err(LIMIT_NOTICE.into()) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> (Connection, std::path::PathBuf) {
        let p = std::env::temp_dir().join(format!("lms-shell-test-{}.db", uuid::Uuid::new_v4()));
        (open(&p).unwrap(), p)
    }

    #[test]
    fn the_daily_limit_stops_at_the_cap_and_zero_means_none() {
        let (c, p) = fresh();
        assert_eq!(daily_cap(&c), DEFAULT_DAILY_CAP);
        set_setting(&c, "ai_daily_cap", "2").unwrap();
        assert!(usage_check(&c).is_ok());
        usage_add(&c, 100);
        usage_add(&c, 50);
        assert_eq!(usage_today(&c), (2, 150));
        assert_eq!(usage_check(&c).unwrap_err(), LIMIT_NOTICE);
        set_setting(&c, "ai_daily_cap", "0").unwrap();
        assert!(usage_check(&c).is_ok(), "0 is no limit");
        drop(c);
        let _ = std::fs::remove_file(p);
    }

    #[test]
    fn the_activity_log_keeps_labels_not_values_and_start_over_clears_it() {
        let (mut c, p) = fresh();
        let a = shared::SidekickActivity { mode: "act".into(), action: "click".into(), label: "D".repeat(200), env: "Production".into(), decision: "block".into(), reason: "delete".into(), ..Default::default() };
        audit_add(&c, &a);
        let rows = audit_list(&c, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label.len(), 80);
        assert!(!rows[0].at.is_empty());
        assert!(audit_csv(&c).starts_with("time,mode,action,label,environment,decision,reason\n"));
        wipe(&mut c).unwrap();
        assert!(audit_list(&c, 10).is_empty() && usage_today(&c) == (0, 0));
        drop(c);
        let _ = std::fs::remove_file(p);
    }
}
