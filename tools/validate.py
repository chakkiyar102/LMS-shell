#!/usr/bin/env python3
"""Validate the course corpus against content/SCHEMA.md.
Usage: python3 tools/validate.py [ROOT ...]   (default: every content/courses/* and content/drafts/* with a course.yaml)"""
import pathlib, re, sys
import yaml

CONTENT = pathlib.Path(__file__).resolve().parent.parent / "content"
MEDIA_DIR = CONTENT.parent / "ui" / "assets" / "media"
MEDIA_REF = re.compile(r"assets/media/([^\s)\"'\]]+)")
BRAND_RS = CONTENT.parent / "shared" / "src" / "brand.rs"


def help_prefix(path=BRAND_RS):
    """`HELP_URL_PREFIX` from shared/src/brand.rs, so the app and the validator agree; "https://" if absent."""
    try:
        m = re.search(r'HELP_URL_PREFIX\s*:\s*&str\s*=\s*"([^"]*)"', path.read_text(encoding="utf-8"))
    except OSError:
        m = None
    return m.group(1) if m else "https://"


HELP_URL_PREFIX = help_prefix()


def help_url_ok(url, prefix=None):
    """Same rule as shared::help_url_ok: https, under the prefix, something after it, no spaces, quotes or angle brackets."""
    prefix = HELP_URL_PREFIX if prefix is None else prefix
    return (isinstance(url, str) and url.startswith("https://") and url.startswith(prefix) and len(url) > len(prefix)
            and len(url) > len("https://") + 3 and not re.search(r"[\s\x00-\x1f\x7f\"'<>\\]", url))
BODY_WORDS = (150, 1100)       # lesson body word count, media lines excluded
KEY_POINTS = (3, 6)
QUICK_CHECK_QUESTIONS = 3
CHECK_QUESTIONS = 10
MIN_FINAL_TASKS = 3
errors = []
notes = []


# lesson ids each shared dir's courses list, to find shared lessons no course shows
shared_listed = {}


def err(where, msg):
    errors.append(f"{where}: {msg}")


# Fields the Rust loader (src-tauri/src/content.rs) requires, with the type serde expects.
# "strs" = list of strings (an unquoted ': ' turns an item into a map and the loader rejects it).
STR, INT, NUM, STRS, LIST = "str", "int", "num", "strs", "list"
LESSON_REQ = {"id": STR, "session": STR, "number": STR, "title": STR, "duration_min": INT, "objective": STR, "body": STR, "key_points": STRS}
CHECK_REQ = {"id": STR, "title": STR, "pass_mark": NUM, "questions": LIST}
EXERCISE_REQ = {"id": STR, "title": STR, "minutes": INT, "brief": STR, "acceptance_criteria": STRS, "reference_walkthrough": STR}
FINAL_REQ = {"id": STR, "title": STR, "minutes": INT, "blurb": STR, "pass_mark": NUM, "fixture": STR, "tasks": LIST}
TASK_REQ = {"id": STR, "minutes": INT, "questions": LIST}
SESSION_REQ = {"id": STR, "title": STR, "outcome": STR, "lessons": STRS, "check": STR, "exercise": STR}
COURSE_REQ = {"id": STR, "title": STR, "summary": STR, "sessions": LIST, "assessment": STR}
OPTIONAL_STRS = {"gotchas": STRS, "setup": STRS}


def _ok(v, t):
    if t == STR:
        return isinstance(v, str)
    if t == INT:
        return isinstance(v, int) and not isinstance(v, bool) and v >= 0
    if t == NUM:
        return isinstance(v, (int, float)) and not isinstance(v, bool)
    if t == STRS:
        return isinstance(v, list) and all(isinstance(x, str) for x in v)
    return isinstance(v, list)


def check_shape(where, d, required, optional=None):
    """Fail where the Rust loader would: a required field is absent or has the wrong type."""
    if not isinstance(d, dict):
        err(where, "not a mapping"); return
    for k, t in required.items():
        if k not in d or d[k] is None:
            err(where, f"missing required field `{k}` (the app's loader rejects this file)")
        elif not _ok(d[k], t):
            hint = " (unquoted ': ' in a list item parses as a map?)" if t == STRS else ""
            err(where, f"field `{k}` must be {t}, got {type(d[k]).__name__}{hint}")
    for k, t in (optional or {}).items():
        if d.get(k) is not None and not _ok(d[k], t):
            err(where, f"field `{k}` must be {t}, got {type(d[k]).__name__} (unquoted ': ' in a list item parses as a map?)")


def check_pass_mark(where, d):
    pm = d.get("pass_mark")
    if _ok(pm, NUM) and not 0 < pm <= 1:
        err(where, f"pass_mark {pm!r}, want a fraction above 0 and at most 1 (0.75 is the convention)")


def check_question(where, q):
    if not isinstance(q, dict):
        err(where, "question is not a mapping"); return
    kind = q.get("kind")
    if kind in ("single", "multi"):
        opts = q.get("options") or []
        if len(opts) < 2:
            err(where, "fewer than 2 options")
        for i, o in enumerate(opts):
            if not (isinstance(o, dict) and isinstance(o.get("text"), str) and o.get("text") and isinstance(o.get("feedback"), str) and o.get("feedback")):
                err(where, f"option {i} missing text/feedback (both must be strings)")
        ans = q.get("answer")
        idx = [ans] if kind == "single" else ans
        if not isinstance(idx, list) or not idx or not all(isinstance(a, int) and not isinstance(a, bool) and 0 <= a < len(opts) for a in idx):
            err(where, f"bad answer {ans!r}")
        elif kind == "multi" and len(set(idx)) != len(idx):
            err(where, f"answer repeats an index {ans!r}")
        else:
            # Convention: the correct option's feedback starts with "Correct". Where any option uses it, the set must equal `answer`.
            marked = {i for i, o in enumerate(opts) if isinstance(o, dict) and str(o.get("feedback") or "").lstrip().startswith("Correct")}
            if marked and marked != set(idx):
                err(where, f"answer {sorted(set(idx))} disagrees with options whose feedback starts 'Correct' {sorted(marked)}: {str(q.get('prompt'))[:70]!r}")
    elif kind == "match":
        left, right, ans = q.get("left") or [], q.get("right") or [], q.get("answer") or []
        if not (_ok(left, STRS) and _ok(right, STRS) and _ok(q.get("feedback") or [], STRS)):
            err(where, "match left/right/feedback must be lists of strings")
        if not left or not right:
            err(where, "match needs non-empty left and right lists")
        if len(ans) != len(left):
            err(where, "answer length != left length")
        if not all(isinstance(a, int) and not isinstance(a, bool) and 0 <= a < len(right) for a in ans):
            err(where, f"match answer out of range {ans!r}")
        if len(q.get("feedback") or []) != len(left):
            err(where, "match needs one feedback per left row")
    else:
        err(where, f"unknown kind {kind!r}")
    if not (isinstance(q.get("prompt"), str) and q.get("prompt")):
        err(where, "missing prompt")


def load(path):
    try:
        return yaml.safe_load(path.read_text())
    except Exception as e:
        err(path.name, f"YAML error: {e}")
        return None


def media_paths(video):
    """Local file paths named by a lesson's `video` (a string, or a {src, poster, captions, chapters} block)."""
    if isinstance(video, str):
        video = {"src": video}
    paths = []
    for key in ("src", "poster", "captions"):
        v = (video or {}).get(key) if isinstance(video, dict) else None
        for item in v if isinstance(v, list) else [v]:
            if isinstance(item, dict):
                item = item.get("src")
            if isinstance(item, str) and not item.startswith(("http://", "https://")):
                paths.append(item)
    return paths


def check_media_refs(where, text):
    """Every assets/media/<file> named in lesson or exercise text must exist in ui/assets/media."""
    for name in dict.fromkeys(MEDIA_REF.findall(text or "")):
        if not (MEDIA_DIR / name).is_file():
            err(where, f"media file missing: assets/media/{name}")


def check_help(where, links):
    """`help:` is an optional list of {title, url}; url starts with HELP_URL_PREFIX (see SCHEMA.md)."""
    if links is None:
        return
    if not isinstance(links, list):
        err(where, "help must be a list of {title, url}"); return
    for i, h in enumerate(links):
        if not (isinstance(h, dict) and isinstance(h.get("title"), str) and h["title"].strip()):
            err(where, f"help {i}: needs a title")
        url = h.get("url") if isinstance(h, dict) else None
        if not help_url_ok(url):
            err(where, f"help {i}: {url!r} is not an https:// link under {HELP_URL_PREFIX!r} (HELP_URL_PREFIX in shared/src/brand.rs)")


def check_lesson(lid, sid, d, media_roots):
    check_shape(lid, d, LESSON_REQ, OPTIONAL_STRS)
    if d.get("id") != lid or d.get("session") != sid:
        err(lid, "id/session mismatch")
    words = len("\n".join(l for l in (d.get("body") or "").splitlines() if not l.lstrip().startswith("![")).split())
    lo, hi = BODY_WORDS
    if not lo <= words <= hi:
        err(lid, f"body {words} words, want {lo}-{hi}")
    if not KEY_POINTS[0] <= len(d.get("key_points") or []) <= KEY_POINTS[1]:
        err(lid, f"key_points must be {KEY_POINTS[0]}-{KEY_POINTS[1]}")
    qc = d.get("quick_check") or []
    if len(qc) != QUICK_CHECK_QUESTIONS:
        err(lid, f"quick_check has {len(qc)}, want {QUICK_CHECK_QUESTIONS}")
    for i, q in enumerate(qc):
        check_question(f"{lid} q{i}", q)
    if not d.get("sources"):
        err(lid, "no sources")
    check_media_refs(lid, d.get("body"))
    check_help(lid, d.get("help"))
    for path in media_paths(d.get("video")):
        if not any((m / path).is_file() for m in media_roots):
            err(lid, f"video file missing: {path}")


def find(kind, name, dirs):
    for d in dirs:
        if (d / kind / f"{name}.yaml").exists():
            return d / kind / f"{name}.yaml"
    return None


def check_unique_ids(root, dirs):
    """Within one course (its own dirs plus shared ones) a lesson, check or exercise id is used once and matches its file name."""
    seen = {}
    for d in dirs:
        for kind in ("lessons", "checks", "exercises"):
            for f in sorted((d / kind).glob("*.yaml")):
                data = load(f)
                fid = data.get("id") if isinstance(data, dict) else None
                if fid is None:
                    continue
                if fid != f.stem:
                    err(f"{kind}/{f.name}", f"id {fid!r} does not match the file name")
                key = (kind, fid)
                if key in seen:
                    err(fid, f"duplicate {kind[:-1]} id in {seen[key]} and {f.relative_to(root.parent)}")
                seen[key] = f.relative_to(root.parent)


def validate_course(root, drafting):
    courses_dir = root.parent if not drafting else root.parent.parent / "courses"
    course = load(root / "course.yaml")
    if not isinstance(course, dict):
        err(root.name, "course.yaml missing or empty")
        return
    check_shape(root.name, course, COURSE_REQ)
    if course.get("id") != root.name:
        err(root.name, f"course id {course.get('id')!r} does not match its folder")
    sessions = [s for s in course.get("sessions") or [] if isinstance(s, dict)]
    for s in course.get("sessions") or []:
        check_shape(f"{root.name} session {s.get('id') if isinstance(s, dict) else s}", s, SESSION_REQ, {"phase": STR})
    sids = [s.get("id") for s in sessions]
    for sid in sorted({x for x in sids if sids.count(x) > 1}, key=str):
        err(root.name, f"duplicate session id {sid}")
    cid = course.get("id")
    media_roots = [root.parent] + ([courses_dir] if drafting else [])
    shared = []
    for name in course.get("shared") or []:
        for base in dict.fromkeys([root.parent / "_shared", courses_dir / "_shared"]):
            if (base / name).is_dir():
                shared.append(base / name)
                break
        else:
            err(cid, f"shared dir missing: {name}")
    dirs = [root] + shared
    for kind in ("lessons", "checks", "exercises"):
        if not (root / kind).is_dir():
            err(root.name, f"folder {kind}/ is missing (the loader reads it even when empty)")
    check_unique_ids(root, dirs)
    listed = {lid for s in sessions for lid in s.get("lessons") or []}
    for d in shared:
        shared_listed.setdefault(d, set()).update(listed)
    # the app loads every file in lessons/, but only shows the ones a session lists: the rest are never seen or checked
    for f in sorted((root / "lessons").glob("*.yaml")):
        lid = (load(f) or {}).get("id") or f.stem
        if lid not in listed and f.stem not in listed:
            err(lid, f"lesson file {f.name} is in no session of {root.name}/course.yaml: it would never be shown")
    for s in sessions:
        sid = s.get("id")
        for lid in s.get("lessons") or []:
            p = find("lessons", lid, dirs)
            if not p:
                err(lid, "lesson file missing"); continue
            check_lesson(lid, sid, load(p) or {}, media_roots)
        for kind, key in (("checks", "check"), ("exercises", "exercise")):
            fid = s.get(key)
            if not fid:
                err(sid, f"no {key} listed"); continue
            p = find(kind, fid, dirs)
            if not p:
                err(fid, f"{kind} file missing"); continue
            d = load(p) or {}
            if kind == "checks":
                check_shape(fid, d, CHECK_REQ)
                qs = d.get("questions") or []
                if len(qs) != CHECK_QUESTIONS:
                    err(fid, f"{len(qs)} questions, want {CHECK_QUESTIONS}")
                for i, q in enumerate(qs):
                    check_question(f"{fid} q{i}", q)
                check_pass_mark(fid, d)
            else:
                check_shape(fid, d, EXERCISE_REQ, OPTIONAL_STRS)
                check_media_refs(fid, yaml.safe_dump(d))
                if not (d.get("acceptance_criteria") and d.get("reference_walkthrough")):
                    err(fid, "exercise missing acceptance_criteria/reference_walkthrough")

    name = course.get("assessment") or "final"
    p = root / "assessment" / f"{name}.yaml"
    if not p.exists():
        if drafting:
            notes.append(f"note: {cid}: final assessment not written yet")
        else:
            err(f"{cid} final", f"assessment missing: assessment/{name}.yaml")
    else:
        d = load(p) or {}
        check_shape(f"{cid} final", d, FINAL_REQ)
        tasks = [t for t in d.get("tasks") or [] if isinstance(t, dict)]
        for t in d.get("tasks") or []:
            check_shape(f"final/{t.get('id') if isinstance(t, dict) else t}", t, TASK_REQ)
        if not d.get("blurb"):
            err("final", "missing blurb")
        check_pass_mark("final", d)
        if len(tasks) < MIN_FINAL_TASKS:
            err("final", f"need at least {MIN_FINAL_TASKS} tasks, found {len(tasks)}")
        tids = [t.get("id") for t in tasks]
        for tid in sorted({x for x in tids if tids.count(x) > 1}, key=str):
            err("final", f"duplicate task id {tid}")
        for t in tasks:
            if not t.get("questions"):
                err(f"final/{t.get('id')}", "no questions")
            for i, q in enumerate(t.get("questions") or []):
                check_question(f"final/{t.get('id')} q{i}", q)


def default_roots():
    return [c.parent for kind in ("courses", "drafts") for c in sorted((CONTENT / kind).glob("*/course.yaml"))
            if not c.parent.name.startswith("_")]


def validate(roots):
    """Validate each course root; returns (errors, notes)."""
    errors.clear(); notes.clear(); shared_listed.clear()
    roots = list(roots)
    if not roots:
        err("content/courses", "no courses found (a course is a folder with a course.yaml)")
    for root in map(pathlib.Path, roots):
        validate_course(root.resolve(), root.resolve().parent.name == "drafts")
    for d, listed in shared_listed.items():
        for f in sorted((d / "lessons").glob("*.yaml")):
            lid = (load(f) or {}).get("id") or f.stem
            if lid not in listed and f.stem not in listed:
                err(lid, f"shared lesson file {d.name}/lessons/{f.name} is in no session of any course that shares {d.name}")
    return list(errors), list(notes)


if __name__ == "__main__":
    errs, nts = validate(sys.argv[1:] or default_roots())
    print("\n".join(nts + errs) or "OK")
    sys.exit(1 if errs else 0)
