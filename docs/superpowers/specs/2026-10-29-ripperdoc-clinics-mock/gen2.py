"""Round 2: chair A on the existing header line; the LOAD change in amber, magenta
when it won't fit. python3 gen2.py OUT.html"""
import sys
sys.dont_write_bytecode = True
from gen import Grid, CSS, S, CYAN, MAGENTA, DIM, FAINT, TEXT, PANEL, RULE, AMBER

FAINTER = "#5c6673"


def shot(name):
    return Grid.from_svg(S + name + ".svg", 100, 30)


def in_chair(g):
    """The chair's mark on the CHROME header (row 5), the divider amber."""
    title_end = 48 + len("CHROME")
    tag = " · IN THE CHAIR"
    g.put(title_end, 5, tag, AMBER, bold=True)
    for x in range(title_end + len(tag), 74):
        g.put(x, 5, " " if x == title_end + len(tag) else "─", RULE)
    for y in range(5, 27):
        g.put(46, y, "║", AMBER)


def hint(g, spans):
    g.put(1, 28, " " * 98, DIM, bg=PANEL)
    x = 2
    for t, col in spans:
        g.put(x, 28, t, col)
        x += len(t)


def over(g):
    """What the change adds goes magenta when it won't fit: the pick's segment and the
    total."""
    for x in range(80, 92):
        if g.c[5][x][1] == "#ffc850":
            g.c[5][x][1] = MAGENTA
    for x in (93, 94):
        g.c[5][x][1] = MAGENTA


a = shot("attributes_ledger_limb_100x30")
in_chair(a)
hint(a, [("↑↓ select · space what fits · x pull it out · ← attributes · esc get out of the chair", DIM)])

b = shot("attributes_picker_site_100x30")
in_chair(b)

c = shot("tmp_picker_over_100x30")
in_chair(c)
over(c)

d = shot("attributes_ledger_limb_100x30")
hint(d, [("↑↓ select · ← attributes · Q/E tabs · tab close", DIM),
         ("   chrome goes in and out in a ripperdoc's chair", FAINTER)])

cards = [
    ("1", "In the chair, on a piece",
     "The chair's mark joins the CHROME header on its own line. The amber divider says "
     "the knives are out. Hints as today, plus <i>esc get out of the chair</i>.", a),
    ("2", "In the chair, FITS on a lighter pick",
     "Grip arm on the empty R ARM: the segment it adds in amber, the total it comes to "
     "(22) in amber. This is how the gauge already works.", b),
    ("3", "In the chair, FITS on a pick that won't fit",
     "Wrecker arm, 19 → 23 of 22: the added segment and the total go magenta, as the "
     "L4 tag and the card's load line already do. Space refuses it. <b>New</b>: today "
     "the header stays amber here.", c),
    ("4", "Anywhere else",
     "The ledger only looks: no space, no x in the hints, a faint note on where chrome "
     "changes. Pressing space or x anyway says so in the log.", d),
]

parts = ["<style>%s .grid{display:flex;flex-wrap:wrap;gap:18px}"
         ".cell{flex:1 1 640px;max-width:760px} .cell h3{margin:4px 0}"
         ".cell p{margin:0 0 6px;line-height:1.45;font-size:14px}"
         ".tag{display:inline-block;background:#ffc850;color:#000;font-weight:700;"
         "border-radius:4px;padding:0 7px;margin-right:8px}</style>" % CSS]
parts.append("<h2>Ripperdocs · round 2 · the chair (A)</h2>")
parts.append('<p class="subtitle">The chair\'s mark moves onto the existing CHROME line. '
             "It reads IN THE CHAIR, not the doc's name, so it fits at 100×30 whatever the doc "
             "is called; the doc's line goes to the log when you sit down. Clinics: all three "
             "layouts, picked at random.</p>")
parts.append('<div class="grid">')
for k, name, desc, g in cards:
    parts.append('<div class="cell"><h3><span class="tag">%s</span>%s</h3><p>%s</p>%s</div>'
                 % (k, name, desc, g.html(0.72)))
parts.append("</div>")
open(sys.argv[1], "w").write("\n".join(parts))
