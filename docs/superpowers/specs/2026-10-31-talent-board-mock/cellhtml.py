"""Cells to HTML the way the window paints them: every cell 9.6 x 20 px, block and
quadrant characters as solid shapes, everything else as a glyph centred in its cell."""

import html

QUADS = {
    "▘": (1, 0, 0, 0), "▝": (0, 1, 0, 0), "▖": (0, 0, 1, 0), "▗": (0, 0, 0, 1),
    "▀": (1, 1, 0, 0), "▄": (0, 0, 1, 1), "▌": (1, 0, 1, 0), "▐": (0, 1, 0, 1),
    "▚": (1, 0, 0, 1), "▞": (0, 1, 1, 0), "▛": (1, 1, 1, 0), "▜": (1, 1, 0, 1),
    "▙": (1, 0, 1, 1), "▟": (0, 1, 1, 1), "█": (1, 1, 1, 1),
}

CSS = """
.tm{display:inline-block;font-family:'JetBrains Mono','Fira Code',Menlo,monospace;font-size:15px;line-height:20px;
 border-radius:4px;overflow:hidden;box-shadow:0 0 0 1px #1a2230,0 8px 30px rgba(0,0,0,.5)}
.tm .r{display:flex;height:20px}
.tm .r>*{flex:0 0 9.6px;width:9.6px;height:20px;text-align:center;overflow:visible;white-space:pre}
.tm i{display:block}
.tm b{font-weight:700}
"""


def cell(ch, fg, bg, bold):
    q = QUADS.get(ch)
    if q:
        ul, ur, ll, lr = [fg if v else bg for v in q]
        style = ("background:linear-gradient(90deg,%s 50%%,%s 50%%) top/100%% 50%% no-repeat,"
                 "linear-gradient(90deg,%s 50%%,%s 50%%) bottom/100%% 50%% no-repeat" % (ul, ur, ll, lr))
        return '<i style="%s"></i>' % style
    if ch == " ":
        return '<i style="background:%s"></i>' % bg
    tag = "b" if bold else "span"
    return '<%s style="color:%s;background:%s">%s</%s>' % (tag, fg, bg, html.escape(ch), tag)


def grid_html(g):
    out = ['<div class="tm">']
    for row in g.c:
        out.append('<div class="r">')
        for ch, fg, bg, bold in row:
            out.append(cell(ch, fg, bg, bold))
        out.append("</div>")
    out.append("</div>")
    return "".join(out)
