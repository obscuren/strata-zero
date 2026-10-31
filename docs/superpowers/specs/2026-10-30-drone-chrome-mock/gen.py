"""Drone chrome mockups, round 1: the Drone cradle's bay folded out in the ledger, and
four animated drone glyphs following the @ around the lot.

    SHOTS=dir/ python3 gen.py out.html

SHOTS holds the shot harness's SVGs: attributes_ledger_limb_100x30 and a throwaway
tmp_street_nodev (the COMBAT lot, three steps east, dev off)."""

import json
import os
import sys

sys.dont_write_bytecode = True
REPO = os.environ.get("REPO", ".")
sys.path.insert(0, os.path.join(REPO, "docs/superpowers/specs/2026-10-19-gear-inventory-mock"))
sys.path.insert(0, os.path.join(REPO, "docs/superpowers/specs/2026-10-29-stats-chrome-art"))
from cells import CSS, Grid, fold  # noqa: E402
import implantart as A  # noqa: E402

S = os.environ.get("SHOTS", "shots/")

CYAN = "#00e5ff"
CYAN_DK = "#00a5b8"
MAGENTA = "#ff00c8"
DIM = "#a0aab9"
FAINT = "#5c6673"
TEXT = "#cdd4de"
PANEL = "#04060a"
RULE = "#283c4b"
BAR = "#003c46"
GOOD = "#50e678"
AMBER = "#ffc850"

# Scout's own light (its Wave), the drone's lamp.
SCOUT = "#78ff78"
SCOUT_GLOW = "#c8ffc8"
SCOUT_DK = "#2a8a3a"

LEDGER_X, LEDGER_W = 47, 19     # the ledger column, the rule at 66
CARD_X, CARD_W = 68, 31         # the card, up to the frame at 99
TOP, BOTTOM = 6, 26             # the pane's rows

# ---------------------------------------------------------------- art


def art(rows, colours):
    assert len(rows) == 12 and all(len(r) == 30 for r in rows), [len(r) for r in rows]
    return [[None if ch == "." else colours[ch] for ch in r] for r in rows]


STEEL = {str(i): c for i, c in enumerate([None, A.G1, A.G2, A.G3, A.G4, A.G5, A.G6, A.G7, A.G8, A.G9])}
STEEL.update({"d": A.DEEP, "g": SCOUT, "G": SCOUT_GLOW, "k": SCOUT_DK, "w": A.SHINE})

SCOUT_DRONE = art([
    "..............................",
    ".2468886421.......2468886421..",
    ".....57...............57......",
    ".....6888888888888888886......",
    ".........479999999974.........",
    "........36888dgGgd8863........",
    ".........2455555555542........",
    ".............3kGk3............",
    "..........3........3..........",
    "........4666......6664........",
    "..............................",
    "..............................",
], STEEL)

# The cradle alone: the drone that sat in its drawing is the bay's now.
CRADLE_C = {
    "a": "#5a6676", "b": "#8a96a6", "c": "#a8b4c0", "d": "#c8d4e0", "e": "#d8e2ec",
    "h": CYAN, "i": "#6a7686", "j": "#0a8aa0", "k": "#4a5666",
}
CRADLE = art([
    "..............................",
    "..............................",
    "......dc..............cd......",
    "......ci..............ci......",
    "......ci..............ci......",
    "......ci..............ci......",
    "......ci.....bhhb.....ci......",
    "......ci.....aiia.....ci......",
    "......ci......ii......ci......",
    "...eeeeeeeeeeeeeeeeeeeeeeee...",
    "...kiiiiiiiiiiiiiiiiiiiiiiik..",
    "..............................",
], CRADLE_C)


def draw_art(g, x, y, pixels):
    cells = fold(pixels, PANEL)
    g.paste(cells, x, y)


# ---------------------------------------------------------------- the ledger

LEDGER = [
    ("group", "head"),
    ("site", "Neural port"),
    ("site", "Target lens"),
    ("empty", "ears"),
    ("site", "Bulldog jaw"),
    ("group", "body"),
    ("site", "Redline spine"),
    ("site", "Drone cradle"),
    ("bay", "└", "Scout drone"),
    ("site", "Adrenal pump"),
    ("site", "Subdermal mesh"),
    ("group", "limbs"),
    ("site", "Piston arm"),
    ("sock", "├", "Arc hand"),
    ("sock", "└", "□ forearm"),
    ("empty", "r arm"),
    ("empty", "legs"),
]


def clear_pane(g):
    for y in range(TOP, BOTTOM + 1):
        g.fill(LEDGER_X, y, LEDGER_W, 1, PANEL)
        g.fill(CARD_X, y, CARD_W, 1, PANEL)


def ledger(g, cursor, bay_empty=False):
    for n, row in enumerate(LEDGER):
        y = TOP + n
        on = n == cursor
        if on:
            g.fill(48, y, 18, 1, BAR)
            g.put(48, y, "▸", CYAN_DK, bold=True)
        kind = row[0]
        if kind == "group":
            g.put(49, y, "── %s ──" % row[1], FAINT)
        elif kind == "site":
            g.put(50, y, row[1], CYAN if on else TEXT, bold=on)
        elif kind == "empty":
            g.put(50, y, "· " + row[1], CYAN_DK if on else FAINT, bold=on)
        else:
            g.put(51, y, row[1], RULE)
            name = row[2]
            if kind == "bay" and bay_empty:
                name = "□ bay"
            empty = name.startswith("□")
            fg = (CYAN_DK if empty else CYAN) if on else (FAINT if empty else DIM)
            g.put(53, y, name, fg, bold=on)


def wrap(s, w):
    out, line = [], ""
    for word in s.split():
        if len(line) + len(word) + (1 if line else 0) > w:
            out.append(line)
            line = word
        else:
            line = (line + " " + word) if line else word
    out.append(line)
    return out


def spans(g, x, y, parts):
    for text, fg, *bold in parts:
        g.put(x, y, text, fg, bold=bool(bold and bold[0]))
        x += len(text)


def hints(g, left, right):
    g.fill(1, 28, 98, 1, PANEL)
    g.put(2, 28, left, DIM)
    g.put(98 - len(right), 28, right, DIM)


def base():
    g = Grid.from_svg(os.path.join(S, "attributes_ledger_limb_100x30.svg"), 100, 30)
    clear_pane(g)
    return g


def on_cradle():
    g = base()
    ledger(g, 7)
    draw_art(g, CARD_X, TOP, CRADLE)
    g.put(CARD_X, 13, "DRONE CRADLE", CYAN, bold=True)
    spans(g, CARD_X, 14, [("BACK · load 3 · ", DIM), ("EMP −2", MAGENTA)])
    g.put(CARD_X, 15, "a bay for one drone", DIM)
    spans(g, CARD_X, 17, [("■ bay     ", CYAN), ("Scout drone", TEXT, True)])
    spans(g, CARD_X, 18, [("          grants ", DIM), ("Scout", CYAN)])
    for i, line in enumerate(wrap("A cradle bolted between the shoulder blades and wired "
                                  "to the spine. Whatever docks in it flies for you.", 30)):
        g.put(CARD_X, 21 + i, line, TEXT)
    hints(g, "↑↓ select · ← attributes · Q/E tabs · tab close",
          "chrome goes in and out in a ripperdoc's chair")
    return g


def on_drone():
    g = base()
    ledger(g, 8)
    draw_art(g, CARD_X, TOP, SCOUT_DRONE)
    g.put(CARD_X, 13, "SCOUT DRONE", CYAN, bold=True)
    spans(g, CARD_X, 14, [("docked in the ", DIM), ("Drone cradle", TEXT)])
    spans(g, CARD_X, 15, [("grants ", DIM), ("Scout", CYAN)])
    g.put(CARD_X, 17, "out with you · follows on foot", FAINT)
    for i, line in enumerate(wrap("A drone the size of a sparrow. It goes up, it comes "
                                  "back, and the floor is on your map.", 30)):
        g.put(CARD_X, 21 + i, line, TEXT)
    hints(g, "↑↓ select · space swap · x stow in the bag · ← attributes · Q/E tabs",
          "drones dock anywhere")
    return g


def swapping():
    g = base()
    ledger(g, 8)
    g.put(CARD_X, 6, "DRONE BAY", CYAN, bold=True)
    spans(g, CARD_X, 7, [("a bay on the ", DIM), ("Drone cradle", TEXT)])
    spans(g, CARD_X, 9, [("FITS ", CYAN, True), ("───────────", RULE), (" from your bag", FAINT)])
    picks = [("Mule drone", "carry +8"), ("Lamp drone", "sight +3")]
    for i, (name, does) in enumerate(picks):
        y = 10 + i
        if i == 0:
            g.fill(CARD_X, y, CARD_W - 1, 1, BAR)
            g.put(CARD_X, y, "▸", CYAN_DK, bold=True)
        g.put(CARD_X + 2, y, name, CYAN if i == 0 else TEXT, bold=i == 0)
        g.put(CARD_X + 29 - len(does), y, does, GOOD)
    spans(g, CARD_X, 13, [("swaps out ", DIM), ("Scout drone", TEXT), (" to the bag", DIM)])
    hints(g, "↑↓ choose · space dock it · esc back · Q/E tabs · tab close",
          "drones dock anywhere")
    return g


# ---------------------------------------------------------------- the lot

FLOOR = ("·", "#4b413d", "#413630", False)


def lot():
    g = Grid.from_svg(os.path.join(S, "tmp_street_nodev.svg"), 100, 30)
    g = g.crop(9, 6, 41, 19)
    for y in range(g.h):
        for x in range(g.w):
            ch, fg, bg, _ = g.c[y][x]
            # the dummy and the goon go; the lot is yours and the drone's
            if ch in "&@":
                g.c[y][x] = list(FLOOR)
    return g


GLYPHS = [
    {
        "key": "a", "name": "Rotor",
        "about": "Two box strokes swapped, a rotor seen from above. Spins twice as fast "
                 "while it flies to catch up.",
        "frames": ["┼", "╳"], "colours": [SCOUT, SCOUT], "idle": 2, "moving": 1,
    },
    {
        "key": "b", "name": "Blade",
        "about": "One blade turning through four box strokes. Faster on the move.",
        "frames": ["─", "╲", "│", "╱"], "colours": [SCOUT] * 4, "idle": 2, "moving": 1,
    },
    {
        "key": "c", "name": "Hover",
        "about": "Half blocks: the drone bobs in its cell, top half then bottom. "
                 "Bobs faster on the move.",
        "frames": ["▀", "▄"], "colours": [SCOUT, SCOUT], "idle": 4, "moving": 2,
    },
    {
        "key": "d", "name": "Nav light",
        "about": "A dot whose lamp breathes bright to dark, with a red tail-light wink "
                 "once a cycle.",
        "frames": ["•", "•", "•", "∙", "∙", "•", "∙"],
        "colours": [SCOUT_GLOW, SCOUT, SCOUT_DK, SCOUT_DK, "#ff4f5e", SCOUT, SCOUT_DK],
        "idle": 2, "moving": 1,
    },
]


def lot_js(g):
    return json.dumps([[c[0], c[1], c[2], c[3]] for row in g.c for c in row])


PAGE = """
<style>
%(css)s
body{background:#0b0e14}
h2,h3{color:#cdd4de}
.cols{display:flex;flex-direction:column;gap:22px}
.row{display:flex;flex-wrap:wrap;gap:20px;align-items:flex-start}
.note{max-width:860px;font-size:14px;line-height:1.5;color:#cdd4de;margin:6px 0 10px}
.note b{color:#00e5ff}
.lots{display:flex;flex-wrap:wrap;gap:18px}
.lotbox{background:#11151d;border:1px solid #1a2230;border-radius:6px;padding:10px 12px;width:440px;cursor:pointer}
.lotbox.selected{outline:2px solid #00e5ff}
.lotbox h3{margin:2px 0 4px;color:#00e5ff;font-size:15px}
.lotbox p{margin:6px 0 0;font-size:13px;line-height:1.45;color:#a0aab9}
.strip{font-family:'JetBrains Mono',Menlo,monospace;font-size:20px;letter-spacing:6px;margin:2px 0 6px}
.state{font-family:'JetBrains Mono',Menlo,monospace;font-size:12px;color:#5c6673;margin-top:4px}
.tag{display:inline-block;background:#ffc850;color:#000;font-weight:700;border-radius:4px;padding:0 7px;margin-right:6px}
</style>

<h2>Drone chrome, round 1</h2>
<p class="note">Your <b>Drone cradle</b> on BACK folds out a <b>bay</b> under it in the ledger, the
way an arm folds out its hand and forearm. The bay holds one drone; the drone decides what you get
(here the Scout drone carries Scout, which the cradle used to grant). The cradle still goes in at a
ripperdoc; the drone swaps anywhere. <span class="tag">guess</span>drones weigh in the bag but take
no LOAD and cost no EMP: they're not in your body.</p>

<div class="cols">
 <div>
  <h3>1 · On the cradle: the bay folded under it</h3>
  %(cradle)s
 </div>
 <div>
  <h3>2 · On the bay: the drone's own card</h3>
  %(drone)s
 </div>
 <div>
  <h3>3 · Space on the bay: what docks, from the bag</h3>
  <p class="note"><span class="tag">placeholder</span>Mule and Lamp are examples of drones that only use
  what the game already does (carry, sight); which drones ship is a later question.</p>
  %(swap)s
 </div>
</div>

<h2 style="margin-top:34px">The drone on the street: four glyphs</h2>
<p class="note">Each lot runs the same walk: the @ stands, the drone drifts within two tiles of you;
the @ walks east, the drone trails and catches up; the @ stands again. All drawn in the Scout
drone's light (each drone would carry its own). Click the one you like.</p>
<div class="lots options">%(lots)s</div>

<script>
const CELLS = %(cells)s, W = %(w)d, H = %(h)d;
const GLYPHS = %(glyphs)s;
const PX = 14 * 1.0;
function mk(box){
  const t = document.createElement('div'); t.className='term'; t.style.fontSize=PX+'px';
  const spans=[];
  for(let y=0;y<H;y++){const r=document.createElement('div');r.className='r';
    for(let x=0;x<W;x++){const s=document.createElement('span');const c=CELLS[y*W+x];
      s.textContent=c[0];s.style.color=c[1];s.style.background=c[2];if(c[3])s.style.fontWeight=700;
      r.appendChild(s);spans.push(s);}
    t.appendChild(r);}
  box.querySelector('.slot').appendChild(t); return spans;
}
function floor(x,y){const c=CELLS[y*W+x];return c && (c[0]==='·'||c[0]==='⠂'||c[0]==='⠈'||c[0]==='⠐'||c[0]==='⠠'||c[0]==='⡀'||c[0]==='⢀'||c[0]==='⠄');}
// The walk: stand, east 11 tiles, stand, west back, in 100 ms beats.
const START={x:9,y:8}, PLAN=[];
for(let i=0;i<40;i++)PLAN.push(0);
for(let i=0;i<11;i++){PLAN.push(1);PLAN.push(0);}
for(let i=0;i<40;i++)PLAN.push(0);
for(let i=0;i<11;i++){PLAN.push(-1);PLAN.push(0);}
function run(box,g){
  const spans=mk(box); const state=box.querySelector('.state');
  let seed=7, beat=0, p={...START}, d={x:START.x-2,y:START.y-1}, frame=0, wait=6, sub=0;
  const rnd=()=>{seed=(seed*1103515245+12345)&0x7fffffff;return seed/0x7fffffff;};
  const cheb=(a,b)=>Math.max(Math.abs(a.x-b.x),Math.abs(a.y-b.y));
  let prevP={...p}, prevD={...d};
  function paint(pos,ch,fg,bold){const s=spans[pos.y*W+pos.x];s.textContent=ch;s.style.color=fg;s.style.fontWeight=bold?700:400;}
  function restore(pos){const c=CELLS[pos.y*W+pos.x];paint(pos,c[0],c[1],c[3]);}
  setInterval(()=>{
    const step=PLAN[beat%%PLAN.length]; beat++;
    if(step){p.x+=step;}
    const walking = PLAN.slice(Math.max(0,(beat%%PLAN.length)-3),(beat%%PLAN.length)+1).some(v=>v!==0);
    let moving=false;
    if(cheb(p,d)>2 || (p.x===d.x&&p.y===d.y)){
      // catch up: step toward the tile behind you
      const goal={x:p.x-(step>0?1:step<0?-1:0)*1, y:p.y+(d.y<p.y?-1:1)};
      const nx=d.x+Math.sign(goal.x-d.x), ny=d.y+Math.sign(goal.y-d.y);
      if(floor(nx,ny)&&!(nx===p.x&&ny===p.y)){d={x:nx,y:ny};}
      moving=true; wait=4+Math.floor(rnd()*6);
    } else if(--wait<=0){
      // idle: drift a tile, staying within two of you
      const opts=[];
      for(let dy=-1;dy<=1;dy++)for(let dx=-1;dx<=1;dx++){
        const q={x:d.x+dx,y:d.y+dy};
        if((dx||dy)&&floor(q.x,q.y)&&cheb(q,p)<=2&&cheb(q,p)>=1)opts.push(q);}
      if(opts.length){d=opts[Math.floor(rnd()*opts.length)];moving=true;}
      wait=5+Math.floor(rnd()*8);
    }
    const every = (moving||walking)? g.moving : g.idle;
    if(++sub>=every){sub=0;frame=(frame+1)%%g.frames.length;}
    restore(prevP); restore(prevD);
    paint(d,g.frames[frame],g.colours[frame],true);
    paint(p,'@','#ffffff',true);
    prevP={...p}; prevD={...d};
    state.textContent = walking? 'walking · the drone follows' : 'standing · the drone drifts';
  },100);
}
document.querySelectorAll('.lotbox').forEach((box,i)=>run(box,GLYPHS[i]));
</script>
"""


def lotbox(gl):
    strip = " ".join('<span style="color:%s">%s</span>' % (c, f)
                     for f, c in zip(gl["frames"], gl["colours"]))
    return ('<div class="lotbox option" data-choice="%s" onclick="toggleSelect(this)">'
            '<h3>%s · %s</h3><div class="strip">%s</div><div class="slot"></div>'
            '<div class="state"></div><p>%s</p></div>'
            % (gl["key"], gl["key"].upper(), gl["name"], strip, gl["about"]))


def main(out):
    g = lot()
    page = PAGE % {
        "css": CSS,
        "cradle": on_cradle().html(0.86),
        "drone": on_drone().html(0.86),
        "swap": swapping().html(0.86),
        "lots": "".join(lotbox(gl) for gl in GLYPHS),
        "cells": lot_js(g), "w": g.w, "h": g.h,
        "glyphs": json.dumps(GLYPHS),
    }
    open(out, "w").write(page)


if __name__ == "__main__":
    main(sys.argv[1])
