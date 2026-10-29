"""python3 board.py [out.html] — every chrome drawing as the card shows it (30x6 cells on
the deck's panel), by site, with its name; then the same drawings at twice the size."""

import os
import sys

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
CELLS = os.path.join(HERE, "..", "2026-10-19-gear-inventory-mock")
sys.path.insert(0, CELLS)
sys.path.insert(0, HERE)
from cells import CELL_W, CELL_H, CSS, fold  # noqa: E402
import implantart as ia  # noqa: E402

STYLE = ("<style>body{background:#0b0e14;color:#cdd4de;font-family:-apple-system,sans-serif;"
         "padding:16px 20px;margin:0}%s"
         "h1{font-size:16px;letter-spacing:.1em;color:#00e5ff;margin:4px 0 2px}"
         "h2{font-size:13px;letter-spacing:.08em;color:#5c6673;margin:14px 0 6px}"
         ".site{margin:12px 0 6px;color:#00e5ff;font-weight:700;letter-spacing:.08em;font-size:12px}"
         ".row{display:flex;gap:18px;flex-wrap:wrap}"
         ".tile{background:#04060a;padding:8px 10px 6px;border-radius:4px;border:1px solid #1a2230}"
         ".tile p{margin:6px 0 0;color:#cdd4de;font:13px Menlo,monospace}"
         ".tile p span{color:#5c6673}"
         ".art{display:block}</style>" % CSS)


def halves(ch, fg, bg):
    """A cell's top and bottom pixel colours, as a terminal draws its half blocks."""
    if ch == "▀":
        return fg, bg
    if ch == "▄":
        return bg, fg
    if ch == "█":
        return fg, fg
    return bg, bg


def svg(g, scale=1.0):
    """The folded cells drawn exactly, each cell's two halves a rect on the panel, at
    the shot harness's cell size. (Grid.html sets them as text, and the font's block
    glyphs leave seams between rows that the game's terminal doesn't.)"""
    cw, ch = CELL_W * scale, CELL_H * scale
    out = ['<svg class="art" width="%.1f" height="%.1f" shape-rendering="crispEdges">' % (g.w * cw, g.h * ch),
           '<rect width="100%%" height="100%%" fill="%s"/>' % ia.PANEL]
    for j, row in enumerate(g.c):
        for i, (c, fg, bg, _) in enumerate(row):
            for k, col in enumerate(halves(c, fg, bg)):
                if col != ia.PANEL:
                    out.append('<rect x="%.2f" y="%.2f" width="%.2f" height="%.2f" fill="%s"/>'
                               % (i * cw, j * ch + k * ch / 2, cw + 0.3, ch / 2 + 0.3, col))
    out.append("</svg>")
    return "".join(out)


def tile(key, scale):
    g = fold(ia.ART[key](), ia.PANEL)
    return '<div class="tile">%s<p>%s <span>%s</span></p></div>' % (svg(g, scale), ia.NAMES[key], key)


def board():
    out = ['<!doctype html><meta charset="utf-8"><title>Chrome art</title>', STYLE]
    out.append("<h1>CHROME · CARD ART</h1><h2>AS THE CARD SHOWS IT · 30×6 CELLS</h2>")
    for site, keys in ia.SITES:
        out.append('<div class="site">%s</div><div class="row">' % site)
        out += [tile(k, 1.0) for k in keys]
        out.append("</div>")
    out.append("<h2>×2 FOR DETAIL</h2>")
    for site, keys in ia.SITES:
        out.append('<div class="site">%s</div><div class="row">' % site)
        out += [tile(k, 2.0) for k in keys]
        out.append("</div>")
    return "".join(out)


def work(keys):
    """Some drawings only, each at 1x beside 2x: for looking closely while drawing."""
    out = ['<!doctype html><meta charset="utf-8">', STYLE]
    for k in keys:
        out.append('<div class="row" style="margin-bottom:12px;align-items:flex-start">%s%s</div>'
                   % (tile(k, 1.0), tile(k, 2.0)))
    return "".join(out)


def small(keys):
    """Some drawings at 1x only, in a grid: the set as the cards show it."""
    out = ['<!doctype html><meta charset="utf-8">', STYLE, '<div class="row">']
    out += [tile(k, 1.0) for k in keys]
    out.append("</div>")
    return "".join(out)


if __name__ == "__main__":
    ia.check()
    path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "board.html")
    keys = sys.argv[2:]
    if keys[:1] == ["--small"]:
        page = small(keys[1:] or list(ia.ART))
    else:
        page = work(keys) if keys else board()
    open(path, "w").write(page)
    print(path)
