"""Deck map, round 2: B (the places list) with C's exploration (a free cursor over the
map) and a pane that folds to the right. Three frames: the cursor on a listed place,
the cursor out on open ground, and the pane folded.

    SHOTS=dir/ python3 -I gen3.py out.html

Reads what gen2.py reads (tmp_mapdump.json, tmp_deckmap_120x40.svg) and draws with its
map, markers and deck."""

import json
import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen2  # noqa: E402
from gen2 import (CYAN, DIM, FAINT, KIND, KINDS, PANEL, PLACES, PLAYER, RED, SEL, TEXT, W,  # noqa: E402
                  CSS, DIST, DISTRICTS, MapView, TRACKED, PATH, TER, card, deck, district_labels,
                  hints, mix, scale)

PANE = 36          # the places pane's width, its border included
HINT_OPEN = "←↑↓→ explore · [ ] places · < > kinds · T track · +− zoom · f fold · tab close"
HINT_FOLDED = "←↑↓→ explore · [ ] places · < > kinds · T track · +− zoom · f unfold · tab close"
PINNED = {"story", "home"}  # always on the map and in the list, whatever the kind


def shown(kind):
    return lambda p: kind == "all" or p["kind"] == kind or p["kind"] in PINNED


def listed(kind):
    keep = shown(kind)
    return sorted((p for p in PLACES if keep(p) and p["m"] is not None), key=lambda p: p["m"])


def view(g, top, bottom, x0, cols, s, centre):
    rows = bottom - top - 1
    ox = max(0, min(gen2.CW - cols * s, centre[0] - cols * s // 2))
    oy = max(0, min(gen2.CH - rows * 2 * s, centre[1] - rows * s))
    return MapView(g, x0, top + 1, cols, rows, s, ox, oy)


def marks(m, kind):
    keep = shown(kind)
    cells = []
    for p in PLACES:
        if not keep(p):
            continue
        _, ch, fg, _ = KIND[p["kind"]]
        c = m.mark(p["tile"], ch, fg)
        if c:
            cells.append(c)
    cells.append(m.mark(PLAYER, "@", "#ffffff"))
    return cells


def reticle(g, m, tile):
    """The cursor: four corners around its cell, the cell lit a little."""
    c = m.cell(tile)
    x, y = c
    out = []
    for dx, dy, ch in ((-1, -1, "┌"), (1, -1, "┐"), (-1, 1, "└"), (1, 1, "┘")):
        cell = g.c[y + dy][x + dx]
        cell[0], cell[1], cell[3] = ch, CYAN, True
        out.append((x + dx, y + dy))
    g.c[y][x][2] = mix(g.c[y][x][2], CYAN, 0.25)
    return out


def title(g, top, kind):
    g.put(2, top, f"{gen2.J['city'].upper()} · STREET", CYAN, bold=True)
    g.put(2 + len(gen2.J["city"]) + 9, top, "· zoom 2 · lit: remembered · faint: the city grid", FAINT)


def pane(g, top, bottom, kind, sel, extra=None):
    """The places pane: the kinds on top, then the places nearest first."""
    px = W - PANE
    for y in range(top, bottom):
        g.put(px - 1, y, "│", "#1a2230")
    label = "all" if kind == "all" else KIND[kind][3]
    g.put(px + 1, top, "PLACES", CYAN, bold=True)
    g.put(px + 8, top, f"‹ {label} ›", TEXT)
    g.put(W - 6, top, "< >", FAINT)
    x = px + 1
    for key, ch, fg, _ in KINDS:
        on = kind == "all" or key == kind or key in PINNED
        bg = SEL if key == kind else None
        g.put(x, top + 1, ch, fg if on else scale(fg, 0.3), bg, bold=on)
        x += 2
    g.put(x, top + 1, "all", CYAN if kind == "all" else FAINT, SEL if kind == "all" else None)
    g.put(px, top + 2, "─" * (PANE - 2), "#1a2230")
    rows = listed(kind)
    y = top + 3
    last = bottom - (5 if extra else 2)
    shown_n = 0
    for p in rows:
        if y >= last:
            break
        _, ch, fg, _ = KIND[p["kind"]]
        is_sel = p is sel
        bg = SEL if is_sel else None
        if is_sel:
            g.fill(px, y, PANE - 2, 3, SEL)
        g.put(px + 1, y, ch, fg, bg, bold=True)
        g.put(px + 3, y, p["name"][:23], TEXT if is_sel else DIM, bg, bold=is_sel)
        g.put(W - 8, y, "%4d m" % p["m"], DIM if is_sel else FAINT, bg)
        y += 1
        shown_n += 1
        if is_sel:
            g.put(px + 3, y, p["what"][:30], DIM, SEL)
            tracked = p is TRACKED
            g.put(px + 3, y + 1, "⌖ tracking" if tracked else "T track", CYAN if tracked else DIM, SEL, bold=tracked)
            if tracked:
                g.put(px + 14, y + 1, "· T stops", FAINT, SEL)
            y += 2
    more = len(rows) - shown_n
    if more > 0:
        g.put(px + 1, last, f"▼ {more} more", FAINT)
    if extra:
        g.put(px, bottom - 4, "─" * (PANE - 2), "#1a2230")
        for k, (text, fg, bold) in enumerate(extra):
            g.put(px + 1, bottom - 3 + k, text[:PANE - 3], fg, bold=bold)


def frame_place():
    """The pane open, the cursor on the clinic: its row lit and opened in the list."""
    g, top, bottom, hint_y = deck()
    title(g, top, "all")
    cols = W - PANE - 3
    m = view(g, top, bottom, 1, cols, 2, TRACKED["tile"])
    mk = marks(m, "all")
    district_labels(m, mk)
    ret = reticle(g, m, TRACKED["tile"])
    pane(g, top, bottom, "all", TRACKED)
    hints(g, hint_y, HINT_OPEN)
    return g, m.anim(PATH, TRACKED["tile"], mk, ret)


def open_ground():
    """A plaza tile of Crown Circle, away from every place: where the cursor roams."""
    d = next(d for d in DISTRICTS.values() if d["name"] == "Crown Circle")
    cx, cy = d["center"]
    best = None
    for y in range(gen2.CH):
        for x in range(gen2.CW):
            if TER[y][x] == "p" and (x, y) in DIST:
                k = abs(x - cx) + abs(y - cy)
                if best is None or k < best[0]:
                    best = (k, (x, y))
    return best[1]


def frame_ground():
    """The pane open, the cursor out on open ground: no row lit; the pane says what is
    under the cursor and that T tracks the spot (the clinic's track still glows)."""
    g, top, bottom, hint_y = deck()
    title(g, top, "doc")
    cols = W - PANE - 3
    spot = open_ground()
    m = view(g, top, bottom, 1, cols, 2, spot)
    mk = marks(m, "doc")
    ret = reticle(g, m, spot)
    c = m.cell(spot)
    district_labels(m, mk + ret + [c, (c[0] - 1, c[1]), (c[0] + 1, c[1])])
    seen = "never been" if gen2.VIS[spot[1]][spot[0]] == "0" else "you've been here"
    pane(g, top, bottom, "doc", None, extra=[
        ("CURSOR · Crown Circle · plaza", CYAN, True),
        (f"walk {DIST[spot]} m · {seen}", DIM, False),
        ("T tracks this spot instead", DIM, False),
    ])
    hints(g, hint_y, HINT_OPEN)
    return g, m.anim(PATH, TRACKED["tile"], mk, ret)


def fold_strip(g, top, bottom, kind):
    """The folded pane: a strip on the right edge says it is there and what it lists."""
    x = W - 4
    for y in range(top, bottom):
        g.c[y][x - 1] = ["▕", "#1a2230", PANEL, False]
        g.c[y][x] = [" ", FAINT, "#0a1018", False]
        g.c[y][x + 1] = [" ", FAINT, "#0a1018", False]
    g.put(x, top, "◂f", CYAN, "#0a1018", bold=True)
    word = "PLACES"
    for k, ch in enumerate(word):
        g.put(x + 1 if k % 2 else x, top + 2 + k, ch, DIM, "#0a1018")
    _, ch, fg, _ = KIND[kind]
    n = len(listed(kind))
    g.put(x, top + 3 + len(word), ch, fg, "#0a1018", bold=True)
    g.put(x, top + 4 + len(word), str(n), DIM, "#0a1018")


def frame_folded():
    """The pane folded away: the map takes the deck, a card opens beside the place under
    the cursor, and a strip on the right edge marks the folded pane."""
    g, top, bottom, hint_y = deck()
    title(g, top, "doc")
    cols = W - 6
    m = view(g, top, bottom, 1, cols, 2, TRACKED["tile"])
    mk = marks(m, "doc")
    district_labels(m, mk)
    ret = reticle(g, m, TRACKED["tile"])
    c = m.cell(TRACKED["tile"])
    cx = c[0] + 3 if c[0] + 3 + 30 <= W - 5 else c[0] - 3 - 30
    cy = max(top + 1, min(c[1] - 2, bottom - 8))
    card(g, cx, cy, 30, [
        ("+ " + TRACKED["name"], RED),
        (TRACKED["what"], DIM),
        (f"{TRACKED['district']} · open · {TRACKED['m']} m", DIM),
        ("⌖ tracking · T stops", CYAN),
    ])
    fold_strip(g, top, bottom, "doc")
    hints(g, hint_y, HINT_FOLDED)
    covered = [(x, y) for y in range(cy, cy + 6) for x in range(cx, cx + 30)] + ret
    return g, m.anim(PATH, TRACKED["tile"], mk, covered)


FRAMES = [
    ("1", "Pane open · the cursor on a place", frame_place,
     "The map is C's: a free cursor (the four corners) that you move with the arrows or hjkl, "
     "the view panning with it, + and − to zoom (here zoom 2; zoomed out, the whole city fits). "
     "The pane is B's: the places of the kind you pick (< >; home and your jobs always show), "
     "nearest walk first, scrolled with [ and ], which jump the cursor to the place. When the "
     "cursor sits on a listed place, its row lights and opens. T tracks it; the walk glows."),
    ("2", "Pane open · the cursor on open ground", frame_ground,
     "Arrowed off into Crown Circle, where you have never been: no row is lit, and the foot "
     "of the pane says what is under the cursor. T would track that spot, replacing the "
     "clinic's track (one at a time). The kind here is ripperdocs, so the map shows only + "
     "(and home, and the job)."),
    ("3", "Pane folded", frame_folded,
     "f folds the pane to the right: the map takes the deck, the place under the cursor "
     "gets C's card instead of its row, and a strip on the right edge marks the folded pane "
     "(◂f, PLACES, the kind and how many). f unfolds it. Every key works the same either way."),
]


def main(out):
    views, body = {}, [
        "<h1>The deck's MAP · round 2: B with C's exploration</h1>",
        "<p>One set of keys, no modes: <b>←↑↓→ / hjkl</b> roam the map, <b>[ ]</b> step the "
        "places list (the cursor jumps there), <b>&lt; &gt;</b> pick the kind, <b>T</b> tracks "
        "what is under the cursor (a place, or a spot), <b>+ −</b> zoom, <b>f</b> folds the pane. "
        "The deck keeps Q/E, the digits and tab. Animated at 30 ticks/s.</p>",
        '<div class="bar">speed <button data-speed="1" class="on">1×</button>'
        '<button data-speed="0.25">¼×</button><button data-speed="0">pause</button>'
        ' · tick <span id="tick">0</span></div>',
    ]
    for key, name, fn, desc in FRAMES:
        g, anim = fn()
        views[key] = {"base": g.c, "anim": anim}
        body.append(f"<h2>{key} · {name}</h2><p>{desc}</p>"
                    f"<div class='term' data-k='{key}' style='font-size:12.5px'></div>")
    html = ("<!DOCTYPE html><html><head><meta charset='utf-8'><title>Deck map 2</title>"
            f"<style>{CSS}{gen2.PAGE_CSS}</style></head><body>{''.join(body)}"
            f"<script>const VIEWS={json.dumps(views)};{gen2.JS}</script></body></html>")
    open(out, "w").write(html)


if __name__ == "__main__":
    main(sys.argv[1])
