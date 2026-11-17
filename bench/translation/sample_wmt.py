"""Rebuild data/wmt.json: 40 random WMT22 general-test sentences per direction.

Downloads the source/reference files from github.com/wmt-conference/wmt22-news-systems.
The sample is fixed by the seed so every model sees the same sentences.
"""
import json, os, random, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
BASE = "https://raw.githubusercontent.com/wmt-conference/wmt22-news-systems/main/txt/"

def lines(path):
    return urllib.request.urlopen(BASE + path).read().decode().split("\n")

random.seed(7)
out = []
for d, src, ref, sl, tl in [
    ("ja-en", "sources/generaltest2022.ja-en.src.ja", "references/generaltest2022.ja-en.ref.A.en", "ja", "en"),
    ("en-ja", "sources/generaltest2022.en-ja.src.en", "references/generaltest2022.en-ja.ref.A.ja", "en", "ja"),
]:
    pairs = [(s, r) for s, r in zip(lines(src), lines(ref)) if s.strip() and r.strip()]
    pairs = [p for p in pairs if (15 <= len(p[0]) <= 160 if sl == "ja" else 40 <= len(p[0]) <= 300)]
    for i, (s, r) in enumerate(random.sample(pairs, 40)):
        out.append(dict(id=f"wmt-{d}-{i}", src_lang=sl, tgt_lang=tl, src=s, ref=r))
json.dump(out, open(os.path.join(HERE, "data", "wmt.json"), "w"), ensure_ascii=False, indent=1)
print(len(out), "sentences")
