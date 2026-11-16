"""GEAR & INVENTORY with the body as a graph of slots (the agreed mock): tiles with the
item's name and what it gives, joined by lines, over the game's real 120 x 40 frame.

Run from the repo root:
  SHOT_DIR=docs/superpowers/specs/2026-11-16-equipment-mock cargo test -p strata-zero \
    --features window --target-dir target/smoke -- --ignored gear_and_inventory
  python3 -I docs/superpowers/specs/2026-11-16-equipment-mock/gen.py
It writes equipment.html beside this file (the shot SVGs and the page are not committed)."""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from cells import Grid  # noqa: E402
import term  # noqa: E402

BG = "#04060a"
BORDER = "#283c4b"
LABEL = "#a0aab9"
EMPTY = "#5c6673"
TEXT = "#cdd4de"
CYAN = "#00e5ff"
CYAN_DIM = "#00a5b8"
SEL_BG = "#003c46"
SOCKET = "#3f7c87"

# key: (label, name lines, what it gives as (text, colour) runs)
SLOTS = {
    "head": ("HEAD", ["Hardhat", "w/ visor"], [("+2 HP", LABEL)]),
    "face": ("FACE", ["Mirror-", "shades"], [("COOL +1", LABEL)]),
    "larm": ("L ARM", None, None),
    "lhand": ("L HAND", ["Monowire", "shiv"], [("○ ", LABEL), ("REF +1", LABEL)]),
    "torso": ("TORSO", ["Salvage", "jacket"], [("+5 HP", LABEL)]),
    "rarm": ("R ARM", None, None),
    "rhand": ("R HAND", ["Ratchet", "SMG"], [("● ", CYAN), ("4×3 ", LABEL), ("■■", SOCKET)]),
    "back": ("BACK", ["Field", "scrambler"], [("◌ 62%", LABEL)]),
    "waist": ("WAIST", ["Rebuilt", "pawn deck"], [("■■■■■", SOCKET), (" □□", SOCKET)]),
    "legs": ("LEGS", ["Cargo", "pants"], [("+4 kg", LABEL)]),
    "feet": ("FEET", None, None),
    "xarm": ("ARM 3", None, None),
    "xhand": ("HAND 3", ["Scrap", "nine"], [("○ ", LABEL), ("5×1", LABEL)]),
}
SELECTED = "rhand"

# Positions relative to the graph's origin: (x, y, w, h).
SMALL, BIG = (11, 5), (12, 7)
POS = {
    "head": (24, 0) + SMALL, "face": (36, 0) + SMALL,
    "lhand": (0, 7) + SMALL, "larm": (12, 7) + SMALL, "torso": (24, 6) + BIG,
    "rarm": (37, 7) + SMALL, "rhand": (49, 7) + SMALL,
    "back": (12, 14) + SMALL, "waist": (24, 14) + SMALL,
    "legs": (24, 20) + SMALL, "feet": (36, 20) + SMALL,
    "xarm": (37, 14) + SMALL, "xhand": (49, 14) + SMALL,
}
SPINE, BACK_J, ARM3_J = 33, 20, 46
SEP = 63


def tile(g, key, ox, oy):
    label, name, gives = SLOTS[key]
    x, y, w, h = POS[key]
    x, y = ox + x, oy + y
    sel = key == SELECTED
    bc = CYAN if sel else BORDER
    g.put(x, y, "┌" + "─" * (w - 2) + "┐", bc)
    g.put(x + 1, y, " %s " % label, CYAN_DIM if sel else LABEL, bold=sel)
    for j in range(1, h - 1):
        g.put(x, y + j, "│", bc)
        g.put(x + w - 1, y + j, "│", bc)
        g.fill(x + 1, y + j, w - 2, 1, SEL_BG if sel else BG)
    g.put(x, y + h - 1, "└" + "─" * (w - 2) + "┘", bc)
    if not name:
        g.put(x + w // 2, y + h // 2, "·", EMPTY)
        return
    top = y + 1 + (h - 2 - 3) // 2
    for i, line in enumerate(name):
        g.put(x + 1, top + i, line, CYAN if sel else TEXT, bold=sel)
    cx = x + 1
    for text, colour in gives:
        g.put(cx, top + 2, text, CYAN if (sel and colour == SOCKET) else colour)
        cx += len(text)


def hline(g, a, b, ox, oy):
    """A line from tile a's right edge to tile b's left edge, on a's middle row."""
    ax, ay, aw, ah = POS[a]
    bx = POS[b][0]
    y = oy + ay + ah // 2
    g.put(ox + ax + aw - 1, y, "├", BORDER)
    for x in range(ox + ax + aw, ox + bx):
        g.put(x, y, "─", BORDER)
    g.put(ox + bx, y, "┤", BORDER)


def vline(g, a, b, ox, oy, x):
    """A line from tile a's bottom edge down to tile b's top edge, in column x."""
    ax, ay, aw, ah = POS[a]
    by = POS[b][1]
    g.put(ox + x, oy + ay + ah - 1, "┬", BORDER)
    for y in range(oy + ay + ah, oy + by):
        g.put(ox + x, y, "│", BORDER)
    g.put(ox + x, oy + by, "┴", BORDER)


def graph(g, ox, oy, third=False):
    keys = [k for k in POS if third or k not in ("xarm", "xhand")]
    for k in keys:
        tile(g, k, ox, oy)
    hline(g, "head", "face", ox, oy)
    hline(g, "lhand", "larm", ox, oy)
    hline(g, "larm", "torso", ox, oy)
    hline(g, "torso", "rarm", ox, oy)
    hline(g, "rarm", "rhand", ox, oy)
    hline(g, "legs", "feet", ox, oy)
    vline(g, "head", "torso", ox, oy, SPINE)
    vline(g, "waist", "legs", ox, oy, SPINE)
    # under the torso: down to the waist, the back (and a third arm) branching off
    ty = oy + 13
    g.put(ox + SPINE, oy + 12, "┬", BORDER)
    g.put(ox + SPINE, ty, "┼" if third else "┤", BORDER)
    g.put(ox + SPINE, oy + 14, "┴", BORDER)
    g.put(ox + BACK_J, ty, "┌", BORDER)
    for x in range(ox + BACK_J + 1, ox + SPINE):
        g.put(x, ty, "─", BORDER)
    g.put(ox + BACK_J, oy + 14, "┴", BORDER)
    if third:
        for x in range(ox + SPINE + 1, ox + ARM3_J):
            g.put(x, ty, "─", BORDER)
        g.put(ox + ARM3_J, ty, "┐", BORDER)
        g.put(ox + ARM3_J, oy + 14, "┴", BORDER)
        hline(g, "xarm", "xhand", ox, oy)


def frame(third=False):
    w, h = 120, 40
    orig = Grid.from_svg(os.path.join(HERE, "gear_and_inventory_rack_120x40.svg"), w, h)
    g = Grid.from_svg(os.path.join(HERE, "gear_and_inventory_rack_120x40.svg"), w, h)
    g.fill(1, 10, w - 2, 27, BG)
    for y in range(10, 37):
        g.put(SEP, y, "║", BORDER)
    # left: the view switch, then the body
    g.put(2, 10, "▌", CYAN)
    g.put(3, 10, "MEAT", CYAN, bold=True)
    g.put(7, 10, "▐", CYAN)
    g.put(10, 10, "CHROME", LABEL)
    g.put(SEP - 11, 10, "c", CYAN)
    g.put(SEP - 9, 10, "switch", LABEL)
    g.put(1, 11, "─" * (SEP - 1), BORDER)
    graph(g, 2, 12, third)
    # right: the pack, the weight, the card with its drawing
    x = SEP + 2
    g.paste(orig.crop(60, 10, 39, 1), x, 10)
    rows = [
        ("group", "weapons"), ("item", "/", "1× Scrap nine"), ("item", "/", "1× Whisper SMG"),
        ("group", "clothes"), ("item", "▣", "1× Ballistic weave"), ("item", "▣", "1× Tread boots"),
        ("item", "▣", "1× Utility belt"),
        ("group", "cyberware"), ("item", "◈", "1× Neural port"), ("item", "◈", "1× Spider mount"),
    ]
    for i, r in enumerate(rows):
        yy = 11 + i
        if r[0] == "group":
            line = "── %s " % r[1]
            g.put(x + 1, yy, line + "─" * (w - 3 - x - len(line)), LABEL)
        else:
            g.put(x + 3, yy, r[1], CYAN_DIM)
            g.put(x + 6, yy, r[2], TEXT)
    g.put(SEP, 21, "╠", BORDER)
    g.put(SEP + 1, 21, "═" * (w - 2 - SEP), BORDER)
    g.put(97, 21, " 68.1 kg · ¤ 108359 ", LABEL)
    g.put(x, 22, "RATCHET SMG", CYAN, bold=True)
    g.put(x, 23, "weapon · right hand · main · 3.9 kg · ¤ 2150", LABEL)
    g.put(x, 24, "■ Hushcan · cool +1", CYAN)
    g.put(x, 25, "■ Redeye sight · reflex +1", CYAN)
    g.put(x, 26, "4 damage × 3 rounds · range 12", LABEL)
    g.put(x, 27, "off hand: Monowire shiv, 1 in 4 to follow up", LABEL)
    g.paste(orig.crop(61, 27, 43, 8), x + 6, 29)
    g.put(104, 36, "space inspect", EMPTY)
    g.fill(1, 38, w - 2, 1, BG)
    g.put(2, 38, "←↑↓→ body, then over to the pack · c meat/chrome · [ ] filter · space act · Q/E tabs · tab close", LABEL)
    return g


PAGE = """<title>Equipment window</title>
<meta name="claude:viewport-fit-sides" content="cover">
<style>%s
/* One look: the game's own dark terminal palette. Mocks scroll sideways in their own frame. */
:root{--bg:#0b0f15;--fg:#cdd4de;--muted:#a0aab9;--faint:#5c6673;--accent:#00e5ff;color-scheme:dark}
body{background:var(--bg);color:var(--fg);font:15px/1.55 -apple-system,system-ui,sans-serif;
 padding-block:24px 40px;padding-left:max(16px,env(safe-area-inset-left,0px));padding-right:max(16px,env(safe-area-inset-right,0px))}
h1{font-size:21px;margin:0 0 6px;text-wrap:balance}
p{max-width:70ch;margin:4px 0 12px;color:var(--muted)}
.row{display:flex;gap:22px;flex-wrap:wrap;align-items:flex-start}
.shot{min-width:0;max-width:100%%;overflow-x:auto}
.cap{font-size:12px;color:var(--faint);margin:6px 0 4px;letter-spacing:.04em;text-transform:uppercase}
</style>
<h1>GEAR &amp; INVENTORY: the body as slots</h1>
<p>The body takes the left half, full height: one tile per part, joined by lines the way the parts
join. Each tile holds the item's name on two lines and, dim under it, what it gives: HP or carry for
clothes, a stat for shades, damage &times; rounds and its mods for a gun (&#9679; main hand, &#9675; off
hand), the charge for the scrambler, the slots and sockets for the deck. The cursor's tile is lit; the
card on the right shows the item in full, with its drawing. <b>c</b> switches the tiles to the chrome in
each part (#103). A third arm from chrome branches off the torso with its own hand.</p>
<div class=row>%s</div>
"""


def main():
    out = []
    for third, cap in [(False, "120 &times; 40"), (True, "later: a third arm from chrome")]:
        g = frame(third)
        out.append("<div class=shot><div class=cap>%s</div>%s</div>" % (cap, term.render(g, 12.5)))
    path = os.path.join(HERE, "equipment.html")
    open(path, "w").write(PAGE % (term.CSS, "\n".join(out)))
    print(path)


if __name__ == "__main__":
    main()
