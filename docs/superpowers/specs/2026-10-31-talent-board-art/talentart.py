"""The talents' icons: 6x6 pixels, drawn three cells tall in stacked half blocks.

    python3 talentart.py export flavor/talent_art.ron   # what the game loads
    python3 talentart.py sheet out.html                 # every icon, lit and greyed

One pixel map a talent: `.` is empty (the chip's plate shows), `c` the tree's colour,
`C` the tree's colour lifted toward white, any other char a material from MAT. The
export bakes the tree's colour in, so the game reads plain colours.
"""

import html
import sys

W = H = 6

# Materials, as the skill and chrome art have them.
MAT = {
    "G": "#b4bec8",  # bright metal
    "g": "#6b717b",  # gunmetal
    "k": "#3a3f47",  # dark polymer
    "w": "#f2f6fa",  # white
    "y": "#e8c54a",  # brass
    "Y": "#a07c1a",  # dark brass, wood
    "b": "#c8d2dc",  # blade steel
    "r": "#ff3b5c",  # red
    "o": "#ff8a3c",  # orange heat
}

# The trees' colours: the attributes' as the deck draws them.
TREES = {
    "MEAT": "#78ff78",
    "EDGE": "#00e5ff",
    "WRENCH": "#e6b43c",
    "STREET": "#ff00c8",
    "ICE": "#6e96ff",
}

ICONS = {
    "MEAT": {
        # a shield, green rim
        "thick_skin": ["cccccc",
                       "cGGGGc",
                       "cGGGGc",
                       ".cGGc.",
                       ".cGGc.",
                       "..cc.."],
        # a crate with a handle
        "long_haul": ["..gg..",
                      ".g..g.",
                      "YYYYYY",
                      "YyyyyY",
                      "YyYYyY",
                      "YYYYYY"],
        # a fist, knuckles up
        "iron_grip": [".GGGG.",
                      "GgGgGG",
                      "GGGGGG",
                      ".GGGGc",
                      ".GGGG.",
                      "..gg.."],
        # two lungs on a windpipe
        "second_lung": ["..ww..",
                        ".cwwc.",
                        "cc..cc",
                        "cc..cc",
                        "cc..cc",
                        ".c..c."],
        # a fist coming in, fast
        "haymaker": ["c.....",
                     ".c.GGG",
                     "c.GGGG",
                     ".cGGgG",
                     "c..GGG",
                     "......"],
        # bricks, and the way up them
        "wall_runner": ["gg..c.",
                        "g.gccc",
                        "gg..c.",
                        "g.g.c.",
                        "gg..c.",
                        "g.g.c."],
        # a syringe, green in it
        "pain_editor": ["......",
                        "g.wcc.",
                        "ggwccG",
                        "g.wcc.",
                        "......",
                        "......"],
        # horns
        "bull_rush": ["c....c",
                      "cc..cc",
                      ".GGGG.",
                      ".GrrG.",
                      "..GG..",
                      "......"],
        # a cat's face
        "cat_fall": ["G....G",
                     "GG..GG",
                     "GGGGGG",
                     "GcGGcG",
                     ".GGGG.",
                     "......"],
        # a plate, stitched
        "scar_tissue": ["GGGGGG",
                        "GwGGwG",
                        "rrrrrr",
                        "GwGGwG",
                        "GGGGGG",
                        "......"],
        # a heavy helmet, visor lit
        "juggernaut": [".GGGG.",
                       "GGGGGG",
                       "GccccG",
                       "GGGGGG",
                       "GG..GG",
                       "g....g"],
        # a heart with a circuit in it
        "meat_machine": ["rr..rr",
                         "rrrrrr",
                         "rcccrr",
                         ".rrcr.",
                         "..rr..",
                         "......"],
    },
    "EDGE": {
        # a pistol, muzzle flash at the end
        "quick_draw": ["......",
                       "GGGGGc",
                       "GGGGG.",
                       ".gg...",
                       "gg....",
                       "......"],
        # a scope's ring, a dot in it
        "steady_hand": [".cccc.",
                        "c....c",
                        "c.ww.c",
                        "c.ww.c",
                        "c....c",
                        ".cccc."],
        # two chevrons: already moving
        "light_step": ["c..c..",
                       ".c..c.",
                       "..c..c",
                       "..w..w",
                       ".c..c.",
                       "c..c.."],
        # two rounds
        "double_tap": ["GG.GG.",
                       "yy.yy.",
                       "yy.yy.",
                       "yy.yy.",
                       "YY.YY.",
                       "......"],
        # a rifle with its scope lit
        "long_shot": ["......",
                      ".cc...",
                      "GGGGGG",
                      "gg.g..",
                      "g.....",
                      "......"],
        # a knife, point down, a drop on it
        "backstab": ["..YY..",
                     "..YY..",
                     ".gggg.",
                     "..bb..",
                     "..bb..",
                     "..b.r."],
        # a bolt
        "hair_trigger": ["...cc.",
                         "..cc..",
                         ".cccww",
                         "ww.cc.",
                         "..cc..",
                         ".c...."],
        # an hourglass run out
        "bullet_time": ["GGGGGG",
                        ".k..k.",
                        "..cc..",
                        "..cc..",
                        ".cccc.",
                        "GGGGGG"],
        # two blades crossed
        "counter_cut": ["b....b",
                        ".b..b.",
                        "..bb..",
                        "..bb..",
                        ".Y..Y.",
                        "Y....Y"],
        # three rounds fanning out of one muzzle
        "fan_fire": ["....cw",
                     "..cc..",
                     "GGcwww",
                     "..cc..",
                     "....cw",
                     "......"],
        # a wire pulled taut between two grips, a glint on it
        "razor_wire": ["YY....",
                       "Y.w...",
                       "..cw..",
                       "...wc.",
                       "....w.",
                       "....YY"],
        # a stopwatch, half its face lit
        "half_second": ["..gg..",
                        ".GGGG.",
                        "GccwkG",
                        "GcckkG",
                        ".GGGG.",
                        "......"],
    },
    "WRENCH": {
        # a soldering iron and a bead of solder
        "solder_hand": ["......",
                        "YYYgg.",
                        "YYYggG",
                        ".....c",
                        "....c.",
                        "......"],
        # an eye, gold iris
        "scrap_eye": ["......",
                      ".GGGG.",
                      "GwccwG",
                      "GwccwG",
                      ".GGGG.",
                      "......"],
        # a red cross on white
        "field_medic": ["wwrrww",
                        "wwrrww",
                        "rrrrrr",
                        "rrrrrr",
                        "wwrrww",
                        "wwrrww"],
        # a chip running hot
        "overclocker": ["o.o.o.",
                        ".o.o.o",
                        "GGGGGG",
                        "GccccG",
                        "GGGGGG",
                        "g.g.g."],
        # a roll of tape, a strip off it
        "jury_rig": [".GGG..",
                     "GG.GG.",
                     "G...G.",
                     "GG.GGc",
                     ".GGGcc",
                     "....c."],
        # a flask
        "stim_chemist": ["..GG..",
                         "..GG..",
                         ".G..G.",
                         "GccccG",
                         "GccccG",
                         ".GGGG."],
        # an arc between two posts
        "arc_lash": ["G....G",
                     "Gcw..G",
                     "G..wcG",
                     "Gcw..G",
                     "G..wcG",
                     "gg..gg"],
        # a wrench
        "tinker": ["...G.G",
                   "...GGG",
                   "..GG..",
                   ".GG...",
                   "cc....",
                   "c....."],
        # a plaster
        "trauma_kit": ["......",
                       ".wwww.",
                       "wwrrww",
                       "wwrrww",
                       ".wwww.",
                       "......"],
        # a plug, sparking
        "live_wire": [".G..G.",
                      ".G..Gc",
                      "GGGGGG",
                      "GGGGGG",
                      "..gg.c",
                      "..gg.."],
        # a chrome hand, gold joints
        "chrome_whisperer": [".G.G.G",
                             ".G.G.G",
                             "GGGGGG",
                             "GcGcGG",
                             ".GGGG.",
                             ".gggg."],
        # a ghost
        "machine_ghost": [".cccc.",
                          "cccccc",
                          "cwccwc",
                          "cccccc",
                          "cccccc",
                          "c.cc.c"],
    },
    "STREET": {
        # a hood, eyes in it
        "ghost": [".gggg.",
                  "gggggg",
                  "gc..cg",
                  "g....g",
                  "gggggg",
                  "gg..gg"],
        # a speech bubble
        "silver_tongue": [".GGGG.",
                          "G....G",
                          "G.cc.G",
                          "G....G",
                          ".GGGG.",
                          ".G...."],
        # a polygraph's line
        "cold_read": ["......",
                      "..c...",
                      ".c.c..",
                      "c...c.",
                      ".....c",
                      "gggggg"],
        # steps, fading
        "shadow_step": ["....GG",
                        "....GG",
                        "..gg..",
                        "..gg..",
                        "kk....",
                        "kk...."],
        # a key
        "skeleton_key": ["......",
                         "ccc...",
                         "c.GGGG",
                         "ccc.G.",
                         "....G.",
                         "......"],
        # sound coming in
        "ear_to_the_ground": ["...c..",
                              ".c..c.",
                              "G.c.c.",
                              "G.c.c.",
                              ".c..c.",
                              "...c.."],
        # a smoke bomb going off
        "smoke": ["..ggg.",
                  ".gGGGg",
                  "gGGGGg",
                  ".gggg.",
                  "..cc..",
                  ".cccc."],
        # a coin
        "fixers_favour": [".yyyy.",
                          "yYYYYy",
                          "yYccYy",
                          "yYccYy",
                          "yYYYYy",
                          ".yyyy."],
        # a snowflake
        "ice_cold": ["w.ww.w",
                     ".wwww.",
                     "wwccww",
                     "wwccww",
                     ".wwww.",
                     "w.ww.w"],
        # a camera, and you under it
        "blind_spot": ["g.....",
                       "gGGGG.",
                       "gGGGGr",
                       "g.....",
                       "....c.",
                       "...ccc"],
        # a stare
        "stare_down": ["c....c",
                       ".c..c.",
                       "GG..GG",
                       "Gw..Gw",
                       "......",
                       "......"],
        # a crown
        "the_name": ["......",
                     "c.cc.c",
                     "cccccc",
                     "cccccc",
                     "yyyyyy",
                     "......"],
    },
    "ICE": {
        # an ice pick
        "ice_pick": ["GGGGGG",
                     "G.cc.G",
                     "..cc..",
                     "..cc..",
                     "..cc..",
                     "..YY.."],
        # an antenna, sending
        "wide_band": ["c....c",
                      ".c..c.",
                      "..GG..",
                      "..GG..",
                      "..GG..",
                      ".GGGG."],
        # a power button
        "cold_boot": ["..cc..",
                      "c.cc.c",
                      "c....c",
                      "c....c",
                      ".cccc.",
                      "......"],
        # an arrow down into a tray
        "deep_dive": ["..cc..",
                      "..cc..",
                      "cccccc",
                      ".cccc.",
                      "G.cc.G",
                      "GGGGGG"],
        # a terminal, answering
        "handshake": ["GGGGGG",
                      "Gc...G",
                      "GcccwG",
                      "GGGGGG",
                      "..gg..",
                      ".gggg."],
        # a spark
        "arc_welder": ["..cc..",
                       "c.cc.c",
                       ".cwwc.",
                       ".cwwc.",
                       "c.cc.c",
                       "..cc.."],
        # a route forking away
        "null_route": ["....cc",
                       "...c..",
                       "cccc..",
                       "...g..",
                       "....gg",
                       "......"],
        # a rat
        "vault_rat": ["......",
                      "GG....",
                      "GGGGG.",
                      "GcGGGG",
                      ".GGGG.",
                      "..g..g"],
        # a black shard
        "black_ice": ["..cw..",
                      ".ckkc.",
                      "ckkkkc",
                      "ckkkkc",
                      ".ckkc.",
                      "..cc.."],
        # a jaw closing on a signal
        "trace_eater": ["GGGGGG",
                        "G.G.G.",
                        "..cc..",
                        ".G.G.G",
                        "GGGGGG",
                        "......"],
        # static
        "static_bloom": ["c.w.cw",
                         ".cw.c.",
                         "wc.wc.",
                         ".w.c.w",
                         "c.wc.c",
                         ".c.w.c"],
        # a tunnel going down
        "deep_end": ["CCCCCC",
                     "C....C",
                     "C.cc.C",
                     "C.cc.C",
                     "C....C",
                     "CCCCCC"],
    },
}


def hexrgb(c):
    return tuple(int(c[i:i + 2], 16) for i in (1, 3, 5))


def mix(a, b, t):
    ra, rb = hexrgb(a), hexrgb(b)
    return "#%02x%02x%02x" % tuple(round(ra[k] + (rb[k] - ra[k]) * t) for k in range(3))


def palette(tree):
    p = dict(MAT)
    p["c"] = TREES[tree]
    p["C"] = mix(TREES[tree], "#ffffff", 0.5)
    return p


def icons():
    """Every icon: (talent, tree, rows)."""
    for tree, by in ICONS.items():
        for tid, rows in by.items():
            yield tid, tree, rows


def check():
    seen = set()
    for tid, tree, rows in icons():
        assert tid not in seen, "%s drawn twice" % tid
        seen.add(tid)
        assert len(rows) == H and all(len(r) == W for r in rows), "%s is not %dx%d" % (tid, W, H)
        for r in rows:
            for ch in r:
                assert ch == "." or ch in palette(tree), "%s: no colour for %r" % (tid, ch)


CHARS = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"


def export():
    out = ["// Every talent's icon, 6x6 pixels. Generated by\n"
           "// docs/superpowers/specs/2026-10-31-talent-board-art/talentart.py: draw there, not here.\n"
           "{\n"]
    for tid, tree, rows in icons():
        pal = palette(tree)
        chars, body = {}, []
        for r in rows:
            line = ""
            for ch in r:
                if ch == ".":
                    line += "."
                    continue
                colour = pal[ch]
                if colour not in chars:
                    chars[colour] = CHARS[len(chars)]
                line += chars[colour]
            body.append('            "%s",' % line)
        colours = ", ".join("'%s': (%d, %d, %d)" % ((ch,) + hexrgb(c)) for c, ch in chars.items())
        out.append('    "%s": (\n        rows: [\n%s\n        ],\n        colours: {%s},\n    ),\n'
                   % (tid, "\n".join(body), colours))
    out.append("}\n")
    return "".join(out)


def grey(c, k=0.38):
    r, g, b = hexrgb(c)
    v = (0.299 * r + 0.587 * g + 0.114 * b) * k
    return "#%02x%02x%02x" % (round(v), round(v), min(255, round(v * 1.08)))


def sheet():
    """Every icon on its plate, lit and greyed, as cells of stacked half blocks."""
    panel = "#04060a"

    def cell_html(rows, pal, plate, lit):
        out = []
        for cy in range(H // 2):
            out.append("<div class=r>")
            for cx in range(W):
                t, b = rows[2 * cy][cx], rows[2 * cy + 1][cx]
                tc = plate if t == "." else pal[t]
                bc = plate if b == "." else pal[b]
                if not lit:
                    tc, bc = grey(tc), grey(bc)
                out.append("<i style='background:linear-gradient(%s 50%%,%s 50%%)'></i>" % (tc, bc))
            out.append("</div>")
        return "".join(out)

    parts = ["<!DOCTYPE html><meta charset=utf-8><title>Talent icons</title><style>"
             "body{background:%s;color:#cdd4de;font:13px Menlo,monospace;margin:20px}"
             "h2{font-size:14px;margin:18px 0 8px}.row{display:flex;flex-wrap:wrap;gap:14px}"
             ".ic{display:flex;flex-direction:column;align-items:center;gap:4px;width:96px}"
             ".pair{display:flex;gap:6px}.r{display:flex}"
             "i{display:block;width:9.6px;height:20px}</style>" % panel]
    for tree, by in ICONS.items():
        pal = palette(tree)
        plate = mix(panel, TREES[tree], 0.16)
        parts.append("<h2 style='color:%s'>%s</h2><div class=row>" % (TREES[tree], tree))
        for tid, rows in by.items():
            parts.append("<div class=ic><div class=pair><div>%s</div><div>%s</div></div>%s</div>"
                         % (cell_html(rows, pal, plate, True), cell_html(rows, pal, plate, False),
                            html.escape(tid)))
        parts.append("</div>")
    return "".join(parts)


if __name__ == "__main__":
    check()
    what, path = sys.argv[1], sys.argv[2]
    open(path, "w").write(export() if what == "export" else sheet())
