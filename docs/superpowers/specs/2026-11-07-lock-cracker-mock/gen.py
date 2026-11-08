"""The lock cracker (issue #6), three devices clipped onto the game's real keypad: seed
8, the first favour's flat door (401, Velform Group Plaza). The shots come from the
throwaway `cracker_mock` shot test in cracker_mock.diff.

    python3 -I gen.py <shots dir> <out.html>
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "2026-10-19-gear-inventory-mock"))
from cells import Grid  # noqa: E402
sys.path.insert(0, os.path.join(HERE, "..", "2026-11-02-minimap-mock"))
import term  # noqa: E402

S = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "shots")
OUT = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "index.html")


def shot(name, w, h, px, prefix="crack"):
    g = Grid.from_svg(os.path.join(S, "%s_%s_%dx%d.svg" % (prefix, name, w, h)), w, h)
    return term.render(g, px)


def fig(name, caption, size=(120, 40), px=10.4, prefix="crack"):
    return ('<figure>%s<figcaption>%s</figcaption></figure>'
            % (shot(name, size[0], size[1], px, prefix), caption))


def kj(name, caption, size=(120, 40), px=10.4):
    return fig(name, caption, size, px, "kj")


def round_two():
    out = ['<h2>Round 2 · the Keyjack <span class="tag">picked: A</span></h2>',
           '<p>The third fault locks the door and burns out the fuse. The fuse sits '
           'in a socket on the board inside: M from Inspect opens the back. No fuse, '
           'no run. Every build starts with one, a Techie builds Keyjacks and fuses, '
           'and a lock\'s rating narrows the zones.</p>',
           '<h3>The clip key on the pad</h3>',
           '<div class="pair">',
           kj("clip", "Carrying a Keyjack with a fuse in: <code>k</code> clips it on; "
              "the digits still work as now."),
           kj("clip_nofuse", "Its fuse burnt out or gone: the same line, greyed, so "
              "you know why it won't run."),
           '</div>',
           '<h3>Inspect and the opened back</h3>',
           '<div class="pair">',
           kj("front", "BLUEPRINT · KEYJACK: the slab, its LCD, the fault lights, the "
              "strike key, the lead and its alligator clip. A fuse light by the badge."),
           kj("back", "M opens the back: the board, the KJ4 chip, the 3V cell, and the "
              "fuse in its holder, the socket boxed."),
           kj("back_tray", "Space on the socket: the fuses you carry, each tuning the "
              "game."),
           kj("back_blown", "After a failed run: the fuse blown, smoked glass and a "
              "broken filament. It won't run until one goes in.", (100, 30), 12.4),
           '</div>']
    return "".join(out)


# One look on purpose: the game's own night, whatever the viewer's theme.
CSS = """
:root{--bg:#0a0d13;--fg:#cdd4de;--muted:#8a95a5;--rule:#1d2a38;--head:#ffffff;
 --cyan:#00e5ff;--magenta:#ff00c8;--amber:#ffaa28;--green:#5aff96;color-scheme:dark}
""" + term.CSS + """
body{background:var(--bg);color:var(--fg);font:15px/1.55 -apple-system,system-ui,sans-serif;
 margin:0;padding-block:24px 80px;padding-left:max(16px,env(safe-area-inset-left,0px));
 padding-right:max(16px,env(safe-area-inset-right,0px))}
main{max-width:1560px;margin:0 auto}
h1{font-size:24px;margin:0 0 4px;color:var(--head);text-wrap:balance}
h2{font-size:18px;margin:44px 0 6px;color:var(--cyan);border-bottom:1px solid var(--rule);
 padding-bottom:6px;text-wrap:balance}
h2 .tag{color:var(--muted);font-weight:400;font-size:14px;margin-left:8px}
h3{font-size:15px;margin:22px 0 6px;color:var(--magenta)}
p,li{max-width:860px}
.sub{color:var(--muted);margin:0 0 18px}
figure{margin:12px 0 18px;overflow-x:auto;max-width:100%;min-width:0}
figcaption{color:var(--muted);font-size:13px;margin-top:6px;max-width:740px}
.pair{display:flex;gap:18px;flex-wrap:wrap;align-items:flex-start}
.pair figure{margin:6px 0}
b.a{color:var(--magenta)} b.b{color:var(--magenta)} b.c{color:var(--amber)}
code{color:var(--head)}
"""


def page():
    out = ["<title>Lock cracker</title>",
           '<meta name="claude:viewport-fit-sides" content="cover">',
           "<style>%s</style><main>" % CSS,
           "<h1>Lock cracker · a way through the door for every build</h1>",
           '<p class="sub">Three devices clipped onto the real keypad of the first '
           "favour's flat (seed 8, flat 401). Your rules: a marker sweeps a bar, space "
           "lands it in the coloured zone, every hit narrows the zone, three faults and "
           "it's over. Four beats, one per digit; each hit keys its digit into the pad.</p>"]

    out.append(round_two())
    out.append('<h2 style="margin-top:64px">Round 1 · three crackers</h2>')
    out.append('<h2>A · KEYJACK <span class="tag">a street-made slab on a lead, '
               'green LCD</span></h2>')
    out.append('<p>A black slab with a magenta case, clipped to the pad\'s edge by a '
               'lead. Its LCD carries the bar and the digits it has cracked; three '
               'fault lights under it; the pad\'s own glass fills in as it goes.</p>')
    out.append('<div class="pair">')
    out.append(fig("a_beat1", "Beat 1: a wide zone, nothing cracked yet."))
    out.append(fig("a_beat4", "Beat 4: three digits in, the zone two cells wide, "
                   "one fault spent."))
    out.append(fig("a_miss", "A miss: the bar flashes red, ERR, a second fault lit."))
    out.append(fig("a_fried", "The third fault: the fuse blows and the pad denies. "
                   "What that costs is the open question below."))
    out.append(fig("a_cracked", "Cracked: four digits keyed, the pad says OPEN."))
    out.append('</div>')

    out.append('<h2>B · LEECH <span class="tag">clamps over the pad and takes its '
               'glass</span></h2>')
    out.append('<p>A parasite: it bites onto the pad\'s crown, three legs into the '
               'board, and plays the game on the pad\'s own display in magenta. No '
               'second box on screen; the door itself looks hijacked.</p>')
    out.append('<div class="pair">')
    out.append(fig("b_beat3", "Beat 3 on the pad's glass: two digits in, one fault."))
    out.append(fig("b_miss", "A miss, at 100×30.", (100, 30), 12.4))
    out.append('</div>')

    out.append('<h2>C · TUMBLER <span class="tag">an amber pin scope</span></h2>')
    out.append('<p>Test gear on two leads: the pad\'s lines hum across a phosphor '
               'trace and the true pin stands out of the noise as the zone. The marker '
               'is a bright column sweeping the trace; space catches the pin. Same '
               'rules, read as listening to the lock.</p>')
    out.append('<div class="pair">')
    out.append(fig("c_beat2", "Pin 2: the zone a wide spike."))
    out.append(fig("c_beat4", "Pin 4: a thin spike, two slips spent."))
    out.append('</div>')

    out.append('</main>')
    return "".join(out)


open(OUT, "w").write(page())
