#!/usr/bin/env python3
"""Build the jobs for `examples/asr_bias_bench.rs` from FLEURS (dev split).

    python3 prepare.py <work-dir> [--lang ja en] [--limit N] [--conditions mixed distract] [--nonspeech]

Downloads the FLEURS transcripts and recordings (Google, CC BY 4.0) for the
sentences listed in `terms.<lang>.tsv`, converts the recordings to 16 kHz
mono PCM16 and writes, per language:

    <work-dir>/<lang>/manifest.json   sentences: reference text, terms, prompts
    <work-dir>/<lang>/jobs.jsonl      one recognition job per sentence and condition
    <work-dir>/<lang>/wav/<id>.wav

Conditions (what the recognizer gets as its prompt, i.e. what a translation
window would have read off the screen):

    none      nothing (today's behaviour)
    oracle    exactly the names and terms the sentence contains
    mixed     those plus 15 unrelated terms, shuffled (a busy screen)
    distract  the 15 unrelated terms only (the screen is about something else)
    cross     the sentence's terms in the other language (Japanese subtitles
              on screen, English speech; and the reverse)

`--nonspeech` adds `<work-dir>/nonspeech`: synthetic sounds without speech
(noise at three levels, a wavering tone, chords) recognized with no prompt
and with a Japanese / English term prompt. A recognizer that repeats its
prompt over such sounds would put screen text into the subtitles of a
game's background music.

Only the first recording of each sentence is used. Everything is derived
from the sentence id, so two runs build identical jobs.
"""

import argparse
import array
import io
import json
import math
import random
import struct
import sys
import tarfile
import urllib.request
import wave
from pathlib import Path

HERE = Path(__file__).resolve().parent
HF = "https://huggingface.co/datasets/google/fleurs/resolve/main/data"
FLEURS = {"ja": "ja_jp", "en": "en_us"}
JOINER = {"ja": ("、", "。"), "en": (", ", ".")}
CONDITIONS = ["none", "oracle", "mixed", "distract", "cross"]
DISTRACTORS = 15


def fetch(url: str, dest: Path) -> Path:
    if not dest.exists():
        dest.parent.mkdir(parents=True, exist_ok=True)
        print(f"download {url}", file=sys.stderr)
        tmp = dest.with_suffix(dest.suffix + ".part")
        with urllib.request.urlopen(url) as r, open(tmp, "wb") as f:
            while chunk := r.read(1 << 20):
                f.write(chunk)
        tmp.rename(dest)
    return dest


def read_terms(lang: str) -> dict[str, list[str]]:
    out = {}
    for line in (HERE / f"terms.{lang}.tsv").read_text(encoding="utf-8").splitlines():
        if line.strip():
            sid, terms = line.split("\t")
            out[sid] = terms.split("|")
    return out


def to_pcm16(raw: bytes) -> bytes:
    """FLEURS ships 16 kHz mono float32 WAV; the app works on PCM16."""
    assert raw[:4] == b"RIFF" and raw[8:12] == b"WAVE", "not a WAV file"
    fmt = bits = channels = rate = 0
    i = 12
    while i + 8 <= len(raw):
        cid, size = raw[i : i + 4], struct.unpack("<I", raw[i + 4 : i + 8])[0]
        body = raw[i + 8 : i + 8 + size]
        if cid == b"fmt ":
            fmt, channels, rate = struct.unpack("<HHI", body[:8])
            bits = struct.unpack("<H", body[14:16])[0]
        elif cid == b"data":
            assert rate == 16000 and channels == 1, (rate, channels)
            if (fmt, bits) == (1, 16):
                return body
            assert (fmt, bits) == (3, 32), (fmt, bits)
            floats = array.array("f")
            floats.frombytes(body[: len(body) // 4 * 4])
            if sys.byteorder == "big":
                floats.byteswap()
            pcm = array.array("h", (int(max(-1.0, min(1.0, v)) * 32767) for v in floats))
            if sys.byteorder == "big":
                pcm.byteswap()
            return pcm.tobytes()
        i += 8 + size + (size & 1)
    raise AssertionError("no data chunk")


def write_wav(path: Path, pcm: bytes) -> None:
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(16000)
        w.writeframes(pcm)


def prompt(lang: str, terms: list[str]) -> str:
    sep, end = JOINER[lang]
    return sep.join(terms) + end if terms else ""


def build(lang: str, work: Path, limit: int, conditions: list[str]) -> None:
    code = FLEURS[lang]
    cache = work / "cache"
    terms = read_terms(lang)
    other = read_terms("en" if lang == "ja" else "ja")
    other_lang = "en" if lang == "ja" else "ja"

    # id, file, raw transcription, …; the first recording of a sentence wins.
    rows = {}
    tsv = fetch(f"{HF}/{code}/dev.tsv", cache / f"{code}-dev.tsv")
    for line in tsv.read_text(encoding="utf-8").splitlines():
        cols = line.split("\t")
        if cols[0] in terms and cols[0] not in rows:
            rows[cols[0]] = (cols[1], cols[2].strip())
    missing = set(terms) - set(rows)
    assert not missing, f"ids not in FLEURS {code} dev: {sorted(missing)}"
    for sid, ts in terms.items():
        for t in ts:
            assert t in rows[sid][1], f"{lang} {sid}: {t!r} is not in the sentence"

    ids = list(terms)[:limit] if limit else list(terms)
    out = work / lang
    (out / "wav").mkdir(parents=True, exist_ok=True)
    wanted = {f"dev/{rows[s][0]}": s for s in ids if not (out / "wav" / f"{s}.wav").exists()}
    if wanted:
        tar = fetch(f"{HF}/{code}/audio/dev.tar.gz", cache / f"{code}-dev.tar.gz")
        with tarfile.open(tar) as t:
            for member in t:
                sid = wanted.pop(member.name, None)
                if sid:
                    pcm = to_pcm16(t.extractfile(member).read())
                    write_wav(out / "wav" / f"{sid}.wav", pcm)
        assert not wanted, f"recordings not in the archive: {wanted}"

    manifest, jobs = [], []
    for sid in ids:
        ref, own = rows[sid][1], terms[sid]
        rng = random.Random(f"{lang}-{sid}")
        pool = sorted({t for s, ts in terms.items() if s != sid for t in ts if t not in ref})
        distract = rng.sample(pool, DISTRACTORS)
        mixed = own + distract
        rng.shuffle(mixed)
        prompts = {
            "none": "",
            "oracle": prompt(lang, own),
            "mixed": prompt(lang, mixed),
            "distract": prompt(lang, distract),
        }
        if sid in other:
            prompts["cross"] = prompt(other_lang, other[sid])
        manifest.append(
            {
                "id": sid,
                "lang": lang,
                "reference": ref,
                "terms": own,
                "distractors": distract,
                "prompts": prompts,
            }
        )
        for c in conditions:
            if c in prompts:
                jobs.append(
                    {
                        "key": f"{lang}-{sid}",
                        "wav": f"wav/{sid}.wav",
                        "condition": c,
                        # Language left to the recognizer, as by default in the app.
                        "language": None,
                        "prompt": prompts[c] or None,
                    }
                )
    (out / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=1), encoding="utf-8"
    )
    with io.open(out / "jobs.jsonl", "w", encoding="utf-8") as f:
        for j in jobs:
            f.write(json.dumps(j, ensure_ascii=False) + "\n")
    print(f"{lang}: {len(ids)} sentences, {len(jobs)} jobs -> {out}", file=sys.stderr)


def nonspeech(work: Path) -> None:
    out = work / "nonspeech"
    (out / "wav").mkdir(parents=True, exist_ok=True)
    rng = random.Random("nonspeech")
    rate, seconds = 16000, 6
    n = rate * seconds

    def noise(level: float) -> list[float]:
        return [rng.gauss(0, level) for _ in range(n)]

    def tone() -> list[float]:
        return [
            3000 * math.sin(2 * math.pi * 440 * i / rate) * (0.5 + 0.5 * math.sin(2 * math.pi * 2 * i / rate))
            for i in range(n)
        ]

    def chords() -> list[float]:
        # A new triad every 0.75 s, each note with a plucked decay.
        roots = [220.0, 261.6, 196.0, 293.7]
        out_ = []
        for i in range(n):
            step, t = divmod(i / rate, 0.75)
            root = roots[int(step) % len(roots)]
            v = sum(math.sin(2 * math.pi * root * k * t) for k in (1.0, 1.26, 1.5))
            out_.append(2500 * v * math.exp(-3 * t))
        return out_

    clips = {
        "noise-faint": noise(150),
        "noise-mid": noise(600),
        "noise-loud": noise(2500),
        "tone": tone(),
        "chords": chords(),
        "chords-noise": [a + b for a, b in zip(chords(), noise(300))],
    }
    picks = {}
    for lang in FLEURS:
        pool = sorted({t for ts in read_terms(lang).values() for t in ts})
        picks[lang] = random.Random(f"nonspeech-{lang}").sample(pool, DISTRACTORS)
    manifest, jobs = [], []
    for name, samples in clips.items():
        pcm = array.array("h", (int(max(-32768, min(32767, v))) for v in samples))
        if sys.byteorder == "big":
            pcm.byteswap()
        write_wav(out / "wav" / f"{name}.wav", pcm.tobytes())
        prompts = {"none": "", "ja": prompt("ja", picks["ja"]), "en": prompt("en", picks["en"])}
        manifest.append(
            {
                "id": name,
                "lang": "nonspeech",
                "reference": "",
                "terms": [],
                "distractors": picks["ja"] + picks["en"],
                "prompts": prompts,
            }
        )
        for c, p in prompts.items():
            jobs.append(
                {
                    "key": f"nonspeech-{name}",
                    "wav": f"wav/{name}.wav",
                    "condition": c,
                    "language": None,
                    "prompt": p or None,
                }
            )
    (out / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=1), encoding="utf-8"
    )
    with io.open(out / "jobs.jsonl", "w", encoding="utf-8") as f:
        for j in jobs:
            f.write(json.dumps(j, ensure_ascii=False) + "\n")
    print(f"nonspeech: {len(clips)} clips, {len(jobs)} jobs -> {out}", file=sys.stderr)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("work", type=Path)
    ap.add_argument("--lang", nargs="*", default=["ja", "en"], choices=list(FLEURS))
    ap.add_argument("--nonspeech", action="store_true", help="also build the no-speech clips")
    ap.add_argument("--limit", type=int, default=0, help="first N sentences only")
    ap.add_argument("--conditions", nargs="+", default=CONDITIONS, choices=CONDITIONS)
    a = ap.parse_args()
    # The scorer compares every condition with `none`.
    conditions = [c for c in CONDITIONS if c == "none" or c in a.conditions]
    for lang in a.lang:
        build(lang, a.work, a.limit, conditions)
    if a.nonspeech:
        nonspeech(a.work)


if __name__ == "__main__":
    main()
