import subprocess, sys
CH = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
src, out, w, h = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
subprocess.run([CH, "--headless=new", "--disable-gpu", "--hide-scrollbars", "--screenshot=" + out,
                "--window-size=%s,%s" % (w, h), "--virtual-time-budget=4000", "file://" + src],
               check=True, capture_output=True)
