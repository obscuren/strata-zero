"""The companion page: python3 -I page.py m1.json out.html"""

import html
import json
import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from trees import TREES, GATE  # noqa: E402

d = json.load(open(sys.argv[1]))
sh = d["shots"]

css = """
/* One dark look on purpose: the page shows terminal screens. Layout: a single column of
   screens, each with its notes beside it (below it when narrow). */
:root{color-scheme:dark;--bg:#0b0d12;--card:#0f131a;--edge:#1d2733;--fg:#cdd4de;--muted:#a0aab9;--faint:#78828f;
 --accent:#00e5ff;--hot:#ff00c8;--rule:#283c4b;--mono:'JetBrains Mono',Menlo,'DejaVu Sans Mono',monospace;
 --sans:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif}
""" + d["css"] + """
body{background:var(--bg);color:var(--fg);margin:0;font:15px/1.55 var(--sans)}
main{max-width:1320px;margin:0 auto;padding-block:28px 60px;
 padding-left:max(16px,env(safe-area-inset-left,0px));padding-right:max(16px,env(safe-area-inset-right,0px))}
.scroll{min-width:0}
.shot>div{min-width:0}
h1{font:700 22px var(--mono);color:var(--accent);text-wrap:balance;letter-spacing:.06em;margin:0 0 6px}
h2{font:700 15px var(--mono);color:var(--accent);letter-spacing:.08em;margin:46px 0 10px;
   border-bottom:1px solid var(--rule);padding-bottom:6px}
h2 .k{color:var(--hot);margin-right:10px}
p.lead{color:var(--muted);max-width:860px;margin:0 0 8px}
.shot{display:grid;grid-template-columns:auto 1fr;gap:22px;align-items:start;background:var(--card);border:1px solid var(--edge);
      border-radius:8px;padding:16px;margin:14px 0}
.shot h3{font:700 14px var(--mono);color:var(--fg);margin:2px 0 8px}
.shot p{color:var(--muted);margin:0 0 8px;font-size:14px}
.shot ul{color:var(--muted);margin:0;padding-left:18px;font-size:14px}
.shot li{margin:3px 0}
.scroll{overflow-x:auto;max-width:100%}
.shot.wide{grid-template-columns:1fr}
@media (max-width:1240px){.shot{grid-template-columns:1fr}}
.trees{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(380px,100%),1fr));gap:14px;margin-top:12px}
.tree{background:var(--card);border:1px solid var(--edge);border-radius:8px;padding:12px 14px}
.tree h3{font:700 14px var(--mono);margin:0 0 2px}
.tree .line{color:var(--faint);font-size:13px;margin-bottom:8px}
.grid{display:grid;grid-template-columns:28px repeat(4,1fr);gap:4px;font:12px/1.3 var(--mono)}
.cell{background:#0b0f15;border:1px solid #1a222d;border-radius:4px;padding:4px 5px;min-height:34px}
.cell.empty{background:transparent;border-color:transparent}
.cell .n{color:var(--fg)}
.cell .r{color:var(--faint)}
.cell .q{color:#5c6673;font-size:11px}
.cell.new{border-color:var(--hot)}
.cell.new .n::after{content:" NEW";color:var(--hot);font-size:10px}
.gate{color:#5c6673;font:11px var(--mono);align-self:center;text-align:right;padding-right:4px}
.notes{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(260px,100%),1fr));gap:12px;margin-top:14px}
.note{background:var(--card);border:1px solid var(--edge);border-radius:8px;padding:12px 14px;color:var(--muted);font-size:14px}
.note b{font-family:var(--mono);color:var(--fg)}
"""

out = ['<title>Strata Zero Talent Trees</title>'
       '<meta name="claude:viewport-fit-sides" content="cover">'
       '<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=JetBrains+Mono:wght@400;700&display=swap">'
       "<style>%s</style><main>" % css]
out.append("<h1>TALENTS · ONE TREE AT A TIME</h1>")
out.append("<p class=lead>Three takes on classic talent trees. In each, one tree fills the tab, talents are small "
           "pixel icons on a grid of four columns and five tiers, arrows show what needs what, and "
           "<b>[</b> <b>]</b> flip between the five trees (the keys GEAR &amp; INVENTORY already uses for its filters). "
           "All on the deck's real frame at 100×30, the smallest the game runs at; 120×40 under each.</p>"
           "<p class=lead>The runner here has 8 points in EDGE (Quick draw 3/3, Double tap 2/2, Light step 2/2, "
           "Steady hand 1/2), 2 in ICE and 2 to spend. Only EDGE's icons are drawn so far.</p>")


def block(key, title, blurb, bullets, shots):
    out.append("<h2><span class=k>%s</span>%s</h2>" % (key, html.escape(title)))
    for i, (s, cap) in enumerate(shots):
        txt = "<div><h3>%s</h3>" % html.escape(cap)
        if i == 0:
            txt += "<p>%s</p><ul>%s</ul>" % (blurb, "".join("<li>%s</li>" % b for b in bullets))
        txt += "</div>"
        wide = " wide" if s.endswith("_big") else ""
        out.append("<div class='shot%s'><div class=scroll>%s</div>%s</div>" % (wide, sh[s], txt))


block("A", "ARMORY", "The closest to the old trees. Every icon sits in a frame that tells its state at a glance, "
      "the rank in the corner, and the talent under the cursor is read out in a panel on the right.",
      ["Frame: <span style='color:#50e678'>green</span> you can put a point in, "
       "<span style='color:#ffc850'>gold</span> maxed, dark grey out of reach; white is the cursor.",
       "Icons out of reach are drawn in grey, the way the old trees did it.",
       "The numbers down the left are the points each tier needs; they light as the tree fills.",
       "Arrows light in the tree's colour once the talent they come from is maxed.",
       "The panel's LINE is the chain the talent sits in, so a path reads in one row."],
      [("a", "100×30 · cursor on Hair trigger, which you can learn"),
       ("a_locked", "100×30 · cursor on Bullet time, out of reach"),
       ("a_big", "120×40 · the same, with room to breathe")])

block("B", "BOARD", "The tree as a circuit board in the deck. Talents are chips, prerequisites are traces, and "
      "points are power: a rail down the left carries it to each tier as the tree fills.",
      ["The five trees are slots down the left, each with its points and how far to the capstone (12).",
       "Rank is a column of LEDs beside each chip; a lit trace means the chip it comes from is maxed.",
       "The readout on the right reads like a file on the deck.",
       "The most diegetic of the three, and the busiest to look at."],
      [("b", "100×30 · cursor on Hair trigger"),
       ("b_big", "120×40 · cursor on Bullet time")])

block("C", "SHOWCASE", "One tree, centred, over a faint emblem of its own (a crosshair for EDGE), with the tree tabs "
      "along the bottom the way the old trees had them, and a tooltip that pops up beside the cursor.",
      ["No frames: colour against grey carries the state, the rank badge in green or gold.",
       "The tooltip sits on the side of the tree with more room, pointing at the icon, so the tree stays visible.",
       "Each tree would get its own emblem (a fist, a wrench, an eye, a chip...).",
       "The closest to the original look. The tooltip is narrow at 100×30."],
      [("c", "100×30 · cursor on Hair trigger"),
       ("c_big", "120×40 · cursor on Fan fire, behind the tier gate")])

# The content.
out.append("<h2><span class=k>·</span>THE FIVE TREES</h2>")
out.append("<p class=lead>Every current talent kept, three new ones a tree (15 in all, marked NEW), now on four "
           "columns with two or three lines each. Tiers still open every %d points, so the capstone wants 12 "
           "in its tree. A prerequisite can now come from the side (an L-shaped arrow) as well as straight above, "
           "and an arrow may skip an empty tier.</p>" % GATE)
out.append("<div class=trees>")
for name, colour, line, rows in TREES:
    by = {(r[0], r[1]): r for r in rows}
    names = {r[2]: r[3] for r in rows}
    cells = []
    for tier in range(5):
        cells.append("<div class=gate>%d</div>" % (GATE * tier))
        for col in range(4):
            r = by.get((tier, col))
            if not r:
                cells.append("<div class='cell empty'></div>")
                continue
            need = ""
            if r[5]:
                src = [x for x in rows if x[2] == r[5]][0]
                arrow = "↑" if src[1] == col else ("↖" if src[1] < col else "↗")
                need = "<div class=q>%s %s</div>" % (arrow, html.escape(names[r[5]]))
            cells.append("<div class='cell%s'><div class=n>%s</div><div class=r>%d rank%s</div>%s</div>"
                         % (" new" if r[6] else "", html.escape(r[3]), r[4], "" if r[4] == 1 else "s", need))
    out.append("<div class=tree><h3 style='color:%s'>%s</h3><div class=line>%s</div><div class=grid>%s</div></div>"
               % (colour, name, html.escape(line), "".join(cells)))
out.append("</div>")

out.append("<h2><span class=k>·</span>NOTES</h2><div class=notes>"
           "<div class=note><b>Same rules.</b> One point at creation and one a level, tiers every 3 points in the tree, "
           "a prerequisite must be maxed. Only the layout, the art and the number of talents change.</div>"
           "<div class=note><b>Icons.</b> 6×6 pixels in three cells, drawn like the skill cards' art and exported to "
           "a .ron the same way. Grey when out of reach. 60 to draw once a concept is picked.</div>"
           "<div class=note><b>The new talents' numbers</b> use perks the game already has (move, armour, resist, "
           "focus, ability sharpening). Flags still read <i>later: ...</i> until combat reads them.</div>"
           "<div class=note><b>Moving.</b> The cursor stays inside the tree: ←→ along a tier, ↑↓ between tiers, "
           "skipping empty slots. <b>[ ]</b> change tree, and each tree remembers where its cursor was.</div>"
           "</div>")
out.append("</main>")
open(sys.argv[2], "w").write("".join(out))
