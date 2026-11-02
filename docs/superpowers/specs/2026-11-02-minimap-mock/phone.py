"""Phone-sized shots of the indoor options: just the MAP pane, two to a row."""
import html, os, subprocess, sys
here = os.path.dirname(os.path.abspath(__file__))
sys.argv = [sys.argv[0], os.path.join(here, "shots"), os.path.join(here, "_ignore.html")]
sys.path.insert(0, here)
import mm_in, term  # noqa: E402

CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
for name, title in mm_in.PLACES:
    p = mm_in.Place(name)
    cells = []
    for key, label, draw in mm_in.CONCEPTS:
        g = p.frame() if draw is None else draw(p)
        cells.append("<div><h3>%s</h3>%s</div>" % (html.escape(label),
                     term.render(g.crop(mm_in.PX, mm_in.PY, mm_in.PW, mm_in.PH + 1), 17)))
    body = ("<style>%s body{background:#10131a;color:#e6e6f0;font-family:system-ui;margin:0;"
            "padding:18px} h2{font-size:22px;margin:0 0 14px} h3{font-size:17px;margin:0 0 6px}"
            " .g{display:grid;grid-template-columns:repeat(2,max-content);gap:18px 18px}</style>"
            "<h2>%s</h2><div class='g'>%s</div>" % (term.CSS, html.escape(title), "".join(cells)))
    page = os.path.join(here, "phone_%s.html" % name)
    open(page, "w").write("<!doctype html><meta charset=utf-8>" + body)
    out = os.path.join(here, "indoor_%s.png" % name.split("_")[1])
    subprocess.run([CHROME, "--headless=new", "--disable-gpu", "--hide-scrollbars",
                    "--force-device-scale-factor=2", "--window-size=880,760",
                    "--screenshot=" + out, "file://" + page], check=True, capture_output=True)
    print("ok", out)
