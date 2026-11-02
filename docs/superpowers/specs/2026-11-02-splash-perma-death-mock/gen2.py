"""Splash and perma death, round 2: B (Cutaway) with the city's art behind one switch,
and the welcome as something the city broadcasts, in three voices.

    SHOTS=dir/ python3 -I gen2.py round2.html

1 · STATIC 66: a pirate DJ off a stolen dish, shouting the city awake.
2 · NITEWIRE: the evening news, the body count read like the weather.
3 · BLOCK 44: your megablock's PA, management talking to tenants.

Without the art (the switch off, or a terminal under 120 x 36) the broadcast takes the
middle of the screen."""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen  # noqa: E402
from gen import (  # noqa: E402
    AMBER, BG, CSS, CYAN, DIM, FAINT, FAINTER, MAGENTA, RED, RULE, TEXT, WHITE,
    Grid, banner, centre, cutaway_art, footer, scale, show, small_logo, tutorial, wrap,
)

# Each broadcast: the tag (who is on air), the time, the shout, the paragraphs and the
# sign-off, with the tag's and the sign-off's colours.
BROADCASTS = [
    {
        "key": "1", "name": "STATIC 66", "voice": "pirate radio",
        "tag": [("●", RED), (" ON AIR  ", RED), ("STATIC 66 · PIRATE", TEXT)],
        "time": "23:41",
        "shout": "HOLLOOOOWSIIIIIDE! WAKE UP!",
        "paras": [
            "Rise and grind, you beautiful wrecks. Static Sixty-Six, coming off a dish on "
            "the roof of Block 44 that we absolutely paid for.",
            "Acid rain on the Heights, so the suits stay in and the Street's ours. Kiln Road "
            "skyway's shut, somebody's car is part of it now. Noodles in Lantern Market, "
            "still four. Cops on Rust Row, still twenty.",
            "Fresh meat in town tonight. Don't take work from anyone who's smiling.",
        ],
        "sign": "Static Sixty-Six. Turn it up, lock your door.",
        "sign_fg": MAGENTA,
        "words": "A pirate DJ shouting the city awake: loud, local, a bit funny. Closest "
                 "to the radio intro you described, in our own city.",
    },
    {
        "key": "2", "name": "NITEWIRE", "voice": "the news",
        "tag": [("◆", CYAN), (" CH9 NITEWIRE  ", CYAN), ("LIVE", RED)],
        "time": "23:40",
        "shout": "GOOD EVENING, HOLLOWSIDE.",
        "paras": [
            "Eleven dead in Cinder Yards since sundown. Police are calling it a quiet night.",
            "Smog advisory on the Street until further notice. The Heights report clear "
            "skies and record bookings.",
            "After the break: who's buying up the Undercity one flooded block at a time, and "
            "why your landlord is smiling.",
        ],
        "sign": "This hour of NITEWIRE is brought to you by Orodyne. Breathe responsibly.",
        "sign_fg": scale(CYAN, 0.75),
        "words": "The evening news, polished and cold: the body count read like the weather, "
                 "an ad from Orodyne (the corp off the deck's sticker) to close.",
    },
    {
        "key": "3", "name": "BLOCK 44", "voice": "your building",
        "tag": [("▌", AMBER), ("BLOCK 44 ", AMBER), ("RESIDENT ANNOUNCEMENT", TEXT)],
        "time": "23:41",
        "shout": "ATTENTION, BLOCK 44.",
        "paras": [
            "Water on floors 12 to 30 is off until further notice. Do not drink from the "
            "cooling lines.",
            "The lift is working. The lift is not safe. Residents use it at their own risk.",
            "A body was found in the stairwell on 9. If it belongs to you, collect it from "
            "the lobby before 06:00.",
        ],
        "sign": "Hollowside Housing Authority. You live here now.",
        "sign_fg": AMBER,
        "words": "Your megablock's PA, crackling through the flat: management talking down "
                 "to tenants. The bleakest and the most local, it's where you wake up.",
    },
]


def put_spans(g, x, y, spans):
    for s, fg in spans:
        g.put(x, y, s, fg, bold=fg != TEXT)
        x += len(s)
    return x


def rows(b, tw, gaps):
    """The rows the broadcast takes: tag and rule, shout, paragraphs, sign-off."""
    n = 3 + len(wrap(b["shout"], tw)) + 1
    n += sum(len(wrap(p, tw)) + (1 if gaps else 0) for p in b["paras"])
    return n + (0 if gaps else 1) + len(wrap(b["sign"], tw))


def broadcast(g, b, x, y, tw, bottom):
    """The broadcast in a column `tw` wide from `y`, above `bottom`: a blank line between
    paragraphs when there is room for one. Returns the row after."""
    gaps = y + rows(b, tw, True) < bottom
    put_spans(g, x, y, b["tag"])
    g.put(x + tw - len(b["time"]), y, b["time"], FAINT)
    g.put(x, y + 1, "─" * tw, RULE)
    y += 3
    for l in wrap(b["shout"], tw):
        g.put(x, y, l, WHITE, bold=True)
        y += 1
    y += 1
    for p in b["paras"]:
        for l in wrap(p, tw):
            g.put(x, y, l, TEXT)
            y += 1
        if gaps:
            y += 1
    if not gaps:
        y += 1
    for l in wrap(b["sign"], tw):
        g.put(x, y, l, b["sign_fg"], bold=True)
        y += 1
    # The feed's cursor, where it stopped typing.
    g.put(x + len(wrap(b["sign"], tw)[-1]), y - 1, "▮", MAGENTA)
    assert y < bottom, f"{b['name']} runs to row {y}, into {bottom}"
    return y


def jack_in(g, x, y):
    g.put(x, y, "[", FAINT)
    g.put(x + 1, y, "space", CYAN, bold=True)
    g.put(x + 6, y, "]", FAINT)
    g.put(x + 8, y, "JACK IN", MAGENTA, bold=True)


def splash(w, h, b, art=True, pd=True, tut_off=False):
    """B: the logo, the broadcast, the city cut open on the right when `art` is on and
    there is room for it, the toggles and the banner."""
    g = Grid(w, h, BG)
    roomy = h >= 36
    art = art and w >= 120 and roomy
    y = 2 if roomy else 1
    small_logo(g, y)
    y += 6
    if roomy:
        centre(g, y, "h i g h   t e c h   ·   l o w   l i f e", CYAN)
        y += 3
    bottom = h - 9  # the toggles' row; a row of air, then the banner
    if art:
        tx, tw = 4, 50
        cut = tx + tw + 13
        art_h = bottom - y + 1
        pic, bands = cutaway_art(w - cut - 5, art_h)
        g.paste(pic, cut + 1, y)
        for j in range(art_h):
            g.put(cut, y + j, "│", RULE)
        for name, row in bands:
            g.put(cut - 2 - len(name), y + row, name + " ─", FAINTER)
    else:
        tw = min(76, w - 12)
        tx = (w - tw) // 2
        # Alone, the broadcast sits in the middle of the room it has.
        gaps = y + rows(b, tw, True) < bottom
        y += max(0, (bottom - 1 - y - rows(b, tw, gaps)) // 2)
    broadcast(g, b, tx, y, tw, bottom)
    tutorial(g, tx, bottom, tut_off)
    jack_in(g, tx + tw - len("[space] JACK IN"), bottom)
    banner(g, h - 7, pd)
    footer(g)
    return g


def page():
    out = [
        "<!doctype html><meta charset='utf-8'><title>Splash and perma death · round 2</title><style>",
        CSS,
        ".tm .r>i{flex:0 0 10px;margin-right:-0.4px;height:20.4px}",
        "body{background:#0b0e13;color:#cdd4de;font:15px/1.5 -apple-system,system-ui,sans-serif;"
        "margin:24px auto;max-width:1240px;padding:0 16px}"
        "h1{font-size:22px;margin:0 0 4px}h2{font-size:18px;margin:40px 0 4px;color:#fff}"
        "p{max-width:860px;color:#a0aab9}.grid{display:grid;grid-template-columns:1fr 1fr;"
        "gap:18px}.cap{font-size:13px;color:#a0aab9;margin:6px 0 0}"
        ".fr{overflow:hidden}@media(max-width:900px){.grid{grid-template-columns:1fr}}"
        "code{color:#00e5ff}",
        "</style>",
        "<h1>Splash and perma death · round 2</h1>",
        "<p>B, with the welcome replaced by something the city broadcasts. It types itself "
        "out like the HOLO feed, so it reads as live. Three voices below; the place names "
        "would come from your city (its districts, a megablock, the corp names), the jokes "
        "from a small bank so it isn't the same every run.</p>",
        "<p>The cutaway art is one switch: off, the broadcast moves to the middle and nothing "
        "else changes. It also hides itself on a terminal under 120×36, where there isn't room "
        "for both (section 2).</p>",
    ]
    for b in BROADCASTS:
        out.append(f"<h2>{b['key']} · {b['name']} <span style='color:#5c6673;font-weight:400'>"
                   f"· {b['voice']}</span></h2><p>{b['words']}</p>")
        out.append("<div class='fr'>" + show(splash(120, 40, b), 1.0) + "</div>")
    out.append("<h2>Without the art</h2><p>The same screen with the switch off, at 120×40, "
               "and at the minimum terminal, where the art always stays off.</p>")
    out.append("<div class='grid'>")
    out.append("<div class='fr'>" + show(splash(120, 40, BROADCASTS[0], art=False), 0.51)
               + "<div class='cap'>120×40, art off</div></div>")
    out.append("<div class='fr'>" + show(splash(100, 30, BROADCASTS[0]), 0.612)
               + "<div class='cap'>100×30</div></div>")
    out.append("</div>")
    return "\n".join(out)


if __name__ == "__main__":
    # Every broadcast has to fit both sizes, art on and off.
    for b in BROADCASTS:
        for size in [(120, 40), (100, 30)]:
            for art in (True, False):
                splash(*size, b, art=art)
    open(sys.argv[1] if len(sys.argv) > 1 else "round2.html", "w").write(page())
