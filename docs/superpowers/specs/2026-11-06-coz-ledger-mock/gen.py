"""Coz's book (issues #4 and #5), drawn through the game's own screens: seed 8, the
morning after done, the doctor seen, a netrunner, 21:40 on Siren Avenue. The shots
come from the throwaway `coz_mock` shot test (coz_mock.diff, run with MOCK_KEEP=1).

    python3 -I gen.py <shots dir> <out.html>
"""

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
    g = Grid.from_svg(os.path.join(S, "coz_%s_%dx%d.svg" % (name, w, h)), w, h)
    return term.render(g, px)


def fig(name, caption, size=(120, 40), px=10.4):
    return ('<figure>%s<figcaption>%s</figcaption></figure>'
            % (shot(name, size[0], size[1], px), caption))


SCRIPT = [
    ("coz", "Not here, choom. Not out in the open."),
    (None, "He pulls you into a doorway."),
    ("coz", "Black tape, three turns. You're holdin' somethin' of mine, choomie."),
    (None, "He turns it under the light."),
    ("coz", "Seal's been popped. Somebody's been in my book. Fuck."),
    ("netrunner", "You got sum skills, huh. You been in it too, ain't you."),
    ("fixer build", "You're in the trade, I can smell it on you. You checked it, ey?"),
    ("coz", "So who sent you?"),
    ("you", "Castellan."),
    (None, "A long breath."),
    ("coz", "Course it was him."),
    ("coz", "Three years of my work in there, choom. Who I sent, where, who paid."),
    ("coz", "Fuck! How'd it all go so sideways?"),
    ("coz", "Last page is his. Three of mine, on his scrip. One job, in an' out."),
    ("coz", "We were s'posed to walk out clean. Damn it."),
    ("coz", "The money came home. They didn't."),
    (None, "He peels off a roll, and doesn't look at you while he counts."),
    ("coz", "Eight hun'. For bringin' it home, an' for not shoppin' it round first."),
    ("coz", "You want work, you come to me. Not Mezz. Me."),
    ("coz", "An' when Castellan sits you in a chair, an' he will, you ask him where my "
            "people are. Then you come tell me."),
]


def script():
    out = ['<ol class="script">']
    for who, text in SCRIPT:
        if who is None:
            out.append('<li class="stage"><i>%s</i></li>' % html.escape(text))
        elif who in ("netrunner", "fixer build"):
            out.append('<li class="need"><span>only a %s</span> %s</li>'
                       % (who, html.escape(text)))
        else:
            out.append('<li><b class="%s">%s</b> %s</li>'
                       % (who, "Coz" if who == "coz" else "You", html.escape(text)))
    out.append('</ol>')
    return "".join(out)


# One look on purpose: the game's own night, whatever the viewer's theme.
CSS = """
:root{--bg:#0a0d13;--fg:#cdd4de;--muted:#8a95a5;--rule:#1d2a38;--head:#ffffff;
 --cyan:#00e5ff;--magenta:#ff00c8;--amber:#ffbe3c;--green:#3ce68c;--note:#141820;
 --note-edge:#ffc850;color-scheme:dark}
""" + term.CSS + """
body{background:var(--bg);color:var(--fg);font:15px/1.55 -apple-system,system-ui,sans-serif;
 margin:0;padding-block:24px 80px;padding-left:max(16px,env(safe-area-inset-left,0px));
 padding-right:max(16px,env(safe-area-inset-right,0px))}
main{max-width:1560px;margin:0 auto}
h1{font-size:24px;margin:0 0 4px;color:var(--head);text-wrap:balance}
h2{font-size:18px;margin:44px 0 6px;color:var(--cyan);border-bottom:1px solid var(--rule);
 padding-bottom:6px;text-wrap:balance}
p,li{max-width:860px}
.sub{color:var(--muted);margin:0 0 18px}
figure{margin:12px 0 18px;overflow-x:auto;max-width:100%;min-width:0}
figcaption{color:var(--muted);font-size:13px;margin-top:6px;max-width:740px}
.pair{display:flex;gap:18px;flex-wrap:wrap;align-items:flex-start}
.pair figure{margin:6px 0}
.script{list-style:none;padding:0;margin:8px 0;line-height:1.75}
.script b{display:inline-block;min-width:44px}
.script b.coz{color:var(--amber)}
.script b.you{color:var(--head)}
.script .stage{color:var(--muted);padding-left:44px}
.script .need span{display:inline-block;min-width:44px;color:var(--cyan);font-size:12px;
 margin-right:6px}
.pick{color:var(--green);font-weight:600}
code{color:var(--head)}
"""


def page():
    out = ["<title>Coz's book</title>",
           '<meta name="claude:viewport-fit-sides" content="cover">',
           "<style>%s</style><main>" % CSS,
           "<h1>Coz's book · the end of act one</h1>",
           '<p class="sub">Drawn through the real screens: seed 8, the morning after done, '
           'the doctor seen, a netrunner, 21:40 on Siren Avenue. Issues #4 and #5.</p>']

    out.append('<h2>1 · The corner</h2>')
    out.append('<p>The objective now gives his hours, since a fixer is only on his corner '
               '18:00 to 04:00. Coz is tracked (⌖ in the pane, <code>$</code> on the '
               'minimap), lit on the map.</p>')
    out.append(fig("corner", "Walking up to Coz (the amber @, lit) on Siren Avenue."))

    out.append('<h2>2 · The sale</h2>')
    out.append('<p>A bubbles scene, like Odile\'s: the street keeps moving around it. '
               'Space moves a line on, esc skips. The whole script:</p>')
    out.append(script())
    out.append('<div class="pair">')
    for name, cap in [
        ("s0_open", "Space on Coz: the scene opens on the corner."),
        ("s2_tape", "He knows the tape."),
        ("s5_netrunner", "Only a netrunner hears this one (a Fixer build gets its own)."),
        ("s8_castellan", "Your line: the name."),
        ("s13_last_page", "The last page is Castellan's."),
        ("s15_money", "The line that replaces \"Nobody's seen them since\"."),
        ("s17_pay", "The pay: ¤800, more than any job, more than any build starts with."),
        ("s19_hook", "The hook. Odile and Coz now ask you the same thing."),
    ]:
        out.append(fig(name, cap))
    out.append('</div>')

    out.append('<h2>3 · The act ends, in the world\'s voice</h2>')
    out.append('<p>Walking off from the corner, the first holo ad you pass. The shard is '
               'gone from the pack, ¤800 more on the status line, and the pane already '
               'shows <b>The three</b>.</p>')
    out.append('<div class="pair">')
    out.append(fig("end_a", '<span class="pick">A · my pick.</span> Seren tears through '
                   'the ad, magenta, and the same words land as a mail ("quiet"). It says '
                   'what to do now (work for Coz) and that the story comes back.'))
    out.append(fig("end_b", "B · Castellan's call, cut short: \"Coz pays. Coz talks. Both "
                   "good habits.\" / \"I'll be away a while. Seren knows where you "
                   "sleep.\" / \"Keep your head down.\" Later his number would be dead."))
    out.append(fig("end_c", "C · Static 66 jacks the ad: the pirates already know a book "
                   "came home. The splash could carry a fuller Static 66 broadcast on the "
                   "next LOAD GAME, as it does for the courier."))
    out.append('</div>')

    out.append('<h2>4 · The JOBS tab after</h2>')
    out.append('<p>Every story mission ticked, <b>The three</b> open with nothing to track. '
               'Coz is on the fixers list as someone who knows you. Saves name steps by '
               'id, so a later version carries on from here.</p>')
    out.append('<div class="pair">')
    out.append(fig("jobs_the_three", "The three: from Coz, open, the one objective."))
    out.append(fig("jobs_ledger", "The ledger: done, its objectives ticked."))
    out.append('</div>')

    out.append('<h2>5 · #5, the story keeps what it gave you</h2>')
    out.append('<p>An item the story gave you stays in the pack until the story takes it '
               'back. The Drop rows go grey with one line, as the jacked-in rule already '
               'does. The taped package gets the same until it is opened.</p>')
    out.append(fig("keep", "The shard's verbs in GEAR &amp; INVENTORY."))

    out.append('<h2>At 100×30</h2>')
    out.append('<div class="pair">')
    out.append(fig("s15_money", "The scene's bubble.", (100, 30), 12.4))
    out.append(fig("end_a", "Ending A.", (100, 30), 12.4))
    out.append('</div>')

    out.append('<h2>Still to settle</h2>')
    out.append('<ul>'
               '<li>The ending: A, B or C (or two of them).</li>'
               '<li>"He pulls you into a doorway" is a stage line only; Coz doesn\'t walk '
               'yet (no doorway mark). Walking him there is possible but new.</li>'
               '<li>The Fixer build\'s line needs a new need for the build; the mock uses '
               'streetwise.</li>'
               '<li>New pieces: pay credits and raise a fixer\'s standing from a mission, '
               'the keep rule, the ad line for A.</li>'
               '</ul>')
    out.append('</main>')
    return "".join(out)


open(OUT, "w").write(page())
