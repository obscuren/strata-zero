"""Level up: four looks played round your @ on the street (the shot harness's walked
city, outside your megablock). The frames come from the throwaway `levelup_mock` shot
test in levelup_mock.diff.

    python3 -I gen.py <shots dir> <out.html>
"""

import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "2026-10-19-gear-inventory-mock"))
from cells import Grid  # noqa: E402
sys.path.insert(0, os.path.join(HERE, "..", "2026-11-02-minimap-mock"))
import term  # noqa: E402

S = sys.argv[1]
OUT = sys.argv[2]
FRAMES = 18
W, H = 120, 40
CW, CH = 51, 17  # the crop round you
cx, cy = (int(v) for v in open(os.path.join(S, "lvl_centre.txt")).read().split())
X0 = max(0, min(W - CW, cx - CW // 2))
Y0 = max(0, min(H - CH, cy - CH // 2))


def grid(name):
    return Grid.from_svg(os.path.join(S, "lvl_%s_120x40.svg" % name), W, H)


def anim(look, px=15.0):
    frames = [grid("%s_%02d" % (look, t)).crop(X0, Y0, CW, CH) for t in range(FRAMES)]
    rest = grid("base").crop(X0, Y0, CW, CH)
    out = ['<div class="anim">']
    for g in frames + [rest]:
        out.append('<div class="f">%s</div>' % term.render(g, px))
    out.append("</div>")
    return "".join(out)


def still(look, t, px=8.6):
    return term.render(grid("%s_%02d" % (look, t)), px)


LOOKS = [
    ("shock", "A · Shockwave", 7,
     "Two rings wash out across the street from you, cyan then magenta, lighting what "
     "they cross. A white flash on you as it starts; LEVEL 2 types out above you."),
    ("beam", "B · Upload", 9,
     "A beam of data falls on you from the top of the view, bits running down it. As it "
     "hits, a scanline runs out along your row; the beam drains away from the top, and "
     "▲ LEVEL 2 types out beside you."),
    ("reticle", "C · Lock-on", 11,
     "Four brackets close in from the edges of the view and lock round you, flashing "
     "white as they bite, then magenta. LVL 2 and what it brings (a talent point, the "
     "SP) type out beside them, like a HUD readout."),
    ("traces", "D · Circuit", 7,
     "Circuit traces race out from you on eight lines, a node at each head, cyan "
     "fading to magenta, then draw back in from the middle. LEVEL 2 above you."),
]

CSS = """
:root{--bg:#0a0d13;--fg:#cdd4de;--muted:#8a95a5;--rule:#1d2a38;--head:#ffffff;
 --cyan:#00e5ff;--magenta:#ff00c8;color-scheme:dark}
""" + term.CSS + """
body{background:var(--bg);color:var(--fg);font:15px/1.55 -apple-system,system-ui,sans-serif;
 margin:0;padding-block:24px 80px;padding-left:max(16px,env(safe-area-inset-left,0px));
 padding-right:max(16px,env(safe-area-inset-right,0px))}
main{max-width:1560px;margin:0 auto}
h1{font-size:24px;margin:0 0 4px;color:var(--head);text-wrap:balance}
h2{font-size:18px;margin:40px 0 6px;color:var(--cyan);border-bottom:1px solid var(--rule);
 padding-bottom:6px}
p{max-width:860px}
.sub{color:var(--muted);margin:0 0 18px}
code{color:var(--head)}
.row{display:flex;gap:22px;flex-wrap:wrap;align-items:flex-start}
figure{margin:8px 0;overflow-x:auto;max-width:100%;min-width:0}
figcaption{color:var(--muted);font-size:13px;margin-top:6px;max-width:620px}
.anim .f{display:none}.anim .f.on{display:block}
button{background:#121a26;color:var(--fg);border:1px solid #2a3a4e;border-radius:6px;
 padding:6px 12px;font:inherit;cursor:pointer}
"""

JS = """
const MS=1000/15, HOLD=1100; let slow=false;
function play(a){clearTimeout(a._t);const f=[...a.querySelectorAll('.f')];let i=0;
 const step=()=>{f.forEach((e,k)=>e.classList.toggle('on',k===i));
  const last=i===f.length-1;i=last?0:i+1;
  a._t=setTimeout(step,last?HOLD:(slow?MS*4:MS));};step();}
const all=()=>document.querySelectorAll('.anim').forEach(play);
all();
document.getElementById('slow').onclick=e=>{slow=!slow;
 e.target.textContent=slow?'normal speed':'slow it down';all();};
"""


def page():
    out = ["<title>Level up</title>",
           '<meta name="claude:viewport-fit-sides" content="cover">',
           "<style>%s</style><main>" % CSS,
           "<h1>Level up · four looks round your @</h1>",
           '<p class="sub">Played on the street outside your megablock, as the game draws '
           "it. Each look runs about 1.2 s (18 frames at 15 a second), then the street "
           "rests before it loops. The log line stays as it is: "
           "<code>level 2 · a talent point · 100 SP</code>.</p>",
           '<p><button id="slow">slow it down</button></p>']
    for look, title, peak, words in LOOKS:
        out.append("<h2>%s</h2><p>%s</p>" % (title, words))
        out.append('<div class="row">')
        out.append("<figure>%s<figcaption>Round you, playing.</figcaption></figure>"
                   % anim(look))
        out.append("<figure>%s<figcaption>The whole screen at frame %d.</figcaption>"
                   "</figure>" % (still(look, peak), peak + 1))
        out.append("</div>")
    out.append("</main><script>%s</script>" % JS)
    return "".join(out)


open(OUT, "w").write(page())
