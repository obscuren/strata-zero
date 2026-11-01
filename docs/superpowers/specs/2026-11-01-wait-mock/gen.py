"""Wait and sleep, round 1: the bed's window, then three ways to choose when you wake
and to watch the night go by, over the game's own frame.

    SHOTS=dir/ python3 -I gen.py out.html

SHOTS holds wait_bed_120x40.svg (120x40), from a throwaway shot test: your flat just
after the house call, both scenes over, you beside the bed at 22:01.

A · Choices: a window of four wake times; asleep, the game keeps its screen.
B · Timeline: a strip of the next day, now and your wake time on it; asleep, it fills.
C · Big clock: a clock you set; asleep, the screen goes dark and the clock runs."""

import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "2026-10-19-gear-inventory-mock"))
from cells import Grid, CSS  # noqa: E402

S = os.environ.get("SHOTS", "shots/")
W, H = 120, 40
PLAYER = (39, 20)
MAP = (0, 5, 79, 35)  # x0, y0, x1, y1 of the map in the frame

PANEL = "#04060a"
TEXT = "#cdd4de"
DIM = "#a0aab9"
FAINT = "#5c6673"
CYAN = "#00e5ff"
TEAL = "#00e6d2"
SEL_BG = "#003c46"
SEL_FG = "#00a5b8"
MAGENTA = "#ff00c8"

NOW, WAKE, ASLEEP = "22:01", "07:00", "03:12"
SLEPT = 0.58  # 22:01 to 03:12 of 22:01 to 07:00


def hexrgb(c):
    return tuple(int(c[i:i + 2], 16) for i in (1, 3, 5))


def scale(c, f):
    r, g, b = hexrgb(c)
    return "#%02x%02x%02x" % (int(r * f), int(g * f), int(b * f))


def minutes(hhmm):
    h, m = hhmm.split(":")
    return int(h) * 60 + int(m)


def wash(g, f=0.35, rect=None):
    """Darkens cells as the game's wash does: odd rows a little more, for scanlines."""
    x0, y0, x1, y1 = rect or (0, 0, g.w, g.h)
    for y in range(y0, y1):
        k = f * (0.8 if y % 2 else 1.0)
        for x in range(x0, x1):
            c = g.c[y][x]
            c[1], c[2] = scale(c[1], k), scale(c[2], k)


def load():
    return Grid.from_svg(os.path.join(S, "wait_bed_120x40.svg"), W, H)


def window(g, x0, y0, x1, y1, title, glyph="▤"):
    """A window as the game draws one: heavy top and bottom, light sides, the thing's
    glyph and name in the top."""
    g.fill(x0, y0, x1 - x0 + 1, y1 - y0 + 1, PANEL)
    g.put(x0, y0, "┍" + "━" * (x1 - x0 - 1) + "┑", CYAN)
    g.put(x0 + 2, y0, " ", TEXT)
    g.put(x0 + 3, y0, glyph, TEAL, bold=True)
    g.put(x0 + 4, y0, " ", TEXT)
    g.put(x0 + 5, y0, title, TEAL, bold=True)
    g.put(x0 + 5 + len(title), y0, " ", TEXT)
    for y in range(y0 + 1, y1):
        g.put(x0, y, "│", CYAN)
        g.put(x1, y, "│", CYAN)
    g.put(x0, y1, "┕" + "━" * (x1 - x0 - 1) + "┙", CYAN)


def item(g, x0, x1, y, key, label, sel=False, value=None):
    """A row: its letter in magenta; the selected one on the teal bar with `▸`."""
    bg = SEL_BG if sel else PANEL
    g.bg(x0 + 1, y, x1 - x0 - 1, bg)
    if sel:
        g.put(x0 + 2, y, "▸", SEL_FG, bg, bold=True)
    g.put(x0 + 4, y, key, MAGENTA, bg, bold=True)
    g.put(x0 + 7, y, label, SEL_FG if sel else TEXT, bg, bold=sel)
    if value:
        g.put(x1 - 3 - len(value), y, value, SEL_FG if sel else CYAN, bg, bold=sel)


def centred(g, y, s, fg, x0=0, x1=W, bold=False):
    g.put(x0 + (x1 - x0 - len(s)) // 2, y, s, fg, bold=bold)


def asleep():
    """The flat with the lights out at 03:12: the map dark, z's over you."""
    g = load()
    x0, y0, x1, y1 = MAP
    for y in range(y0, y1):
        for x in range(x0, x1):
            if (x, y) != PLAYER:
                c = g.c[y][x]
                c[1], c[2] = scale(c[1], 0.4), scale(c[2], 0.4)
    g.c[PLAYER[1]][PLAYER[0]][1] = DIM
    g.put(PLAYER[0] + 1, PLAYER[1] - 1, "z", SEL_FG)
    g.put(PLAYER[0] + 2, PLAYER[1] - 2, "Z", CYAN, bold=True)
    g.put(113, 0, f"☾ {ASLEEP}", TEXT)
    return g


# ---- frame 0: the bed's window, the same in every concept

def bed():
    g = load()
    wash(g)
    window(g, 40, 16, 79, 21, "Bed")
    item(g, 40, 79, 18, "x", "Look", sel=True)
    item(g, 40, 79, 19, "i", "Interact: ")
    g.put(57, 19, "Sleep", CYAN, bold=True)
    return g


# ---- A · choices

A_ROWS = [("1", "An hour", "23:01"), ("2", "Four hours", "02:01"),
          ("3", "Until morning", "07:00"), ("4", "Until noon", "12:00")]


def a_pick():
    g = load()
    wash(g)
    x0, y0, x1, y1 = 40, 12, 79, 27
    window(g, x0, y0, x1, y1, "Sleep")
    g.put(x0 + 4, y0 + 2, f"now ☾ {NOW}", DIM)
    for i, (k, label, at) in enumerate(A_ROWS):
        item(g, x0, x1, y0 + 4 + i, k, label, sel=(i == 2), value=f"wake {at}")
    g.put(x0 + 4, y0 + 9, "8h 59m asleep · you wake rested", DIM)
    g.put(x0 + 4, y0 + 10, "a call or a knock wakes you early", FAINT)
    g.put(x0 + 4, y0 + 13, "1-4 or j k · enter sleep · esc back", FAINT)
    return g


def a_sleep():
    g = asleep()
    g.fill(0, 36, 110, 1, PANEL)
    g.put(1, 36, "asleep", SEL_FG, bold=True)
    n = 30
    done = round(n * SLEPT)
    g.put(9, 36, "━" * done, CYAN)
    g.put(9 + done, 36, "━" * (n - done), scale(SEL_FG, 0.35))
    g.put(9 + n + 2, 36, f"{ASLEEP} → {WAKE}", DIM)
    g.put(9 + n + 17, 36, "esc wakes you", FAINT)
    return g


# ---- B · timeline

T_START = 20 * 60  # the strip runs 24 hours from 20:00, two cells an hour
T_CELLS = 48


def strip(g, x, y, until=None):
    """Hour labels, the day as cells (night blue, day amber), the sleep in teal and
    the part slept bright, then markers for now (or the time slept to) and waking."""
    now, wake = minutes(NOW), minutes(WAKE) + 24 * 60
    for h in range(0, 24, 2):
        g.put(x + h * 2, y, "%02d" % ((20 + h) % 24), FAINT)
    for i in range(T_CELLS):
        m = T_START + i * 30
        hour = (m // 60) % 24
        if now <= m + 29 and m < wake:
            fg = CYAN if until is not None and m < until else SEL_FG
        elif m < now:
            fg = "#23262d"
        elif hour >= 20 or hour < 6:
            fg = "#17213a"
        else:
            fg = "#3d3318"
        g.put(x + i, y + 1, "█", fg)
    at = until if until is not None else now
    label = ASLEEP if until is not None else f"now {NOW}"
    ia, iw = (at - T_START) // 30, (wake - T_START) // 30
    g.put(x + ia, y + 2, "▲", CYAN if until is not None else TEXT, bold=True)
    g.put(x + ia, y + 3, label, CYAN if until is not None else TEXT)
    g.put(x + iw, y + 2, "▲", SEL_FG, bold=True)
    g.put(x + iw, y + 3, f"wake {WAKE}", SEL_FG)


def b_window(g, title, until=None):
    x0, y0, x1, y1 = 12, 23, 68, 34
    window(g, x0, y0, x1, y1, title)
    strip(g, x0 + 4, y0 + 2, until)
    return x0, y0


def b_pick():
    g = load()
    wash(g)
    x0, y0 = b_window(g, "Sleep")
    g.put(x0 + 4, y0 + 7, "8h 59m asleep · you wake rested", DIM)
    g.put(x0 + 4, y0 + 9, "h l an hour · H L ten minutes · enter sleep · esc", FAINT)
    return g


def b_sleep():
    g = asleep()
    x0, y0 = b_window(g, "Asleep", until=minutes(ASLEEP) + 24 * 60)
    g.put(x0 + 4, y0 + 7, "5h 11m slept · 3h 48m to go", DIM)
    g.put(x0 + 4, y0 + 9, "esc wakes you", FAINT)
    return g


# ---- C · big clock

SEGMENTS = {"0": "abcdef", "1": "bc", "2": "abged", "3": "abgcd", "4": "fgbc",
            "5": "afgcd", "6": "afgedc", "7": "abc", "8": "abcdefg", "9": "abfgcd"}


def digit_px(d):
    """Seven segments, four pixels wide and seven high."""
    s = SEGMENTS[d]
    px = [[False] * 4 for _ in range(8)]
    for r in range(7):
        for c in range(4):
            px[r][c] = ((r == 0 and "a" in s) or (r == 3 and "g" in s) or (r == 6 and "d" in s)
                        or (c == 0 and ((1 <= r <= 2 and "f" in s) or (4 <= r <= 5 and "e" in s)))
                        or (c == 3 and ((1 <= r <= 2 and "b" in s) or (4 <= r <= 5 and "c" in s))))
    return px


def big(g, x, y, text, fg):
    """`hh:mm` in half-block digits, four rows high; returns the width."""
    cx = x
    for ch in text:
        if ch == ":":
            px = [[False], [False], [True], [False], [True], [False], [False], [False]]
        else:
            px = digit_px(ch)
        for row in range(4):
            for col in range(len(px[0])):
                top, bottom = px[2 * row][col], px[2 * row + 1][col]
                cell = "█" if top and bottom else "▀" if top else "▄" if bottom else " "
                if cell != " ":
                    g.put(cx + col, y + row, cell, fg)
        cx += len(px[0]) + 1
    return cx - x - 1


def c_pick():
    g = load()
    wash(g)
    x0, y0, x1, y1 = 21, 10, 57, 28
    window(g, x0, y0, x1, y1, "Sleep")
    centred(g, y0 + 2, "wake at", DIM, x0, x1 + 1)
    w = 21
    x = x0 + (x1 + 1 - x0 - w) // 2
    big(g, x, y0 + 5, WAKE, CYAN)
    for col in (x + 4, x + 16):
        g.put(col, y0 + 4, "▴", SEL_FG, bold=True)
        g.put(col, y0 + 9, "▾", SEL_FG, bold=True)
    centred(g, y0 + 11, f"now {NOW} · 8h 59m asleep", DIM, x0, x1 + 1)
    centred(g, y0 + 12, "you wake rested", DIM, x0, x1 + 1)
    centred(g, y0 + 15, "k j hour · l h ten minutes", FAINT, x0, x1 + 1)
    centred(g, y0 + 16, "enter sleep · esc back", FAINT, x0, x1 + 1)
    return g


def c_sleep():
    g = load()
    wash(g, 0.12)
    x0, _, x1, _ = MAP
    w = 21
    x = x0 + (x1 - x0 - w) // 2
    big(g, x, 15, ASLEEP, SEL_FG)
    n = 21
    done = round(n * SLEPT)
    g.put(x, 21, "━" * done, CYAN)
    g.put(x + done, 21, "━" * (n - done), scale(SEL_FG, 0.35))
    centred(g, 23, "asleep · 3h 48m to go", DIM, x0, x1)
    centred(g, 25, "esc wakes you", FAINT, x0, x1)
    g.put(113, 0, f"☾ {ASLEEP}", scale(TEXT, 0.5))
    return g


CONCEPTS = [
    ("A", "Choices", a_pick, a_sleep,
     "The bed's window turns into a short list: an hour, four hours, until morning, "
     "until noon, each with the time you'd wake. Quickest to read and to pick. While "
     "you sleep the game keeps its own screen: the lights are out, z's float over you, "
     "the clock races in the corner and the status strip fills as the night goes."),
    ("B", "Timeline", b_pick, b_sleep,
     "A strip of the next 24 hours, night blue and day amber, with now and your wake "
     "time marked on it. h and l move the wake time an hour, H and L ten minutes, so "
     "any time is a few keys away. Later the strip could carry a shop's hours or a "
     "meeting. While you sleep the window stays and the strip fills."),
    ("C", "Big clock", c_pick, c_sleep,
     "A clock you set, in big digits: k and j for the hour, l and h for ten minutes. "
     "While you sleep the screen goes nearly black and only the clock runs, fast, "
     "with a bar under it. The most like a cut in a film."),
]


def phone(key):
    """One concept alone, its frames stacked, for a phone: `bed`, `A`, `B` or `C`."""
    head = ("<!doctype html><meta charset='utf-8'><style>" + CSS
            + "body{background:#0b0e13;color:#cdd4de;font:22px/1.45 -apple-system,system-ui,"
            "sans-serif;margin:24px;width:820px}h2{font-size:28px;margin:0 0 8px;color:#fff}"
            "p{color:#a0aab9;margin:0 0 14px}.cap{font-size:18px;color:#a0aab9;margin:6px 0 22px}"
            "</style>")
    if key == "bed":
        return (head + "<h2>Space beside the bed</h2><p>The bed gets a window like the rig's: "
                "<b>i</b> (Interact: Sleep) opens the sleep interface.</p>" + bed().html(0.8))
    _, name, pick, sleep, words = next(c for c in CONCEPTS if c[0] == key)
    return (head + f"<h2>{key} · {name}</h2><p>{words}</p>" + pick().html(0.8)
            + "<div class='cap'>choosing when to wake</div>" + sleep().html(0.8)
            + "<div class='cap'>asleep, 03:12</div>")


def page():
    frame = lambda g: "<div class='fr'>" + g.html(0.56) + "</div>"  # noqa: E731
    out = [
        "<!doctype html><meta charset='utf-8'><title>Wait and sleep · round 1</title><style>",
        CSS,
        "body{background:#0b0e13;color:#cdd4de;font:15px/1.5 -apple-system,system-ui,sans-serif;"
        "margin:24px auto;max-width:1240px;padding:0 16px}"
        "h1{font-size:22px;margin:0 0 4px}h2{font-size:18px;margin:36px 0 4px;color:#fff}"
        "p,li{max-width:820px;color:#a0aab9}.grid{display:grid;grid-template-columns:1fr 1fr;"
        "gap:18px}.cap{font-size:13px;color:#a0aab9;margin:6px 0 0}"
        ".fr{overflow:hidden}@media(max-width:900px){.grid{grid-template-columns:1fr}}",
        "</style>",
        "<h1>Wait and sleep · when to wake</h1>",
        "<p>Home after the house call, 22:01, the objective says <b>Sleep</b>. Space beside "
        "the bed opens its window like any other thing in reach; <b>i</b> (Interact: "
        "Sleep) opens the sleep interface. Below: that window, then three ways to choose "
        "when you wake and to show the night going by, over the game's own frame at "
        "120×40. Later the same interface opens from anywhere as <b>Wait</b>.</p>",
        "<div class='grid'><div>" + frame(bed()) + "<div class='cap'>Space beside the bed"
        "</div></div><div><p><b>My guesses, to correct:</b></p><ul>"
        "<li>Sleeping until rested fills HP and FOCUS; waiting elsewhere doesn't.</li>"
        "<li>A call, a knock or someone walking in wakes you early, and the story's "
        "next beat (Castellan in touch) can be one of them.</li>"
        "<li>The night passes in a few seconds and the city really runs through it; "
        "esc wakes you whenever you like.</li></ul></div></div>",
    ]
    for key, name, pick, sleep, words in CONCEPTS:
        out.append(f"<h2>{key} · {name}</h2><p>{words}</p><div class='grid'>")
        out.append("<div>" + frame(pick()) + "<div class='cap'>choosing when to wake</div></div>")
        out.append("<div>" + frame(sleep()) + "<div class='cap'>asleep, 03:12</div></div>")
        out.append("</div>")
    return "\n".join(out)


if __name__ == "__main__":
    # gen.py out.html [bed|A|B|C]: the whole page, or one part alone for a phone.
    only = sys.argv[2] if len(sys.argv) > 2 else None
    out = phone(only) if only else page()
    open(sys.argv[1] if len(sys.argv) > 1 else "round1.html", "w").write(out)
