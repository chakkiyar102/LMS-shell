//! Loads the YAML corpus. Grader-only keys (e.g. `answer_notes`, `sources`, `unverified`)
//! are simply not declared here, so serde drops them and they never reach memory.
use serde::Deserialize;
use std::{collections::{HashMap, HashSet}, path::{Path, PathBuf}};

#[derive(Deserialize, Clone)]
pub struct Opt { pub text: String, pub feedback: String }

/// Single and multi options are shuffled once, at load (see `RawQuestion`), so the view the UI gets
/// and the indices `grade` checks come from the same order. Match questions keep their authored order.
#[derive(Deserialize, Clone)]
#[serde(from = "RawQuestion")]
pub enum Question {
    Single { prompt: String, options: Vec<Opt>, answer: usize },
    Multi { prompt: String, options: Vec<Opt>, answer: Vec<usize> },
    Match { prompt: String, left: Vec<String>, right: Vec<String>, answer: Vec<usize>, feedback: Vec<String> },
}

/// A question as authored. Authors tend to put the right option first, so the loaded `Question`
/// gets its options in an order seeded from the prompt: the same on every view and every launch.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RawQuestion {
    Single { prompt: String, options: Vec<Opt>, answer: usize },
    Multi { prompt: String, options: Vec<Opt>, answer: Vec<usize> },
    Match { prompt: String, left: Vec<String>, right: Vec<String>, answer: Vec<usize>, feedback: Vec<String> },
}

/// `perm[new] = old`: a Fisher-Yates shuffle driven by FNV-1a of the prompt and splitmix64,
/// both fixed algorithms so the order never changes with the Rust version.
fn permutation(prompt: &str, n: usize) -> Vec<usize> {
    let mut x = prompt.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    let mut next = || {
        x = x.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = x;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    };
    let mut p: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() { p.swap(i, (next() % (i as u64 + 1)) as usize); }
    p
}

/// Reorders options (each keeps its feedback) and maps answer indices to the new positions.
/// An out-of-range answer stays out of range, so a malformed file still grades as wrong, not as a panic.
fn shuffle(prompt: &str, options: Vec<Opt>, answer: &[usize]) -> (Vec<Opt>, Vec<usize>) {
    let perm = permutation(prompt, options.len());
    let new_pos = |old: usize| perm.iter().position(|&o| o == old).unwrap_or(old);
    let mut slots: Vec<Option<Opt>> = options.into_iter().map(Some).collect();
    let options = perm.iter().map(|&o| slots[o].take().expect("perm is a permutation")).collect();
    (options, answer.iter().map(|&a| new_pos(a)).collect())
}

impl From<RawQuestion> for Question {
    fn from(r: RawQuestion) -> Self {
        match r {
            RawQuestion::Single { prompt, options, answer } => {
                let (options, a) = shuffle(&prompt, options, &[answer]);
                Question::Single { prompt, options, answer: a[0] }
            }
            RawQuestion::Multi { prompt, options, answer } => {
                let (options, mut answer) = shuffle(&prompt, options, &answer);
                answer.sort_unstable();
                Question::Multi { prompt, options, answer }
            }
            RawQuestion::Match { prompt, left, right, answer, feedback } => Question::Match { prompt, left, right, answer, feedback },
        }
    }
}

impl Question {
    pub fn prompt(&self) -> &str {
        match self { Question::Single { prompt, .. } | Question::Multi { prompt, .. } | Question::Match { prompt, .. } => prompt }
    }

    pub fn view(&self) -> shared::QuestionView {
        use shared::QuestionView as V;
        let texts = |o: &[Opt]| o.iter().map(|o| o.text.clone()).collect();
        match self {
            Question::Single { prompt, options, .. } => V::Single { prompt: prompt.clone(), options: texts(options) },
            Question::Multi { prompt, options, .. } => V::Multi { prompt: prompt.clone(), options: texts(options) },
            Question::Match { prompt, left, right, .. } => V::Match { prompt: prompt.clone(), left: left.clone(), right: right.clone() },
        }
    }
}

/// `video:` accepts a bare path/URL or a full block with captions and chapters.
#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum VideoSrc {
    Path(String),
    Full { src: String, poster: Option<String>, captions: Option<String>, #[serde(default)] chapters: Vec<shared::Chapter> },
}

#[derive(Deserialize, Clone)]
pub struct Lesson {
    pub id: String,
    pub session: String,
    pub number: String,
    pub title: String,
    pub duration_min: u32,
    pub objective: String,
    pub video: Option<VideoSrc>,
    pub body: String,
    pub key_points: Vec<String>,
    #[serde(default)] pub gotchas: Vec<String>,
    #[serde(default)] pub quick_check: Vec<Question>,
    #[serde(default)] pub help: Vec<shared::HelpLink>,
}

#[derive(Deserialize, Clone)]
pub struct Check { pub id: String, pub title: String, pub pass_mark: f32, pub questions: Vec<Question> }

#[derive(Deserialize, Clone)]
pub struct Exercise {
    pub id: String,
    pub title: String,
    pub minutes: u32,
    pub brief: String,
    #[serde(default)] pub setup: Vec<String>,
    pub acceptance_criteria: Vec<String>,
    pub reference_walkthrough: String,
}

#[derive(Deserialize, Clone)]
pub struct Task { pub id: String, pub minutes: u32, pub questions: Vec<Question> }

#[derive(Deserialize, Clone)]
pub struct Assessment { pub id: String, pub title: String, pub minutes: u32, pub blurb: String, pub pass_mark: f32, pub fixture: String, pub tasks: Vec<Task> }

#[derive(Deserialize, Clone)]
pub struct Session { pub id: String, #[serde(default)] pub phase: Option<String>, pub title: String, pub outcome: String, pub lessons: Vec<String>, pub check: String, pub exercise: String }

#[derive(Deserialize, Clone)]
pub struct Course {
    pub id: String, pub title: String, #[serde(default)] pub short: Option<String>, pub summary: String,
    #[serde(default = "default_order")] pub order: u32,
    #[serde(default)] pub shared: Vec<String>,
    /// Locked until an access code is entered (see access.rs).
    #[serde(default)] pub gated: bool,
    pub sessions: Vec<Session>, pub assessment: String,
}

fn default_order() -> u32 { 100 }

pub struct Content {
    pub dir: PathBuf,
    /// The `content/courses` dir: media paths resolve against it so shared and course videos use one scheme.
    pub root: PathBuf,
    /// Ids of lessons/checks/exercises that came from a `_shared` dir.
    pub shared: HashSet<String>,
    pub course: Course,
    pub lessons: HashMap<String, Lesson>,
    pub checks: HashMap<String, Check>,
    pub exercises: HashMap<String, Exercise>,
    pub assessment: Assessment,
}

fn read<T: serde::de::DeserializeOwned>(p: &Path) -> Result<T, String> {
    let s = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    serde_yaml::from_str(&s).map_err(|e| format!("{}: {e}", p.display()))
}

fn read_all<T: serde::de::DeserializeOwned>(dir: &Path, id: impl Fn(&T) -> String) -> Result<HashMap<String, T>, String> {
    let mut out = HashMap::new();
    for e in std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?.flatten() {
        if e.path().extension().is_some_and(|x| x == "yaml") {
            let v: T = read(&e.path())?;
            out.insert(id(&v), v);
        }
    }
    Ok(out)
}

/// Adds shared items to a course's map, recording their ids; an id clash is an error.
fn merge<T>(into: &mut HashMap<String, T>, from: HashMap<String, T>, ids: &mut HashSet<String>) -> Result<(), String> {
    for (k, v) in from {
        if into.contains_key(&k) { return Err(format!("duplicate id {k} between course and shared sessions")); }
        ids.insert(k.clone());
        into.insert(k, v);
    }
    Ok(())
}

impl Content {
    /// Progress and attempts are stored per course so ids can repeat across courses; shared sessions
    /// use one key so finishing them in one course counts in every course that includes them.
    pub fn key(&self, id: &str) -> String {
        if self.shared.contains(id) { format!("shared/{id}") } else { format!("{}/{id}", self.course.id) }
    }

    /// Fails loudly on any malformed file: a silently missing lesson is worse than a startup error.
    pub fn load(dir: &Path) -> Result<Self, String> {
        let course: Course = read(&dir.join("course.yaml"))?;
        let root = dir.parent().unwrap_or(dir).to_path_buf();
        let mut c = Content {
            dir: dir.to_path_buf(),
            root: root.clone(),
            shared: HashSet::new(),
            lessons: read_all(&dir.join("lessons"), |l: &Lesson| l.id.clone())?,
            checks: read_all(&dir.join("checks"), |c: &Check| c.id.clone())?,
            exercises: read_all(&dir.join("exercises"), |e: &Exercise| e.id.clone())?,
            assessment: read(&dir.join("assessment").join(format!("{}.yaml", course.assessment)))?,
            course,
        };
        for name in c.course.shared.clone() {
            let sd = root.join("_shared").join(&name);
            merge(&mut c.lessons, read_all(&sd.join("lessons"), |l: &Lesson| l.id.clone())?, &mut c.shared)?;
            merge(&mut c.checks, read_all(&sd.join("checks"), |c: &Check| c.id.clone())?, &mut c.shared)?;
            merge(&mut c.exercises, read_all(&sd.join("exercises"), |e: &Exercise| e.id.clone())?, &mut c.shared)?;
        }
        for s in &c.course.sessions {
            for l in &s.lessons {
                if !c.lessons.contains_key(l) { return Err(format!("course.yaml references missing lesson {l}")); }
            }
        }
        Ok(c)
    }

    /// Local paths are served through Tauri's asset protocol; https URLs pass through (CDN-hosted video later).
    pub fn media_url(&self, src: &str) -> String { media_url(&self.root, src) }

    pub fn video(&self, l: &Lesson) -> Option<shared::VideoView> {
        Some(match l.video.as_ref()? {
            VideoSrc::Path(p) => shared::VideoView { url: self.media_url(p), poster: None, captions: None, chapters: vec![] },
            VideoSrc::Full { src, poster, captions, chapters } => shared::VideoView {
                url: self.media_url(src),
                poster: poster.as_deref().map(|p| self.media_url(p)),
                captions: captions.as_deref().map(|p| self.media_url(p)),
                chapters: chapters.clone(),
            },
        })
    }
}

/// The content dir as an absolute path with no `..` or symlinks: the asset protocol refuses any
/// path containing `..`, and the scope `run()` grants is matched against canonical paths.
/// A missing dir is returned as given so `load_all` reports it.
pub fn content_root(p: &Path) -> PathBuf { std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()) }

/// A media path (relative to `courses`, the `content/courses` dir) as a URL the webview can load: local
/// files go through Tauri's asset protocol; https URLs pass through (CDN-hosted video later).
pub fn media_url(courses: &Path, src: &str) -> String {
    if src.starts_with("https://") { return src.to_string(); }
    let enc = urlencoding::encode(&courses.join(src).to_string_lossy()).into_owned();
    // Tauri serves the asset protocol at http://asset.localhost on Windows and Android.
    if cfg!(any(windows, target_os = "android")) { format!("http://asset.localhost/{enc}") } else { format!("asset://localhost/{enc}") }
}

/// `content/courses/_shared/app-tour.yaml` (see docs/course-tour.md). Paths are relative to
/// `content/courses`, like a lesson's `video:` block.
#[derive(Deserialize)]
struct TourFile {
    version: u32,
    title: String,
    src: String,
    #[serde(default)] poster: Option<String>,
    #[serde(default)] captions: Option<String>,
    duration_s: f64,
    #[serde(default)] chapters: Vec<shared::Chapter>,
}

pub const APP_TOUR: &str = "_shared/app-tour.yaml";

/// The app tour under `<root>/courses`, or None for "no tour": no file, a file that doesn't parse, the
/// version-0 stub, or a video that isn't on disk. A missing poster or captions file is dropped, not fatal.
/// The learner is never blocked on a tour that won't start. `seen_version` is left at 0 for the caller.
pub fn app_tour(root: &Path) -> Option<shared::AppTour> {
    let courses = root.join("courses");
    let t: TourFile = match read(&courses.join(APP_TOUR)) {
        Ok(t) => t,
        Err(e) => { if courses.join(APP_TOUR).exists() { eprintln!("app tour ignored: {e}") } return None }
    };
    // a local file must exist and stay inside content/courses (the asset protocol refuses `..` anyway)
    let local_ok = |p: &str| p.starts_with("https://") || (!p.split(['/', '\\']).any(|s| s == "..") && courses.join(p).is_file());
    if t.version == 0 || !local_ok(&t.src) { return None; }
    let opt = |p: Option<String>| p.filter(|p| local_ok(p)).map(|p| media_url(&courses, &p));
    Some(shared::AppTour {
        version: t.version,
        title: t.title,
        src: media_url(&courses, &t.src),
        poster: opt(t.poster),
        captions: opt(t.captions),
        duration_s: t.duration_s,
        chapters: t.chapters,
        seen_version: 0,
    })
}

/// Every course under `<root>/courses/<id>/course.yaml`, ordered by (`order`, folder name); `_`-prefixed dirs are skipped.
pub fn load_all(root: &Path) -> Result<Vec<Content>, String> {
    let dir = root.join("courses");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten().map(|e| e.path()).filter(|p| p.join("course.yaml").exists())
        .filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('_'))).collect();
    dirs.sort();
    if dirs.is_empty() { return Err(format!("no courses found under {}", dir.display())); }
    let mut all = dirs.iter().map(|d| Content::load(d)).collect::<Result<Vec<_>, _>>()?;
    all.sort_by_key(|c| c.course.order); // stable: ties keep folder-name order
    Ok(all)
}

/// Markdown from our own corpus only. LLM output is never passed through here.
/// `![caption](url)` becomes a captioned figure: an mp4 plays as a muted loop, a YouTube or Vimeo link
/// embeds, anything else (png, gif) is a lazy image. Media is hotlinked from the source article.
pub fn md(s: &str) -> String {
    use pulldown_cmark::{Event, Tag, TagEnd};
    let mut out = String::new();
    let mut media: Option<(String, String)> = None; // (url, caption so far)
    let events = pulldown_cmark::Parser::new_ext(s, pulldown_cmark::Options::ENABLE_TABLES).filter_map(|e| match e {
        Event::Start(Tag::Image { dest_url, .. }) => { media = Some((dest_url.to_string(), String::new())); None }
        Event::Text(t) | Event::Code(t) if media.is_some() => { media.as_mut().unwrap().1.push_str(&t); None }
        Event::End(TagEnd::Image) => media.take().map(|(url, cap)| Event::Html(figure(&url, &cap).into())),
        _ if media.is_some() => None,
        e => Some(e),
    });
    pulldown_cmark::html::push_html(&mut out, events);
    // a figure on its own line is still wrapped in <p>; unwrap it so the HTML stays valid
    out.replace("<p><figure", "<figure").replace("</figure></p>", "</figure>")
}

fn figure(url: &str, cap: &str) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;");
    let (u, c) = (esc(url), esc(cap));
    let el = if let Some(id) = url.split_once("youtube.com/watch?v=").map(|x| x.1).or_else(|| url.split_once("youtu.be/").map(|x| x.1)) {
        let id: String = id.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
        format!(r#"<iframe src="https://www.youtube-nocookie.com/embed/{id}" title="{c}" loading="lazy" allow="encrypted-media; fullscreen; picture-in-picture"></iframe>"#)
    } else if let Some(rest) = url.split_once("player.vimeo.com/video/").map(|x| x.1) {
        // unlisted videos need their ?h= hash; dnt=1 keeps Vimeo from tracking the learner
        let (id, hash) = rest.split_once("?h=").unwrap_or((rest, ""));
        let keep = |s: &str| s.chars().take_while(|c| c.is_ascii_alphanumeric()).collect::<String>();
        format!(r#"<iframe src="https://player.vimeo.com/video/{}?h={}&amp;dnt=1" title="{c}" loading="lazy" allow="fullscreen; picture-in-picture"></iframe>"#, keep(id), keep(hash))
    } else if url.ends_with(".mp4") || url.ends_with(".webm") {
        format!(r#"<video src="{u}" autoplay loop muted playsinline preload="metadata" aria-label="{c}"></video>"#)
    } else {
        format!(r#"<img src="{u}" alt="{c}" loading="lazy">"#)
    };
    format!(r#"<figure class="media">{el}<figcaption>{c}</figcaption></figure>"#)
}

/// Lesson text without media lines, for the tutor's context and word counts.
pub fn strip_media(s: &str) -> String {
    s.lines().filter(|l| !l.trim_start().starts_with("![")).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    #[test]
    fn media_lines_become_figures() {
        let h = super::md("Text.\n\n![Form builder & fields](https://x.test/a.gif)\n\n![Demo](https://youtu.be/abc_12?t=3)\n");
        assert!(h.contains(r#"<figure class="media"><img src="https://x.test/a.gif" alt="Form builder &amp; fields" loading="lazy"><figcaption>Form builder &amp; fields</figcaption></figure>"#), "{h}");
        assert!(h.contains("youtube-nocookie.com/embed/abc_12\""));
        assert!(!h.contains("<p><figure"));
        assert!(super::md("![v](https://player.vimeo.com/video/914754751?h=313c98a0bb)").contains(r#"src="https://player.vimeo.com/video/914754751?h=313c98a0bb&amp;dnt=1""#));
        assert_eq!(super::strip_media("a\n![x](y)\nb"), "a\nb");
    }

    fn texts(q: &super::Question) -> (Vec<String>, Vec<String>, Vec<usize>) {
        use super::Question::*;
        match q {
            Single { options, answer, .. } => (options.iter().map(|o| o.text.clone()).collect(), options.iter().map(|o| o.feedback.clone()).collect(), vec![*answer]),
            Multi { options, answer, .. } => (options.iter().map(|o| o.text.clone()).collect(), options.iter().map(|o| o.feedback.clone()).collect(), answer.clone()),
            Match { right, answer, .. } => (right.clone(), vec![], answer.clone()),
        }
    }

    /// Authors tend to put the right option first, so options are shuffled at load with a seed taken
    /// from the prompt: the same order on every view and every launch, and the answer index follows
    /// its option so grading and feedback stay attached to the option the learner picked.
    #[test]
    fn options_shuffle_stably_and_answers_follow_their_option() {
        let opts = "options:\n  - { text: right, feedback: f-right }\n  - { text: w1, feedback: f-w1 }\n  - { text: w2, feedback: f-w2 }\n  - { text: w3, feedback: f-w3 }\n  - { text: w4, feedback: f-w4 }\n";
        let mut moved = 0;
        for n in 0..20 {
            let y = format!("kind: single\nprompt: Which one {n}?\n{opts}answer: 0\n");
            let a: super::Question = serde_yaml::from_str(&y).unwrap();
            let b: super::Question = serde_yaml::from_str(&y).unwrap();
            let (t, f, ans) = texts(&a);
            assert_eq!(t, texts(&b).0, "order must be stable");
            assert_eq!(t[ans[0]], "right");
            assert!(t.iter().zip(&f).all(|(t, f)| *f == format!("f-{t}")), "feedback left its option: {t:?} {f:?}");
            assert_eq!(crate::grade::grade(&a, &ans).score, 1.0);
            moved += (ans[0] != 0) as usize;

            let y = format!("kind: multi\nprompt: Which ones {n}?\n{opts}answer: [0, 1, 2]\n");
            let m: super::Question = serde_yaml::from_str(&y).unwrap();
            let (t, f, ans) = texts(&m);
            let mut picked: Vec<&str> = ans.iter().map(|i| t[*i].as_str()).collect();
            picked.sort();
            assert_eq!(picked, ["right", "w1", "w2"]);
            assert!(t.iter().zip(&f).all(|(t, f)| *f == format!("f-{t}")));
            assert_eq!(crate::grade::grade(&m, &ans).score, 1.0);
        }
        assert!(moved >= 10, "the right option stayed first in {} of 20", 20 - moved);

        // match questions keep their authored order
        let x: super::Question = serde_yaml::from_str("kind: match\nprompt: p\nleft: [a, b]\nright: [x, y]\nanswer: [1, 0]\nfeedback: []\n").unwrap();
        assert_eq!(texts(&x).0, ["x", "y"]);
        assert_eq!(texts(&x).2, [1, 0]);
    }

    #[test]
    fn shuffled_answers_are_not_all_first() {
        let all = super::load_all(&real_content_root()).unwrap();
        let firsts: Vec<bool> = all.iter().flat_map(|c| c.checks.values()).flat_map(|ch| &ch.questions).filter_map(|q| match q {
            super::Question::Single { answer, .. } => Some(*answer == 0),
            _ => None,
        }).collect();
        assert!(firsts.len() > 2 && firsts.iter().filter(|f| **f).count() * 2 < firsts.len() + 2, "{firsts:?}");
    }

    fn fixture_root() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/content") }
    fn dup_fixture() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dup/courses/c-course") }

    #[test]
    fn ordering_and_shared_sessions() {
        let all = super::load_all(&fixture_root()).unwrap();
        assert_eq!(all.iter().map(|c| c.course.id.as_str()).collect::<Vec<_>>(), ["b-course", "a-course", "d-course"]);
        let b = &all[0];
        assert!(b.lessons.contains_key("base-1") && b.shared.contains("base-1") && b.shared.contains("base-check"));
        assert!(!b.shared.contains("b-course-1"));
        assert!(b.media_url("_shared/base/videos/x.mp4").contains("_shared%2Fbase%2Fvideos%2Fx.mp4"));
    }

    /// The asset protocol 403s any path holding `..` and any path outside the scope `run()` allows
    /// (`<root>/courses`), so every lesson's video, poster and captions must decode to an existing file
    /// under that dir with no `..`, even when LMS_CONTENT_DIR is given as `src-tauri/../content`.
    #[test]
    fn media_urls_resolve_inside_the_allowed_dir() {
        let root = super::content_root(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../content"));
        let allowed = root.join("courses");
        let mut n = 0;
        for c in super::load_all(&root).unwrap() {
            for l in c.lessons.values() {
                let Some(v) = c.video(l) else { continue };
                for u in [Some(v.url), v.poster, v.captions].into_iter().flatten() {
                    let enc = u.strip_prefix("asset://localhost/").or(u.strip_prefix("http://asset.localhost/")).expect(&u);
                    let p = std::path::PathBuf::from(urlencoding::decode(enc).unwrap().into_owned());
                    assert!(tauri::path::SafePathBuf::new(p.clone()).is_ok(), "`..` in {}", p.display());
                    assert!(p.starts_with(&allowed) && p.is_file(), "{} not a file under {}", p.display(), allowed.display());
                    n += 1;
                }
            }
        }
        // the sample course ships no videos, so this checks whatever lessons you add with media
        let with_video: usize = super::load_all(&root).unwrap().iter().map(|c| c.lessons.values().filter(|l| c.video(l).is_some()).count()).sum();
        assert!(n >= with_video, "{n} media files for {with_video} lessons with video");
    }

    /// A temp content root holding `courses/_shared/app-tour.yaml` and the given media files.
    fn tour_root(yaml: &str, files: &[&str]) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("lms-shell-tour-{}", uuid::Uuid::new_v4()));
        let shared = root.join("courses/_shared");
        std::fs::create_dir_all(shared.join("videos")).unwrap();
        std::fs::write(shared.join("app-tour.yaml"), yaml).unwrap();
        for f in files { std::fs::write(root.join("courses").join(f), b"x").unwrap(); }
        root
    }

    const TOUR: &str = concat!("version: 2\ntitle: Your course tour\nsrc: _shared/videos/app-tour.mp4\nposter: _shared/videos/app-tour.jpg\n\
        captions: _shared/videos/app-tour.vtt\nduration_s: 94.5\nchapters:\n  - { t: 0, title: Home }\n  - { t: 31.2, title: ", shared::companion_name!(), " }\n");

    #[test]
    fn app_tour_loads_with_resolved_urls() {
        let root = tour_root(TOUR, &["_shared/videos/app-tour.mp4", "_shared/videos/app-tour.jpg", "_shared/videos/app-tour.vtt"]);
        let t = super::app_tour(&root).expect("tour");
        assert_eq!((t.version, t.title.as_str(), t.duration_s, t.seen_version), (2, "Your course tour", 94.5, 0));
        assert_eq!(t.chapters.len(), 2);
        assert_eq!(t.chapters[1].title, concat!(shared::companion_name!()));
        assert!(t.src.ends_with("courses%2F_shared%2Fvideos%2Fapp-tour.mp4"), "{}", t.src);
        assert!(t.poster.as_deref().is_some_and(|p| p.ends_with("app-tour.jpg")));
        assert!(t.captions.as_deref().is_some_and(|p| p.ends_with("app-tour.vtt")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn app_tour_is_none_unless_it_can_play() {
        // no file at all
        let empty = std::env::temp_dir().join(format!("lms-shell-tour-{}", uuid::Uuid::new_v4()));
        assert!(super::app_tour(&empty).is_none());
        // the version-0 stub, even with the video present
        let stub = tour_root(&TOUR.replace("version: 2", "version: 0"), &["_shared/videos/app-tour.mp4"]);
        assert!(super::app_tour(&stub).is_none());
        // a tour whose video hasn't landed
        let novideo = tour_root(TOUR, &["_shared/videos/app-tour.jpg"]);
        assert!(super::app_tour(&novideo).is_none());
        // malformed yaml
        let bad = tour_root("version: two\n", &[]);
        assert!(super::app_tour(&bad).is_none());
        // `..` never leaves content/courses
        let out = tour_root(&TOUR.replace("src: _shared/videos/app-tour.mp4", "src: ../app-tour.mp4"), &["_shared/videos/app-tour.mp4"]);
        std::fs::write(out.join("app-tour.mp4"), b"x").unwrap();
        assert!(super::app_tour(&out).is_none());
        // poster and captions missing: the tour still plays, without them
        let bare = tour_root(TOUR, &["_shared/videos/app-tour.mp4"]);
        let t = super::app_tour(&bare).expect("video alone is enough");
        assert!(t.poster.is_none() && t.captions.is_none());
        // an https video needs no local file
        let cdn = tour_root(&TOUR.replace("src: _shared/videos/app-tour.mp4", "src: https://cdn.example/t.mp4"), &[]);
        assert_eq!(super::app_tour(&cdn).unwrap().src, "https://cdn.example/t.mp4");
        for r in [stub, novideo, bad, out, bare, cdn] { let _ = std::fs::remove_dir_all(r); }
    }

    /// Whatever the real corpus ships (the stub or the real tour), it loads without error and any URL it
    /// gives decodes to a file inside the asset protocol's scope.
    #[test]
    fn real_app_tour_resolves_inside_the_allowed_dir() {
        let root = super::content_root(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../content"));
        let yaml = root.join("courses").join(super::APP_TOUR);
        assert!(yaml.is_file(), "{} missing (a version-0 stub is fine)", yaml.display());
        let _: super::TourFile = super::read(&yaml).expect("app-tour.yaml parses");
        if let Some(t) = super::app_tour(&root) {
            for u in [Some(t.src), t.poster, t.captions].into_iter().flatten() {
                let Some(enc) = u.strip_prefix("asset://localhost/").or(u.strip_prefix("http://asset.localhost/")) else { continue };
                let p = std::path::PathBuf::from(urlencoding::decode(enc).unwrap().into_owned());
                assert!(p.starts_with(root.join("courses")) && p.is_file(), "{}", p.display());
            }
        }
    }

    #[test]
    fn duplicate_shared_id_is_an_error() {
        // c-course defines lesson base-1 locally and also lists shared: [base]
        assert!(super::Content::load(&dup_fixture()).err().expect("dup id must fail").contains("base-1"));
    }

    #[test]
    fn real_corpus_loads() {
        let all = super::load_all(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../content")).expect("corpus loads");
        assert_eq!(all.iter().map(|c| c.course.id.as_str()).collect::<Vec<_>>(), ["brightline-basics"]);
        let c = &all[0];
        assert_eq!(c.course.sessions.len(), 2);
        assert_eq!(c.lessons.len(), 6);
        assert!(!c.course.gated);
        for s in &c.course.sessions {
            assert!(c.checks.contains_key(&s.check), "{}: check {} missing", s.id, s.check);
            assert!(c.exercises.contains_key(&s.exercise), "{}: exercise {} missing", s.id, s.exercise);
        }
        assert_eq!(c.assessment.tasks.len(), 3);
        assert!(c.assessment.blurb.starts_with("Help a small team"));
        assert!(c.media_url("videos/x.mp4").ends_with("%2Fcontent%2Fcourses%2Fvideos%2Fx.mp4"), "{}", c.media_url("videos/x.mp4"));
        assert_eq!(c.media_url("https://cdn.example/x.mp4"), "https://cdn.example/x.mp4");
    }

    /// Loads a corpus laid out as `<LMS_CORPUS>/courses/...` (for example a copy of the published courses
    /// plus drafts). Run with `LMS_CORPUS=/path cargo test -p lms-shell corpus_from_env -- --ignored`.
    #[test]
    #[ignore]
    fn corpus_from_env() {
        let root = std::env::var("LMS_CORPUS").expect("set LMS_CORPUS to a content root");
        let dir = std::path::Path::new(&root).join("courses");
        let mut bad = vec![];
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.join("course.yaml").exists() && !p.file_name().unwrap().to_string_lossy().starts_with('_') {
                if let Err(m) = super::Content::load(&p) { bad.push(m); }
            }
        }
        assert!(bad.is_empty(), "corpus failures:\n{}", bad.join("\n"));
    }

    fn real_content_root() -> std::path::PathBuf { std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../content") }

    #[test]
    fn shared_items_key_across_courses() {
        let all = super::load_all(&fixture_root()).unwrap();
        let (b, a) = (&all[0], &all[1]);
        assert_eq!(b.key("base-1"), "shared/base-1");
        assert_eq!(b.key("b-course-1"), "b-course/b-course-1");
        assert_eq!(a.key("a-course-1"), "a-course/a-course-1");
    }

    #[test]
    fn progress_keys_are_course_scoped() {
        let all = super::load_all(&real_content_root()).unwrap();
        assert_eq!(all[0].key("s1-check"), "brightline-basics/s1-check");
    }

    #[test]
    fn shared_progress_counts_in_every_course() {
        let all = super::load_all(&fixture_root()).unwrap();
        let (b, d) = (&all[0], &all[2]);
        let path = std::env::temp_dir().join(format!("lms-shell-keys-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let db = crate::db::open(&path).unwrap();
        crate::db::save_progress(&db, &b.key("base-1"), 0.0, true).unwrap();
        let g = shared::Graded { score: 1.0, passed: true, results: vec![] };
        crate::db::record_attempt(&db, "check", &b.key("base-check"), "[]", &g).unwrap();
        assert!(crate::db::progress(&db, &d.key("base-1")).1);
        assert_eq!(crate::db::best_score(&db, &d.key("base-check")), Some(1.0));
        assert!(!crate::db::progress(&db, &d.key("d-course-1")).1);
        drop(db);
        let _ = std::fs::remove_file(&path);
    }
}
