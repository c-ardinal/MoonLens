"""MoonLens アプリアイコンの生成。

図案：魔法陣（外環・ルーン帯・内環）＋三日月を四隅括弧で捕捉。配色は本体（琥珀）と共通。
ドット絵のため、寸法単位の格子（16〜64）へ直接描画し、拡大は整数倍のみ（補間なし）。

使用法：python3 src-tauri/icons/source/gen.py（要 Pillow・NumPy）
出力先：src-tauri/icons/
"""
import math
from pathlib import Path

import numpy as np
from PIL import Image

BLACK, FAINT, MID, AMBER, CREAM = '#0b0805', '#8a5f24', '#c98a2e', '#ffb547', '#ffe0a6'
# src/lib/pixel/glyphs.ts の MINI_RUNES と同一。
MINI_RUNES = [['.#.', '###', '.#.', '#.#', '.#.'], ['#.#', '.#.', '###', '.#.', '#.#'],
              ['###', '#..', '##.', '#..', '#.#'], ['.#.', '#.#', '.#.', '.#.', '###'],
              ['#.#', '###', '#.#', '.#.', '.#.'], ['..#', '.##', '#.#', '.##', '..#']]
BAYER = np.array([[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]])
# 格子寸法 → (括弧の余白, 括弧の腕長, 魔法陣の半径)
LAYOUT = {16: (0, 3, 6), 24: (1, 5, 9), 32: (2, 7, 13), 48: (3, 10, 19), 64: (4, 14, 25)}


def rgba(hex_):
    return [int(hex_[i:i + 2], 16) for i in (1, 3, 5)] + [255]


class Canvas:
    def __init__(self, n):
        self.n = n
        self.a = np.zeros((n, n, 4), np.uint8)
        self.iy, self.ix = np.mgrid[0:n, 0:n]
        self.x, self.y = self.ix + .5, self.iy + .5

    def put(self, mask, col):
        self.a[mask] = rgba(col)

    def dist(self, cx, cy):
        return np.hypot(self.x - cx, self.y - cy)

    def disc(self, cx, cy, r):
        return self.dist(cx, cy) < r

    def ring(self, cx, cy, r, w):
        d = self.dist(cx, cy)
        return (d < r) & (d >= r - w)

    def rect(self, x0, y0, x1, y1):
        return (self.ix >= x0) & (self.ix < x1) & (self.iy >= y0) & (self.iy < y1)

    def tile(self):
        """角丸の地。"""
        n, r = self.n, self.n * .2
        m = np.ones((n, n), bool)
        for cx, cy, sx, sy in ((r, r, -1, -1), (n - r, r, 1, -1), (r, n - r, -1, 1), (n - r, n - r, 1, 1)):
            corner = ((self.x - cx) * sx > 0) & ((self.y - cy) * sy > 0)
            m &= ~(corner & (self.dist(cx, cy) >= r))
        return m

    def glyph(self, rows, x0, y0, col):
        for j, row in enumerate(rows):
            for i, ch in enumerate(row):
                if ch == '#':
                    self.a[y0 + j, x0 + i] = rgba(col)


def crescent(c, cx, cy, r):
    """左側が明部の三日月。"""
    return c.disc(cx, cy, r) & ~c.disc(cx + r * .42, cy - r * .231, r * .92)


def circle(c, cx, cy, R):
    """魔法陣＋月。格子寸法に応じ段階的に簡略化。"""
    n = c.n
    if n >= 64:
        c.put(c.ring(cx, cy, R, 1), MID)
        for i in range(12):
            a = -math.pi / 2 + (i + .5) * math.pi / 6
            gx, gy = cx + (R - 4.5) * math.cos(a), cy + (R - 4.5) * math.sin(a)
            c.glyph(MINI_RUNES[i % len(MINI_RUNES)], round(gx - 1.5), round(gy - 2.5), AMBER)
        c.put(c.ring(cx, cy, R - 8, 3), AMBER)
        mr = R - 13
    elif n >= 48:
        c.put(c.ring(cx, cy, R, 1), MID)
        for i in range(12):
            a = i * math.pi / 6
            x, y = round(cx + (R - 3.5) * math.cos(a) - 1), round(cy + (R - 3.5) * math.sin(a) - 1)
            c.put(c.rect(x, y, x + 2, y + 2), AMBER)
        c.put(c.ring(cx, cy, R - 6, 2), AMBER)
        mr = R - 10
    elif n >= 32:
        c.put(c.ring(cx, cy, R, 1), MID)
        for i in range(8):
            a = i * math.pi / 4 + math.pi / 8
            x, y = math.floor(cx + (R - 2.5) * math.cos(a)), math.floor(cy + (R - 2.5) * math.sin(a))
            c.put(c.rect(x, y, x + 1, y + 1), AMBER)
        c.put(c.ring(cx, cy, R - 4, 2), AMBER)
        mr = R - 7
    elif n >= 24:
        c.put(c.ring(cx, cy, R, 1), MID)
        c.put(c.ring(cx, cy, R - 2, 1), AMBER)
        mr = R - 4
    else:
        c.put(c.ring(cx, cy, R, 1), AMBER)
        mr = R - 2
    if n >= 32:  # 月の暗部：網点
        dark = c.disc(cx, cy, mr) & ~crescent(c, cx, cy, mr)
        c.put(dark & (BAYER[c.iy % 4, c.ix % 4] < (5 if n >= 48 else 4)), FAINT)
    c.put(crescent(c, cx, cy, mr), CREAM)


def draw(n):
    c = Canvas(n)
    c.put(c.tile(), BLACK)
    m, ln, R = LAYOUT[n]
    t = max(1, round(n / 22))
    for left in (True, False):
        for top in (True, False):
            x0 = m if left else n - m - ln
            y0 = m if top else n - m - ln
            tx = m if left else n - m - t
            ty = m if top else n - m - t
            c.put(c.rect(x0, ty, x0 + ln, ty + t) | c.rect(tx, y0, tx + t, y0 + ln), AMBER)
    circle(c, n / 2, n / 2, R)
    return Image.fromarray(c.a, 'RGBA')


def scaled(n, k, pad_to=None):
    im = draw(n)
    im = im.resize((n * k, n * k), Image.NEAREST)
    if pad_to:
        out = Image.new('RGBA', (pad_to, pad_to))
        out.paste(im, ((pad_to - im.width) // 2,) * 2)
        return out
    return im


def main():
    out = Path(__file__).resolve().parent.parent
    # Windows / Linux：余白なし。
    full = {16: scaled(16, 1), 24: scaled(24, 1), 32: scaled(32, 1), 48: scaled(48, 1), 64: scaled(64, 1),
            128: scaled(64, 2), 256: scaled(64, 4), 512: scaled(64, 8)}
    full[32].save(out / '32x32.png')
    full[128].save(out / '128x128.png')
    full[256].save(out / '128x128@2x.png')
    full[512].save(out / 'icon.png')
    sizes = [16, 24, 32, 48, 64, 128, 256]
    full[256].save(out / 'icon.ico', sizes=[(s, s) for s in sizes], append_images=[full[s] for s in sizes[:-1]])
    # macOS：図案 75 %、周囲は余白。
    mac = [scaled(64, 12, 1024), scaled(64, 6, 512), scaled(64, 3, 256), scaled(48, 2, 128),
           scaled(48, 1, 64), scaled(24, 1, 32)]
    mac[0].save(out / 'icon.icns', append_images=mac[1:])


if __name__ == '__main__':
    main()
