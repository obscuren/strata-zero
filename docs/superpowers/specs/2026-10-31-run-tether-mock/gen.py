"""Run tether mockups: ways to hang a calm run's progress off its target.

    python3 gen.py options.html

Two rounds, 2026-10-31. Round one drew A-E (the callout, the tag, the live wire, the
dock, A's box on C's wire); round two showed B and C again with two blends (F, G).
C, the live wire, was chosen and built; the spec is 2026-10-31-run-tether-design.md.

shots/ holds two 120x36 frames as cells (x, y, symbol, fg, bg, bold), dumped by a
throwaway test in explore.rs's test module: a Medium city (seed 8), Novmed Media's
construct at security 120 with every tile remembered, the player beside a door lock,
`calm_idle` before the run and `calm_run` after `.`, half a beat and `1` `space` (a
crack one turn in). Each option is a list of frames over that base; page.html plays
them on a canvas, drawing box and block characters as shapes as the window does."""

import json
import os
import sys

S = os.environ.get("SHOTS", os.path.join(os.path.dirname(os.path.abspath(__file__)), "shots"))
W, H = 120, 36

BG = "#0a0b10"
TEXT = "#cdd4de"
CYAN = "#00e5ff"
MAGENTA = "#ff00c8"
DIM = "#a0aab9"
FAINT = "#5c6673"
GOOD = "#50e678"
WIN_BG = "#05090d"
EDGE = "#0e7c8a"
WIRE = "#0a8fa0"
WIRE_OFF = "#17434a"
BEAT_ON = "#00a5b8"
BEAT_OFF = "#183238"
LOCK_BG = "#003c46"
SLIP_BG = "#3d0030"
LAND_BG = "#0f3a1f"

ME = (33, 20)
TARGET = (33, 21)
TARGET_CH = "▪"
NAME = "door lock"
OP = "CRACK"
PCT = 60
WORK = 3


def load(name):
    g = [[[" ", TEXT, BG, False] for _ in range(W)] for _ in range(H)]
    for line in open(os.path.join(S, name)):
        x, y, ch, fg, bg, bold = line.rstrip("\n").split("\t")
        x, y = int(x), int(y)
        g[y][x] = [
            ch or " ",
            fg if fg.startswith("#") else TEXT,
            bg if bg.startswith("#") else BG,
            bold == "true",
        ]
    return g


class Frame:
    def __init__(self, base):
        self.base = base
        self.c = [[list(cell) for cell in row] for row in base]

    def put(self, x, y, s, fg, bg=None, bold=False):
        for i, ch in enumerate(s):
            if 0 <= x + i < W and 0 <= y < H:
                cell = self.c[y][x + i]
                cell[0], cell[1], cell[3] = ch, fg, bold
                if bg:
                    cell[2] = bg

    def fill(self, x, y, w, h, bg):
        for j in range(y, y + h):
            for i in range(x, x + w):
                self.c[j][i] = [" ", TEXT, bg, False]

    def tint(self, x, y, fg=None, bg=None, bold=None):
        cell = self.c[y][x]
        if fg:
            cell[1] = fg
        if bg:
            cell[2] = bg
        if bold is not None:
            cell[3] = bold

    def box(self, x, y, w, h, edge, title=None, title_fg=CYAN, right=None, right_fg=DIM):
        self.fill(x, y, w, h, WIN_BG)
        self.put(x, y, "┌" + "─" * (w - 2) + "┐", edge, WIN_BG)
        for j in range(1, h - 1):
            self.put(x, y + j, "│", edge, WIN_BG)
            self.put(x + w - 1, y + j, "│", edge, WIN_BG)
        self.put(x, y + h - 1, "└" + "─" * (w - 2) + "┘", edge, WIN_BG)
        if title:
            self.put(x + 2, y, f" {title} ", title_fg, WIN_BG, True)
        if right:
            self.put(x + w - 3 - len(right), y, f" {right} ", right_fg, WIN_BG)

    def diff(self, prev=None):
        """The cells that differ from `prev` (the base when None)."""
        prev = prev or self.base
        out = []
        for y in range(H):
            for x in range(W):
                if self.c[y][x] != prev[y][x]:
                    ch, fg, bg, bold = self.c[y][x]
                    out.append([x, y, ch, fg, bg, 1 if bold else 0])
        return out


# ---------------------------------------------------------------- the run's story

def story():
    """One loop: starts, a hit, a slip, a hit, the hit that lands, then quiet."""
    frames = []
    log = ["22:00 crack starts · 3 turns"]

    def beat(done, flash=None, heat=4):
        for phase in range(9):
            frames.append(dict(done=done, phase=phase, flash=flash if phase < 2 else None,
                               log=list(log), heat=heat, land=None, gone=False))

    frames.append(dict(done=0, phase=0, flash="start", log=list(log), heat=4, land=None, gone=False))
    beat(0)
    log.append("22:00 crack 1/3")
    beat(1, "hit", 5)
    log.append("22:00 crack slips · heat 7")
    beat(1, "miss", 7)
    log.append("22:01 crack 2/3")
    beat(2, "hit", 8)
    log.append("22:01 lock cracked · `Novmed Media`'s")
    for i in range(8):
        frames.append(dict(done=3, phase=0, flash=None, log=list(log), heat=9, land=i, gone=False))
    for i in range(5):
        frames.append(dict(done=3, phase=0, flash=None, log=list(log), heat=9, land=None, gone=True))
    return frames


def chrome(f, s):
    """What every option shares: no RUN in the footer, the log and the trace move."""
    # The sidebar log, newest last, from row 27.
    for i in range(4):
        f.put(80, 27 + i, " " * 40, DIM, BG)
    for i, line in enumerate(s["log"][-4:]):
        f.put(81, 27 + i, line[:38], DIM, BG)
    bar = "▰" * (s["heat"] // 10) + "▱" * (10 - s["heat"] // 10)
    f.put(81, 5, f"TRACE {bar} {s['heat']:<3}", f.base[5][81][1], BG)
    f.put(48, 32, f"TRACE {bar} {s['heat']:<3}", CYAN, BG)


def pips(done):
    return "▰" * done + "▱" * (WORK - done)


def beat_bar(phase, cells=8):
    filled = round(phase * cells / 8)
    return filled, cells - filled


def target_mark(f, s, idle_bg=LOCK_BG):
    x, y = TARGET
    if s["gone"]:
        return
    if s["land"] is not None:
        f.put(x, y, TARGET_CH, GOOD, LAND_BG, True)
    elif s["flash"] == "hit":
        f.put(x, y, TARGET_CH, BG, CYAN, True)
    elif s["flash"] == "miss":
        f.put(x, y, TARGET_CH, MAGENTA, SLIP_BG, True)
    else:
        f.put(x, y, TARGET_CH, CYAN, idle_bg, True)


def state_colour(s, idle):
    if s["land"] is not None:
        return GOOD
    if s["flash"] == "hit":
        return CYAN
    if s["flash"] == "miss":
        return MAGENTA
    return idle


# ---------------------------------------------------------------- A: the callout

def option_a(f, s):
    x, y, w, h = 7, 24, 24, 4
    target_mark(f, s)
    if s["gone"]:
        return
    edge = state_colour(s, EDGE)
    wire = state_colour(s, WIRE)
    # The leader: down-left off the lock, through the wall, into the box's edge.
    for (cx, cy) in [(32, 22), (31, 23)]:
        f.put(cx, cy, "╱", wire, None)
    if s["land"] is not None and s["land"] >= 6:
        # Folding: the title bar only, then nothing.
        if s["land"] == 6:
            f.box(x, y, w, 2, GOOD, "LOCK CRACKED", GOOD)
        return
    f.box(x, y, w, h, edge, f"{OP} · {NAME}", CYAN)
    f.put(x + w - 1, y, "┐", wire, WIN_BG)
    f.put(x + w - 6, y + 1, f"{PCT:>3}%", DIM, WIN_BG)
    if s["land"] is not None:
        f.put(x + 2, y, " LOCK CRACKED ", GOOD, WIN_BG, True)
        f.put(x + 2, y + 1, "doors open for an hour", TEXT, WIN_BG)
        f.put(x + 2, y + 2, "▰" * WORK + f" {WORK}/{WORK}", GOOD, WIN_BG)
        return
    p = pips(s["done"])
    for i, ch in enumerate(p):
        lit = i < s["done"]
        fresh = s["flash"] == "hit" and i == s["done"] - 1
        f.put(x + 2 + i, y + 1, ch, "#ffffff" if fresh else (CYAN if lit else FAINT), WIN_BG, fresh)
    f.put(x + 6, y + 1, f"{s['done']}/{WORK}", DIM, WIN_BG)
    if s["flash"] == "miss":
        f.put(x + 2, y + 2, "slips · heat +3", MAGENTA, WIN_BG)
    elif s["flash"] == "start":
        f.put(x + 2, y + 2, f"starts · {WORK} turns", CYAN, WIN_BG)
    else:
        on, off = beat_bar(s["phase"])
        f.put(x + 2, y + 2, "▓" * on, BEAT_ON, WIN_BG)
        f.put(x + 2 + on, y + 2, "░" * off, BEAT_OFF, WIN_BG)
        f.put(x + 12, y + 2, "next roll", FAINT, WIN_BG)


# ---------------------------------------------------------------- B: the tag

def option_b(f, s):
    y = TARGET[1]
    target_mark(f, s)
    if s["gone"]:
        return
    label = f" {OP} {NAME}  {pips(s['done'])}  {PCT}% "
    if s["land"] is not None:
        label = f" LOCK CRACKED · doors open "
    if s["flash"] == "miss":
        label = f" {OP} slips · heat +3 "
    w = 26
    x = 2
    if s["land"] is not None and s["land"] >= 6:
        if s["land"] == 6:
            f.put(x + w - 4, y, "▐██", GOOD, None)
        return
    # The band: its fill is the beat; it rolls when the fill reaches the end.
    on = round(s["phase"] * w / 8)
    for i in range(w):
        ch = label[i] if i < len(label) else " "
        if s["land"] is not None:
            f.put(x + i, y, ch, "#0a0b10", GOOD, True)
        elif s["flash"] == "hit":
            f.put(x + i, y, ch, "#0a0b10", CYAN, True)
        elif s["flash"] == "miss":
            f.put(x + i, y, ch, "#ffffff", "#7a0060", True)
        else:
            lit = i < on
            fg = CYAN if ch in "▰" else (TEXT if lit else DIM)
            if i <= len(OP) and ch != " ":
                fg = CYAN
            f.put(x + i, y, ch, fg, "#00566a" if lit else "#0b2a31", i <= len(OP))
    # The tie: a heavy line from the tag's end through the wall to the lock.
    tie = state_colour(s, WIRE)
    f.put(x + w, y, "▌", "#00566a" if on >= w else "#0b2a31", None)
    for cx in range(x + w + 1, TARGET[0]):
        f.put(cx, y, "━", tie, None)


# ---------------------------------------------------------------- C: the live wire

def option_c(f, s):
    y = TARGET[1]
    target_mark(f, s, BG)
    if s["gone"]:
        return
    x0, x1 = 17, TARGET[0] - 1  # the wire's cells, chip side first
    n = x1 - x0 + 1
    seg = n / WORK
    # The chip.
    cx = 5
    if not (s["land"] is not None and s["land"] >= 6):
        if s["land"] is not None:
            f.put(cx, y - 1, "CRACKED    ", GOOD, None, True)
            f.put(cx, y, "doors open ", TEXT, None)
        else:
            f.put(cx, y - 1, f"{OP}", CYAN, None, True)
            f.put(cx + 7, y - 1, f"{PCT}%", DIM, None)
            f.put(cx, y, f"{NAME}", DIM, None)
    # The wire: lit from the chip as turns land; the rest waits, faint.
    retract = None
    if s["land"] is not None and s["land"] >= 4:
        retract = (s["land"] - 3) * n // 4
    for i in range(n):
        if retract is not None and i >= n - retract:
            continue
        done_cells = round(s["done"] * seg)
        if s["land"] is not None:
            f.put(x0 + i, y, "━", GOOD, None)
        elif i < done_cells:
            fg = "#ffffff" if (s["flash"] == "hit" and i >= round((s["done"] - 1) * seg)) else CYAN
            f.put(x0 + i, y, "━", fg, None)
        elif s["flash"] == "miss":
            f.put(x0 + i, y, "╌", MAGENTA, None)
        else:
            f.put(x0 + i, y, "╌", WIRE_OFF, None)
    f.put(x0 - 1, y, "╺", CYAN if s["land"] is None else GOOD, None)
    # The packet: rides the wire from the chip to the lock over a beat.
    if s["land"] is None and s["flash"] not in ("hit", "miss", "start"):
        at = x0 + round(s["phase"] * (n - 1) / 8)
        f.put(at, y, "█", "#e6fdff", None)
        if at - 1 >= x0:
            f.tint(at - 1, y, fg="#7ff3ff")
    if s["flash"] == "miss":
        f.put(TARGET[0], y, "╳", MAGENTA, SLIP_BG, True)


# ---------------------------------------------------------------- D: the dock

def option_d(f, s):
    x, y, w, h = 2, 6, 30, 4
    tx, ty = TARGET
    gone = s["gone"]
    if not gone:
        target_mark(f, s, BG)
        ret = state_colour(s, CYAN)
        f.put(tx - 1, ty - 1, "┌", ret, None)
        f.put(tx + 1, ty - 1, "┐", ret, None)
        f.put(tx - 1, ty + 1, "└", ret, None)
        f.put(tx + 1, ty + 1, "┘", ret, None)
    if gone or (s["land"] is not None and s["land"] >= 6):
        return
    edge = state_colour(s, EDGE)
    f.box(x, y, w, h, edge, "RUN", CYAN)
    # The tether: down from the dock, along the lock's row, to the reticle.
    line = state_colour(s, WIRE)
    col = 12
    f.put(col, y + h - 1, "┬", edge, WIN_BG)
    for j in range(y + h, ty):
        f.put(col, j, "┆", line, None)
    f.put(col, ty, "└", line, None)
    for i in range(col + 1, tx - 1):
        f.put(i, ty, "┄", line, None)
    f.put(tx - 2, ty, "╴", line, None)
    if s["land"] is not None:
        f.put(x + 2, y + 1, "LOCK CRACKED", GOOD, WIN_BG, True)
        f.put(x + 2, y + 2, "doors open for an hour", TEXT, WIN_BG)
        return
    f.put(x + 2, y + 1, OP, CYAN, WIN_BG, True)
    f.put(x + 2 + len(OP), y + 1, f" › {NAME}", TEXT, WIN_BG)
    f.put(x + w - 6, y + 1, f"{PCT}%", DIM, WIN_BG)
    for i, ch in enumerate(pips(s["done"])):
        lit = i < s["done"]
        fresh = s["flash"] == "hit" and i == s["done"] - 1
        f.put(x + 2 + i, y + 2, ch, "#ffffff" if fresh else (CYAN if lit else FAINT), WIN_BG)
    f.put(x + 6, y + 2, f"{s['done']}/{WORK}", DIM, WIN_BG)
    if s["flash"] == "miss":
        f.put(x + 12, y + 2, "slips · heat +3", MAGENTA, WIN_BG)
    elif s["flash"] == "start":
        f.put(x + 12, y + 2, f"starts · {WORK} turns", CYAN, WIN_BG)
    else:
        on, off = beat_bar(s["phase"])
        f.put(x + 12, y + 2, "▓" * on, BEAT_ON, WIN_BG)
        f.put(x + 12 + on, y + 2, "░" * off, BEAT_OFF, WIN_BG)
        f.put(x + 21, y + 2, "roll", FAINT, WIN_BG)


# ---------------------------------------------------------------- E: A's box on C's wire

def option_e(f, s):
    y = TARGET[1]
    x, w = 1, 21
    target_mark(f, s, BG)
    if s["gone"]:
        return
    land = s["land"]
    if land is not None and land >= 6:
        if land == 6:
            f.box(x, y - 1, w, 2, GOOD, "LOCK CRACKED", GOOD)
        return
    edge = state_colour(s, EDGE)
    f.box(x, y - 1, w, 3, edge, f"{OP} · {NAME}", CYAN)
    if land is not None:
        f.put(x + 2, y - 1, " LOCK CRACKED ", GOOD, WIN_BG, True)
        f.put(x + 2, y, "doors open · 1h", TEXT, WIN_BG)
    elif s["flash"] == "miss":
        f.put(x + 2, y, "slips · heat +3", MAGENTA, WIN_BG)
    elif s["flash"] == "start":
        f.put(x + 2, y, f"starts · {WORK} turns", CYAN, WIN_BG)
    else:
        f.put(x + 2, y, f"{s['done']}/{WORK}", DIM, WIN_BG)
        f.put(x + w - 6, y, f"{PCT:>3}%", DIM, WIN_BG)
    # The wire: from the box's edge to the lock, lit as turns land.
    x0, x1 = x + w, TARGET[0] - 1
    n = x1 - x0 + 1
    seg = n / WORK
    f.put(x + w - 1, y, "├", edge, WIN_BG)
    retract = None
    if land is not None and land >= 4:
        retract = (land - 3) * n // 3
    done_cells = round(s["done"] * seg)
    for i in range(n):
        if retract is not None and i >= n - retract:
            continue
        if land is not None:
            f.put(x0 + i, y, "━", GOOD, None)
        elif i < done_cells:
            fresh = s["flash"] == "hit" and i >= round((s["done"] - 1) * seg)
            f.put(x0 + i, y, "━", "#ffffff" if fresh else CYAN, None)
        elif s["flash"] == "miss":
            f.put(x0 + i, y, "╌", MAGENTA, None)
        else:
            f.put(x0 + i, y, "╌", WIRE_OFF, None)
    if land is None and s["flash"] not in ("hit", "miss", "start"):
        at = x0 + round(s["phase"] * (n - 1) / 8)
        f.put(at, y, "█", "#e6fdff", None)
        if at - 1 >= x0:
            f.tint(at - 1, y, fg="#7ff3ff")
    if s["flash"] == "miss":
        f.put(TARGET[0], y, "╳", MAGENTA, SLIP_BG, True)


# ---------------------------------------------------------------- F: B's tag on C's wire

def band(f, x, y, w, label, s, lit=None):
    """B's band: the label on a dark teal band; cyan on a hit, magenta on a slip,
    green when it lands. `lit` cells from the left draw brighter."""
    for i in range(w):
        ch = label[i] if i < len(label) else " "
        if s["land"] is not None:
            f.put(x + i, y, ch, "#0a0b10", GOOD, True)
        elif s["flash"] == "hit":
            f.put(x + i, y, ch, "#0a0b10", CYAN, True)
        elif s["flash"] == "miss":
            f.put(x + i, y, ch, "#ffffff", "#7a0060", True)
        else:
            on = lit is not None and i < lit
            head = i <= len(OP) and ch != " "
            fg = CYAN if head else (TEXT if on else DIM)
            f.put(x + i, y, ch, fg, "#00566a" if on else "#0b2a31", head)


def wire(f, s, x0, x1, y):
    """C's wire from x0 to x1: lit as turns land, the packet riding it each beat."""
    land = s["land"]
    n = x1 - x0 + 1
    seg = n / WORK
    retract = None
    if land is not None and land >= 4:
        retract = (land - 3) * n // 3
    done_cells = round(s["done"] * seg)
    for i in range(n):
        if retract is not None and i >= n - retract:
            continue
        if land is not None:
            f.put(x0 + i, y, "━", GOOD, None)
        elif i < done_cells:
            fresh = s["flash"] == "hit" and i >= round((s["done"] - 1) * seg)
            f.put(x0 + i, y, "━", "#ffffff" if fresh else CYAN, None)
        elif s["flash"] == "miss":
            f.put(x0 + i, y, "╌", MAGENTA, None)
        else:
            f.put(x0 + i, y, "╌", WIRE_OFF, None)
    if land is None and s["flash"] not in ("hit", "miss", "start"):
        at = x0 + round(s["phase"] * (n - 1) / 8)
        f.put(at, y, "█", "#e6fdff", None)
        if at - 1 >= x0:
            f.tint(at - 1, y, fg="#7ff3ff")
    if s["flash"] == "miss":
        f.put(TARGET[0], y, "╳", MAGENTA, SLIP_BG, True)


def option_f(f, s):
    y = TARGET[1]
    x, w = 2, 21
    target_mark(f, s, BG)
    if s["gone"]:
        return
    land = s["land"]
    if land is not None and land >= 6:
        if land == 6:
            f.put(x + w - 3, y, "▐██", GOOD, None)
        return
    if land is not None:
        label = " LOCK CRACKED"
    elif s["flash"] == "miss":
        label = " slips · heat +3"
    elif s["flash"] == "start":
        label = f" {OP} · {WORK} turns"
    else:
        label = f" {OP} {NAME}  {PCT}%"
    band(f, x, y, w, label, s)
    wire(f, s, x + w, TARGET[0] - 1, y)


# ---------------------------------------------------------------- G: the bar is the wire

def option_g(f, s):
    y = TARGET[1]
    x = 2
    w = TARGET[0] - x  # right up to the lock, through the wall
    target_mark(f, s, BG)
    if s["gone"]:
        return
    land = s["land"]
    if land is not None:
        label = " LOCK CRACKED · doors open"
    elif s["flash"] == "miss":
        label = f" {OP} slips · heat +3"
    elif s["flash"] == "start":
        label = f" {OP} {NAME} · {WORK} turns"
    else:
        label = f" {OP} {NAME}  {s['done']}/{WORK}  {PCT}%"
    if land is not None and land >= 4:
        # Reeling in: the bar shortens from the lock back to its start.
        w = max(0, w - (land - 3) * w // 3)
    band(f, x, y, w, label, s, lit=round(s["done"] * w / WORK))
    # The beat: a bright stripe sweeping the bar toward the lock.
    if land is None and s["flash"] not in ("hit", "miss", "start"):
        at = x + round(s["phase"] * (w - 1) / 8)
        f.tint(at, y, bg="#9ff6ff", fg="#0a0b10")
        if at - 1 >= x:
            f.tint(at - 1, y, bg="#2fb8c8")
    if s["flash"] == "miss":
        f.put(TARGET[0], y, "╳", MAGENTA, SLIP_BG, True)


OPTIONS = [
    ("a", "The callout", option_a, [
        "A small boxed window in the nearest open space, its leader running off the target.",
        "Title names the program and what it works on; pips, odds and the beat bar inside.",
        "The window flashes cyan on a hit, magenta on a slip, green when it lands, then folds away.",
    ]),
    ("b", "The tag", option_b, [
        "One row, no box: a band beside the target, tied to it by a heavy line.",
        "The band's fill is the beat. When it reaches the end, the program rolls.",
        "Covers the least map. Short names only, so it reads at a glance.",
    ]),
    ("c", "The live wire (chosen)", option_c, [
        "The tether is the progress: each turn that lands lights another stretch of wire.",
        "A packet rides the wire toward the target each beat; when it arrives, the roll.",
        "A slip bursts at the target. Landing lights the whole wire green, then it reels in.",
    ]),
    ("d", "The dock", option_d, [
        "The window always sits in the map's top-left corner, so you know where to look.",
        "A reticle locks onto the target; a dotted tether runs from the dock to it.",
        "Room for more (odds, heat, the target's name) without ever covering the action.",
    ]),
    ("e", "A's box on C's wire", option_e, [
        "A small box says what is running and the odds; the wire from it to the target says how far along it is.",
        "Each beat a pulse rides the wire to the target, and the roll happens when it arrives. No pips, no beat bar.",
        "Hits light the wire, a slip bursts magenta at the target, a crack turns the wire green and it reels in.",
    ]),
    ("f", "The tag on the wire (B + C)", option_f, [
        "B's band says what is running and the odds, in one row on the target's line.",
        "C's wire runs from the band's end to the target: lit stretches are turns landed, the pulse is the beat.",
        "Everything stays on one row. The band flashes like B's, the wire and the target react like C's.",
    ]),
    ("g", "The bar is the wire (B + C)", option_g, [
        "One band from the label to the target: it fills toward the target as turns land.",
        "The beat is a bright stripe sweeping the band; when it reaches the target, the roll.",
        "When the crack lands, the band goes green and shortens back to the label.",
    ]),
]

LEDE = ("Two rounds of mockups for showing a calm run on the map instead of the footer. "
        "Round one drew A to E; round two showed B and C again with two blends, F and G. "
        "C, the live wire, was chosen and built. Each plays the same crack on a door lock "
        "in Novmed Media's construct: it starts, lands a turn, slips, lands, then cracks "
        "the lock. The RUN row is gone from the footer in all of them.")


def palette_encode(frames_by_option, base):
    """Colours to indices so the page stays small."""
    pal = {}

    def ix(c):
        if c not in pal:
            pal[c] = len(pal)
        return pal[c]

    b = [[[cell[0], ix(cell[1]), ix(cell[2]), 1 if cell[3] else 0] for cell in row] for row in base]
    enc = {}
    for key, frames in frames_by_option.items():
        enc[key] = [[[x, y, ch, ix(fg), ix(bg), bold] for x, y, ch, fg, bg, bold in fr] for fr in frames]
    return b, enc, [c for c, _ in sorted(pal.items(), key=lambda kv: kv[1])]


def main(out):
    today = load("calm_run_120x36.tsv")
    idle = load("calm_idle_120x36.tsv")
    base = [[list(c) for c in row] for row in today]
    # The footer without the RUN row, and the slots without the fired flash.
    base[34] = [list(c) for c in idle[34]]
    base[35] = [list(c) for c in idle[35]]
    st = story()
    frames = {}
    for key, _, draw, _ in OPTIONS:
        # Each frame as its change from the one before; the first from the base.
        fs, prev = [], None
        for s in st:
            f = Frame(base)
            chrome(f, s)
            draw(f, s)
            fs.append(f.diff(prev))
            prev = f.c
        frames[key] = fs
    # Today: the shot as it is, the RUN row marked.
    t = Frame(base)
    t.c = [[list(c) for c in row] for row in today]
    frames["today"] = [t.diff()]
    b, enc, pal = palette_encode(frames, base)
    data = dict(w=W, h=H, base=b, frames=enc, pal=pal, lede=LEDE, today=True,
                options=[dict(key=k, name=n, notes=notes) for k, n, _, notes in OPTIONS])
    page = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "page.html")).read()
    open(out, "w").write(page.replace("/*DATA*/null", json.dumps(data, ensure_ascii=False, separators=(",", ":"))))


if __name__ == "__main__":
    main(sys.argv[1])
