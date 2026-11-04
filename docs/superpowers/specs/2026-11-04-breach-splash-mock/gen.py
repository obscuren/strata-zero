"""Breach splash, round 1: four splash screens that play at launch, before the title.

    python3 -I gen.py round1.html

The animation lives in splash.js; this inlines it with the window's own font (Fira Code)
so the page opens anywhere."""

import base64
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
FONTS = os.path.join(HERE, "..", "..", "..", "..", "crates", "strata-zero", "fonts")


def face(weight, name):
    data = base64.b64encode(open(os.path.join(FONTS, name), "rb").read()).decode()
    return ("@font-face{font-family:'StrataFira';font-weight:%d;"
            "src:url(data:font/ttf;base64,%s) format('truetype')}" % (weight, data))


CONCEPTS = [
    ("A", "Hijack", "Your rig boots, and someone else finishes the boot.",
     "The closest to that 2077 opening: the screen you trust turns on you.",
     [("0 – 2.2 s", "The rig posts: ARASU·TEK RIGBIOS, optics, the neural jack, relays shaking "
                    "hands. One relay never answers."),
      ("2.7 s", "<code>!! unsigned packet on jack port 7</code>, then the local ICE drops a layer "
                "at a time."),
      ("3 – 4 s", "The log tears: rows jump sideways in cyan and magenta, glyphs rot, red "
                  "INTRUSION bars cut across."),
      ("3.7 – 5 s", "<b>BREACH</b> in red block letters at twice the logo's size; under it, "
                    "<i>relax. we're only borrowing your eyes.</i>"),
      ("5 – 5.5 s", "The picture folds to a line like an old set switching off, and the title "
                    "lights up a letter at a time.")]),
    ("B", "ICE wall", "You're the one breaking in.",
     "The breach seen from the attacker's side: a wall, a pick, the hole you make.",
     [("0 – 0.7 s", "A firewall builds down the screen, brick by brick, each brick a byte; a "
                    "shimmer runs across it."),
      ("1.2 – 2.6 s", "ICEPICK hits it four times, right where the logo sits: a white flash, a "
                      "ring, a shake, cracks spreading further each hit. INTEGRITY 100 → 0 %, "
                      "LAYER 1/4 → 4/4."),
      ("2.6 – 2.9 s", "The cracks glow magenta. <i>› ICE down. you're in.</i>"),
      ("2.9 – 4 s", "The wall shatters from the impact outwards and the shards fall away; the "
                    "logo is already lit behind it.")]),
    ("C", "Drill", "Breach down through the city, to stratum zero.",
     "Ties the name to the city: every run starts by going down through it.",
     [("0 – 0.5 s", "The Heights: spires, stars, an ad. <b>STRATUM 4</b> on the right, metres "
                    "on the left."),
      ("0.5 – 3.4 s", "The drill speeds up, smears, hits each slab (flash, sparks, shake, "
                      "<b>BREACH 1/4</b>) and punches through: SKYWAYS, STREET, UNDERCITY, the "
                      "count running down 3, 2, 1."),
      ("3.5 – 3.8 s", "<b>STRATUM 0</b>: below the last slab it's black, and the logo slams "
                      "in.")]),
    ("D", "Key", "The game's name is the password.",
     "Quieter and cleverer: a crack you can read, and the logo built out of it.",
     [("0 – 2.3 s", "A memory dump races past the read head and slows; the scanner fills in "
                    "the key a byte at a time, near misses blinking magenta."),
      ("2.3 s", "It locks on one row: <code>53 54 52 41 54 41 20 5A 45 52 4F</code>, "
                "STRATA ZERO in hex. <b>KEY FOUND</b>."),
      ("2.8 – 3.5 s", "The dump dissolves around it."),
      ("3 – 4.8 s", "Each byte flies up and becomes its letter of the logo.")]),
]

CSS = """
body{background:#07080c;color:#cdd4de;font:15px/1.55 -apple-system,system-ui,sans-serif;
  margin:28px auto;max-width:1240px;padding:0 16px}
h1{font-size:22px;margin:0 0 6px;color:#fff}
h2{font-size:19px;margin:46px 0 2px;color:#fff}
h2 .k{color:#ff00c8;margin-right:8px}
.tag{font-size:16px;color:#00e5ff;margin:0 0 4px}
p{max-width:900px;color:#a0aab9;margin:6px 0}
code{color:#00e5ff;font-size:13px}
ol.beats{list-style:none;padding:0;margin:10px 0 14px;max-width:960px}
ol.beats li{display:grid;grid-template-columns:96px 1fr;gap:12px;color:#a0aab9;margin:3px 0}
ol.beats .w{color:#78828f;font-variant-numeric:tabular-nums}
.screen{background:#0a0b10;border-radius:8px;padding:10px;display:block;max-width:1220px;
  box-sizing:border-box;box-shadow:0 0 0 1px #1b2230}
.screen canvas{display:block;width:100%;cursor:pointer}
.ctl{display:flex;flex-wrap:wrap;align-items:center;gap:8px;margin:8px 0 0}
.ctl button{background:#141924;color:#cdd4de;border:1px solid #283041;border-radius:6px;
  padding:4px 10px;font:13px system-ui;cursor:pointer}
.ctl button:hover{border-color:#00e5ff}
.ctl .seg{display:inline-flex}
.ctl .seg button{border-radius:0}
.ctl .seg button:first-child{border-radius:6px 0 0 6px}
.ctl .seg button:last-child{border-radius:0 6px 6px 0}
.ctl .seg button.on{background:#00323a;border-color:#00e5ff;color:#fff}
.ctl input{flex:1;min-width:160px;accent-color:#ff00c8}
.ctl .tm{font:13px ui-monospace,monospace;color:#78828f;width:56px}
.note{border-left:3px solid #ff00c8;padding:2px 0 2px 12px}
"""


def page():
    js = open(os.path.join(HERE, "splash.js"), encoding="utf-8").read()
    out = [
        "<!doctype html><meta charset='utf-8'><title>Breach splash · round 1</title>",
        "<meta name='viewport' content='width=device-width,initial-scale=1'>",
        "<style>", face(400, "FiraCode-Regular.ttf"), face(700, "FiraCode-Bold.ttf"), CSS, "</style>",
        "<h1>Breach splash · round 1</h1>",
        "<p>A splash that plays each time the game starts, before the title: four or five "
        "seconds, and any key skips it. Each one ends with STRATA ZERO lit exactly where "
        "the title draws it, so it hands straight over to the title's menu with no cut. "
        "All drawn in cells (block characters as shapes, Fira Code, the game's colours), so "
        "it plays the same in the terminal and in the window.</p>",
        "<p class='note'>They loop. Click a screen (or <b>skip</b>) to do what a key would; "
        "the slider scrubs; <b>¼×</b> slows it down to see the frames; <b>100×30</b> is "
        "the smallest terminal. Names are our own (ARASU·TEK, ICEPICK, ORODYNE); nothing "
        "is lifted from 2077's screens.</p>",
    ]
    for key, name, tag, why, beats in CONCEPTS:
        out.append(f"<h2><span class='k'>{key}</span>{name}</h2>")
        out.append(f"<p class='tag'>{tag}</p><p>{why}</p><ol class='beats'>")
        for when, what in beats:
            out.append(f"<li><span class='w'>{when}</span><span>{what}</span></li>")
        out.append(f"</ol><div data-concept='{key}'></div>")
    out.append("<script>" + js + "</script>")
    return "\n".join(out)


if __name__ == "__main__":
    open(os.path.join(HERE, sys.argv[1] if len(sys.argv) > 1 else "round1.html"), "w",
         encoding="utf-8").write(page())
