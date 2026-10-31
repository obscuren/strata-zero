"""Talent icons: 6x6 pixels, three cells of stacked half blocks.

'.' is the icon's own tile; 'c' is the tree's colour, 'C' the tree's colour lifted.
"""

MAT = {
    "G": "#b4bec8", "g": "#6b717b", "k": "#3a3f47", "w": "#f2f6fa", "y": "#e8c54a",
    "Y": "#a07c1a", "b": "#c8d2dc", "r": "#ff3b5c", "o": "#ff8a3c",
}

ICONS = {
    # EDGE
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
}
