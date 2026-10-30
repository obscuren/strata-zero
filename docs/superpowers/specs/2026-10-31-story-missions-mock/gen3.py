"""Round 3: the HOLO face, glitching. Calm frames flicker the scanlines; every few
seconds a burst of glitch frames slips pixel rows sideways, splits the colour into a
magenta and a cyan fringe, and cuts a band to one channel or drops it out."""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen  # noqa: E402
from gen import CSS, CYAN, MAGENTA, FACE, scale  # noqa: E402

PAD = 2


def base(phase):
    """The face's pixels, scanlines on rows of `phase`, padded for the slips."""
    ramp = {
        "d": scale(CYAN, 0.22), "m": scale(CYAN, 0.45), "l": scale(CYAN, 0.75),
        "h": CYAN, "b": scale(CYAN, 0.3), "e": "#e8ffff", "X": MAGENTA,
    }
    px = []
    for j, row in enumerate(FACE):
        line = [None] * PAD
        for ch in row:
            c = ramp.get(ch)
            if c and j % 2 == phase and ch not in "Xe":
                c = scale(c, 0.6)
            line.append(c)
        px.append(line + [None] * PAD)
    return px


def slip(px, rows, dx):
    for j in rows:
        r = px[j]
        px[j] = ([None] * dx + r[:-dx]) if dx > 0 else (r[-dx:] + [None] * -dx)


def fringe(px, rows):
    """Chromatic split: a magenta ghost one pixel left, a cyan one right."""
    for j in rows:
        r = list(px[j])
        for i, c in enumerate(r):
            if c is None:
                continue
            if i > 0 and r[i - 1] is None:
                px[j][i - 1] = scale(MAGENTA, 0.8)
            if i + 1 < len(r) and r[i + 1] is None:
                px[j][i + 1] = "#7ff6ff"


def cut(px, rows, mul):
    """One band keeps only some of its colour: mul is (r, g, b) factors."""
    for j in rows:
        for i, c in enumerate(px[j]):
            if c is None:
                continue
            r, g, b = (int(c[k:k + 2], 16) for k in (1, 3, 5))
            px[j][i] = "#%02x%02x%02x" % (min(255, int(r * mul[0])), min(255, int(g * mul[1])), min(255, int(b * mul[2])))


def drop(px, rows):
    for j in rows:
        px[j] = [None if c is None else scale(c, 0.25) for c in px[j]]


def frames():
    calm = [base(1), base(0)]
    g = []
    p = base(1); slip(p, range(6, 10), 2); fringe(p, range(6, 10)); g.append(p)
    p = base(0); slip(p, range(2, 5), -1); slip(p, range(12, 15), 1); cut(p, range(10, 12), (3.0, 0.2, 1.0)); g.append(p)
    p = base(1); slip(p, range(0, 18), 1); fringe(p, range(0, 18)); drop(p, range(5, 8)); g.append(p)
    p = base(0); slip(p, range(8, 11), -2); cut(p, range(14, 18), (0.2, 1.0, 0.4)); fringe(p, range(8, 11)); g.append(p)
    p = base(1); slip(p, [3, 4, 9, 10, 15], 1); slip(p, [6, 7, 12], -1); cut(p, range(6, 8), (2.4, 0.3, 1.2)); g.append(p)
    return calm, g


_put = gen.put_face


def card(px):
    g = gen.street()
    gen.face = lambda *a: px
    gen.put_face = lambda g_, x, y, p, bg=gen.PANEL: _put(g_, x - PAD + 1, y, p, bg)
    gen.card_talking(g)
    return g.crop(0, 5, 66, 15)


def page():
    calm, glitch = frames()
    out = ["<style>%s" % CSS]
    out.append(""".anim{position:relative;display:inline-block}
.anim .f{display:none}.anim .f.on{display:block}
.cap{font:13px/1.4 system-ui;color:#8a94a3;margin:6px 0 18px}</style>""")
    out.append("<h2>The HOLO face, glitching</h2>")
    out.append('<p class="subtitle">Calm, the scanlines crawl. Every couple of seconds a burst: '
               "pixel rows slip sideways, the colour splits into a magenta and a cyan fringe, a "
               "band loses a channel or drops out. Each glitch is whole cells, as the terminal "
               "and the window would draw it.</p>")
    out.append('<div class="anim" id="a">')
    for i, px in enumerate(calm):
        out.append('<div class="f calm%s">%s</div>' % (" on" if i == 0 else "", card(px).html(1.25)))
    for px in glitch:
        out.append('<div class="f glitch">%s</div>' % card(px).html(1.25))
    out.append("</div>")
    out.append('<div class="cap">answered, live</div>')
    out.append("<h3>The glitch frames, held still</h3>")
    for px in glitch:
        out.append('<div style="display:inline-block;margin:0 10px 10px 0">%s</div>' % card(px).crop(0, 0, 22, 12).html(0.9))
    out.append("""<script>
(function(){
  const a=document.getElementById('a');
  const calm=[...a.querySelectorAll('.calm')], gl=[...a.querySelectorAll('.glitch')];
  let cur=calm[0];
  function show(f){cur.classList.remove('on');f.classList.add('on');cur=f;}
  let k=0;
  function calmLoop(left){
    if(left<=0){return burst(2+Math.floor(Math.random()*4));}
    k^=1; show(calm[k]); setTimeout(()=>calmLoop(left-140),140);
  }
  function burst(n){
    if(n<=0){return calmLoop(1200+Math.random()*2200);}
    show(gl[Math.floor(Math.random()*gl.length)]);
    setTimeout(()=>burst(n-1),50+Math.random()*60);
  }
  calmLoop(800);
})();
</script>""")
    return "\n".join(out)


if __name__ == "__main__":
    open(sys.argv[1], "w").write(page())
