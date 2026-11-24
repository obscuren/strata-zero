"""Turret mockups (#97): one small server room, a turret on its north wall, drawn as a
fight goes.

    python3 -I gen.py mock.html

A · an arrowhead on a steel body that points where it faces (▲ ◥ ► ◢ ▼ ◣ ◄ ◤).
B · a round body (◉) with a barrel stub drawn on the floor tile it faces.
Cones as the floor cameras draw theirs: red while it sees you, grey blinded, green
turned. Colours are the game's indoor palette."""

import html
import sys

OUT = sys.argv[1]
W, H = 24, 9

FLOOR_BG = "#1a222e"
FLOOR_DOT = "#2a364a"
WALL_FG = "#6e87a0"
WALL_BG = "#0a0b10"
FURN = "#84a2c0"
DOOR = "#dcebf5"
YOU = "#ffffff"
GUARD = "#ff5050"
DIM = "#5b6b7d"
STEEL = "#2b2f36"

SEEING_BG = "#500c0c"
BLIND_BG = "#2c2e32"
TURNED_BG = "#0a2812"
TRACER = "#ffd75a"

# the room: x 1..22, y 1..7; a door on the south wall (y 8) into the corridor
grid = [["#"] * W for _ in range(H)]
for y in range(1, 8):
    for x in range(1, W - 1):
        grid[y][x] = "."
grid[8][16] = "+"
racks = {(x, y) for x in (3, 4, 19, 20) for y in (3, 4, 5, 6)} | {(x, 6) for x in range(10, 14)}

DOUBLE = {0: "═", 1: "║", 2: "═", 3: "╚", 4: "║", 5: "║", 6: "╔", 7: "╠",
          8: "═", 9: "╝", 10: "═", 11: "╩", 12: "╗", 13: "╣", 14: "╦", 15: "╬"}

DIRS = {"N": (0, -1), "NE": (1, -1), "E": (1, 0), "SE": (1, 1),
        "S": (0, 1), "SW": (-1, 1), "W": (-1, 0), "NW": (-1, -1)}
ARROW = {"N": "▲", "NE": "◥", "E": "►", "SE": "◢", "S": "▼", "SW": "◣", "W": "◄", "NW": "◤"}
BARREL = {"N": "│", "NE": "╱", "E": "─", "SE": "╲", "S": "│", "SW": "╱", "W": "─", "NW": "╲"}

TURRET = (8, 1)
DEPTH = 8


def wallish(x, y):
    return 0 <= x < W and 0 <= y < H and grid[y][x] in "#+"


def wall_glyph(x, y):
    m = (1 if wallish(x, y - 1) else 0) | (2 if wallish(x + 1, y) else 0) \
        | (4 if wallish(x, y + 1) else 0) | (8 if wallish(x - 1, y) else 0)
    return DOUBLE[m]


def blocks(x, y):
    return not (0 <= x < W and 0 <= y < H) or grid[y][x] != "." or (x, y) in racks


def line(a, b):
    (x0, y0), (x1, y1) = a, b
    dx, dy = abs(x1 - x0), -abs(y1 - y0)
    sx, sy = (1 if x0 < x1 else -1), (1 if y0 < y1 else -1)
    err, out = dx + dy, []
    while True:
        out.append((x0, y0))
        if (x0, y0) == (x1, y1):
            return out
        e2 = 2 * err
        if e2 >= dy:
            err += dy
            x0 += sx
        if e2 <= dx:
            err += dx
            y0 += sy


def in_cone(facing, p):
    dx, dy = DIRS[facing]
    ox, oy = p[0] - TURRET[0], p[1] - TURRET[1]
    depth = ox * dx + oy * dy
    side = abs(ox * dy - oy * dx)
    return 1 <= depth <= DEPTH and side <= depth


def cone(facing):
    out = set()
    for y in range(H):
        for x in range(W):
            p = (x, y)
            if in_cone(facing, p) and not blocks(x, y) and all(
                    not blocks(*q) for q in line(TURRET, p)[1:-1]):
                out.add(p)
    return out


STATE_FG = {"asleep": "#8a4a4e", "awake": "#ff3c46", "off": DIM, "blind": "#9aa0a8",
            "turned": "#5fd38a"}
CONE_BG = {"awake": SEEING_BG, "blind": BLIND_BG, "turned": TURNED_BG}


def frame(look, facing, state, you=None, guard=None, tracer=None, spark=False):
    """The room as cells: (glyph, fg, bg, bold)."""
    cone_tiles = cone(facing) if state in CONE_BG else set()
    stub = (TURRET[0] + DIRS[facing][0], TURRET[1] + DIRS[facing][1])
    trail = set(line(*tracer)[1:-1]) if tracer else set()
    rows = []
    for y in range(H):
        row = []
        for x in range(W):
            p = (x, y)
            t = grid[y][x]
            bg = FLOOR_BG
            if p in cone_tiles:
                bg = CONE_BG[state]
            if t == "#":
                cell = (wall_glyph(x, y), WALL_FG, WALL_BG, False)
            elif t == "+":
                cell = ("▯", DOOR, FLOOR_BG, False)
            elif p == TURRET:
                fg = TRACER if spark else STATE_FG[state]
                g = ARROW[facing] if look == "A" else "◉"
                cell = (g, fg, STEEL, state != "off")
            elif p == you:
                cell = ("@", YOU, bg, True)
            elif p == guard:
                cell = ("G", GUARD, bg, True)
            elif p in racks:
                cell = ("≣", FURN, bg, False)
            elif p in trail:
                cell = ("·", TRACER, bg, True)
            elif look == "B" and p == stub and not blocks(*p):
                cell = (BARREL[facing], STATE_FG[state], bg, True)
            else:
                dot = "·" if (x * 7 + y * 13) % 23 == 0 else " "
                cell = (dot, FLOOR_DOT, bg, False)
            row.append(cell)
        rows.append(row)
    return rows


def cells_html(rows):
    out = []
    for row in rows:
        spans = []
        for g, fg, bg, bold in row:
            cls = ' class="b"' if bold else ""
            spans.append(f'<span{cls} style="color:{fg};background:{bg}">{html.escape(g)}</span>')
        out.append("".join(spans))
    return "\n".join(out)


def column(turn, chips, theirs):
    """The turn column, 18 wide, as the game pins it to the map's corner."""
    lines = [f'<span class="ch hd"> TURN {turn:<12} </span>']
    for name, hp, acting in chips:
        mark = "▶" if acting else " "
        cls = "ch act" if acting else ("ch you" if name == "YOU" else "ch foe")
        lines.append(f'<span class="{cls}"> {mark} {name:<9}{hp:>5} </span>')
    if theirs:
        lines.append(f'<span class="ch th"> {theirs:<16} </span>')
    return "\n".join(lines)


def panel(title, rows, note, msg=None, col=None, look_line=None):
    side = f'<pre class="col">{column(*col)}</pre>' if col else ""
    m = f'<div class="msg">{html.escape(msg)}</div>' if msg else ""
    lk = f'<div class="look">{html.escape(look_line)}</div>' if look_line else ""
    return (f'<div class="panel"><div class="t">{html.escape(title)}</div>'
            f'<div class="row"><pre class="map">{cells_html(rows)}</pre>{side}</div>'
            f'{m}{lk}<div class="n">{html.escape(note)}</div></div>')


def story(look):
    you0, you1, you2 = (16, 7), (12, 5), (13, 3)
    guard = (17, 5)
    p = []
    p.append(panel("1 · asleep", frame(look, "S", "asleep", you=you0),
                   "It sits dark until it or a camera sees you. Its cone isn't drawn, "
                   "as a camera's isn't until it sees you.",
                   look_line="Turret · facing S · asleep"))
    p.append(panel("2 · it wakes", frame(look, "S", "awake", you=you1),
                   "You step into its cone: it wakes, the cone goes red and a fight "
                   "starts. Its turn comes in the order like a guard's.",
                   msg="turret wakes · TURNS",
                   col=(1, [("YOU", "", True), ("turret", "6·40", False)], None)))
    p.append(panel("3 · it slews and fires",
                   frame(look, "SE", "awake", you=you2, tracer=(TURRET, you2)),
                   "You stepped out of the cone. On its turn it slews one eighth "
                   "toward you (S → SE), and fires if that puts you in it.",
                   msg="turret slews SE › fires · 2 hit · 14",
                   col=(2, [("YOU", "", False), ("turret", "6·40", True)], "slews › fires")))
    p.append(panel("4 · you shoot its plate",
                   frame(look, "SE", "awake", you=you2, tracer=(you2, TURRET), spark=True),
                   "Rounds chip its armour first: ARM 6 → 4. Once the plate is gone "
                   "they go into its 40.",
                   msg="you fire · 3 hit · plate 6 → 4",
                   col=(2, [("YOU", "", True), ("turret", "4·40", False)], None),
                   look_line="Turret · facing SE · ARM 4 · HP 40 · 22 rounds"))
    p.append(panel("5 · Net: off", frame(look, "SE", "off", you=you2),
                   "Held from the Net and shut off: grey, no cone, it sleeps "
                   "while you hold it.",
                   look_line="Turret · off"))
    p.append(panel("6 · Net: blinded", frame(look, "SE", "blind", you=you2),
                   "Blinded for 3 turns: it slews and fires at nothing it can see. "
                   "The cone dims grey, as a looped camera's does.",
                   look_line="Turret · blind · 3 turns"))
    p.append(panel("7 · Net: turned on the guards",
                   frame(look, "E", "turned", you=you2, guard=guard,
                         tracer=(TURRET, guard)),
                   "Turned: it slews to the nearest guard it sees and fires on its "
                   "turn. The cone goes green. The guards shoot back.",
                   msg="turret fires on guard · 2 hit · 14",
                   col=(3, [("YOU", "", False), ("turret", "6·40", False),
                            ("guard", "26", True)], "fires")))
    return "".join(p)


def facings(look):
    rows = []
    for f in DIRS:
        stub = BARREL[f] if look == "B" else ""
        g = ARROW[f] if look == "A" else "◉"
        rows.append(f'<span class="fc"><span class="b" style="color:#ff3c46;'
                    f'background:{STEEL}">{g}</span>{stub}<em>{f}</em></span>')
    return "".join(rows)


CSS = """
body{background:#06070a;color:#c8d0d8;font:14px/1.45 -apple-system,system-ui,sans-serif;
margin:24px}
h1{font-size:18px;margin:0 0 6px} h2{font-size:15px;margin:28px 0 6px;color:#00e6d2}
p.lead{color:#8a96a3;max-width:900px;margin:0 0 10px}
.grid{display:flex;flex-wrap:wrap;gap:18px}
.panel{background:#0b0d12;border:1px solid #1e2632;padding:10px;width:max-content}
.t{font-weight:600;margin-bottom:6px;color:#e6edf3}
.row{display:flex;gap:8px;align-items:flex-start}
pre{font:19px/1.15 "Fira Code",Menlo,monospace;margin:0}
pre.map span{display:inline-block;width:.6em;text-align:center}
.b{font-weight:700}
.ch{display:block;white-space:pre}
.hd{background:#0a0b10;color:#00e6d2}
.act{background:#0b4652;color:#fff;font-weight:700}
.you{background:#00141a;color:#c8c8c8}
.foe{background:#28080c;color:#ff5050}
.th{background:#28080c;color:#c8c8c8}
.msg{font:13px "Fira Code",Menlo,monospace;color:#ffd75a;margin-top:6px}
.look{font:13px "Fira Code",Menlo,monospace;color:#9fb6cc;margin-top:4px}
.n{color:#8a96a3;font-size:12.5px;max-width:300px;margin-top:6px}
.fcs{display:flex;gap:14px;font:16px "Fira Code",Menlo,monospace;margin:6px 0 4px}
.fc em{font:11px system-ui;color:#8a96a3;margin-left:4px;font-style:normal}
"""

doc = f"""<!doctype html><html><head><meta charset="utf-8"><title>Turrets mock</title>
<style>{CSS}</style></head><body>
<h1>Turrets (#97) · how a turret looks and fights</h1>
<p class="lead">A server room with a turret on its north wall, a fight told frame by frame.
A turret faces one of eight ways and sees a 90° cone 8 deep, as a floor camera does. Awake,
it slews one eighth toward you each turn, then fires if you're in its cone. The turn
column's figure reads ARM·HP.</p>
<h2>A · arrowhead on a steel body</h2>
<div class="fcs">{facings("A")}</div>
<div class="grid">{story("A")}</div>
<h2>B · round body, barrel stub on the tile it faces</h2>
<div class="fcs">{facings("B")}</div>
<div class="grid">{story("B")}</div>
</body></html>"""

open(OUT, "w").write(doc)
