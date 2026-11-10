"""A pixel canvas for the armor drawings: half cells, as the game's `Pixels`, with the
shapes the drawings are made of. `None` is the panel showing through."""


def hexrgb(c):
    return tuple(int(c[i:i + 2], 16) for i in (1, 3, 5))


def rgbhex(t):
    return "#%02x%02x%02x" % tuple(max(0, min(255, int(v))) for v in t)


def mix(a, b, t):
    a, b = hexrgb(a), hexrgb(b)
    return rgbhex(tuple(a[k] + (b[k] - a[k]) * t for k in range(3)))


class Canvas:
    def __init__(self, w, h):
        self.w, self.h = w, h
        self.p = [[None] * w for _ in range(h)]
        # Text laid over the folded cells: (cell x, cell y, text, fg, bold).
        self.labels = []

    def inside(self, x, y):
        return 0 <= x < self.w and 0 <= y < self.h

    def px(self, x, y, c):
        if self.inside(x, y):
            self.p[y][x] = c

    def get(self, x, y):
        return self.p[y][x] if self.inside(x, y) else None

    def on(self, x, y):
        return self.get(x, y) is not None

    def rect(self, x0, y0, x1, y1, c):
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                self.px(x, y, c)

    def hline(self, x0, x1, y, c):
        self.rect(min(x0, x1), y, max(x0, x1), y, c)

    def vline(self, x, y0, y1, c):
        self.rect(x, min(y0, y1), x, max(y0, y1), c)

    def line(self, x0, y0, x1, y1, c):
        dx, dy = abs(x1 - x0), -abs(y1 - y0)
        sx, sy = (1 if x0 < x1 else -1), (1 if y0 < y1 else -1)
        err = dx + dy
        while True:
            self.px(x0, y0, c)
            if x0 == x1 and y0 == y1:
                return
            e2 = 2 * err
            if e2 >= dy:
                err += dy
                x0 += sx
            if e2 <= dx:
                err += dx
                y0 += sy

    def poly(self, pts, c):
        """Fills the polygon, pixel centres inside, edges included."""
        ys = [p[1] for p in pts]
        for y in range(max(0, min(ys)), min(self.h, max(ys) + 1)):
            yc = y + 0.5
            xs = []
            n = len(pts)
            for i in range(n):
                (x0, y0), (x1, y1) = pts[i], pts[(i + 1) % n]
                if (y0 <= yc < y1) or (y1 <= yc < y0):
                    xs.append(x0 + (yc - y0) * (x1 - x0) / (y1 - y0))
            xs.sort()
            for a, b in zip(xs[::2], xs[1::2]):
                for x in range(int(round(a)), int(round(b)) + 1):
                    self.px(x, y, c)
        for i in range(len(pts)):
            self.line(*pts[i], *pts[(i + 1) % len(pts)], c)

    def mirrored(self, pts):
        """The left half's points and their mirror, as one outline."""
        right = [(self.w - 1 - x, y) for x, y in reversed(pts)]
        return list(pts) + right

    def recolour(self, test, c):
        for y in range(self.h):
            for x in range(self.w):
                if self.p[y][x] is not None and test(x, y, self.p[y][x]):
                    self.p[y][x] = c

    def shade(self, base, hi, lo, edge=None):
        """Light from the upper left: the outline's upper-left pixels `hi`, its lower
        right `lo`, on pixels still `base`. `edge` darkens every outline pixel first."""
        out = [row[:] for row in self.p]
        for y in range(self.h):
            for x in range(self.w):
                if self.p[y][x] != base:
                    continue
                if not self.on(x - 1, y) or not self.on(x, y - 1):
                    out[y][x] = hi
                elif not self.on(x + 1, y) or not self.on(x, y + 1):
                    out[y][x] = lo
        self.p = out

    def stamp(self, x, y, rows, pal):
        """A small sprite: one char per pixel, '.' clear, others from `pal`."""
        for j, row in enumerate(rows):
            for i, ch in enumerate(row):
                if ch != "." and ch in pal:
                    self.px(x + i, y + j, pal[ch])

    def label(self, cx, cy, text, fg, bold=True):
        self.labels.append((cx, cy, text, fg, bold))

    def half(self):
        """At half size, as KIT shows it: each 2x2 keeps its commonest colour, and a block
        at least half clear stays clear."""
        h, w = self.h // 2, self.w // 2
        out = Canvas(w, h)
        for y in range(h):
            for x in range(w):
                got = [self.p[2 * y + j][2 * x + i] for j in (0, 1) for i in (0, 1)]
                cols = [c for c in got if c]
                if len(cols) * 2 < len(got):
                    continue
                out.p[y][x] = max(set(cols), key=lambda c: (cols.count(c), sum(hexrgb(c))))
        return out

    def cells(self, panel):
        """Folded into cells, as the game folds them, with the labels on top."""
        import sys
        import os
        here = os.path.dirname(os.path.abspath(__file__))
        sys.path.insert(0, os.path.join(here, "..", "2026-10-19-gear-inventory-mock"))
        from cells import fold
        p = self.p if self.h % 2 == 0 else self.p + [[None] * self.w]
        g = fold(p, panel)
        for cx, cy, text, fg, bold in self.labels:
            for i, ch in enumerate(text):
                x = cx + i
                if 0 <= x < g.w and 0 <= cy < g.h:
                    cell = g.c[cy][x]
                    # Text keeps the cell's lower pixel as its ground.
                    t, b = self.get(x, 2 * cy), self.get(x, 2 * cy + 1)
                    ground = b or t or panel
                    g.c[cy][x] = [ch, fg, ground, bold]
        return g
