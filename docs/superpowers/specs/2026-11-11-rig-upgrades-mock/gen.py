"""Rig upgrades, mockups round 1, drawn over the game's own frames (seed 8, VEX at home).

    python3 -I gen.py [1|2]     writes round1.html or round2.html beside it

1 · Fitting parts, three ways:
    A · on the blueprint: the rig in the blueprint window's modding mode, like any item.
    B · at the desk: the map becomes the desk, the ports on the left, the part on the right.
    C · SETUP on the glass: the rig's own setup screen, inside its terminal.
2 · Jobs that run while you sleep: `ps` and `run` on the rig, and the overnight mail.
3 · The chassis: the same glass in each of the city's four machines.

The frames in shots/ come from the shot tests the_rig_at_home, modding_the_rebuilt_deck,
mail_shots, terminal_on_the_street and terminals_by_theme."""

import html
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SPECS = os.path.join(HERE, "..")
sys.path.insert(0, os.path.join(SPECS, "2026-10-19-gear-inventory-mock"))
sys.path.insert(0, os.path.join(SPECS, "2026-11-02-minimap-mock"))
sys.path.insert(0, os.path.join(SPECS, "2026-11-10-armor-art-mock"))
from cells import Grid  # noqa: E402
import term  # noqa: E402
from px import Canvas, mix  # noqa: E402

SHOTS = os.path.join(HERE, "shots")
ROUND = sys.argv[1] if len(sys.argv) > 1 else "2"

# the blueprint window
WIN = "#04060a"
CYAN = "#00e5ff"
CYAN_DIM = "#00a5b8"
CARD = "#2b3138"
CARD_TX = "#b4bec8"
TEXT = "#cdd4de"
DIM = "#6b7686"
FAINT = "#3a414c"
WHITE = "#e6f0f5"
GREEN = "#50e678"
# the rig: the street theme, a Zenbo
BODY = "#1c1a1e"
EDGE = "#783e28"
GLASS = "#140e06"
AMBER = "#ffb000"
AMBER2 = "#e09b00"
AMBER_DIM = "#8c6014"
AMBER_FAINT = "#4a3410"
SIDE = "#040406"


def frame(name):
    return Grid.from_svg(os.path.join(SHOTS, name), 120, 40)


# ---- the Zenbo 64: a wedge with its cards standing in ports along the top

W, H = 60, 30
PORTS = [("CORE", 14), ("RELAY", 30), ("LINE", 46)]
KINDS = {
    "CORE": dict(base="#1d5c3a", hi="#2f7d52", lo="#123a25"),
    "RELAY": dict(base="#1d3f5c", hi="#2f5f86", lo="#122638"),
    "LINE": dict(base="#7a4f26", hi="#b87a3e", lo="#4a2f16"),
    "FIREWALL": dict(base="#6a1e1e", hi="#943030", lo="#3e1010"),
}
STICKER = "#d8d0b8"
STICKER_TX = "#1a1410"


def cartridge(c, kind, cx, model):
    k = KINDS[kind]
    x0 = cx - 6
    c.rect(x0, 1, x0 + 11, 10, k["base"])
    c.hline(x0, x0 + 11, 1, k["hi"])
    c.vline(x0, 1, 10, k["hi"])
    c.vline(x0 + 11, 1, 10, k["lo"])
    for x in range(x0 + 2, x0 + 10, 2):
        c.px(x, 2, k["lo"])
    c.rect(x0 + 1, 4, x0 + 10, 7, STICKER)
    if kind == "CORE":
        c.rect(x0 + 3, 8, x0 + 8, 9, "#0c0c0e")
        c.hline(x0 + 3, x0 + 8, 8, "#2a2a30")
    elif kind == "RELAY":
        for i in range(3):
            c.px(x0 + 3 + i * 3, 9, GREEN)
    elif kind == "LINE":
        for x in range(x0 + 2, x0 + 10):
            c.px(x, 8 + (x % 2), "#e0a060")
    else:
        for x in range(x0 + 1, x0 + 11):
            c.px(x, 9, k["lo"] if x % 3 else k["hi"])
    name = kind if len(kind) <= 10 else kind[:10]
    c.label(x0 + 1 + (10 - len(name)) // 2, 2, name, STICKER_TX, True)
    c.label(x0 + 1 + (10 - len(model)) // 2, 3, model, "#5a4a38", False)


def zenbo(dim=False, fitted=None):
    """The rig, front on. `fitted` maps a port to its card's model; a port left out is
    empty. `dim` greys the body, as modding greys an item's drawing."""
    fitted = {"CORE": "ZB-8", "RELAY": "R-3", "LINE": "copper"} if fitted is None else fitted
    d = (lambda col: mix(col, WIN, 0.3)) if dim else (lambda col: col)
    c = Canvas(W, H)
    body, top, lo = d(BODY), d("#2a262d"), d("#141216")
    key, key_hi = d("#2c2930"), d("#3d3942")
    c.poly([(3, 10), (56, 10), (59, 29), (0, 29)], body)
    c.rect(2, 10, 57, 13, top)
    # the ports' mouths, and the cards standing in them
    for kind, cx in PORTS:
        c.rect(cx - 7, 10, cx + 6, 11, "#070608")
    for kind, cx in PORTS:
        if kind in fitted:
            cartridge(c, kind, cx, fitted[kind])
    # keys
    for r in range(3):
        y = 15 + r * 3
        for i in range(13):
            x = 5 + i * 4 + (r % 2) * 2
            if x + 2 > 54:
                continue
            if r == 2 and 18 <= x <= 38:
                continue
            c.rect(x, y, x + 2, y + 1, key)
            c.hline(x, x + 2, y, key_hi)
    c.rect(19, 21, 40, 22, key)
    c.hline(19, 40, 21, key_hi)
    # the front: a rust hairline with gaps, the badge, the lamp, a crack or two
    for x in range(1, 59):
        if x % 7:
            c.px(x, 24, d(EDGE))
    c.rect(1, 25, 58, 29, lo)
    c.line(8, 25, 11, 27, d("#3a3438"))
    c.line(11, 27, 10, 29, d("#3a3438"))
    c.line(41, 11, 44, 13, d("#3a3438"))
    c.rect(53, 26, 54, 27, AMBER)
    c.label(4, 13, "ZENBO 64", d(AMBER_DIM), True)
    return c


# ---- shared drawing helpers

def half_box(g, x0, y0, x1, y1, col, bg, title=None):
    g.fill(x0, y0, x1 - x0 + 1, y1 - y0 + 1, bg)
    g.put(x0, y0, "▗" + "▄" * (x1 - x0 - 1) + "▖", col)
    for y in range(y0 + 1, y1):
        g.put(x0, y, "▐", col, bg)
        g.put(x1, y, "▌", col, bg)
    g.put(x0, y1, "▝" + "▀" * (x1 - x0 - 1) + "▘", col)
    if title:
        g.put(x0 + 4, y0, " " + title + " ", col, bg, bold=True)


def dashed(g, x0, y0, x1, y1, col, bg=None, bottom=True):
    g.put(x0, y0, "┌" + "╌" * (x1 - x0 - 1) + "┐", col, bg)
    for y in range(y0 + 1, y1 if bottom else y1 + 1):
        g.put(x0, y, "╎", col, bg)
        g.put(x1, y, "╎", col, bg)
    if bottom:
        g.put(x0, y1, "└" + "╌" * (x1 - x0 - 1) + "┘", col, bg)


def glass(g, rows=range(10, 30), bg=GLASS):
    for y in rows:
        for x in range(26, 94):
            g.c[y][x] = [" ", AMBER, bg, False]


def lines(g, y, items, x=28, bg=GLASS):
    for i, it in enumerate(items):
        if isinstance(it, str):
            it = (it, AMBER)
        text, fg = it[0], it[1]
        bold = len(it) > 2 and it[2]
        g.put(x, y + i, text, fg, bg, bold=bold)


# ---- 1 · fitting parts

def concept_a():
    g = frame("rig.svg")
    x0, y0, x1, y1 = 10, 4, 109, 35
    half_box(g, x0, y0, x1, y1, CYAN, WIN, "MODDING · ZENBO 64 · YOUR RIG")
    # what the rig does now, as the deck shows its programs; the pick previewed
    chips = [("JOBS", "1 at once", None), ("LINES", "3", "5"), ("HOLD", "3 days", None),
             ("FIREWALL", "none", None)]
    cx = 31
    for name, now, then in chips:
        g.fill(cx, 7, 13, 2, CARD)
        g.put(cx + 1, 7, name, CARD_TX, CARD, bold=True)
        g.put(cx + 1, 8, now, CARD_TX, CARD)
        if then:
            g.put(cx + 1 + len(now), 8, " ▸ " + then, GREEN, CARD, bold=True)
        cx += 15
    art = zenbo(dim=True).cells(WIN)
    ax, ay = 30, 10
    g.paste(art, ax, ay, skip_bg=WIN)
    # the RELAY port picked
    rx = ax + 30 - 7
    dashed(g, rx, ay - 1, rx + 13, ay + 5, CYAN, bottom=False)
    for i, (kind, px) in enumerate(PORTS):
        g.put(ax + px - 1, ay - 1, " " + "ABC"[i] + " ", CYAN if kind == "RELAY" else DIM, WIN)
    g.put(ax + 2, ay + 16, "3 ports · core · relay · line", DIM, WIN)
    g.put(ax + 34, ay + 16, "no firewall port", FAINT, WIN)
    # the tray: what fits the picked port
    g.put(14, 30, "PARTS", DIM, WIN, bold=True)
    cards = [("SWITCHBACK", "relay · lines 5", True), ("HYDRA", "relay · lines 8", False)]
    tx = 24
    for name, what, picked in cards:
        g.fill(tx, 29, 24, 2, CARD)
        g.put(tx, 29, "▐▌", KINDS["RELAY"]["hi"], CARD)
        g.put(tx, 30, "▐▌", KINDS["RELAY"]["base"], CARD)
        g.put(tx + 3, 29, name, WHITE, CARD, bold=True)
        g.put(tx + 3, 30, what, CARD_TX, CARD)
        if picked:
            g.put(tx - 1, 28, "┏", CYAN_DIM, WIN)
            g.put(tx + 24, 28, "┓", CYAN_DIM, WIN)
            g.put(tx - 1, 31, "┗", CYAN_DIM, WIN)
            g.put(tx + 24, 31, "┛", CYAN_DIM, WIN)
        tx += 28
    g.put(14, 32, "SWITCHBACK", CYAN, WIN, bold=True)
    g.put(24, 32, " ▸ port B · lines 3 ▸ 5 · the old R-3 goes back in your pack", TEXT, WIN)
    g.put(14, 34, "←→ parts · space fit · x pull · esc back", DIM, WIN)
    return g


def concept_b(art=None, at=(19, 12), box=None):
    """The desk; `art` a Canvas of the rig and `box` its picked port in pixels, else the
    round-one Zenbo with its CORE card picked."""
    g = frame("rig.svg")
    wall, desk, desk_front = "#0a090c", "#2b2119", "#16110d"
    g.fill(0, 5, 79, 30, wall)
    g.put(2, 6, "ZENBO 64", AMBER, wall, bold=True)
    g.put(11, 6, "· your rig, on the desk · 3 ports", AMBER_DIM, wall)
    g.put(2, 9, "PORTS", AMBER_DIM, wall, bold=True)
    rows = [("CORE", "ZB-8", True), ("RELAY", "R-3", False), ("LINE", "copper", False)]
    for i, (kind, model, picked) in enumerate(rows):
        y = 11 + i * 2
        bg = AMBER_FAINT if picked else wall
        g.fill(1, y, 18, 1, bg)
        g.put(2, y, ("▸ " if picked else "  ") + kind, AMBER if picked else AMBER2, bg, bold=picked)
        g.put(11, y, model, AMBER if picked else AMBER_DIM, bg)
    g.put(2, 17, "─" * 16, AMBER_FAINT, wall)
    g.put(4, 18, "FIREWALL", AMBER_FAINT, wall)
    g.put(4, 19, "no port on", AMBER_FAINT, wall)
    g.put(4, 20, "this chassis", AMBER_FAINT, wall)
    ax, ay = at
    if art is None:
        g.paste(zenbo().cells(wall), ax, ay, skip_bg=wall)
        dashed(g, ax + 14 - 7, ay - 1, ax + 14 + 6, ay + 5, AMBER, wall, bottom=False)
    else:
        g.paste(art.cells(wall), ax, ay, skip_bg=wall)
        x0, y0, x1, y1 = box
        dashed(g, ax + x0 - 1, ay + y0 // 2 - 1, ax + x1 + 1, ay + y1 // 2 + 1, AMBER)
    # the desk under it, a lead off to the wall
    g.fill(0, 27, 79, 1, desk)
    g.put(0, 27, "▀" * 79, "#3a2d22", desk)
    g.fill(0, 28, 79, 7, desk_front)
    sep = g.c[33][79][1]
    for y in range(5, 35):
        g.put(79, y, "│", sep, WIN)
    # the part on the right, where the side panel sits
    g.fill(80, 5, 40, 30, SIDE)
    lines(g, 6, [("CORE · ZB-8", AMBER, True), ("came with the box", AMBER_DIM)], x=82, bg=SIDE)
    g.put(81, 9, "─" * 38, "#1a1c22", SIDE)
    lines(g, 10, [("jobs at once        1", TEXT), ("job speed           ×1", TEXT)], x=82, bg=SIDE)
    g.put(81, 13, "─" * 38, "#1a1c22", SIDE)
    g.put(82, 14, "FITS HERE · IN YOUR PACK", DIM, SIDE, bold=True)
    g.fill(81, 16, 38, 2, "#141a20")
    lines(g, 16, [("▸ TINMAN 2X core", WHITE, True), ("    jobs 1 ▸ 2 · ¤ 640", GREEN)], x=82,
          bg="#141a20")
    lines(g, 19, [("  Q-STACK 4 core", CARD_TX, True), ("    jobs 1 ▸ 3 · speed ×1.5", CARD_TX)],
          x=82, bg=SIDE)
    g.put(81, 22, "─" * 38, "#1a1c22", SIDE)
    g.put(82, 23, "space fit · x pull", DIM, SIDE)
    g.fill(0, 36, 120, 1, WIN)
    g.put(1, 36, "at your rig · ↑↓ ports · space what fits · x pull · esc stand up", DIM, WIN)
    return g


def concept_c():
    g = frame("rig.svg")
    glass(g)
    g.put(28, 10, " ZENBO 64 SETUP 2.2 ", GLASS, AMBER, bold=True)
    g.put(78, 10, "(C) ZENBO", AMBER_DIM, GLASS)
    lines(g, 12, [
        ("CHASSIS   ZENBO 64 · STREET · 3 PORTS", AMBER2),
        ("═" * 64, AMBER_DIM),
    ])
    rows = [
        ("PORT 1", "CORE", "ZB-8 CORE", "JOBS ", 1, 3, "1"),
        ("PORT 2", "RELAY", "R-3 RELAY", "LINES", 3, 8, "3"),
        ("PORT 3", "LINE", "COPPER LINE", "HOLD ", 3, 7, "3D"),
    ]
    for i, (port, kind, part, stat, n, most, val) in enumerate(rows):
        y = 14 + i
        picked = i == 0
        fg, bg = (GLASS, AMBER) if picked else (AMBER, GLASS)
        g.fill(27, y, 66, 1, bg)
        g.put(28, y, port, fg, bg)
        g.put(36, y, ("▸ " if picked else "  ") + kind, fg, bg, bold=picked)
        g.put(48, y, part, fg, bg)
        g.put(67, y, stat, fg, bg)
        g.put(73, y, "▮" * n + "·" * (most - n), fg, bg)
        g.put(83, y, val, fg, bg, bold=True)
    lines(g, 17, [
        ("--        FIREWALL    NO PORT", AMBER_DIM),
        ("═" * 64, AMBER_DIM),
        ("FITS PORT 1 · IN YOUR PACK", AMBER2),
        ("  TINMAN 2X CORE      JOBS 1 > 2", AMBER),
        ("  Q-STACK 4 CORE      JOBS 1 > 3 · SPEED X1.5", AMBER),
        "",
        ("MEM CHK 64K .. OK?", AMBER_DIM),
    ])
    lines(g, 26, [("═" * 64, AMBER_DIM), ("↑↓ PORT   SPACE SWAP   X PULL   ESC SAVE + REBOOT", AMBER2)])
    return g


# ---- 2 · jobs while you sleep

def jobs_shell():
    g = frame("rig.svg")
    glass(g)
    lines(g, 10, [
        ("▲ PGUP", AMBER_DIM),
        ("ZENBO rig · your flat", AMBER2),
        "Authorised users only. Sessions are logged.",
        "",
        "YOU HAVE 1 NEW MAIL · TYPE mail",
        "2 JOBS RAN WHILE YOU SLEPT · TYPE ps",
        ("vex@zenbo:~$ ps", AMBER2),
        (" #  JOB        ON                      UP      FOUND", AMBER_DIM),
        " 1  sniff      hollis-court            7h 12m  2 logins · 1 code",
        " 2  sift       velform paydata shard   done    1 stash · 1 lead",
        ("TINMAN 2X core · 1 of 2 seats busy", AMBER_DIM),
        ("vex@zenbo:~$ run keepalive lumnet-industries", AMBER2),
        "keepalive on lumnet-industries · its hold stays full while it runs",
        ("noise: low · their admins read the logs on Sundays", AMBER_DIM),
        ("vex@zenbo:~$ run rent", AMBER2),
        "run: both seats are busy · kill a job, or fit a bigger core",
        ("vex@zenbo:~$ ▮", AMBER2, True),
    ])
    return g


def jobs_mail():
    g = frame("mail_read_120x40.svg")
    glass(g, range(10, 27))
    g.put(28, 10, " HELIX mail 2.2 ", GLASS, AMBER, bold=True)
    g.put(45, 10, "1 of 3 · NEW", AMBER, GLASS)
    lines(g, 12, [
        "From:     cron@zenbo",
        "Subject:  overnight · 2 jobs",
        "Time:     06:00",
        "",
        ("sniff · hollis-court · 7h 12m, still running", AMBER, True),
        "  login  mara-voss · user                in your journal",
        "  login  ops-3 · admin                   in your journal",
        "  code   4471 · service door, 2F         in your journal",
        ("  their auth.log is getting long. somebody reads those.", AMBER_DIM),
        "",
        ("sift · velform paydata shard · done in 5h 40m", AMBER, True),
        "  stash  locker 12 · Docks fish market   on your map",
        "  lead   Velform moves a vault on the 14th · Dez would pay",
    ])
    g.put(28, 27, "[ 2 new left ]", AMBER_DIM, GLASS)
    return g


# ---- 3 · the chassis

CHASSIS = [
    ("rig.svg", "ZENBO 64", "street · what you have",
     [("HELIX/OS 2.2 .. OK?", AMBER2), ("MEM CHK 64K", AMBER2), ("REDY", AMBER2), "",
      ("PORTS   core · relay · line", AMBER), ("FROM    came with the flat", AMBER_DIM)]),
    ("terminal.svg", "CIVITAS KIOSK 9", "municipal · the careful one",
     [("HELIX/OS 4.1 · YOUR FLAT SUBNET", None), ("MEMORY CHECK OK", None), ("READY", None), "",
      ("PORTS   core · relay · line · firewall", None),
      ("FROM    unbolted from a street corner at 4am", "dim")]),
    ("terminal_neon.svg", "LOTUS MARQUEE", "neon · the busy one",
     [("HELIX/OS 8.0 · YOUR FLAT ~ STAY A WHILE ~", None), ("ALL SYSTEMS LIT", None),
      ("READY", None), "", ("PORTS   core · core · relay · line", None),
      ("FROM    a night-market back room, no receipt", "dim")]),
    ("terminal_megacorp.svg", "HALCYON NODE 9", "megacorp · everything",
     [("HELIX/OS 9.4 · YOUR FLAT SECURE NODE", None), ("INTEGRITY VERIFIED", None),
      ("WELCOME", None), "", ("PORTS   core · core · relay · line · firewall", None),
      ("FROM    a corp server room, down the service lift", "dim")]),
]


def chassis(name, rows):
    f = frame(name)
    gl = f.c[20][50][2]
    text = f.c[20][28][1] if f.c[20][28][0] != " " else TEXT
    # the theme's text colours, read off the glass
    seen = [f.c[y][x][1] for y in range(10, 30) for x in range(28, 90) if f.c[y][x][0] != " "]
    bright = max(set(seen), key=seen.count) if seen else text
    dim = mix(bright, gl, 0.45)
    g = Grid(72, 11)
    for j, y in enumerate([8, 9] + list(range(10, 18)) + [31]):
        for i in range(72):
            g.c[j][i] = list(f.c[y][24 + i])
    for j in range(2, 10):
        for i in range(2, 70):
            g.c[j][i] = [" ", bright, gl, False]
    for k, r in enumerate(rows):
        if isinstance(r, str):
            continue
        t, fg = r[0], r[1]
        fg = dim if fg == "dim" else (bright if fg is None else fg)
        g.put(4, 2 + k, t, fg, gl)
    return g


# ---- the page

def section(title, text, g, px=12.0, note=None):
    n = f"<p class='note'>{html.escape(note)}</p>" if note else ""
    return (f"<section><h2>{html.escape(title)}</h2><p>{text}</p>"
            f"<div class='frame'>{term.render(g, px)}</div>{n}</section>")


def main():
    parts = ["<h1>Rig upgrades · round 1</h1>",
             "<p class='lead'>Seed 8, VEX at home. The rig is a machine you fit parts into "
             "(<b>A</b>), and the parts decide how many jobs it can run while you're out "
             "(<b>B</b>). The firewall card is in. Three questions on this page: where you "
             "fit parts, how jobs read in HELIX, and what a better chassis looks like.</p>",
             "<h1 class='part'>1 · Fitting parts: three ways</h1>"]
    parts.append(section(
        "A · on the blueprint",
        "At the rig, <code>space</code> then <code>m Mod the rig</code> opens the same "
        "blueprint window every item uses, in modding mode. The ports are the cards standing "
        "in the Zenbo's back; the tray lists what in your pack fits the port you picked, and "
        "the chips on top preview what changes. Most familiar; nothing new to learn.",
        concept_a()))
    parts.append(section(
        "B · at the desk",
        "Sitting at the rig turns the map into the desk, as the ripperdoc's chair does for "
        "your body: the ports listed on the left, the machine in the middle, the part and "
        "what you could fit on the right. Feels like working on <i>your</i> machine, at home, "
        "and only at home.",
        concept_b()))
    parts.append(section(
        "C · SETUP on the glass",
        "The rig's own setup program, inside its terminal: <code>setup</code> in HELIX, or "
        "a key at boot. Ports, bars for what each gives, what fits from your pack. Pure "
        "text, in the chassis' own voice: a Halcyon's setup would be clean and white.",
        concept_c()))
    parts.append("<h1 class='part'>2 · Jobs that run while you sleep</h1>")
    parts.append(section(
        "On the rig: ps and run",
        "The core decides how many jobs run at once (two seats on a Tinman). "
        "<code>sniff</code> sits on a backdoor and pulls logins and codes; "
        "<code>sift</code> digs through a paydata shard for leads; <code>keepalive</code> "
        "holds a backdoor open; <code>rent</code> sells spare cycles for a trickle of "
        "credits. Every job on someone else's server makes noise there.",
        jobs_shell()))
    parts.append(section(
        "In the morning: the overnight mail",
        "Wake up, sit down, read what the rig caught. What it finds goes where it belongs: "
        "logins and codes in the journal, a stash on the map, a lead toward a fixer.",
        jobs_mail()))
    parts.append("<h1 class='part'>3 · The chassis: what the rig looks like as it grows</h1>")
    parts.append("<p class='lead'>Each chassis is one of the city's four machines brought "
                 "home, and it decides which ports you get. The careful one has a firewall, "
                 "the busy one two cores, the corp one both. You swap the box; the cards move "
                 "over.</p><div class='grid'>")
    for name, model, what, rows in CHASSIS:
        g = chassis(name, rows)
        parts.append(f"<div><h3>{html.escape(model)} <span>{html.escape(what)}</span></h3>"
                     f"<div class='frame'>{term.render(g, 11.0)}</div></div>")
    parts.append("</div>")
    page = f"""<!doctype html><html><head><meta charset="utf-8"><title>Rig upgrades</title>
<style>{term.CSS}
body{{background:#0b0d12;color:#cdd4de;font-family:-apple-system,system-ui,sans-serif;margin:24px;max-width:1100px}}
h1{{font-size:20px;margin:8px 0}} h1.part{{font-size:17px;color:#ffb000;margin:44px 0 4px;border-top:1px solid #283c4b;padding-top:18px}}
h2{{font-size:15px;color:#00e5ff;margin:26px 0 6px}}
h3{{font-size:13px;color:#e6f0f5;margin:16px 0 6px}} h3 span{{color:#6b7686;font-weight:400}}
p{{max-width:880px;color:#9aa4b2;margin:0 0 10px;line-height:1.45}} p.lead{{color:#b4bec8}}
code{{color:#ffb000}}
.frame{{display:inline-block;border:1px solid #283c4b}}
.grid{{display:grid;grid-template-columns:repeat(auto-fill,minmax(520px,1fr));gap:4px 24px}}
</style></head><body>
{''.join(parts)}
</body></html>"""
    out = os.path.join(HERE, "round1.html")
    open(out, "w").write(page)
    print("wrote", out)


# ---- round 2: the rig, more street than home computer

K0, K1, K2, K3, K4 = "#0b0a0d", "#141318", "#1f1d24", "#2b2832", "#3a3644"
ST0, ST1, ST2, ST3 = "#23252b", "#3a3d45", "#555a66", "#7a808c"
PCB, PCB2, PCB_HI = "#0c1814", "#11241d", "#1d3a2f"
GOLD = "#9a7a2a"
NEON = {"CORE": CYAN, "RELAY": GREEN, "LINE": AMBER, "FIREWALL": "#ff3a5a"}
MAG = "#ff2a8a"
CORD, CORD_HI = "#18171c", "#34323b"
HAZ = "#e8c020"
WALL = "#0a090c"
MODELS = {"CORE": "ZB-8", "RELAY": "R-3", "LINE": "CU-1"}


def thick(c, pts, col, hi=None):
    for (x0, y0), (x1, y1) in zip(pts, pts[1:]):
        c.line(x0, y0, x1, y1, col)
        c.line(x0, y0 + 1, x1, y1 + 1, col)
        if hi:
            c.line(x0, y0, x1, y1, hi)


def hazard(c, x0, y0, x1, y1):
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            c.px(x, y, HAZ if (x + y) % 4 < 2 else K0)


def bare_board():
    """D · no case at all: cards standing in a riser on a scrap plate, a fan and a brick of
    a power supply zip-tied beside them, the jack's cord off the end."""
    c = Canvas(58, 40)
    # the plate, bolted, on stubby feet
    c.rect(1, 35, 56, 38, ST1)
    c.hline(1, 56, 35, ST2)
    c.hline(1, 56, 38, ST0)
    for x in (3, 28, 54):
        c.px(x, 36, ST3)
    c.rect(3, 39, 6, 39, K1)
    c.rect(51, 39, 54, 39, K1)
    # the riser the cards stand in
    c.rect(1, 31, 44, 34, PCB)
    for x in range(2, 44, 3):
        c.px(x, 33, GOLD)
    boxes = {}
    for i, kind in enumerate(["CORE", "RELAY", "LINE"]):
        x0 = 4 + i * 14
        n = NEON[kind]
        c.rect(x0 - 1, 31, x0 + 12, 32, K0)
        c.rect(x0, 9, x0 + 11, 30, PCB2)
        c.vline(x0, 9, 30, PCB_HI)
        # the steel bracket, its tab bent over
        c.vline(x0 - 1, 7, 31, ST2)
        c.rect(x0 - 2, 7, x0, 7, ST3)
        # a lit edge, and its glow on the air above
        c.hline(x0 + 1, x0 + 11, 9, n)
        c.hline(x0 + 2, x0 + 10, 8, mix(n, WALL, 0.75))
        # heatsink fins
        c.rect(x0 + 2, 11, x0 + 10, 18, ST1)
        for y in range(11, 19, 2):
            c.hline(x0 + 2, x0 + 10, y, ST2)
        c.rect(x0 + 2, 20, x0 + 4, 21, K0)
        c.rect(x0 + 7, 20, x0 + 10, 21, K0)
        c.px(x0 + 5, 22, GOLD)
        c.px(x0 + 9, 23, GOLD)
        c.hline(x0 + 1, x0 + 10, 30, GOLD)
        c.label(x0 + 1 + (11 - len(kind)) // 2, 12, kind, n, True)
        c.label(x0 + 1 + (11 - len(MODELS[kind])) // 2, 13, MODELS[kind], mix(n, PCB2, 0.45), False)
        boxes[kind] = (x0 - 2, 7, x0 + 11, 31)
    # the fan, its hub lit
    c.rect(46, 6, 57, 17, K2)
    for y in range(6, 18):
        for x in range(46, 58):
            r2 = (x - 51.5) ** 2 + (y - 11.5) ** 2
            if r2 <= 22:
                c.px(x, y, K0)
            elif r2 <= 30:
                c.px(x, y, K3)
    c.line(48, 8, 55, 15, K3)
    c.line(55, 8, 48, 15, K3)
    c.rect(51, 11, 52, 12, MAG)
    for x, y in ((46, 6), (57, 6), (46, 17), (57, 17)):
        c.px(x, y, K4)
    # the power brick under it, taped
    c.rect(46, 19, 57, 34, ST0)
    for y in (21, 23, 25):
        c.hline(48, 55, y, ST1)
    hazard(c, 47, 28, 56, 31)
    c.rect(56, 22, 57, 23, AMBER)
    # leads: the brick into the riser, the jack's cord off the end of the plate
    c.line(45, 30, 44, 30, "#c03030")
    c.line(45, 31, 44, 31, K0)
    thick(c, [(44, 33), (48, 34), (52, 33), (55, 31), (57, 31)], CORD, CORD_HI)
    c.rect(56, 29, 57, 30, ST3)
    c.px(57, 28, CYAN)
    return c, boxes


def slab():
    """E · a tall black slab with a smoked window on its cards, a neon seam, taped glass,
    and the jack's cord hanging off its side."""
    c = Canvas(58, 40)
    x0, x1, y0, y1 = 3, 44, 0, 36
    c.rect(x0, y0, x1, y1, K1)
    c.hline(x0, x1, y0, K3)
    c.vline(x0, y0, y1, K3)
    c.vline(x1, y0, y1, K0)
    c.hline(x0, x1, y1, K0)
    # the neon seam down its face
    c.vline(5, 2, 34, MAG)
    c.vline(6, 2, 34, mix(MAG, K1, 0.6))
    # the window and the cards behind it
    gx0, gx1, gy0, gy1 = 9, 41, 3, 29
    c.rect(gx0 - 1, gy0 - 1, gx1 + 1, gy1 + 1, K3)
    tint = "#0b0d14"
    c.rect(gx0, gy0, gx1, gy1, tint)
    boxes = {}
    for i, kind in enumerate(["CORE", "RELAY", "LINE"]):
        bx = gx0 + 2 + i * 10
        n = NEON[kind]
        body = mix(PCB2, tint, 0.3)
        c.rect(bx, gy0 + 3, bx + 8, gy1 - 1, body)
        c.hline(bx, bx + 8, gy0 + 3, mix(n, tint, 0.15))
        c.hline(bx + 1, bx + 7, gy0 + 2, mix(n, tint, 0.75))
        c.vline(bx, gy0 + 3, gy1 - 1, mix(n, tint, 0.5))
        for y in range(gy0 + 11, gy0 + 21, 2):
            c.hline(bx + 2, bx + 7, y, mix(ST1, tint, 0.25))
        c.rect(bx + 2, gy0 + 22, bx + 4, gy0 + 23, K0)
        c.label(bx + (9 - len(kind) + 1) // 2, 3, kind, mix(n, tint, 0.05), True)
        c.label(bx + (9 - len(MODELS[kind]) + 1) // 2, 4, MODELS[kind], mix(n, tint, 0.45), False)
        boxes[kind] = (bx, gy0 + 2, bx + 8, gy1 - 1)
    # a crack across the glass's corner, taped over
    c.line(gx1 - 10, gy1, gx1 - 6, gy1 - 5, "#3a4256")
    c.line(gx1 - 6, gy1 - 5, gx1, gy1 - 8, "#3a4256")
    for i in range(8):
        c.px(gx1 - 7 + i, gy1 - i, HAZ if i % 2 == 0 else K0)
        c.px(gx1 - 6 + i, gy1 - i, HAZ if i % 2 else K0)
    # its foot: a maker's stamp, the lamp
    c.label(10, 16, "ZENBO", AMBER_DIM, True)
    c.rect(39, 32, 40, 33, AMBER)
    c.rect(x0 + 2, 37, x0 + 6, 37, K0)
    c.rect(x1 - 6, 37, x1 - 2, 37, K0)
    c.hline(x0 - 2, x1 + 2, 38, mix(MAG, WALL, 0.82))
    c.hline(x0 + 2, x1 - 2, 39, mix(MAG, WALL, 0.9))
    # the jack: its cord off the side, the plug hanging
    thick(c, [(45, 20), (49, 21), (53, 25), (55, 30), (54, 33)], CORD, CORD_HI)
    c.rect(53, 34, 56, 36, ST2)
    c.hline(53, 56, 34, ST3)
    c.rect(54, 37, 55, 37, CYAN)
    return c, boxes


def rack():
    """F · blade sleds in a bent half-rack on a milk crate, a head unit with a little LCD,
    the loom running off to the wall."""
    c = Canvas(58, 40)
    # the crate
    blue, blue_hi, blue_lo = "#1d3d6b", "#2b5590", "#0f2340"
    c.rect(4, 31, 53, 39, blue)
    c.hline(4, 53, 31, blue_hi)
    for y in (33, 36):
        for x in range(6, 52, 4):
            c.rect(x, y, x + 1, y + 1, blue_lo)
    # the rails
    for rx in (5, 50):
        c.rect(rx, 0, rx + 2, 30, ST1)
        c.vline(rx, 0, 30, ST2)
        for y in range(2, 30, 3):
            c.px(rx + 1, y, K0)
    # the head unit
    c.rect(8, 1, 49, 6, K2)
    c.hline(8, 49, 1, K3)
    c.rect(10, 2, 30, 5, "#081410")
    c.label(11, 1, "ZENBO 64  REDY", "#50e678", False)
    c.rect(44, 3, 45, 4, AMBER)
    c.rect(40, 2, 41, 5, ST2)
    c.px(40, 2, ST3)
    boxes = {}
    for i, kind in enumerate(["CORE", "RELAY", "LINE"]):
        y = 8 + i * 8
        n = NEON[kind]
        c.rect(8, y, 49, y + 6, K2)
        c.hline(8, 49, y, K3)
        c.hline(8, 49, y + 6, K0)
        # the pull handle
        c.rect(9, y + 1, 11, y + 5, ST2)
        c.rect(10, y + 2, 10, y + 4, K0)
        # vents
        for x in range(38, 48, 2):
            c.vline(x, y + 2, y + 4, K0)
        # its lamps along the bottom
        for k in range(6):
            c.px(14 + k * 2, y + 5, n if k % 3 != 2 else mix(n, K2, 0.6))
        c.label(14, (y + 2) // 2, kind, n, True)
        c.label(14 + len(kind) + 2, (y + 2) // 2, MODELS[kind], mix(n, K2, 0.45), False)
        boxes[kind] = (8, y, 49, y + 6)
    # a sticker on the bottom sled
    hazard(c, 30, 26, 35, 27)
    # the loom off to the wall
    for k, col in enumerate([CORD, ST2, "#2a6a7a"]):
        y = 10 + k * 8
        c.line(53, y, 55, y + 4, col)
        c.line(55, y + 4, 57, y + 12, col)
        c.line(52, y, 53, y, col)
    return c, boxes


def round2():
    options = [
        ("D · the bare board",
         "No case at all. The cards stand in a riser bolted to a scrap plate, each with its "
         "bracket, fins and a lit edge. A fan and a taped-up power brick are zip-tied beside "
         "them, and the jack's cord runs off the end. Pure street: every upgrade is another "
         "board on the plate.", bare_board()),
        ("E · the slab",
         "A tall black slab with a neon seam and a smoked window onto the cards. The glass "
         "is cracked and taped, and the jack hangs off the side on its cord. Sleek, but "
         "still a street machine.", slab()),
        ("F · the rack",
         "Three blade sleds in a bent half-rack on a milk crate, under a head unit with a "
         "little green LCD. Each port is a sled with a pull handle and lamps in its card's "
         "colour, and the loom runs off to the wall. Industrial and stacked, like a server "
         "someone carried home.", rack()),
    ]
    parts = ["<h1>Rig upgrades · round 2 · the rig, more cyberpunk</h1>",
             "<p class='lead'>B, the desk, with three new drawings of the Zenbo 64 in place "
             "of the round-one wedge. Each card keeps its colour on every chassis: CORE "
             "cyan, RELAY green, LINE amber, and FIREWALL red when a box has the port. "
             "The CORE port is picked.</p>"]
    for name, text, (art, boxes) in options:
        parts.append(section(name, text, concept_b(art, (20, 7), boxes["CORE"])))
    page = f"""<!doctype html><html><head><meta charset="utf-8"><title>Rig upgrades · round 2</title>
<style>{term.CSS}
body{{background:#0b0d12;color:#cdd4de;font-family:-apple-system,system-ui,sans-serif;margin:24px;max-width:1100px}}
h1{{font-size:20px;margin:8px 0}}
h2{{font-size:15px;color:#00e5ff;margin:26px 0 6px}}
p{{max-width:880px;color:#9aa4b2;margin:0 0 10px;line-height:1.45}} p.lead{{color:#b4bec8}}
.frame{{display:inline-block;border:1px solid #283c4b}}
</style></head><body>
{''.join(parts)}
</body></html>"""
    out = os.path.join(HERE, "round2.html")
    open(out, "w").write(page)
    print("wrote", out)


if __name__ == "__main__":
    main() if ROUND == "1" else round2()
