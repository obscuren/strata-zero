"""Job tracking, map round 1: the cyberdeck's MAP redrawn as a half-block city map with
places to show, one picked, and its walk tracked in the floor glow Jeffrey chose (C).
Three layouts, animated at 30 ticks a second.

    SHOTS=dir/ python3 -I gen2.py out.html

SHOTS holds tmp_mapdump.json (the Street of seed 8 as tiles, districts, buildings,
fixers, your memory of the walk from home to Marlo's body) and tmp_deckmap_120x40.svg
(the deck on MAP as it is today), from the throwaway tmp_mapdump test in
screens/shots.rs."""

import heapq
import json
import os
import sys
from collections import Counter, deque

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "2026-10-19-gear-inventory-mock"))
from cells import Grid, CSS  # noqa: E402

S = os.environ.get("SHOTS", "shots/")
W, H = 120, 40

CYAN = "#00e5ff"
MAGENTA = "#ff00c8"
AMBER = "#ffc850"
RED = "#ff4f5e"
GREEN = "#7cff9a"
PINK = "#ff8ad8"
TEXT = "#cdd4de"
DIM = "#a0aab9"
FAINT = "#5c6673"
PANEL = "#04060a"
SEL = "#14202c"

WALK = set("rsapfDbkc")


def hexrgb(c):
    return tuple(int(c[i:i + 2], 16) for i in (1, 3, 5))


def rgbhex(t):
    return "#%02x%02x%02x" % tuple(max(0, min(255, round(v))) for v in t)


def scale(c, f):
    return rgbhex(tuple(v * f for v in (hexrgb(c) if isinstance(c, str) else c)))


def mix(a, b, f):
    a, b = hexrgb(a), hexrgb(b)
    return rgbhex(tuple(a[i] + (b[i] - a[i]) * f for i in range(3)))


# ---------------------------------------------------------------- the city

J = json.load(open(os.path.join(S, "tmp_mapdump.json")))
CW, CH = J["w"], J["h"]
TER, DIS, VIS = J["terrain"], J["district"], J["vis"]
DISTRICTS = {d["id"]: d for d in J["districts"]}


def walkable(x, y):
    return 0 <= x < CW and 0 <= y < CH and TER[y][x] in WALK


# You stand outside your block's door, the deck open: the clinic is a real walk away.
_home = next(b for b in J["buildings"] if b["id"] == J["home"])
PLAYER = next((_home["door"][0] + dx, _home["door"][1] + dy)
              for dx, dy in ((0, -1), (0, 1), (-1, 0), (1, 0))
              if walkable(_home["door"][0] + dx, _home["door"][1] + dy))


def bfs(start):
    dist = {start: 0}
    q = deque([start])
    while q:
        x, y = q.popleft()
        for n in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
            if n not in dist and walkable(*n):
                dist[n] = dist[(x, y)] + 1
                q.append(n)
    return dist


def astar(a, b):
    def h(p):
        return abs(p[0] - b[0]) + abs(p[1] - b[1])
    seen, came = {a: 0}, {}
    heap = [(h(a), 0, a)]
    while heap:
        _, g, p = heapq.heappop(heap)
        if p == b:
            out = [p]
            while p in came:
                p = came[p]
                out.append(p)
            return out[::-1]
        x, y = p
        for n in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
            if walkable(*n) and seen.get(n, 1 << 30) > g + 1:
                seen[n] = g + 1
                came[n] = p
                heapq.heappush(heap, (g + 1 + h(n), g + 1, n))
    return None


DIST = bfs(PLAYER)


def near_walk(p):
    """The walkable tile a place is reached at: itself, else the nearest beside it."""
    if walkable(*p):
        return p
    best = None
    for dy in range(-2, 3):
        for dx in range(-2, 3):
            q = (p[0] + dx, p[1] + dy)
            if walkable(*q) and q in DIST and (best is None or DIST[q] < DIST[best]):
                best = q
    return best or p


# Places worth a marker, by kind: (key, glyph, colour, label).
KINDS = [
    ("story", "◆", MAGENTA, "jobs & story"),
    ("home", "⌂", GREEN, "home"),
    ("doc", "+", RED, "ripperdocs"),
    ("fixer", "$", AMBER, "fixers"),
    ("bar", "Y", PINK, "bars & clubs"),
    ("noodle", "≈", "#ffb070", "noodle bars"),
    ("shop", "▪", "#8fd0ff", "shops & markets"),
    ("pawn", "¢", "#e0c070", "pawnshops"),
    ("arcade", "▣", "#b080ff", "arcades"),
    ("hotel", "H", "#c0c8d8", "hotels"),
]
KIND = {k[0]: k for k in KINDS}
BKIND = {"bar": "bar", "club": "bar", "noodle bar": "noodle", "shop": "shop", "market": "shop",
         "pawn shop": "pawn", "arcade": "arcade", "hotel": "hotel"}

PLACES = []


def place(kind, name, tile, what, district):
    at = near_walk(tile)
    PLACES.append({"kind": kind, "name": name, "tile": tile, "at": at, "what": what,
                   "district": DISTRICTS[district]["name"] if district is not None else "",
                   "m": DIST.get(at)})


def district_at(p):
    c = DIS[p[1]][p[0]]
    return None if c == "." else ord(c) - ord("a")


place("story", "Marlo", tuple(J["body"]), "the drop · story", district_at(tuple(J["body"])))
for b in J["buildings"]:
    door = tuple(b["door"]) if b["door"] else (b["fp"][0] + b["fp"][2] // 2, b["fp"][1] + b["fp"][3] // 2)
    if b["id"] == J["home"]:
        place("home", "home · " + b["name"], door, "your flat, 18F", b["district"])
    elif b["clinic"]:
        place("doc", b["name"], door, "ripperdoc · front: " + b["kind"], b["district"])
    elif b["kind"] in BKIND:
        place(BKIND[b["kind"]], b["name"], door, b["kind"], b["district"])
for f in J["fixers"]:
    if f["work"] == "spot":
        t = (f["at"] % 10000, f["at"] // 10000)
        place("fixer", f["name"], t, "fixer · street corner", district_at(t))

COUNTS = Counter(p["kind"] for p in PLACES)
TRACKED = next(p for p in PLACES if p["name"] == "Miracorp Tattoo")
PATH = astar(PLAYER, TRACKED["at"])


# ---------------------------------------------------------------- the map in pixels

def pixel(ox, oy, s, px, py):
    """The colour of one half-block pixel covering s x s tiles from (ox, oy)."""
    x0, y0 = int(ox + px * s), int(oy + py * s)
    x1, y1 = max(x0 + 1, int(ox + (px + 1) * s)), max(y0 + 1, int(oy + (py + 1) * s))
    cls, dis, vis = Counter(), Counter(), 0
    n = 0
    for y in range(y0, min(y1, CH)):
        for x in range(x0, min(x1, CW)):
            if x < 0 or y < 0:
                continue
            n += 1
            t = TER[y][x]
            cls["B" if t in "BD" else "r" if t == "r" else "w" if t == "w" else "s" if t in WALK else " "] += 1
            if DIS[y][x] != ".":
                dis[DIS[y][x]] += 1
            vis = max(vis, int(VIS[y][x]))
    if n == 0:
        return PANEL
    d = DISTRICTS[ord(dis.most_common(1)[0][0]) - ord("a")] if dis else None
    if cls["B"] * 2 > n:
        kind = "B"
    elif cls["r"]:
        kind = "r"
    elif cls["s"]:
        kind = "s"
    elif cls["w"] * 2 >= n:
        kind = "w"
    else:
        kind = " "
    if kind == " ":
        return PANEL
    if kind == "w":
        base = (14, 40, 72)
    elif d is None:
        base = (40, 44, 52)
    elif kind == "B":
        base = d["roof"]
    elif kind == "r":
        base = tuple(v * 1.9 for v in d["road"])
    else:
        base = tuple(v * 1.15 for v in d["sidewalk"])
    if vis == 0:
        # The city grid off the net: the shape of the place, faint.
        return mix(PANEL, rgbhex(base), 0.32 if kind != "B" else 0.22)
    return scale(rgbhex(base), 1.0 if vis == 1 else 1.3)


class MapView:
    """The map drawn into cells (x0, y0, cols, rows) at s tiles a pixel from (ox, oy)."""

    def __init__(self, g, x0, y0, cols, rows, s, ox, oy):
        self.g, self.x0, self.y0, self.cols, self.rows, self.s, self.ox, self.oy = g, x0, y0, cols, rows, s, ox, oy
        self.px = {}
        for cy in range(rows):
            for cx in range(cols):
                t = pixel(ox, oy, s, cx, 2 * cy)
                b = pixel(ox, oy, s, cx, 2 * cy + 1)
                self.px[(cx, 2 * cy)] = t
                self.px[(cx, 2 * cy + 1)] = b
                g.c[y0 + cy][x0 + cx] = ["▀", t, b, False] if t != b else [" ", t, t, False]

    def where(self, tile):
        """The pixel of a tile, or None off the view."""
        px = int((tile[0] - self.ox) / self.s)
        py = int((tile[1] - self.oy) / self.s)
        if 0 <= px < self.cols and 0 <= py < 2 * self.rows:
            return px, py
        return None

    def cell(self, tile):
        p = self.where(tile)
        return None if p is None else (self.x0 + p[0], self.y0 + p[1] // 2)

    def mark(self, tile, ch, fg, bold=True):
        c = self.cell(tile)
        if c is None:
            return None
        x, y = c
        cx, cy = x - self.x0, y - self.y0
        bg = mix(self.px[(cx, 2 * cy)], self.px[(cx, 2 * cy + 1)], 0.5)
        self.g.c[y][x] = [ch, fg, scale(bg, 0.45), bold]
        return c

    def label(self, tile, text, fg, taken):
        """A name over the map, on the nearest row where it covers no marker."""
        c = self.cell(tile)
        if c is None:
            return
        x = max(self.x0, min(c[0] - len(text) // 2, self.x0 + self.cols - len(text)))
        for dy in (0, 1, -1, 2, -2, 3, -3):
            y = c[1] + dy
            if not (self.y0 <= y < self.y0 + self.rows):
                continue
            if any((x + k, y) in taken for k in range(-1, len(text) + 1)):
                continue
            for k, ch in enumerate(text):
                cell = self.g.c[y][x + k]
                cell[0], cell[1] = ch, fg
            return

    def anim(self, path, target, marks, mask=()):
        """What the page animates: the path's pixels by index, the ripple's pixels by
        distance from the target, and the cells whose glyph must stay. Nothing animates
        under `mask` (cells a card covers)."""
        keep = set(marks)
        mask = set(mask)

        def hidden(p):
            return (self.x0 + p[0], self.y0 + p[1] // 2) in mask

        px, seen = [], set()
        for i, t in enumerate(path):
            p = self.where(t)
            if p is None or p in seen or hidden(p):
                continue
            seen.add(p)
            px.append([p[0], p[1], i])
        ring = []
        tp = self.where(target)
        if tp:
            for dy in range(-7, 8):
                for dx in range(-7, 8):
                    q = (tp[0] + dx, tp[1] + dy)
                    if q in self.px and not hidden(q):
                        ring.append([q[0], q[1], (dx * dx + dy * dy) ** 0.5])
        return {"x0": self.x0, "y0": self.y0, "px": {f"{k[0]},{k[1]}": v for k, v in self.px.items()},
                "path": px, "ring": ring, "keep": [list(k) for k in keep], "n": len(path)}


# ---------------------------------------------------------------- the deck around it

def deck():
    g = Grid.from_svg(os.path.join(S, "tmp_deckmap_120x40.svg"), W, H)
    hint_y = next(y for y in range(H) if "Q/E tabs" in "".join(c[0] for c in g.c[y]))
    body_top, body_bottom = 10, hint_y - 1
    g.fill(1, body_top, W - 2, body_bottom - body_top, PANEL, " ", TEXT)
    for x in range(1, W - 1):
        g.c[hint_y][x] = [" ", FAINT, PANEL, False]
    return g, body_top, body_bottom, hint_y


def hints(g, y, text):
    g.put(2, y, text, FAINT)


def draw_marks(m, shown, sel):
    cells = []
    for p in PLACES:
        if p["kind"] not in shown:
            continue
        _, ch, fg, _ = KIND[p["kind"]]
        c = m.mark(p["tile"], ch, fg)
        if c:
            cells.append(c)
    c = m.mark(PLAYER, "@", "#ffffff")
    cells.append(c)
    return cells


def district_labels(m, marks):
    taken = {c for c in marks if c}
    for d in DISTRICTS.values():
        m.label(tuple(d["center"]), d["name"].upper(), scale(rgbhex(tuple(d["accent"])), 0.55), taken)


def card(g, x, y, w, rows, border=CYAN):
    g.put(x, y, "┌" + "─" * (w - 2) + "┐", border)
    for k, (text, fg) in enumerate(rows):
        g.put(x, y + 1 + k, "│", border)
        g.fill(x + 1, y + 1 + k, w - 2, 1, PANEL, " ", fg)
        g.put(x + 1, y + 1 + k, text[:w - 2], fg)
        g.put(x + w - 1, y + 1 + k, "│", border)
    g.put(x, y + 1 + len(rows), "└" + "─" * (w - 2) + "┘", border)


SHOWN = {"story", "home", "doc", "fixer"}


def concept_a():
    g, top, bottom, hint_y = deck()
    s = 3
    cols, rows = CW // s, CH // (2 * s)
    m = MapView(g, 2, top + 2, cols, rows, s, 0, 0)
    marks = draw_marks(m, SHOWN, TRACKED)
    district_labels(m, marks)
    g.put(2, top, f"{J['city'].upper()} · STREET", CYAN, bold=True)
    g.put(2 + len(J["city"]) + 9, top, "· lit: what you remember · faint: the city grid", FAINT)
    px = 84
    g.put(px, top, "SHOW", CYAN, bold=True)
    for k, (key, ch, fg, label) in enumerate(KINDS):
        y = top + 1 + k
        on = key in SHOWN
        if k == 2:
            g.fill(px - 1, y, W - px, 1, SEL)
        g.put(px, y, "▣" if on else "□", CYAN if on else FAINT, SEL if k == 2 else None)
        g.put(px + 2, y, ch, fg if on else scale(fg, 0.45), SEL if k == 2 else None, bold=on)
        g.put(px + 4, y, label, TEXT if on else FAINT, SEL if k == 2 else None)
        g.put(W - 5, y, "%3d" % COUNTS.get(key, 0), DIM if on else FAINT, SEL if k == 2 else None)
    y = top + 2 + len(KINDS)
    g.put(px - 1, y, "─" * (W - px), "#1a2230")
    g.put(px, y + 1, "SELECTED", CYAN, bold=True)
    g.put(px, y + 2, "+ " + TRACKED["name"], RED, bold=True)
    g.put(px + 2, y + 3, TRACKED["what"], DIM)
    g.put(px + 2, y + 4, f"{TRACKED['district']} · open · {TRACKED['m']} m", DIM)
    g.put(px + 2, y + 6, "⌖ tracking", CYAN, bold=True)
    g.put(px + 13, y + 6, "· T again to stop", FAINT)
    hints(g, hint_y, "Q/E tabs · ←↑↓→ cursor · [ ] next place · space show · T track · +− zoom · tab close")
    return g, m.anim(PATH, TRACKED["tile"], marks)


def concept_b():
    g, top, bottom, hint_y = deck()
    s = 3
    cols, rows = CW // s, CH // (2 * s)
    m = MapView(g, 2, top + 2, cols, rows, s, 0, 0)
    marks = draw_marks(m, SHOWN, TRACKED)
    g.put(2, top, f"{J['city'].upper()} · STREET", CYAN, bold=True)
    px = 84
    x = px
    for k, (key, ch, fg, label) in enumerate(KINDS):
        on = key in SHOWN
        g.put(x, top, ch, fg if on else scale(fg, 0.35), bold=on)
        x += 2 if k < len(KINDS) - 1 else 0
    g.put(x + 2, top, "1-0", FAINT)
    g.put(px - 1, top + 1, "─" * (W - px), "#1a2230")
    rows_ = sorted((p for p in PLACES if p["kind"] in SHOWN and p["m"] is not None), key=lambda p: p["m"])
    y = top + 2
    for p in rows_[:bottom - top - 4]:
        _, ch, fg, _ = KIND[p["kind"]]
        sel = p is TRACKED
        bg = SEL if sel else None
        if sel:
            g.fill(px - 1, y, W - px, 1, SEL)
        g.put(px, y, ch, fg, bg, bold=True)
        g.put(px + 2, y, p["name"][:22], TEXT if sel else DIM, bg, bold=sel)
        g.put(W - 7, y, "%4d m" % p["m"], DIM if sel else FAINT, bg)
        y += 1
        if sel:
            g.fill(px - 1, y, W - px, 2, SEL)
            g.put(px + 2, y, p["what"], DIM, SEL)
            g.put(px + 2, y + 1, "⌖ tracking", CYAN, SEL, bold=True)
            g.put(px + 13, y + 1, "· T stops", FAINT, SEL)
            y += 2
    hints(g, hint_y, "Q/E tabs · ↑↓ place · 1-0 show/hide · T track · enter centre the map · tab close")
    return g, m.anim(PATH, TRACKED["tile"], marks)


def concept_c():
    g, top, bottom, hint_y = deck()
    s = 2
    cols, rows = W - 3, bottom - top - 1
    ox = max(0, min(CW - cols * s, TRACKED["tile"][0] - cols * s // 2))
    oy = max(0, min(CH - rows * 2 * s, TRACKED["tile"][1] - rows * s))
    m = MapView(g, 1, top, cols, rows, s, ox, oy)
    shown = SHOWN | {"bar"}
    marks = draw_marks(m, shown, TRACKED)
    district_labels(m, marks)
    c = m.cell(TRACKED["tile"])
    # Beside the place, on whichever side has room, inside the map.
    cx = c[0] + 3 if c[0] + 3 + 30 <= W - 2 else c[0] - 3 - 30
    cy = max(top + 1, min(c[1] - 2, bottom - 8))
    card(g, cx, cy, 30, [
        ("+ " + TRACKED["name"], RED),
        (TRACKED["what"], DIM),
        (f"{TRACKED['district']} · open · {TRACKED['m']} m", DIM),
        ("⌖ tracking · T stops", CYAN),
    ])
    y = bottom - 1
    g.fill(1, y, W - 2, 1, PANEL)
    x = 2
    for k, (key, ch, fg, label) in enumerate(KINDS):
        on = key in shown
        num = str((k + 1) % 10)
        g.put(x, y, num, FAINT)
        g.put(x + 1, y, ch, fg if on else scale(fg, 0.35), bold=on)
        g.put(x + 3, y, label.split(" ")[0], TEXT if on else FAINT)
        x += 4 + len(label.split(" ")[0]) + 1
    hints(g, hint_y, "Q/E tabs · ←↑↓→ cursor · [ ] next place · 1-0 show · T track · +− zoom · tab close")
    covered = [(x, yy) for yy in range(cy, cy + 6) for x in range(cx, cx + 30)]
    covered += [(x, y) for x in range(1, W - 1)]
    return g, m.anim(PATH, TRACKED["tile"], marks, covered)


CONCEPTS = [
    ("A", "Atlas", concept_a, "The whole city fits on the left, three tiles to a half-block pixel: lit where "
     "you remember it, faint where only the city grid knows it, district names over it. "
     "On the right, what to show (space toggles the row under the cursor), and the place "
     "under the map's cursor. [ and ] jump the cursor to the next shown place."),
    ("B", "Directory", concept_b, "The same map; on the right the shown places as a list, "
     "nearest walk first. The digits toggle kinds (the glyph row on top). Up and down pick a "
     "place; it pulses on the map and T tracks it. Best for 'the nearest ripperdoc'."),
    ("C", "Wide", concept_c, "The map takes the whole deck at two tiles a pixel and pans with "
     "the cursor; the kinds sit along the bottom (digits toggle), and a card opens beside the "
     "place under the cursor. Bars are on here, to show a busy kind."),
]

JS = r"""
const CYAN=[0,229,255], MAG=[255,0,200];
function hx(c){return [parseInt(c.slice(1,3),16),parseInt(c.slice(3,5),16),parseInt(c.slice(5,7),16)]}
function mix(a,b,f){f=Math.max(0,Math.min(1,f));return 'rgb('+a.map((v,i)=>Math.round(v+(b[i]-v)*f)).join(',')+')'}
function mixA(a,b,f){f=Math.max(0,Math.min(1,f));return a.map((v,i)=>Math.round(v+(b[i]-v)*f))}
function css(a){return 'rgb('+a.join(',')+')'}
function build(el, base){
  el.innerHTML='';const spans=[];
  for(let y=0;y<base.length;y++){const r=document.createElement('div');r.className='r';
    for(let x=0;x<base[y].length;x++){const s=document.createElement('span');const c=base[y][x];
      s.textContent=c[0];s.style.color=c[1];s.style.background=c[2];if(c[3])s.style.fontWeight=700;
      r.appendChild(s);spans.push(s);}
    el.appendChild(r);}
  return spans;
}
const views=[];
for(const el of document.querySelectorAll('.term[data-k]')){
  const v=VIEWS[el.dataset.k];
  const keep=new Set(v.anim.keep.map(k=>k[0]+','+k[1]));
  views.push({v,spans:build(el,v.base),keep});
}
let t=0,speed=1,acc=0,last=performance.now();
function frame(now){
  acc+=(now-last)*speed;last=now;
  while(acc>=1000/30){acc-=1000/30;t++}
  for(const view of views){
    const a=view.v.anim, px=new Map();
    const n=a.n, cycle=n+24, head=(t*0.9)%cycle, breathe=0.30+0.08*Math.sin(t/60*Math.PI*2);
    for(const [x,y,i] of a.path){
      const d=head-i; let f=breathe; if(d>=0&&d<12)f=Math.max(breathe,0.95*(1-d/12));
      px.set(x+','+y, mixA(hx(a.px[x+','+y]),CYAN,f));
    }
    const r=(t/14)%4;
    for(const [x,y,dist] of a.ring){
      const ring=Math.max(0,1-Math.abs(dist-r*1.6)/1.2)*(1-r/4);
      const core=dist<0.8?0.4+0.25*Math.sin(t/15*Math.PI):0;
      const f=Math.max(ring*0.7,core); if(f<0.02)continue;
      const k=x+','+y; px.set(k, mixA(px.get(k)||hx(a.px[k]),MAG,f));
    }
    const cells=new Map();
    for(const [k,c] of px){const [x,y]=k.split(',').map(Number);const cy=y>>1;const ck=x+','+cy;
      if(!cells.has(ck))cells.set(ck,[hx(a.px[x+','+(cy*2)]),hx(a.px[x+','+(cy*2+1)])]);
      cells.get(ck)[y&1]=c;}
    if(!view.prev)view.prev=new Set();
    for(const ck of view.prev){if(!cells.has(ck)){const [x,cy]=ck.split(',').map(Number);const s=view.spans[(a.y0+cy)*120+a.x0+x];const b=view.v.base[a.y0+cy][a.x0+x];s.textContent=b[0];s.style.color=b[1];s.style.background=b[2];}}
    for(const [ck,[tp,bt]] of cells){const [x,cy]=ck.split(',').map(Number);
      const s=view.spans[(a.y0+cy)*120+a.x0+x];
      if(view.keep.has((a.x0+x)+','+(a.y0+cy))){s.style.background=css(mixA(mixA(tp,bt,0.5),[0,0,0],0.4));continue;}
      s.textContent='▀';s.style.color=css(tp);s.style.background=css(bt);}
    view.prev=new Set(cells.keys());
  }
  document.getElementById('tick').textContent=t;
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);
for(const b of document.querySelectorAll('button[data-speed]'))b.onclick=()=>{speed=+b.dataset.speed;
  for(const o of document.querySelectorAll('button[data-speed]'))o.classList.toggle('on',o===b)};
"""

PAGE_CSS = """
body{background:#0b0e14;color:#cdd4de;font-family:-apple-system,system-ui,sans-serif;margin:0;padding:24px}
h1{font-size:20px;margin:0 0 6px} h2{font-size:17px;margin:28px 0 4px}
p{margin:4px 0 10px;color:#a0aab9;max-width:1100px;line-height:1.45}
.bar{position:sticky;top:0;background:#0b0e14;padding:8px 0;z-index:2;font-size:13px;color:#7d8796}
button{background:#141a24;color:#cdd4de;border:1px solid #263142;border-radius:4px;padding:3px 10px;margin-left:6px;cursor:pointer}
button.on{border-color:#00e5ff;color:#00e5ff}
"""


def main(out):
    views, body = {}, [
        "<h1>The deck's MAP · round 1</h1>",
        "<p>MAP redrawn as a city map in half-block pixels, with places to show (ripperdocs, "
        "fixers, jobs and the story, home, bars…), one picked, and its walk tracked in the floor "
        "glow (C): the walk lights up, a band runs along it, rings ripple from the target. "
        f"Here: you by Marlo's noodle bar, the ripperdoc behind <b>{TRACKED['name']}</b> "
        "picked and tracked. Tracking is one at a time; the street view shows the same walk.</p>",
        '<div class="bar">speed <button data-speed="1" class="on">1×</button>'
        '<button data-speed="0.25">¼×</button><button data-speed="0">pause</button>'
        ' · tick <span id="tick">0</span></div>',
    ]
    for key, name, fn, desc in CONCEPTS:
        g, anim = fn()
        views[key] = {"base": g.c, "anim": anim}
        body.append(f"<h2>{key} · {name}</h2><p>{desc}</p>"
                    f"<div class='term' data-k='{key}' style='font-size:12.5px'></div>")
    html = ("<!DOCTYPE html><html><head><meta charset='utf-8'><title>Deck map</title>"
            f"<style>{CSS}{PAGE_CSS}</style></head><body>{''.join(body)}"
            f"<script>const VIEWS={json.dumps(views)};{JS}</script></body></html>")
    open(out, "w").write(html)


if __name__ == "__main__":
    main(sys.argv[1])
