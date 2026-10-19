"""Companion pages: page.py out.html — the concepts as clickable options."""

import sys
from cells import CSS
import wb

STYLE = "<style>%s .shot{overflow-x:auto;margin:10px 0 4px} .why{color:#9aa4b2;max-width:880px;line-height:1.5}</style>" % CSS


def option(choice, letter, title, why, screens):
    out = ['<div class="option" data-choice="%s" onclick="toggleSelect(this)" style="display:block">' % choice,
           '<div class="letter">%s</div><div class="content"><h3>%s</h3><p class="why">%s</p>' % (letter, title, why)]
    for caption, fn in screens:
        if caption:
            out.append('<p class="label">%s</p>' % caption)
        out.append('<div class="shot">%s</div>' % fn().html())
    out.append("</div></div>")
    return "".join(out)


def concepts():
    return "".join([
        STYLE,
        "<h2>WORKBENCH — three ways to build</h2>",
        '<p class="subtitle">100×30, drawn over the game\'s own frame and the Ratchet\'s real drawing, '
        'cut into five parts. The part names are placeholders. The tab row is the new one from '
        'GEAR &amp; INVENTORY.</p>',
        '<div class="options">',
        option("a", "A", "The drawing sheet",
               "Each build is an engineering drawing: the item <b>exploded</b> into its parts with numbered "
               "balloons, a parts list underneath, a title block. ←→ flips through the drawings, ↑↓ walks "
               "the parts and brackets the one you point at. A missing part stays an outline. "
               "Space assembles once every part is on hand.",
               [(None, wb.concept_a)]),
        option("b", "B", "The bench",
               "A cutting mat with a ruler. You build <b>step by step</b>: the steps are listed on the left, the loose "
               "parts lie at the mat's foot, and space fits the next one. It slides into place on the "
               "drawing. Backspace takes one back out. A half-built item stays on the mat until you finish it.",
               [(None, wb.concept_b)]),
        option("c", "C", "The register + an ASSEMBLY mode",
               "The tab works like GEAR &amp; INVENTORY: a list of builds, its parts, a card and a half-size "
               "drawing. Space opens the blueprint window in an <b>assembly mode</b>, a sibling of modding: "
               "the empty places are like sockets, the PARTS tray offers what fits, and you fit each "
               "part into its place.",
               [("the tab", wb.concept_c_tab), ("space → the blueprint window, assembly mode", wb.concept_c_window)]),
        "</div>",
    ])


if __name__ == "__main__":
    open(sys.argv[1], "w").write(concepts())
