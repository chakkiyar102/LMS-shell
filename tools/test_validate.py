#!/usr/bin/env python3
"""Tests for tools/validate.py. Usage: python3 tools/test_validate.py"""
import pathlib, sys, tempfile
import yaml

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import validate


def q(i=0):
    return {"kind": "single", "prompt": f"p{i}", "answer": 0,
            "options": [{"text": "a", "feedback": "f"}, {"text": "b", "feedback": "f"}]}


def put(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(yaml.safe_dump(data) if not isinstance(data, str) else data)


def lesson(lid, sid, words=200, **extra):
    d = {"id": lid, "session": sid, "number": "1.1", "title": "T", "objective": "O", "duration_min": 4,
         "body": " ".join(["word"] * words), "key_points": ["a", "b", "c"],
         "quick_check": [q(0), q(1), q(2)], "sources": [{"kind": "internal", "ref": "x"}]}
    d.update(extra)
    return d


def build(base, cid="sample-course", shared=None, **kw):
    """One-session course under base/<cid>; kw tweaks: lesson=, exercise=, final=, check=."""
    r = base / cid
    course = {"id": cid, "title": "C", "summary": "S", "assessment": "final",
              "sessions": [{"id": "s1", "title": "S1", "outcome": "O", "lessons": ["l1"], "check": "s1-check", "exercise": "s1-ex"}]}
    if shared:
        course["shared"] = shared
    put(r / "course.yaml", course)
    put(r / "lessons" / "l1.yaml", kw.get("lesson") or lesson("l1", "s1"))
    put(r / "checks" / "s1-check.yaml", kw.get("check") or {"id": "s1-check", "title": "K", "session": "s1", "pass_mark": 0.75, "questions": [q(i) for i in range(10)]})
    put(r / "exercises" / "s1-ex.yaml", kw.get("exercise") or {
        "id": "s1-ex", "session": "s1", "title": "E", "minutes": 20, "brief": "Do it.",
        "acceptance_criteria": ["x"], "reference_walkthrough": "y"})
    final = kw.get("final", {"id": "final", "title": "F", "minutes": 30, "fixture": "fx", "blurb": "b", "pass_mark": 0.75,
                             "tasks": [{"id": f"t{i}", "minutes": 5, "questions": [q()]} for i in range(3)]})
    if final is not None:
        put(r / "assessment" / "final.yaml", final)
    return r


def run(root):
    return validate.validate([root])


def tmp():
    return pathlib.Path(tempfile.mkdtemp())


def has(errors, text):
    return any(text in e for e in errors)


# clean course passes
errors, _ = run(build(tmp() / "courses"))
assert errors == [], errors

# no courses at all is an error, not a silent OK
errors, _ = validate.validate([])
assert has(errors, "no courses found"), errors

# missing video file names the file; present file passes; https is not checked
b = tmp()
errors, _ = run(build(b / "courses", lesson=lesson("l1", "s1", video="sample-course/videos/a-1.mp4")))
assert has(errors, "sample-course/videos/a-1.mp4"), errors
put(b / "courses" / "sample-course" / "videos" / "a-1.mp4", "x")
errors, _ = run(b / "courses" / "sample-course")
assert errors == [], errors
blk = {"src": "https://x.test/v.mp4", "poster": "sample-course/videos/p.webp", "captions": "sample-course/videos/c.vtt"}
put(b / "courses" / "sample-course" / "lessons" / "l1.yaml", lesson("l1", "s1", video=blk))
errors, _ = run(b / "courses" / "sample-course")
assert has(errors, "p.webp") and has(errors, "c.vtt") and not has(errors, "v.mp4"), errors

# figure references in lesson body and exercise text must exist in ui/assets/media
b = tmp()
saved_media = validate.MEDIA_DIR
validate.MEDIA_DIR = b / "media"
put(b / "media" / "ok.webp", "x")
body = " ".join(["word"] * 200) + "\n![alt](assets/media/ok.webp)\n![alt](assets/media/gone.webp)"
errors, _ = run(build(b / "courses", lesson=lesson("l1", "s1", body=body)))
assert has(errors, "assets/media/gone.webp") and not has(errors, "ok.webp"), errors
ex = {"id": "s1-ex", "title": "E", "minutes": 20, "session": "s1", "brief": "Do it. ![x](assets/media/nope.webp)",
      "acceptance_criteria": ["x"], "reference_walkthrough": "y"}
errors, _ = run(build(tmp() / "courses", exercise=ex))
assert has(errors, "assets/media/nope.webp"), errors
validate.MEDIA_DIR = saved_media

# lesson length, key points, quick check count, sources
errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s1", words=1200)))
assert has(errors, "1200 words"), errors
errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s1", words=100)))
assert has(errors, "100 words"), errors
errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s1", key_points=["a", "b"])))
assert has(errors, "key_points must be 3-6"), errors
errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s1", quick_check=[q(0), q(1)])))
assert has(errors, "quick_check has 2"), errors
errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s1", sources=[])))
assert has(errors, "no sources"), errors

# ids: lesson id/session must match the spine, file names match ids, course id matches its folder
errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s9")))
assert has(errors, "id/session mismatch"), errors
r = build(tmp() / "courses")
put(r / "checks" / "other.yaml", {"id": "s1-check", "title": "K", "pass_mark": 0.75, "questions": []})
errors, _ = run(r)
assert has(errors, "does not match the file name") and has(errors, "duplicate check id"), errors
r = build(tmp() / "courses")
c = yaml.safe_load((r / "course.yaml").read_text()); c["id"] = "something-else"
put(r / "course.yaml", c)
errors, _ = run(r)
assert has(errors, "does not match its folder"), errors
r = build(tmp() / "courses")
c = yaml.safe_load((r / "course.yaml").read_text()); c["sessions"].append(dict(c["sessions"][0], lessons=[]))
put(r / "course.yaml", c)
errors, _ = run(r)
assert has(errors, "duplicate session id s1"), errors

# files named in course.yaml must exist
r = build(tmp() / "courses")
(r / "exercises" / "s1-ex.yaml").unlink()
errors, _ = run(r)
assert has(errors, "exercises file missing"), errors

# pass_mark: a fraction in (0, 1]; check size; final shape
for bad_pm in (0, 1.5, -0.2):
    bad = {"id": "s1-check", "title": "K", "session": "s1", "pass_mark": bad_pm, "questions": [q(i) for i in range(10)]}
    errors, _ = run(build(tmp() / "courses", check=bad))
    assert has(errors, "pass_mark"), (bad_pm, errors)
ok = {"id": "s1-check", "title": "K", "session": "s1", "pass_mark": 0.8, "questions": [q(i) for i in range(10)]}
errors, _ = run(build(tmp() / "courses", check=ok))
assert errors == [], errors
short = {"id": "s1-check", "title": "K", "session": "s1", "pass_mark": 0.75, "questions": [q(i) for i in range(9)]}
errors, _ = run(build(tmp() / "courses", check=short))
assert has(errors, "9 questions, want 10"), errors
final = {"id": "final", "title": "F", "minutes": 30, "fixture": "fx", "pass_mark": 0.75, "tasks": [{"id": "t", "minutes": 5, "questions": [q()]}, {"id": "t", "minutes": 5, "questions": []}]}
errors, _ = run(build(tmp() / "courses", final=final))
assert has(errors, "blurb") and has(errors, "at least 3 tasks") and has(errors, "no questions") and has(errors, "duplicate task id"), errors

# missing final: error under courses, note under drafts
errors, _ = run(build(tmp() / "courses", final=None))
assert has(errors, "assessment"), errors
errors, notes = run(build(tmp() / "drafts", final=None))
assert errors == [] and any("final assessment not written yet" in n for n in notes), (errors, notes)

# shared session: lesson lives in _shared and is validated like any other
b = tmp()
r = build(b / "courses", shared=["basics"])
(r / "lessons" / "l1.yaml").unlink()
put(b / "courses" / "_shared" / "basics" / "lessons" / "l1.yaml", lesson("l1", "s1", words=1200))
errors, _ = run(r)
assert has(errors, "1200 words"), errors
put(b / "courses" / "_shared" / "basics" / "lessons" / "l1.yaml", lesson("l1", "s1"))
errors, _ = run(r)
assert errors == [], errors
errors, _ = run(build(tmp() / "courses", shared=["missing"]))
assert has(errors, "shared dir missing: missing"), errors

# answer ranges and the "Correct" feedback convention
bad = q(0); bad["answer"] = 2
errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s1", quick_check=[bad, q(1), q(2)])))
assert has(errors, "bad answer 2"), errors
bad = q(0); bad["kind"] = "multi"; bad["answer"] = [0, 0]
errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s1", quick_check=[bad, q(1), q(2)])))
assert has(errors, "repeats an index"), errors
bad = q(0)
bad["options"] = [{"text": "a", "feedback": "Correct. yes"}, {"text": "b", "feedback": "no"}]
bad["answer"] = 1
errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s1", quick_check=[bad, q(1), q(2)])))
assert has(errors, "disagrees") and has(errors, "p0"), errors
bad["answer"] = 0
errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s1", quick_check=[bad, q(1), q(2)])))
assert errors == [], errors
m = {"kind": "match", "prompt": "m", "left": ["a", "b"], "right": ["x"], "answer": [0, 1], "feedback": ["f"]}
errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s1", quick_check=[m, q(1), q(2)])))
assert has(errors, "out of range") and has(errors, "one feedback per left row"), errors
errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s1", quick_check=[{"kind": "essay", "prompt": "x"}, q(1), q(2)])))
assert has(errors, "unknown kind 'essay'"), errors

# fields the Rust loader requires: each omission is a validate error
for field in ("number", "title", "objective"):
    l = lesson("l1", "s1"); del l[field]
    errors, _ = run(build(tmp() / "courses", lesson=l))
    assert has(errors, f"`{field}`") and has(errors, "loader"), (field, errors)
l = lesson("l1", "s1"); del l["body"]
errors, _ = run(build(tmp() / "courses", lesson=l))
assert has(errors, "`body`"), errors
# an unquoted ': ' list item parses as a map; the loader wants strings
errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s1", key_points=["a", {"Tip": "a map"}, "c"])))
assert has(errors, "key_points") and has(errors, "must be strs"), errors
ex = {"id": "s1-ex", "title": "E", "minutes": 20, "brief": "b", "acceptance_criteria": ["x"],
      "reference_walkthrough": "y", "setup": ["ok", {"A source for Part B": "any list"}]}
errors, _ = run(build(tmp() / "courses", exercise=ex))
assert has(errors, "`setup`") and has(errors, "map"), errors
del ex["setup"]; del ex["title"]
errors, _ = run(build(tmp() / "courses", exercise=ex))
assert has(errors, "`title`"), errors
final = {"id": "final", "title": "F", "minutes": 30, "blurb": "b", "pass_mark": 0.75, "tasks": [{"id": f"t{i}", "minutes": 5, "questions": [q()]} for i in range(3)]}
errors, _ = run(build(tmp() / "courses", final=final))
assert has(errors, "`fixture`"), errors
bad = q(0); bad["options"][0]["feedback"] = {"Correct": "x"}
errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s1", quick_check=[bad, q(1), q(2)])))
assert has(errors, "text/feedback"), errors

# help links: optional, an https:// URL under HELP_URL_PREFIX, with a title
saved_prefix = validate.HELP_URL_PREFIX
validate.HELP_URL_PREFIX = "https://"
good = {"title": "Getting started", "url": "https://docs.example.com/getting-started"}
errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s1", help=[good])))
assert errors == [], errors
for bad_url in ("http://docs.example.com/x", "docs.example.com/x", "https://", "https://a b.com/x", 'https://x.com/"q"', "ftp://example.com/x"):
    errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s1", help=[{"title": "x", "url": bad_url}])))
    assert has(errors, "help 0") and has(errors, "https:// link"), (bad_url, errors)
errors, _ = run(build(tmp() / "courses", lesson=lesson("l1", "s1", help=[{"url": good["url"]}])))
assert has(errors, "needs a title"), errors
# a fork that sets a docs-site prefix only accepts links under it
validate.HELP_URL_PREFIX = "https://docs.example.com/"
assert validate.help_url_ok("https://docs.example.com/notes/search")
assert not validate.help_url_ok("https://docs.example.com/") and not validate.help_url_ok("https://other.example.com/x")
validate.HELP_URL_PREFIX = saved_prefix
# the prefix is read from shared/src/brand.rs, falling back to https:// when the file or const is absent
brand = tmp() / "brand.rs"
brand.write_text('pub const HELP_URL_PREFIX: &str = "https://docs.example.org/";\n')
assert validate.help_prefix(brand) == "https://docs.example.org/"
assert validate.help_prefix(tmp() / "missing.rs") == "https://"

# a lesson file no session lists loads in the app but is never shown: an error, own or shared
r = build(tmp() / "courses")
put(r / "lessons" / "l9.yaml", lesson("l9", "s1"))
errors, _ = run(r)
assert has(errors, "l9: lesson file l9.yaml is in no session of sample-course/course.yaml"), errors
base = tmp() / "courses"
put(base / "_shared" / "f" / "lessons" / "f9.yaml", lesson("f9", "s1"))
r = build(base, shared=["f"])
errors, _ = run(r)
assert has(errors, "f9: shared lesson file f/lessons/f9.yaml is in no session"), errors

# the real corpus is clean; absent content/drafts is fine
roots = validate.default_roots()
assert roots, "no courses under content/courses"
errors, _ = validate.validate(roots)
assert errors == [], errors
print("ok")
