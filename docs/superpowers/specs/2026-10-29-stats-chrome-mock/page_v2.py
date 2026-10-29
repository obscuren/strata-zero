"""The companion page for the ledger's states: python3 page_v2.py m2.json out.html"""

import html
import json
import sys

d = json.load(open(sys.argv[1]))

css = d["css"] + """
:root{color-scheme:dark}
body{background:#0b0d12;color:#cdd4de;margin:0;font:15px/1.5 -apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif}
main{max-width:1180px;margin:0 auto;padding:28px 16px 60px}
h1{font:700 22px 'JetBrains Mono',Menlo,monospace;color:#00e5ff;letter-spacing:.06em;margin:0 0 6px}
h2{font:700 15px 'JetBrains Mono',Menlo,monospace;color:#00e5ff;letter-spacing:.08em;margin:44px 0 10px;
   border-bottom:1px solid #283c4b;padding-bottom:6px}
p.lead{color:#a0aab9;max-width:820px;margin:0 0 8px}
.shot{display:grid;grid-template-columns:auto 1fr;gap:20px;align-items:start;background:#0f131a;border:1px solid #1d2733;
      border-radius:8px;padding:16px;margin:16px 0}
.shot h3{font:700 15px 'JetBrains Mono',Menlo,monospace;color:#cdd4de;margin:4px 0 8px}
.shot p{color:#a0aab9;margin:0}
.scroll{overflow-x:auto;max-width:100%}
@media (max-width:1100px){.shot{grid-template-columns:1fr}}
.notes{display:grid;grid-template-columns:repeat(auto-fit,minmax(250px,1fr));gap:12px;margin-top:14px}
.note{background:#0f131a;border:1px solid #1d2733;border-radius:8px;padding:12px 14px;color:#a0aab9;font-size:14px}
.note b{font-family:'JetBrains Mono',Menlo,monospace;color:#cdd4de}
"""

out = ["<!DOCTYPE html><html><head><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>"
       "<title>The chrome ledger</title><style>%s</style></head><body><main>" % css]
out.append("<h1>B · THE LEDGER</h1>")
out.append("<p class=lead>The ledger in each of its states, on the game's own frame: 100×30 first, then 120×40. "
           "The left pane is the same as before.</p>")
for key, title, blurb, h in d["screens"]:
    out.append("<div class=shot><div class=scroll>%s</div><div><h3>%s</h3><p>%s</p></div></div>"
               % (h, html.escape(title), html.escape(blurb)))
out.append("<h2>HOW IT BEHAVES</h2><div class=notes>"
           "<div class=note><b>↑↓ walks the ledger.</b> Sites, then a chrome limb's sockets under it. ← crosses to the stats, → back.</div>"
           "<div class=note><b>Space on chrome pulls it out</b> into your bag. Whatever sat in its sockets comes out with it.</div>"
           "<div class=note><b>Space on an empty place lists what fits</b> from your bag. ↑↓ picks, space installs, esc backs out. LOAD shows the cost in amber first.</div>"
           "<div class=note><b>No ripperdoc yet.</b> Until there's a clinic, you do it yourself, here. A clinic can take this over later.</div>"
           "</div>")
out.append("</main></body></html>")
open(sys.argv[2], "w").write("".join(out))
