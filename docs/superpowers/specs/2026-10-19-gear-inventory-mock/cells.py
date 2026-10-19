"""Terminal cells: read the game's shot SVGs into a grid, draw on it, write HTML."""

import html
import re

CELL_W, CELL_H = 9.6, 20.0
RECT = re.compile(r'<rect x="([\d.]+)" y="([\d.]+)" width="[\d.]+" height="[\d.]+" fill="(#[0-9a-f]{6})"/>')
TEXT = re.compile(r'<text text-anchor="middle" x="([\d.]+)" y="([\d.]+)" fill="(#[0-9a-f]{6})"([^>]*)>(.*?)</text>')


class Grid:
    def __init__(self, w, h, bg="#04060a"):
        self.w, self.h = w, h
        # [ch, fg, bg, bold]
        self.c = [[[" ", "#c8d0dc", bg, False] for _ in range(w)] for _ in range(h)]

    @staticmethod
    def from_svg(path, w, h):
        g = Grid(w, h)
        s = open(path).read()
        for m in RECT.finditer(s):
            x = round(float(m.group(1)) / CELL_W)
            y = round(float(m.group(2)) / CELL_H)
            if 0 <= x < w and 0 <= y < h:
                g.c[y][x][2] = m.group(3)
        for m in TEXT.finditer(s):
            x = int(float(m.group(1)) / CELL_W)
            y = int(float(m.group(2)) / CELL_H)
            if 0 <= x < w and 0 <= y < h:
                g.c[y][x][0] = html.unescape(m.group(5))
                g.c[y][x][1] = m.group(3)
                g.c[y][x][3] = "bold" in m.group(4)
        return g

    def text(self):
        return "\n".join("".join(cell[0] for cell in row) for row in self.c)

    def crop(self, x, y, w, h):
        g = Grid(w, h)
        for j in range(h):
            for i in range(w):
                g.c[j][i] = list(self.c[y + j][x + i])
        return g

    def paste(self, other, x, y, skip_bg=None):
        for j in range(other.h):
            for i in range(other.w):
                if 0 <= x + i < self.w and 0 <= y + j < self.h:
                    cell = other.c[j][i]
                    if skip_bg and cell[0] == " " and cell[2] == skip_bg:
                        continue
                    self.c[y + j][x + i] = list(cell)

    def put(self, x, y, s, fg, bg=None, bold=False, right=None):
        right = self.w if right is None else right
        for i, ch in enumerate(s):
            if 0 <= x + i < right and 0 <= y < self.h:
                cell = self.c[y][x + i]
                cell[0], cell[1], cell[3] = ch, fg, bold
                if bg:
                    cell[2] = bg

    def fill(self, x, y, w, h, bg, ch=" ", fg=None):
        for j in range(y, y + h):
            for i in range(x, x + w):
                if 0 <= i < self.w and 0 <= j < self.h:
                    self.c[j][i] = [ch, fg or self.c[j][i][1], bg, False]

    def bg(self, x, y, w, bg):
        for i in range(x, x + w):
            if 0 <= i < self.w and 0 <= y < self.h:
                self.c[y][i][2] = bg

    def html(self, scale=1.0):
        out = ['<div class="term" style="font-size:%.1fpx">' % (14 * scale)]
        for row in self.c:
            out.append('<div class="r">')
            run, key = [], None
            def flush():
                if run:
                    fg, bg, bold = key
                    st = "color:%s;background:%s%s" % (fg, bg, ";font-weight:700" if bold else "")
                    out.append('<span style="%s">%s</span>' % (st, html.escape("".join(run))))
            for ch, fg, bg, bold in row:
                k = (fg, bg, bold)
                if k != key:
                    flush()
                    run, key = [], k
                run.append(ch)
            flush()
            out.append("</div>")
        out.append("</div>")
        return "".join(out)


# ---- half-block pixels

def pixels(g, x, y, w, h, panel):
    """The cells' pixels: two per cell, top then bottom; None where the panel shows."""
    p = [[None] * w for _ in range(2 * h)]
    for j in range(h):
        for i in range(w):
            ch, fg, bg, _ = g.c[y + j][x + i]
            if ch == "▀":
                t, b = fg, bg
            elif ch == "▄":
                t, b = bg, fg
            elif ch == "█":
                t, b = fg, fg
            elif ch == " ":
                t, b = bg, bg
            else:
                # text: keep its background; the glyph is lost at preview size
                t, b = bg, bg
            p[2 * j][i] = None if t == panel else t
            p[2 * j + 1][i] = None if b == panel else b
    return p


def hexrgb(c):
    return tuple(int(c[i:i + 2], 16) for i in (1, 3, 5))


def rgbhex(t):
    return "#%02x%02x%02x" % t


def shrink(p, fx=2, fy=2, mode="avg"):
    """Pixels down by fx, fy; a block at least half panel stays panel. `avg` blends a
    block's colours, `mode` keeps its commonest, `bright` its most contrasting."""
    h, w = len(p), len(p[0])
    out = []
    for y in range(0, h - h % fy, fy):
        row = []
        for x in range(0, w - w % fx, fx):
            got = [p[y + j][x + i] for j in range(fy) for i in range(fx)]
            cols = [c for c in got if c]
            if len(cols) * 2 < len(got):
                row.append(None)
                continue
            if mode == "mode":
                best = max(set(cols), key=lambda c: (cols.count(c), sum(hexrgb(c))))
                row.append(best)
            elif mode == "bright":
                lum = lambda c: sum(hexrgb(c))
                mean = sum(lum(c) for c in cols) / len(cols)
                row.append(max(cols, key=lambda c: abs(lum(c) - mean)))
            else:
                rgb = [hexrgb(c) for c in cols]
                row.append(rgbhex(tuple(sum(c[k] for c in rgb) // len(rgb) for k in range(3))))
        out.append(row)
    return out


def fold(p, panel):
    """Pixels to cells, as the game folds them."""
    h = len(p) // 2
    w = len(p[0])
    g = Grid(w, h, panel)
    for j in range(h):
        for i in range(w):
            t, b = p[2 * j][i], p[2 * j + 1][i]
            if t is None and b is None:
                g.c[j][i] = [" ", "#000000", panel, False]
            elif t == b:
                g.c[j][i] = [" ", t, t, False]
            elif t is None:
                g.c[j][i] = ["▄", b, panel, False]
            elif b is None:
                g.c[j][i] = ["▀", t, panel, False]
            else:
                g.c[j][i] = ["▀", t, b, False]
    return g


CSS = """
.term{font-family:'JetBrains Mono','Menlo','DejaVu Sans Mono',monospace;line-height:1.43;
 display:inline-block;white-space:pre;letter-spacing:0;border-radius:4px;overflow:hidden;
 box-shadow:0 0 0 1px #1a2230, 0 8px 30px rgba(0,0,0,.5)}
.term .r{height:1.43em}
.term span{display:inline-block;height:1.43em;vertical-align:top}
"""
