"""python3 board.py <out.html> — every skill's art by category, learned, then unlearned."""

import sys
from cells import CSS
import skillart as sa

STYLE = ("<style>body{background:#0b0e14;color:#cdd4de;font-family:sans-serif;padding:16px}%s"
         ".cat{margin:18px 0 6px;color:#00e5ff;font-weight:700;letter-spacing:.08em}"
         ".row{display:flex;gap:18px;flex-wrap:wrap}.tile p{margin:6px 0 0;color:#9aa4b2;"
         "font-size:13px}</style>" % CSS)


def tile(skill, learned):
    kind = "neon" if skill in sa.NEON else "hardware"
    return '<div class="tile">%s<p>%s · %s</p></div>' % (
        sa.render(skill, learned).grid().html(), skill, kind)


def board():
    out = ['<!doctype html><meta charset="utf-8">', STYLE]
    for learned in (True, False):
        out.append("<h2>%s</h2>" % ("learned" if learned else "unlearned"))
        for cat, skills in sa.ORDER:
            out.append('<div class="cat">%s</div><div class="row">' % cat)
            out += [tile(s, learned) for s in skills]
            out.append("</div>")
    return "".join(out)


if __name__ == "__main__":
    open(sys.argv[1], "w").write(board())
