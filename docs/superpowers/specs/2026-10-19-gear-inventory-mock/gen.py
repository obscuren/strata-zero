"""Layout reference for docs/superpowers/specs/2026-10-19-gear-inventory-design.md.

The mockups Jeffrey approved ("Rack and icons, perfect and EXACTLY what I mean"), as
the code that drew them: the GEAR & INVENTORY tab at 100x30 with the rack in A, the
pack with its icon filter in B, and the card with the half-size drawing in C. The
frame, header and drawings come from the game's own shots; render them first:

    SHOT_DIR=/some/dir cargo test -p strata-zero shots:: -- --ignored
    SHOT_DIR=/some/dir python3 gen.py out.html          # every screen
    SHOT_DIR=/some/dir python3 gen.py out.html rack_gun # one

Columns and rows below are terminal cells; x ranges are half-open.
"""

import sys
from cells import CSS, Grid
from screen import *

A = (1, 49)      # the rack, x0..x1
B = (50, 99)     # the pack
TOP = (5, 14)    # rows of A and B: the rack's nine
HR = 14          # the rule over C
C = (15, 27)     # rows of C: twelve
CARD = (3, 41)   # card x and width
VIEW = (46, 52)  # viewport x and width
TILE_W = 16      # A's width over three

SLOTS = ["HEAD", "EYES", "NEURAL", "ARMS", "TORSO", "LEGS", "PRIMARY", "SECONDARY", "DECK"]
KINDS = [("/", "WEAPON"), ("▣", "ARMOR"), ("◈", "CYBERWARE"), ("⌗", "DECK"), ("◍", "CONSUMABLE"),
         ("▪", "COMPONENT"), ("·", "JUNK"), ("⌁", "PROGRAM"), ("≡", "HARDWARE")]
HINTS_RACK = "←↑↓→ rack, then over to the pack · [ ] filter · space act · tab tabs · esc close"
HINTS_PACK = "←→ gear/pack · ↑↓ select · [ ] filter · space act · tab tabs · esc close"


def worn(smg_fitted):
    """slot -> (name, marks: (program slots used, free, sockets filled))"""
    return {
        "EYES": ("Nightglass optics", None),
        "TORSO": ("Salvage jacket", None),
        "PRIMARY": ("Ratchet SMG", (0, 0, [smg_fitted, smg_fitted])),
        "DECK": ("Rebuilt pawn deck", (5, 0, [True, False])),
    }


PACK = [
    ("deck", [("⌗", 1, "Pawnshop deck", (0, 2, []))]),
    ("consumable", [("◍", 2, "Soy-noodle cup", None), ("◍", 1, "Stim patch", None)]),
    ("component", [("▪", 3, "Copper coil", None), ("▪", 1, "Burner shard", None),
                   ("▪", 2, "Scrap servo", None), ("▪", 1, "Keycard · your flat, Velform Group Plaza", None)]),
    ("junk", [("·", 1, "Dead phone", None), ("·", 12, "Bottle caps", None)]),
    ("program", [("⌁", 1, "Crack", None), ("⌁", 1, "Copy", None), ("⌁", 1, "Attack", None), ("⌁", 1, "Backdoor", None)]),
    ("hardware", [("≡", 1, "Coldfin vI", None), ("≡", 1, "Sidecar co-processor", None)]),
]
PACK_SPARE = [("≡", 1, "Hushcan", None), ("≡", 1, "Redeye sight", None)]


def mk(spec, sel):
    if not spec:
        return ()
    used, free, socks = spec
    return marks_spans(used, free, socks, sel)


def chips(g, y, active, focused, have):
    """ALL, then one icon per kind in ItemKind order; the active one names itself.
    Kinds the pack holds none of are faint."""
    x = B[0] + 1
    items = [("ALL", "ALL")] + KINDS
    for i, (icon, name) in enumerate(items):
        on = i == active
        text = ("▌%s▐" % (name if i == 0 else icon + " " + name)) if on else (" %s " % icon)
        col = (LIT if focused else DIM) if on else (DIM if (i == 0 or name in have) else FAINT)
        g.put(x, y, text, col, bold=on and focused)
        x += len(text)


def pack(g, active, cursor, focused, spare=False):
    """B: the chips, then the list. ALL keeps the kind headers; one kind drops them."""
    groups = [list(x) for x in PACK]
    if spare:
        groups[-1][1] = groups[-1][1] + PACK_SPARE
    have = {k.upper() for k, _ in groups}
    chips(g, TOP[0], active, focused, have)
    y = TOP[0] + 1
    n = 0
    kind = KINDS[active - 1][1].lower() if active else None
    for k, items in groups:
        if kind and k != kind:
            continue
        if not kind:
            if y >= TOP[1]:
                break
            g.put(B[0] + 3, y, "── %s ──" % k, DIM)
            y += 1
        for icon, qty, name, spec in items:
            if y >= TOP[1]:
                break
            sel = n == cursor
            row(g, B[0], B[1], y, "%s %2d× " % (icon, qty), name, mk(spec, sel and focused), sel, focused)
            y += 1
            n += 1


def tile(g, x, y, w, label, name, spec, sel, focused, empty):
    """One slot: its label in the top edge, the name inside, the marks in the bottom
    edge. The selected tile's edge is cyan while the rack has focus, else dim."""
    col = (CYAN if focused else DIM) if sel else RULE
    g.put(x, y, "┌ " + label + " " + "─" * (w - len(label) - 4) + "┐", col)
    g.put(x + 2, y, label, (LIT if focused else DIM) if sel else DIM, bold=sel and focused)
    g.put(x, y + 1, "│", col)
    g.put(x + w - 1, y + 1, "│", col)
    if sel and focused:
        g.bg(x + 1, y + 1, w - 2, BAR)
    g.put(x + 1, y + 1, cut(name, w - 2), FAINT if empty else (LIT if sel and focused else TEXT),
          bold=sel and focused)
    g.put(x, y + 2, "└" + "─" * (w - 2) + "┘", col)
    if spec:
        put_spans(g, x + 2, y + 2, [(" ", col)] + list(mk(spec, sel and focused)) + [(" ", col)], x + w - 1)


def rack(g, cursor, focused, smg_fitted):
    items = worn(smg_fitted)
    for i, s in enumerate(SLOTS):
        cx = A[0] + (i % 3) * TILE_W
        cy = TOP[0] + (i // 3) * 3
        name, spec = items.get(s, ("— empty —", None))
        tile(g, cx, cy, TILE_W, s, name, spec, i == cursor, focused, s not in items)


def panes(g, focus):
    """The divider, the rule over C with the focused pane's cyan segment, and the
    pack's weight and worth set into the rule's right end."""
    vrule(g, A[1], TOP[0], HR)
    hrule(g, 1, 99, HR)
    g.put(A[1], HR, "╩", RULE)
    x0, x1 = (A[0], A[1]) if focus == "A" else (B[0], B[1])
    for x in range(x0, x1):
        g.put(x, HR, "━", CYAN)
    tot = " 15.3 kg · ¤ 16909 "
    g.put(B[1] - len(tot) - 2, HR, tot, DIM)


def c_panel(g, lines, art=None, missing=None, caption="space inspect"):
    card(g, CARD[0], C[0], CARD[1], C[1] - C[0], lines)
    viewport(g, VIEW[0], C[0], VIEW[1], C[1] - C[0], art, caption, missing)


def card_lines(name, kind, kg, value, cyan=(), sockets=(), state=None, desc=""):
    """The item card: kind, slot, weight and value share the second line."""
    out = [(name, CYAN, True), ("%s · %s kg · ¤ %s" % (kind, kg, value), DIM, False)]
    out += [(c, CYAN, False) for c in cyan]
    out += [(s, CYAN if s.startswith("■") else DIM, False) for s in sockets]
    if state:
        out.append((state, CYAN, False))
    out.append(("", TEXT, False))
    out.append((desc, TEXT, False))
    return out


RATCHET = dict(name="Ratchet SMG", kind="weapon · primary", kg="3.9", value="2150",
               sockets=["■ Hushcan · cool +1", "■ Redeye sight · reflex +1"], state="equipped",
               desc="Stamped-receiver machine pistol with a filed-down selector. Jams on cheap ammo, sings on the good stuff.")
HUSHCAN = dict(name="Hushcan", kind="hardware · muzzle socket", kg="0.5", value="700", cyan=["cool +1"],
               desc="A baffle can that threads onto a muzzle. The shot still happens; the street just takes longer to turn round.")
DECK = dict(name="Rebuilt pawn deck", kind="deck · deck", kg="2.4", value="7600",
            cyan=["+2 net", "grants Netscan · Arc trace"],
            sockets=["■ Coldfin vII · trace per cast −20%", "□ empty socket · deck hardware"], state="equipped",
            desc="The pawnshop shell, gutted and rebuilt by someone who knew what they were doing. Five slots, and it still runs hot.")
OPTICS = dict(name="Nightglass optics", kind="cyberware · eyes", kg="0.2", value="2200",
              cyan=["+4 sight", "grants Low-light"], state="equipped",
              desc="Low-light lenses pulled from a security contractor's surplus. The left one still shows a corporate boot logo on cold start.")


def rack_gun():
    g = base()
    rack(g, 6, True, True)
    pack(g, 0, 0, False)
    panes(g, "A")
    c_panel(g, card_lines(**RATCHET), art=preview("ratchet_fitted", "mode"))
    hints(g, HINTS_RACK)
    return g


def rack_deck():
    g = base()
    rack(g, 8, True, True)
    pack(g, 0, 0, False)
    panes(g, "A")
    c_panel(g, card_lines(**DECK), art=preview("deck", "mode"))
    hints(g, HINTS_RACK)
    return g


def rack_missing():
    g = base()
    rack(g, 1, True, True)
    pack(g, 0, 0, False)
    panes(g, "A")
    c_panel(g, card_lines(**OPTICS), missing=("◈", "MISSING BLUEPRINT"))
    hints(g, HINTS_RACK)
    return g


def pack_filtered():
    g = base()
    rack(g, 6, False, False)
    pack(g, 9, 2, True, spare=True)
    panes(g, "B")
    c_panel(g, card_lines(**HUSHCAN), art=preview("hushcan", "mode"))
    hints(g, HINTS_PACK)
    return g


SCREENS = {
    "rack_gun": rack_gun,
    "rack_deck": rack_deck,
    "rack_missing": rack_missing,
    "pack_filtered": pack_filtered,
}

if __name__ == "__main__":
    out = sys.argv[1]
    which = sys.argv[2:] or list(SCREENS)
    page = ["<html><head><meta charset=utf-8><style>body{background:#0b0d12;margin:16px}%s</style></head><body>" % CSS]
    for k in which:
        page.append("<p style='color:#888;font:12px sans-serif'>%s</p>" % k)
        page.append(SCREENS[k]().html())
    page.append("</body></html>")
    open(out, "w").write("".join(page))
