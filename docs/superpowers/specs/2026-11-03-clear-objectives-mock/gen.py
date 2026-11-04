"""Clear objectives mockups, drawn through the game's own screens (seed 8, the morning
after, six jobs from three fixers) by the throwaway `objectives_mock` shot test in
objectives_mock.diff.

    python3 -I gen.py shots index.html

1 · The JOBS list keeps no walk line under its rows; the card on the right has it.
2 · The chain: the morning after in three objectives, the ledger after it.
3 · The mail mark, three ways (A status line, B the band's third row, C a pill on the
    map), each lit as the mail lands and settled until it is read. Chosen: C, then A.
4 · The rig reads the mail (a row with the count, the mail tagged NEW): drawn, then left
    for a session of its own on the mail."""

import html
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "2026-10-19-gear-inventory-mock"))
from cells import Grid  # noqa: E402
sys.path.insert(0, os.path.join(HERE, "..", "2026-11-02-minimap-mock"))
import term  # noqa: E402

S = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "shots")
OUT = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "index.html")


def shot(name, w, h, px):
    g = Grid.from_svg(os.path.join(S, "mock_%s_%dx%d.svg" % (name, w, h)), w, h)
    return term.render(g, px)


def fig(name, w, h, px, caption):
    return ('<figure>%s<figcaption>%s</figcaption></figure>'
            % (shot(name, w, h, px), caption))


CHAIN = """
<ol class="chain">
<li><b>The morning after</b> · from Castellan
 <ol>
 <li>Catch the news on the holo <i>· the track walks you to the set</i></li>
 <li>Read Mezz's mail <i>· ends when you open it</i></li>
 <li>Open the package · it's in your pack <i>· Seren breaks in on the holo as soon as
 Mezz's mail is read; her word moves here from the doctor's mission</i></li>
 </ol></li>
<li><b>The ledger</b> · from Castellan, opens as the package does
 <ol>
 <li>Find out whose shard this is · <i>the hint by build:</i> read it at your rig
 (hacking) · open the casing at your bench (electronics) · look it over (streetwise) ·
 ask the doctor (anyone else) <i>· ends on the reading, or when Odile names him</i></li>
 <li>Bring it to Coz · his place · his district</li>
 </ol></li>
<li><b>Doctor's in by ten</b> runs beside the ledger, its two objectives as today.</li>
</ol>
"""

CSS = term.CSS + """
body{background:#0a0d13;color:#cdd4de;font:15px/1.5 -apple-system,system-ui,sans-serif;
 margin:0;padding:24px 16px 80px}
main{max-width:1060px;margin:0 auto}
h1{font-size:22px;margin:0 0 4px;color:#fff}
h2{font-size:17px;margin:40px 0 6px;color:#00e5ff;border-bottom:1px solid #1d2a38;
 padding-bottom:6px}
h3{font-size:15px;margin:22px 0 6px;color:#ff00c8}
p,li{max-width:820px}
.sub{color:#8a95a5;margin:0 0 18px}
figure{margin:12px 0 18px;overflow-x:auto}
figcaption{color:#8a95a5;font-size:13px;margin-top:6px;max-width:820px}
.pair{display:flex;gap:14px;flex-wrap:wrap;align-items:flex-start}
.pair figure{margin:6px 0}
.chain{line-height:1.7}
.chain i{color:#8a95a5;font-style:normal}
.note{border-left:3px solid #ffc850;padding:8px 14px;background:#141820;max-width:800px}
"""


def page():
    out = ['<!doctype html><html><head><meta charset="utf-8">',
           '<meta name="viewport" content="width=device-width,initial-scale=1">',
           '<title>Clear objectives</title><style>%s</style></head><body><main>' % CSS,
           '<h1>Clear objectives · the mail · the JOBS list</h1>',
           '<p class="sub">Drawn through the real screens: the morning after, seed 8.</p>']

    out.append('<h2>1 · The JOBS list without walk lines</h2>')
    out.append('<p>The line under each story and job ("24 m · Marlo", "93 m · lift to L") '
               'is gone; the card on the right still carries the walk. A tracked story '
               'keeps its ⌖ on its own row, as jobs already do.</p>')
    out.append(fig("jobs_morning", 120, 40, 10.4,
                   "A story selected: the card lists its objectives, the done ones "
                   "ticked above the one it waits on."))
    out.append(fig("jobs_job", 120, 40, 10.4,
                   "A fixer's job selected: the card already lists both stops, take the "
                   "shard and hand it in."))

    out.append('<h2>2 · The chain</h2>')
    out.append(CHAIN)
    out.append(fig("jobs_ledger", 100, 30, 12.4,
                   "The ledger open after the morning, its first objective fit to a "
                   "hacker (100×30)."))

    out.append('<h2>3 · The mail mark</h2>')
    out.append('<p>All three log the mail as it lands ("Mezz · mail · the news") and '
               'stay until it is read.</p>')
    for key, title, text in [
        ("a", "A · on the status line",
         "<code>▣ NEW MAIL</code> after your credits: lit magenta as it lands, "
         "magenta text after."),
        ("b", "B · on the band's third row",
         "Who wrote and about what, right, above the F-keys. On the street that row also "
         "says how exposed you are (\"dim · unseen\"), so the two share it."),
        ("c", "C · a pill on the map",
         "Hardest to miss; it covers a strip of the map. Settled, it would hand over to "
         "A."),
    ]:
        out.append('<h3>%s</h3><p>%s</p>' % (html.escape(title), text))
        out.append('<div class="pair">')
        size = (120, 40) if key == "c" else (100, 30)
        px = 10.4 if key == "c" else 12.4
        out.append(fig("mail_%s" % key, size[0], size[1], px, "as it lands"))
        out.append(fig("mail_%s_settled" % key, size[0], size[1], px, "settled, unread"))
        out.append('</div>')

    # 4 · At the rig (a mail row, a NEW tag) was drawn and left for the mail session.

    out.append('<h2>Found in passing</h2>')
    out.append('<p class="note">After the sleep the newest log line likely stays hidden '
               'behind its typing cursor for minutes: the log stamps a line with the '
               "city's clock (8 hours on after a sleep) and types it out by the frame "
               'counter. The shots showed it until staged around. Not yet seen in a '
               'running game; worth fixing with this work.</p>')
    out.append('</main></body></html>')
    return "".join(out)


open(OUT, "w").write(page())
