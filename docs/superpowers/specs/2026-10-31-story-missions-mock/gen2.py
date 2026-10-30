"""Round 2: the picks (welcome B, call 1) with HOLO in place of BCI."""
import os
import sys
sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen  # noqa: E402
from gen import CSS, frame, street, card_ringing, card_talking, welcome_holo  # noqa: E402

gen_holo = welcome_holo()
for y in range(gen_holo.h):
    row = "".join(c[0] for c in gen_holo.c[y])
    i = row.find("BCI ▸ incoming feed")
    if i >= 0:
        gen_holo.put(i, y, "HOLO", gen.MAGENTA, bold=True)
        gen_holo.put(i + 4, y, " ▸ incoming       ", gen.DIM)
a = street(); card_ringing(a)
b = street(); card_talking(b)
out = ["<style>%s.shot{margin:10px 0 22px}.cap{font:13px/1.4 system-ui;color:#8a94a3;margin:0 0 6px}</style>" % CSS]
out.append("<h2>Story missions · your picks</h2>")
out.append('<p class="subtitle">The welcome as a HOLO feed, the call as a card in the map\'s corner. '
           "Both come in through the neural port behind your right ear.</p>")
out.append("<h3>The welcome</h3>" + frame(gen_holo, ""))
out.append("<h3>The call</h3>" + frame(a, "ringing") + frame(b, "answered"))
open(sys.argv[1], "w").write("\n".join(out))
