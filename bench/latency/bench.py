"""Time-to-first-translation benchmark for text-heavy captures.

Usage: python bench.py <model.gguf> [strategy ...]
Needs: pip install llama-cpp-python==0.3.37 "sacrebleu[ja]"

Each screen in screens.json is a list of OCR blocks (paragraphs) as the
capture flow produces them. Strategies:

  whole        one request with every block joined by a blank line (the
               behaviour before chunking); per-block retry without context
               when the paragraph count does not match.
  chunked      small first chunk, larger later chunks, each request in
               PFML3 with the earlier chunks as translation history.
  chunked_ctx  chunked + the text after the first chunk as `context`.

Reports, per screen: time to the first streamed token, time until the
first block is complete, total time, and chrF against references.
See docs/translation-latency.md for the method and results.
"""
import json, os, sys, time
from llama_cpp import Llama
import sacrebleu

HERE = os.path.dirname(os.path.abspath(__file__))
TAG, MSG, KEY, VAL = "<|plamo:tag|>", "<|plamo:msg|>", "<|plamo:key|>", "<|plamo:val|>"
STOP = ["<|plamo:tag|>", "<|plamo:bos|>"]
SEP = "\n\n"

FIRST = int(os.environ.get("FIRST", "160"))
REST = int(os.environ.get("REST", "600"))
CTX = int(os.environ.get("CTX", "600"))


def prompt(cur, tgt, history=(), context=None):
    p = TAG + "dataset" + MSG + "translation"
    if context:
        p += TAG + "context" + MSG + context
    for s, t in history:
        p += TAG + "input" + MSG + s + TAG + "output" + KEY + "lang" + VAL + tgt + MSG + t
    return p + TAG + "input" + MSG + cur + TAG + "output" + KEY + "lang" + VAL + tgt + MSG


def cost(s):
    """Same as translate_plan::cost: ASCII 1, other characters 3."""
    return sum(1 if ord(c) < 128 else 3 for c in s)


def take_reference(texts, budget):
    """Same as translate_plan::take_reference (blocks joined by SEP)."""
    out, used = "", 0
    for t in texts:
        c = cost(t) + (cost(SEP) if out else 0)
        if used + c > budget:
            if not out:
                acc = 0
                for ch in t:
                    acc += cost(ch)
                    if acc > budget:
                        break
                    out += ch
            break
        out += (SEP if out else "") + t
        used += c
    return out or None


def plan(blocks, first=FIRST, rest=REST):
    """Same as translate_plan::plan, on block costs."""
    chunks, cur, size, limit = [], [], 0, first
    for i, b in enumerate(blocks):
        if cur and size + cost(b) > limit:
            chunks.append(cur)
            cur, size, limit = [], 0, rest
        cur.append(i)
        size += cost(b)
    if cur:
        chunks.append(cur)
    return chunks


def split(text, n):
    if n == 1:
        return [text.strip()]
    parts = [p.strip() for p in text.split(SEP) if p.strip()]
    if len(parts) == n:
        return parts
    lines = [l.strip() for l in text.splitlines() if l.strip()]
    return lines if len(lines) == n else None


class Run:
    def __init__(self, m, tgt):
        self.m, self.tgt, self.t0 = m, tgt, time.time()
        self.first_tok = None
        self.first_block = None
        self.calls = 0

    def gen(self, p, n_blocks_before):
        toks = self.m.tokenize(p.encode(), add_bos=True, special=True)
        self.calls += 1
        out = ""
        for ch in self.m.create_completion(toks, max_tokens=1024, temperature=0.0, stream=True, stop=STOP):
            d = ch["choices"][0]["text"]
            if d and self.first_tok is None:
                self.first_tok = time.time() - self.t0
            out += d
            if self.first_block is None and n_blocks_before == 0 and SEP in out.strip():
                self.first_block = time.time() - self.t0
        if self.first_block is None:
            self.first_block = time.time() - self.t0
        return out.strip()


def whole(m, blocks, tgt):
    r = Run(m, tgt)
    out = r.gen(prompt(SEP.join(blocks), tgt), 0)
    parts = split(out, len(blocks))
    fallback = parts is None
    if fallback:
        parts = [r.gen(prompt(b, tgt), 1) for b in blocks]
    return parts, r, fallback


def chunked(m, blocks, tgt, with_ctx=False):
    r = Run(m, tgt)
    chunks = plan(blocks)
    context = None
    if with_ctx and len(chunks) > 1:
        context = take_reference(blocks[chunks[1][0]:], CTX)
    history, res, fallback = [], [None] * len(blocks), False
    for k, idx in enumerate(chunks):
        src = SEP.join(blocks[i] for i in idx)
        out = r.gen(prompt(src, tgt, history, context), k)
        parts = split(out, len(idx))
        if parts is None:
            fallback = True
            parts = []
            h = list(history)
            for i in idx:
                t = r.gen(prompt(blocks[i], tgt, h, context), 1)
                parts.append(t)
                h.append((blocks[i], t))
            out = SEP.join(parts)
        for i, t in zip(idx, parts):
            res[i] = t
        history.append((src, out))
    return res, r, fallback


def chrf(hyps, refs, tgt):
    tok = "ja-mecab" if tgt == "ja" else "13a"
    return sacrebleu.corpus_chrf(hyps, [refs], word_order=0 if tgt == "ja" else 2).score


def main():
    path = sys.argv[1]
    strategies = sys.argv[2:] or ["whole", "chunked", "chunked_ctx"]
    m = Llama(path, n_ctx=8192, n_threads=int(os.environ.get("THREADS", "4")), n_batch=512, verbose=False)
    screens = json.load(open(os.path.join(HERE, "screens.json")))
    m.create_completion(m.tokenize(prompt("Hello.", "ja").encode(), add_bos=True, special=True), max_tokens=8, stop=STOP)
    results = []
    for sc in screens:
        for st in strategies:
            m.reset()
            m._ctx.kv_cache_clear()
            if st == "whole":
                parts, r, fb = whole(m, sc["blocks"], sc["tgt"])
            else:
                parts, r, fb = chunked(m, sc["blocks"], sc["tgt"], st == "chunked_ctx")
            total = time.time() - r.t0
            row = dict(screen=sc["id"], strategy=st, first_token=round(r.first_tok, 2),
                       first_block=round(r.first_block, 2), total=round(total, 2), calls=r.calls,
                       fallback=fb, chrf=round(chrf(parts, sc["refs"], sc["tgt"]), 1) if sc.get("refs") else None, out=parts)
            results.append(row)
            print(json.dumps({k: v for k, v in row.items() if k != "out"}, ensure_ascii=False), flush=True)
    name = os.environ.get("OUT", "results.json")
    os.makedirs(os.path.join(HERE, "results"), exist_ok=True)
    json.dump(dict(first=FIRST, rest=REST, ctx=CTX, results=results),
              open(os.path.join(HERE, "results", name), "w"), ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main()
