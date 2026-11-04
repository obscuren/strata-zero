"""Mail in the rig's terminal: mockups over the game's own frames (seed 8, the morning
after, Mezz's news mail just in, the older mails read), from the throwaway
`term_mail_mock` shot test in mail_mock.diff.

    python3 -I gen.py shots index.html

A · the shell's own `mail`, made readable: the boot says what waits, `mail` lists the
    inbox newest first with a status column, `mail new` reads the next new one with a
    head block and says what's left. Real code, through the real terminal.
B · HELIX mail, a program that owns the glass as nano does: an inbox with a cursor bar,
    space reads, n the next new, esc back. Drawn here over a real blank terminal frame."""

import copy
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "2026-10-19-gear-inventory-mock"))
from cells import Grid  # noqa: E402
sys.path.insert(0, os.path.join(HERE, "..", "2026-11-02-minimap-mock"))
import term  # noqa: E402

S = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "shots")
OUT = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "index.html")
W, H = 120, 40


def load(name):
    return Grid.from_svg(os.path.join(S, "term_%s_120x40.svg" % name), W, H)


def glass_of(g):
    """The glass: the box of cells sharing the prompt's background."""
    for y, row in enumerate(g.c):
        line = "".join(c[0] for c in row)
        x = line.find("vex@zenbo")
        if x >= 0:
            bg, fg = row[x][2], row[x][1]
            break
    xs = [i for i in range(W) if g.c[y][i][2] == bg]
    x0, x1 = min(xs), max(xs) + 1
    ys = [j for j in range(H) if g.c[j][x0 + 2][2] == bg]
    return x0, ys[0], x1 - x0, ys[-1] + 1 - ys[0], bg, fg


def program(base, title, rows, cursor, status, keys):
    """Paints a full-glass program: an inverse title bar, rows, a status, two key rows."""
    g = copy.deepcopy(base)
    x0, y0, w, h, bg, fg = glass_of(g)
    dim = "#b0802a"
    for j in range(h):
        g.put(x0, y0 + j, " " * w, fg, bg)
    g.put(x0, y0, " " * w, bg, fg, bold=True)
    g.put(x0, y0, title, bg, fg, bold=True)
    for i, r in enumerate(rows):
        y = y0 + 2 + i
        if i == cursor:
            g.put(x0, y, " " * w, bg, fg)
            g.put(x0, y, r, bg, fg, bold=True)
        else:
            g.put(x0, y, r, fg, bg, bold=r.startswith("  ▸"))
    g.put(x0, y0 + h - 3, "  " + status, fg, bg)
    g.put(x0, y0 + h - 2, keys[0], dim, bg)
    g.put(x0, y0 + h - 1, keys[1], dim, bg)
    return g


def fig(g, caption, px=10.4):
    return '<figure>%s<figcaption>%s</figcaption></figure>' % (term.render(g, px), caption)


CSS = term.CSS + """
body{background:#0a0d13;color:#cdd4de;font:15px/1.5 -apple-system,system-ui,sans-serif;
 margin:0;padding:24px 16px 80px}
main{max-width:1060px;margin:0 auto}
h1{font-size:22px;margin:0 0 4px;color:#fff}
h2{font-size:17px;margin:40px 0 6px;color:#00e5ff;border-bottom:1px solid #1d2a38;
 padding-bottom:6px}
p,li{max-width:820px}
.sub{color:#8a95a5;margin:0 0 18px}
figure{margin:12px 0 18px;overflow-x:auto}
figcaption{color:#8a95a5;font-size:13px;margin-top:6px;max-width:820px}
"""


def page():
    blank = load("blank")
    inbox = program(
        blank,
        "  HELIX mail 2.2   INBOX · 4 mail · 1 new",
        [
            "  ▸ NEW   06:03  Mezz          the news",
            "          22:03  Mezz          lay low",
            "          22:01  Mezz          voicemail",
            "          22:00  Teodora-11    a favour",
        ],
        0,
        "[ 1 new ]",
        ("space Read     n Next new    ↑↓ Move", "esc Exit"),
    )
    reading = program(
        blank,
        "  HELIX mail 2.2   4 of 4 · NEW",
        [
            "  From:     Mezz",
            "  Subject:  the news",
            "  Time:     06:03",
            "",
            "  Saw the news.",
            "  Don't go out looking like you.",
        ],
        -1,
        "[ 1 new left ]",
        ("n Next new     ↑↓ Other mail", "esc Inbox"),
    )
    out = ['<!doctype html><html><head><meta charset="utf-8">',
           '<meta name="viewport" content="width=device-width,initial-scale=1">',
           '<title>Mail in the terminal</title><style>%s</style></head><body><main>' % CSS,
           '<h1>Mail in the terminal</h1>',
           '<p class="sub">The rig\'s terminal stays where mail lives. Today it boots '
           'without a word about mail; <code>mail</code> lists oldest first with a bare '
           '<code>*</code> for unread; <code>mail 4</code> prints From and Subject in '
           'capitals and the body.</p>']
    out.append('<h2>A · the shell\'s <code>mail</code>, made readable</h2>')
    out.append('<p>Real code through the real terminal. The boot says what waits. '
               '<code>mail</code> lists the inbox newest first: a head line, N, NEW or '
               'read, the time it came, who, what; the next step on the last line. '
               '<code>mail new</code> reads the next new one: a head block, the body '
               'between rules, and what\'s left.</p>')
    out.append(fig(load("boot"), "boot: YOU HAVE 1 NEW MAIL · TYPE mail"))
    out.append(fig(load("inbox"), "mail: the inbox, newest first"))
    out.append(fig(load("read"), "mail new: Mezz's mail; reading it brought Seren's, so 1 new left"))
    out.append('<h2>B · HELIX mail, a program on the glass</h2>')
    out.append('<p>Like nano: <code>mail</code> opens a program that owns the glass. '
               'An inbox with a cursor bar on the newest new one; space reads it, '
               '<b>n</b> jumps to the next new, ↑↓ move, esc goes back (to the inbox, then '
               'the prompt). The boot line from A stays; <code>mail 4</code> still prints '
               'one to the scrollback.</p>')
    out.append(fig(inbox, "mail: the inbox, the cursor on the new one"))
    out.append(fig(reading, "space: the mail; n for the next new one, esc for the inbox"))
    out.append('</main></body></html>')
    return "".join(out)


open(OUT, "w").write(page())
