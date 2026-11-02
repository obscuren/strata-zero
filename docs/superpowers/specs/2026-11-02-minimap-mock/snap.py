import subprocess, sys, os
src, out, w, h = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
body = open(src).read()
page = os.path.abspath(src) + ".standalone.html"
open(page, "w").write("<!doctype html><meta charset=utf-8><body style='background:#10131a;color:#dde;font-family:system-ui;padding:16px'>" + body)
chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
subprocess.run([chrome, "--headless=new", "--disable-gpu", "--hide-scrollbars", f"--window-size={w},{h}", f"--screenshot={os.path.abspath(out)}", "file://" + page], check=True, capture_output=True)
print("ok", out)
