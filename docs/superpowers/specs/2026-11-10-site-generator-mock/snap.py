import subprocess, sys
chrome = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
src, out, w, h = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
subprocess.run([chrome, "--headless=new", "--disable-gpu", "--hide-scrollbars",
                f"--window-size={w},{h}", "--virtual-time-budget=4000",
                f"--screenshot={out}", "file://" + src], check=True,
               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
