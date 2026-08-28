#!/usr/bin/env python3
"""OCR the AIS textbook and build a resumable case-to-page index.

The source PDF is intentionally not redistributed. OCR text is stored under
tmp/ by default and is evidence for candidate extraction, never verified gold.
"""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_IMAGES = REPO_ROOT / "tmp" / "pdfs" / "ais100-pages"
DEFAULT_CACHE = REPO_ROOT / "tmp" / "pdfs" / "ais100-ocr-cache.json"
DEFAULT_INDEX = REPO_ROOT / "tmp" / "pdfs" / "ais100-page-case-index.json"
CASE_LINE = re.compile(
    r"^案例\s*[:：]?\s*([1-9]\d{0,2})(?:\s*[-—–至]\s*([1-9]\d{0,2}))?"
)
# OCR dropped one digit on this visually reviewable heading: "案例44" -> "案例4".
OCR_CORRECTIONS = {(109, 4): 44}


def read_cache(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"schema_version": 1, "engine": "rapidocr_onnxruntime", "pages": {}}
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )


def case_numbers(lines: list[dict[str, Any]], pdf_page: int) -> list[int]:
    found: list[int] = []
    for line in lines:
        normalized = re.sub(r"\s+", "", line["text"])
        match = CASE_LINE.match(normalized)
        if match:
            first = OCR_CORRECTIONS.get((pdf_page, int(match.group(1))), int(match.group(1)))
            last = int(match.group(2)) if match.group(2) else first
            for number in range(first, last + 1):
                if 1 <= number <= 100 and number not in found:
                    found.append(number)
    return found


def scan(start: int, end: int, images: Path, cache_path: Path) -> dict[str, Any]:
    try:
        from rapidocr_onnxruntime import RapidOCR
    except ImportError as exc:
        raise SystemExit(
            "rapidocr_onnxruntime is required: python -m pip install rapidocr_onnxruntime"
        ) from exc

    cache = read_cache(cache_path)
    pages = cache["pages"]
    engine = RapidOCR()
    for pdf_page in range(start, end + 1):
        key = str(pdf_page)
        if key in pages:
            continue
        image_path = images / f"page-{pdf_page:03d}.jpg"
        if not image_path.exists():
            raise SystemExit(f"missing rendered page: {image_path}")
        result, _ = engine(str(image_path))
        lines = []
        for box, text, confidence in result or []:
            lines.append(
                {
                    "text": text,
                    "confidence": round(float(confidence), 4),
                    "box": [[round(float(x), 1), round(float(y), 1)] for x, y in box],
                }
            )
        pages[key] = {
            "pdf_page": pdf_page,
            "book_page": pdf_page - 13,
            "case_starts": case_numbers(lines, pdf_page),
            "lines": lines,
        }
        # A page-level checkpoint makes a cancelled run safely resumable.
        write_json(cache_path, cache)
        print(f"page={pdf_page} cases={pages[key]['case_starts']}", flush=True)
    return cache


def build_index(cache: dict[str, Any]) -> dict[str, Any]:
    starts: dict[int, int] = {}
    duplicate_starts: dict[int, list[int]] = {}
    for page in sorted(cache["pages"].values(), key=lambda value: value["pdf_page"]):
        # Re-parse cached OCR so parser improvements never require a rescan.
        page["case_starts"] = case_numbers(page["lines"], page["pdf_page"])
        for number in page["case_starts"]:
            if number in starts:
                duplicate_starts.setdefault(number, [starts[number]]).append(page["pdf_page"])
            else:
                starts[number] = page["pdf_page"]

    cases = []
    for number in range(1, 101):
        start = starts.get(number)
        next_start = next(
            (starts[n] for n in range(number + 1, 101) if n in starts), None
        )
        end = next_start if start is not None and next_start is not None else start
        cases.append(
            {
                "source_case_number": number,
                "case_id": f"AIS-C{number:03d}",
                "start_pdf_page": start,
                "end_pdf_page_inclusive": end,
                "boundary_policy": "include_next_start_page_for_split-page_cases",
                "index_status": "ocr_candidate" if start is not None else "missing",
            }
        )
    return {
        "schema_version": 1,
        "source": "docs/脊柱侧弯保守治疗100例_14996973.pdf",
        "page_offset": 13,
        "case_count_expected": 100,
        "case_starts_found": len(starts),
        "missing_case_numbers": [number for number in range(1, 101) if number not in starts],
        "duplicate_starts": duplicate_starts,
        "ocr_corrections": [
            {"pdf_page": page, "ocr_value": old, "corrected_value": new}
            for (page, old), new in OCR_CORRECTIONS.items()
        ],
        "cases": cases,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--start", type=int, default=41)
    parser.add_argument("--end", type=int, default=188)
    parser.add_argument("--images", type=Path, default=DEFAULT_IMAGES)
    parser.add_argument("--cache", type=Path, default=DEFAULT_CACHE)
    parser.add_argument("--index", type=Path, default=DEFAULT_INDEX)
    args = parser.parse_args()
    cache = scan(args.start, args.end, args.images, args.cache)
    index = build_index(cache)
    write_json(args.index, index)
    print(
        f"found={index['case_starts_found']} missing={index['missing_case_numbers']} "
        f"index={args.index}"
    )
    return 0 if index["case_starts_found"] == 100 else 2


if __name__ == "__main__":
    raise SystemExit(main())
