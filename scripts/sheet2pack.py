#!/usr/bin/env python3
"""sheet2pack — convert a SODA course-tracker spreadsheet into a LearnForge
importable course pack (exported-course JSON, consumed by Settings → Import).

Input convention (per-course xlsx, e.g. "SFD402 - MLOps Bootcamp.xlsx"):
  - "Tracker" sheet:
      module rows : col A = module number (1.0), col C = module name
      lesson rows : col B = lesson number (101), col C = title,
                    col D = filename, any cell containing a YouTube URL
      resource rows: no lesson number; col C = label, col D = text/URL —
                    appended to the previous lesson's markdown
  - "Quizzes" sheet (optional):
      chapter header rows: col A = "Chapter N - Title"
      question rows: B=S.No, C=Type, D=Question, E-H=Options A-D, I=Answer,
                     J=Explanation
      Questions attach as a module quiz block ONLY when "Chapter N" matches a
      module number from Tracker; unmatched chapters are reported and skipped
      (guards against stale template sheets reused across course workbooks).

Output: exported-course JSON (exportVersion present → relaxed schema branch),
videos keyed by SECTION id (= block id) so per-lesson hero videos display
after import (requires the section-keyed video fix, commit 98c3fcf).

Usage:
  python3 scripts/sheet2pack.py "<course>.xlsx" -o out.json \
      [--id sfd402-mlops] [--title "MLOps Bootcamp"] [--domain devops] \
      [--channel "School of Devops"]

Validation: if `jsonschema` is installed, the output is validated against
learnforge-core/topic-packs/pack-schema.json before writing.
"""

import argparse
import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

YT_RE = re.compile(r"(?:youtu\.be/|youtube\.com/watch\?v=)([A-Za-z0-9_-]{6,20})")
NOW = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

# R1 caps from pack-schema.json
MAX_OBJECTIVES = 12
MAX_OBJECTIVE_LEN = 280
MAX_TITLE_LEN = 200
MAX_MODULE_DESC_LEN = 1000


def cell(row, i):
    v = row[i] if i < len(row) else None
    return str(v).strip() if v is not None else ""


def find_yt(row):
    for v in row:
        if v is None:
            continue
        m = YT_RE.search(str(v))
        if m:
            return m.group(1)
    return None


def is_number(s):
    try:
        float(s)
        return True
    except ValueError:
        return False


def parse_tracker(ws):
    """Return [{num, title, lessons: [{num, title, filename, video_id, resources: []}]}]."""
    modules = []
    current = None
    last_lesson = None
    for row in ws.iter_rows(values_only=True):
        if not row or not any(v is not None for v in row):
            continue
        a, b, c, d = cell(row, 0), cell(row, 1), cell(row, 2), cell(row, 3)
        # module row: numeric col A + name in col C
        if a and is_number(a) and c and not b:
            current = {"num": int(float(a)), "title": c, "lessons": []}
            modules.append(current)
            last_lesson = None
            continue
        if current is None:
            continue  # preamble/stats rows before first module
        # lesson row: numeric col B + a title in col C
        if b and is_number(b) and c:
            last_lesson = {
                "num": int(float(b)),
                "title": c,
                "filename": d,
                "video_id": find_yt(row),
                "resources": [],
            }
            current["lessons"].append(last_lesson)
            continue
        # resource row: no lesson number, but has label text — attach to prev lesson
        if not b and c and last_lesson is not None:
            last_lesson["resources"].append({"label": c, "text": d})
    return [m for m in modules if m["lessons"]]


def title_overlap(a, b):
    """Token overlap ratio between two titles (0..1), stopwords ignored."""
    stop = {"the", "a", "an", "and", "or", "to", "of", "with", "for", "in", "on"}
    ta = {w for w in re.findall(r"[a-z]+", a.lower()) if w not in stop}
    tb = {w for w in re.findall(r"[a-z]+", b.lower()) if w not in stop}
    if not ta or not tb:
        return 0.0
    return len(ta & tb) / min(len(ta), len(tb))


def parse_quizzes(ws):
    """Return {chapter_num: {"title": str, "questions": [...]}}."""
    chapters = {}
    current_num = None
    chap_re = re.compile(r"Chapter\s+(\d+)\s*-?\s*(.*)", re.I)
    for row in ws.iter_rows(values_only=True):
        if not row or not any(v is not None for v in row):
            continue
        a = cell(row, 0)
        m = chap_re.search(a) if a else None
        if m:
            current_num = int(m.group(1))
            chapters.setdefault(current_num, {"title": m.group(2).strip(), "questions": []})
            continue
        stem = cell(row, 3)
        if current_num is None or not stem or stem == "Question":
            continue
        options = [cell(row, i) for i in (4, 5, 6, 7)]
        options = [o for o in options if o]
        answer = cell(row, 8)
        if not options or not answer:
            continue
        chapters[current_num]["questions"].append(
            {"stem": stem, "options": options, "answer": answer,
             "explanation": cell(row, 9)}
        )
    return {k: v for k, v in chapters.items() if v["questions"]}


def lesson_markdown(lesson):
    parts = [f"# {lesson['title']}"]
    if lesson["video_id"]:
        parts.append(
            "▶ **Video lesson** — the reference video for this lesson plays in the panel above."
        )
    for r in lesson["resources"]:
        if r["text"]:
            if r["text"].startswith("http"):
                parts.append(f"**{r['label']}**: <{r['text']}>")
            else:
                parts.append(f"**{r['label']}** — {r['text']}")
        else:
            parts.append(f"**{r['label']}**")
    return "\n\n".join(parts)


def quiz_payload(questions, module_slug):
    qs = []
    for qi, q in enumerate(questions, 1):
        opts = [{"id": f"opt-{qi}-{oi}", "text": o} for oi, o in enumerate(q["options"], 1)]
        correct = next(
            (o["id"] for o in opts if o["text"].strip().lower() == q["answer"].strip().lower()),
            None,
        )
        if correct is None:
            continue  # answer text doesn't match any option — skip defensively
        qs.append(
            {
                "id": f"q-{module_slug}-{qi}",
                "stem": q["stem"],
                "options": opts,
                "correctOptionId": correct,
                "explanation": q.get("explanation") or "",
            }
        )
    return {"questions": qs} if qs else None


def block(bid, module_id, ordering, block_type, payload, params=None):
    return {
        "id": bid,
        "moduleId": module_id,
        "ordering": ordering,
        "blockType": block_type,
        "status": "ready",
        "paramsJson": json.dumps(params or {}, ensure_ascii=False),
        "payloadJson": json.dumps(payload, ensure_ascii=False),
        "sourceAnchorsJson": "[]",
        "metadataJson": "{}",
        "retryCount": 0,
        "createdAt": NOW,
        "updatedAt": NOW,
    }


def convert(xlsx_path, pack_id, title, domain, channel):
    import openpyxl

    wb = openpyxl.load_workbook(xlsx_path, read_only=True, data_only=True)
    if "Tracker" not in wb.sheetnames:
        sys.exit("ERROR: no 'Tracker' sheet found")
    modules_raw = parse_tracker(wb["Tracker"])
    if not modules_raw:
        sys.exit("ERROR: no modules with lessons found in Tracker")
    quizzes = parse_quizzes(wb["Quizzes"]) if "Quizzes" in wb.sheetnames else {}

    modules, blocks_map, videos_map = [], {}, {}
    warnings = []
    matched_chapters = set()

    for m in modules_raw:
        slug = f"mod-{m['num']}"
        lesson_titles = [l["title"] for l in m["lessons"]]
        objectives = [t[:MAX_OBJECTIVE_LEN] for t in lesson_titles[:MAX_OBJECTIVES]]
        desc = f"{len(m['lessons'])} video lessons: " + "; ".join(lesson_titles[:4])
        modules.append(
            {
                "id": slug,
                "title": m["title"][:MAX_TITLE_LEN],
                "description": desc[:MAX_MODULE_DESC_LEN],
                "objectives": objectives,
            }
        )

        mblocks = []
        for i, lesson in enumerate(m["lessons"]):
            bid = f"blk-{lesson['num']}"
            mblocks.append(
                block(
                    bid, slug, i, "section",
                    {"markdown": lesson_markdown(lesson)},
                    # LessonNavList reads params.lesson_title for the sidebar label
                    params={"lesson_title": lesson["title"][:MAX_TITLE_LEN]},
                )
            )
            if lesson["video_id"]:
                videos_map[bid] = [
                    {
                        "videoId": lesson["video_id"],
                        "title": lesson["title"][:500],
                        "channelTitle": channel,
                        "relevanceScore": 1.0,
                    }
                ]
            else:
                warnings.append(f"no video: {m['num']}/{lesson['num']} {lesson['title']}")

        # Attach a chapter quiz ONLY when both the number AND the title agree —
        # quiz sheets are often stale template leftovers from other courses.
        if m["num"] in quizzes:
            ch = quizzes[m["num"]]
            if title_overlap(ch["title"], m["title"]) >= 0.5:
                qp = quiz_payload(ch["questions"], slug)
                if qp:
                    mblocks.append(block(f"quiz-{slug}", slug, len(mblocks), "quiz", qp))
                    matched_chapters.add(m["num"])
            else:
                warnings.append(
                    f"quiz chapter {m['num']} \"{ch['title'][:40]}\" != module \"{m['title'][:40]}\" — skipped (stale template sheet?)"
                )
        blocks_map[slug] = mblocks

    for ch_num in sorted(set(quizzes) - matched_chapters):
        if not any(m["num"] == ch_num for m in modules_raw):
            warnings.append(f"quiz chapter {ch_num} matches no module number — skipped")

    edges = [
        {"from": modules[i]["id"], "to": modules[i + 1]["id"]}
        for i in range(len(modules) - 1)
    ]

    payload = {
        "id": pack_id,
        "title": title,
        "description": f"{title} — converted from course tracker ({len(modules)} modules, "
        f"{sum(len(m['lessons']) for m in modules_raw)} lessons).",
        "domain_module": domain,
        "modules": modules,
        "edges": edges,
        "exportVersion": "1.0.0",
        "exportedAt": NOW,
        "exportedFrom": f"imported:{pack_id}",
        "blocks": blocks_map,
        "videos": videos_map,
    }
    return payload, warnings


def validate(payload, schema_path):
    try:
        import jsonschema
    except ImportError:
        return "jsonschema not installed — skipped (pip install jsonschema)"
    schema = json.loads(Path(schema_path).read_text())
    jsonschema.validate(payload, schema)
    return "schema valid (draft 2020-12)"


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("xlsx")
    ap.add_argument("-o", "--out", required=True)
    ap.add_argument("--id", dest="pack_id", default=None)
    ap.add_argument("--title", default=None)
    ap.add_argument("--domain", default="devops")
    ap.add_argument("--channel", default="School of Devops")
    args = ap.parse_args()

    stem = Path(args.xlsx).stem
    title = args.title or re.sub(r"^[A-Z]{2,4}\d+\s*-\s*", "", stem)
    pack_id = args.pack_id or re.sub(r"[^a-z0-9-]+", "-", title.lower()).strip("-")

    payload, warnings = convert(args.xlsx, pack_id, title, args.domain, args.channel)

    schema_path = Path(__file__).resolve().parent.parent / "learnforge-core/topic-packs/pack-schema.json"
    vmsg = validate(payload, schema_path) if schema_path.exists() else "schema file not found — skipped"

    Path(args.out).write_text(json.dumps(payload, ensure_ascii=False, indent=2))

    n_lessons = sum(len(b) for b in payload["blocks"].values())
    n_videos = sum(len(v) for v in payload["videos"].values())
    print(f"pack: {pack_id} — \"{title}\"")
    print(f"modules: {len(payload['modules'])}  blocks: {n_lessons}  videos: {n_videos}")
    print(f"validation: {vmsg}")
    print(f"wrote: {args.out}")
    if warnings:
        print(f"\nwarnings ({len(warnings)}):")
        for w in warnings:
            print(f"  - {w}")


if __name__ == "__main__":
    main()
