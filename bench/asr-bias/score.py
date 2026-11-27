#!/usr/bin/env python3
"""Score `asr_bias_bench` results against the FLEURS references.

    python3 score.py <work-dir>/<lang> [more dirs…] [--results NAME] [--json out.json]

Reads `manifest.json` and `<NAME>` (default `results.jsonl`) from each
directory and prints a Markdown table per language:

    error    character error rate (ja) / word error rate (en) of the whole text
    terms    share of the annotated names and terms that came out as written
    intruded share of sentences where an unrelated prompt term appeared in the
             text although nobody said it
    foreign  share of sentences recognized in the wrong script (ja: no kana or
             kanji left; en: kana or kanji present)

Each prompt condition is compared with `none` on the same sentences; the
interval is a paired bootstrap (95 %, 10 000 resamples of sentences).
"""

import argparse
import json
import random
import re
import unicodedata
from pathlib import Path

JA_SCRIPT = re.compile(r"[぀-ヿ一-鿿]")


def normalize(lang: str, text: str) -> list[str]:
    """Comparable units: characters (ja) or words (en), punctuation-free."""
    t = unicodedata.normalize("NFKC", text).lower()
    if lang == "ja":
        return [c for c in t if unicodedata.category(c)[0] in "LN"]
    t = "".join(c for c in unicodedata.normalize("NFD", t) if unicodedata.category(c) != "Mn")
    t = re.sub(r"(?<=\d),(?=\d)", "", t)
    t = re.sub(r"[.'’]", "", t)
    return re.sub(r"[^a-z0-9]+", " ", t).split()


def edits(ref: list[str], hyp: list[str]) -> int:
    prev = list(range(len(hyp) + 1))
    for i, r in enumerate(ref, 1):
        cur = [i]
        for j, h in enumerate(hyp, 1):
            cur.append(min(prev[j] + 1, cur[-1] + 1, prev[j - 1] + (r != h)))
        prev = cur
    return prev[-1]


def contains(units: list[str], term: list[str]) -> bool:
    n = len(term)
    return n > 0 and any(units[i : i + n] == term for i in range(len(units) - n + 1))


def score(lang: str, item: dict, text: str) -> dict:
    ref, hyp = normalize(lang, item["reference"]), normalize(lang, text)
    terms = [normalize(lang, t) for t in item["terms"]]
    stray = [normalize(lang, t) for t in item["distractors"]]
    has_ja = bool(JA_SCRIPT.search(text))
    return {
        "edits": edits(ref, hyp),
        "ref": len(ref),
        "hits": sum(contains(hyp, t) for t in terms),
        "terms": len(terms),
        "intruded": any(contains(hyp, t) and not contains(ref, t) for t in stray),
        "foreign": bool(text.strip()) and (has_ja != (lang == "ja")),
    }


def rate(rows: list[dict], num: str, den: str) -> float:
    d = sum(r[den] for r in rows)
    return sum(r[num] for r in rows) / d if d else 0.0


def bootstrap(base: list[dict], cond: list[dict], num: str, den: str) -> tuple[float, float]:
    rng = random.Random(0)
    n, deltas = len(base), []
    for _ in range(10_000):
        pick = [rng.randrange(n) for _ in range(n)]
        deltas.append(
            rate([cond[i] for i in pick], num, den) - rate([base[i] for i in pick], num, den)
        )
    deltas.sort()
    return deltas[250], deltas[9749]


def pct(v: float) -> str:
    return f"{v * 100:.1f}"


def report_nonspeech(directory: Path, results_name: str, manifest: dict) -> dict:
    """Sounds without speech: any text is wrong, prompt terms in it doubly so."""
    by_cond: dict[str, list[dict]] = {}
    for line in (directory / results_name).read_text("utf-8").splitlines():
        try:
            r = json.loads(line)
        except json.JSONDecodeError:
            continue  # half a line left by a run that was stopped
        item = manifest[r["key"].split("-", 1)[1]]
        units = normalize("ja", r["text"])
        echoed = any(
            contains(units, normalize("ja", t)) for t in item["distractors"]
        )
        by_cond.setdefault(r["condition"], []).append(
            {"id": item["id"], "text": r["text"], "spoke": len(units) >= 2, "echoed": echoed}
        )
    print(f"\n### 非音声（{results_name}）\n")
    print("| 条件 | 音数 | 文字出力 % | プロンプト用語の出力 % |")
    print("|---|---:|---:|---:|")
    for cond, rows in by_cond.items():
        print(
            f"| {cond} | {len(rows)} | {pct(sum(r['spoke'] for r in rows) / len(rows))} "
            f"| {pct(sum(r['echoed'] for r in rows) / len(rows))} |"
        )
    print()
    for cond, rows in by_cond.items():
        for r in rows:
            if r["spoke"]:
                print(f"- `{cond}` {r['id']}：{r['text']}")
    return {"lang": "nonspeech", "results": results_name, "conditions": by_cond}


def report(directory: Path, results_name: str) -> dict:
    manifest = {m["id"]: m for m in json.loads((directory / "manifest.json").read_text("utf-8"))}
    lang = next(iter(manifest.values()))["lang"]
    if lang == "nonspeech":
        return report_nonspeech(directory, results_name, manifest)
    by_cond: dict[str, dict[str, dict]] = {}
    for line in (directory / results_name).read_text("utf-8").splitlines():
        try:
            r = json.loads(line)
        except json.JSONDecodeError:
            continue  # half a line left by a run that was stopped
        sid = r["key"].split("-", 1)[1]
        s = score(lang, manifest[sid], r["text"])
        s.update(text=r["text"], spent=r["spent_s"])
        by_cond.setdefault(r["condition"], {})[sid] = s

    unit = "CER" if lang == "ja" else "WER"
    print(f"\n### {lang}（{results_name}、{len(by_cond.get('none', {}))} 文）\n")
    print(f"| 条件 | 文数 | {unit} % | 差 (95% 区間) | 用語再現 % | 差 (95% 区間) | 混入 % | 別言語 % | 秒/文 |")
    print("|---|---:|---:|---|---:|---|---:|---:|---:|")
    summary = {}
    for cond, rows in by_cond.items():
        ids = [i for i in rows if i in by_cond["none"]]
        cur, base = [rows[i] for i in ids], [by_cond["none"][i] for i in ids]
        err, rec = rate(cur, "edits", "ref"), rate(cur, "hits", "terms")
        entry = {
            "sentences": len(cur),
            "error": err,
            "terms": rec,
            "intruded": sum(r["intruded"] for r in cur) / len(cur),
            "foreign": sum(r["foreign"] for r in cur) / len(cur),
            "seconds": sum(r["spent"] for r in cur) / len(cur),
        }
        d_err = d_rec = ""
        if cond != "none":
            lo, hi = bootstrap(base, cur, "edits", "ref")
            entry["error_delta"] = [err - rate(base, "edits", "ref"), lo, hi]
            d_err = f"{pct(entry['error_delta'][0])} ({pct(lo)}〜{pct(hi)})"
            lo, hi = bootstrap(base, cur, "hits", "terms")
            entry["terms_delta"] = [rec - rate(base, "hits", "terms"), lo, hi]
            d_rec = f"{pct(entry['terms_delta'][0])} ({pct(lo)}〜{pct(hi)})"
        print(
            f"| {cond} | {len(cur)} | {pct(err)} | {d_err} | {pct(rec)} | {d_rec} "
            f"| {pct(entry['intruded'])} | {pct(entry['foreign'])} | {entry['seconds']:.1f} |"
        )
        summary[cond] = entry
    return {"lang": lang, "results": results_name, "conditions": summary, "sentences": by_cond}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("dirs", nargs="+", type=Path)
    ap.add_argument("--results", default="results.jsonl")
    ap.add_argument("--json", type=Path)
    a = ap.parse_args()
    out = [report(d, a.results) for d in a.dirs]
    if a.json:
        a.json.write_text(json.dumps(out, ensure_ascii=False, indent=1), encoding="utf-8")


if __name__ == "__main__":
    main()
