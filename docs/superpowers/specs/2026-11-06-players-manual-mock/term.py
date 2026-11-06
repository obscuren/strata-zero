"""Cells to HTML as the window paints them: Fira Code (the window's own font), a fixed
cell width, and half blocks as two-colour cells rather than glyphs."""

import base64
import html
import os

FONTS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", "crates", "strata-zero", "fonts")


def _face(weight, name):
    data = base64.b64encode(open("%s/%s" % (FONTS, name), "rb").read()).decode()
    return ("@font-face{font-family:'StrataFira';font-weight:%d;"
            "src:url(data:font/ttf;base64,%s) format('truetype')}" % (weight, data))


CSS = (_face(400, "FiraCode-Regular.ttf") + _face(700, "FiraCode-Bold.ttf") + """
.tm{font-family:'StrataFira',monospace;display:inline-block;line-height:1;border-radius:4px;
 overflow:hidden;box-shadow:0 0 0 1px #1a2230,0 8px 30px rgba(0,0,0,.5)}
.tm .r{display:flex;height:1.25em}
.tm .c{display:inline-flex;align-items:center;justify-content:center;flex:none;width:.6em;
 height:1.25em;white-space:pre;overflow:visible}
""")


def render(g, px=14.0):
    """`g` (a cells.Grid) as HTML at a font size of `px`."""
    out = ['<div class="tm" style="font-size:%.2fpx">' % px]
    for row in g.c:
        out.append('<div class="r">')
        for ch, fg, bg, bold in row:
            if ch == "▀":
                st = "background:linear-gradient(%s 50%%,%s 50%%)" % (fg, bg)
                out.append('<span class="c" style="%s"></span>' % st)
            elif ch == "▄":
                st = "background:linear-gradient(%s 50%%,%s 50%%)" % (bg, fg)
                out.append('<span class="c" style="%s"></span>' % st)
            elif ch == "█":
                out.append('<span class="c" style="background:%s"></span>' % fg)
            else:
                st = "color:%s;background:%s%s" % (
                    fg, bg, ";font-weight:700" if bold else "")
                out.append('<span class="c" style="%s">%s</span>' % (st, html.escape(ch)))
        out.append("</div>")
    out.append("</div>")
    return "".join(out)
