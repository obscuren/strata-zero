"""Round 3: the door after hours, the chair's window, CHROMING UP, the doc's fees, a
pull that asks first, the dev command. python3 gen3.py OUT.html"""
import sys
sys.dont_write_bytecode = True
from gen import (Grid, CSS, S, CYAN, MAGENTA, DIM, FAINT, TEXT, PANEL, RULE, AMBER, GOOD,
                 map_frame, stamp, NOODLE)

FAINTER = "#5c6673"
DEVBG = "#ffc850"


def shot(name):
    return Grid.from_svg(S + name + ".svg", 100, 30)


def mark(g, tag=" · CHROMING UP", col=AMBER, bg=None):
    x0 = 48 + len("CHROME")
    g.put(x0, 5, tag, col, bold=True, bg=bg)
    for x in range(x0 + len(tag), 74):
        g.put(x, 5, " " if x == x0 + len(tag) else "─", RULE, bg=PANEL)
    for y in range(5, 27):
        g.put(46, y, "║", col if bg is None else DEVBG)


def hint(g, spans):
    g.put(1, 28, " " * 98, DIM, bg=PANEL)
    x = 2
    for t, col in spans:
        g.put(x, 28, t, col)
        x += len(t)


def credits(g, n):
    """The header's credits: `CARRY ¤ 320 · ...` on row 1."""
    row = "".join(c[0] for c in g.c[1])
    i = row.index("¤ ")
    old = row[i + 2:].split(" ")[0]
    new = str(n).rjust(len(old)) if len(str(n)) <= len(old) else str(n)
    g.put(i + 2, 1, new, g.c[1][i + 2][1])


def card_line(g, y, spans):
    g.put(67, y, " " * 31, TEXT, bg=PANEL)
    x = 68
    for t, col, *b in spans:
        g.put(x, y, t, col, bold=bool(b and b[0]))
        x += len(t)


def shift_flavour(g, y0, y1):
    """The card's flavour text one row down to make room."""
    for y in range(y1, y0 - 1, -1):
        for x in range(67, 98):
            g.c[y + 1][x] = list(g.c[y][x])


def paste_popup(g, src):
    for y in range(11, 19):
        for x in range(29, 70):
            g.c[y][x] = list(src.c[y][x])


# 1 · the door after hours
door = shot("tmp_pop_closed_100x30")
door.put(1, 26, " " * 90, TEXT)
door.put(1, 26, "Hot Pot Lou is closed · opens at 10:00", AMBER)

# 2 · the chair's window
layout = list(NOODLE)
layout[2] = "#.....X@......#...............#"
layout[3] = "#.*...=d......~.......*....n..#"
layout[8] = "#..O.S...O.S..................#"
chair = map_frame("Hot Pot Lou · L", "Operating chair",
                  [("Doc Lou is at the chair.", DIM)])
stamp(chair, 14, 8, layout, cold_rooms=[(1, 1, 13, 4)])
paste_popup(chair, shot("tmp_pop_chair_100x30"))
chair.put(31, 13, " " * 37, TEXT, bg=chair.c[13][31][2])
chair.put(31, 13, "Doc Lou's · ", DIM, bg=chair.c[13][31][2])
chair.put(43, 13, "“Sit. Don't touch the tray.”", DIM, bg=chair.c[13][31][2])

# 3 · sitting down: CHROMING UP, the pull's fee in the hint
sit = shot("attributes_ledger_limb_100x30")
mark(sit)
hint(sit, [("↑↓ select · space what fits · x pull it out ", DIM), ("¤110", AMBER),
           (" · ← attributes · esc get out of the chair", DIM)])

# 4 · FITS, the fee you can pay
pay = shot("attributes_picker_site_100x30")
mark(pay)
credits(pay, 1200)
shift_flavour(pay, 18, 23)
card_line(pay, 18, [("fee ", DIM), ("¤370", AMBER, True), (" · leaves ¤830", DIM)])

# 5 · FITS, the fee you can't
broke = shot("attributes_picker_site_100x30")
mark(broke)
shift_flavour(broke, 18, 23)
card_line(broke, 18, [("fee ", DIM), ("¤370", MAGENTA, True), (" · you have ¤320", MAGENTA)])
hint(broke, [("↑↓ choose · ", DIM), ("space install", FAINTER), (" · esc back · Q/E tabs · tab close", DIM)])

# 6 · a swap: in and out
swap = shot("attributes_picker_swap_100x30")
mark(swap)
credits(swap, 1200)
shift_flavour(swap, 18, 23)
card_line(swap, 18, [("fee ", DIM), ("¤480", AMBER, True), (" · in ¤370 · out ¤110", DIM)])

# 7 · x asks before it charges
ask = shot("attributes_ledger_limb_100x30")
mark(ask)
hint(ask, [("x again to pull ", AMBER, ), ("Piston arm", TEXT), (" for ", AMBER), ("¤110", AMBER),
           (" · it comes out with its Arc hand · esc keep it", DIM)])

# 8 · dev: ripper on
dev = shot("attributes_ledger_limb_100x30")
mark(dev, tag=" · FREE ", col="#000000", bg=DEVBG)
dev.put(48 + 6, 5, " ", TEXT, bg=PANEL)
dev.put(48 + 7, 5, " FREE ", "#000000", bold=True, bg=DEVBG)
hint(dev, [("↑↓ select · space what fits · x pull it out · ← attributes · Q/E tabs · tab close", DIM)])

cards = [
    ("1", "At the door, after hours",
     "The doc is the clinic's hours: no doc on shift, the street door is shut. Space beside "
     "it gives its window: CLOSED with the hours. Walking into it says so in the log "
     "(drawn here on the status line).", door),
    ("2", "At the chair",
     "Space beside the chair: its window, like everything else. <b>i Interact: Sit in the "
     "chair</b> opens the deck on ATTRIBUTES, CHROMING UP. The doc's line is the subtitle "
     "and goes to the log.", chair),
    ("3", "In the chair: CHROMING UP",
     "The mark on the CHROME line now reads CHROMING UP. The hint carries what pulling "
     "the pointed piece costs.", sit),
    ("4", "FITS: the fee, and what it leaves",
     "A line under the load line: the doc's fee in amber and what you'd have left.", pay),
    ("5", "FITS: a fee you can't pay",
     "The fee and your credits in magenta, as the load goes when it won't fit; space "
     "install dims and refuses.", broke),
    ("6", "FITS: a swap",
     "Swapping is the new piece in and the old one out: one fee, both parts.", swap),
    ("7", "x asks first",
     "Pulling costs now, so x arms it: the hint asks, x again pulls and pays, esc keeps it. "
     "What's fitted in the piece rides out with it.", ask),
    ("8", "Dev: free chrome",
     "The console's <b>ripper</b> toggles free chrome: the ledger works anywhere and charges "
     "nothing, the mark reads FREE in the DEV badge's colours. Off by default (the app "
     "starts in dev mode, and clinics should be what you play).", dev),
]

prices = [("Smart ink", "skin · L1", 700, 80, 30), ("Neural port", "neural · L1", 1500, 180, 60),
          ("Target lens", "eyes · L2", 2200, 260, 90), ("Arc hand", "socket · L1", 2600, 310, 100),
          ("Piston arm", "arm · L3", 2800, 340, 110), ("Grip arm", "arm · L3", 3100, 370, 120),
          ("Wrecker arm", "arm · L4", 4900, 590, 200), ("Redline spine", "spine · L3", 5200, 620, 210),
          ("Ceramic plating", "skin · L4", 6800, 820, 270)]

parts = ["<style>%s .grid{display:flex;flex-wrap:wrap;gap:18px}"
         ".cell{flex:1 1 640px;max-width:760px} .cell h3{margin:4px 0}"
         ".cell p{margin:0 0 6px;line-height:1.45;font-size:14px}"
         ".tag{display:inline-block;background:#ffc850;color:#000;font-weight:700;"
         "border-radius:4px;padding:0 7px;margin-right:8px}"
         "table.px{border-collapse:collapse;margin:8px 0 18px;font-size:14px}"
         "table.px td,table.px th{padding:3px 14px;border-bottom:1px solid #222c38;text-align:right}"
         "table.px td:first-child,table.px th:first-child,table.px td:nth-child(2),table.px th:nth-child(2){text-align:left}"
         "</style>" % CSS]
parts.append("<h2>Ripperdocs · round 3 · doors, the chair, fees</h2>")
parts.append('<p class="subtitle">Prices: the doc charges <b>12%</b> of a piece\'s value to put it '
             "in and <b>4%</b> to take it out, rounded to ¤10. A swap is both. Values are "
             "already in content (¤600–6800); a fetch pays ¤200, a runner starts with ¤250–600, "
             "so a first piece is a job or two. The rates sit in content.</p>")
parts.append('<table class="px"><tr><th>piece</th><th></th><th>value</th><th>put in</th><th>take out</th></tr>')
for n, k, v, i, o in prices:
    parts.append("<tr><td>%s</td><td style='color:#8a94a3'>%s</td><td>¤%d</td><td>¤%d</td><td>¤%d</td></tr>" % (n, k, v, i, o))
parts.append("</table>")
parts.append('<div class="grid">')
for k, name, desc, g in cards:
    parts.append('<div class="cell"><h3><span class="tag">%s</span>%s</h3><p>%s</p>%s</div>'
                 % (k, name, desc, g.html(0.72)))
parts.append("</div>")
open(sys.argv[1], "w").write("\n".join(parts))
