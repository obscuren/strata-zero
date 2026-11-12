"""The #95 mock page: the screens from parts.json with their words around them.

    python3 -I page.py parts.json out.html
"""

import base64
import json
import os
import sys

REPO = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..")
PARTS = json.load(open(sys.argv[1]))
OUT = sys.argv[2]


def face(weight, name):
    data = base64.b64encode(open("%s/crates/strata-zero/fonts/%s" % (REPO, name), "rb").read()).decode()
    return ("@font-face{font-family:'StrataFira';font-weight:%d;font-display:block;"
            "src:url(data:font/ttf;base64,%s) format('truetype')}" % (weight, data))


LOOKS = [
    ("a", "A", "Floor plan",
     "The level is the floor itself. Walls are the floor's walls, every device is a node "
     "where it stands, and thin traces show which hub each one hangs off. You see one "
     "level at a time; the hub is the way up and down.",
     ["You walk the construct the way you'll walk the floor, so a run teaches you the "
      "building: the turnstiles, the closet, the lift lobby.",
      "Card doors are walls in the Net too until you crack them, and cracking one opens "
      "it in the street. Zones on the Net are the zones on the floor.",
      "The hub in each floor's closet is where the riser goes up and down. A floor whose "
      "hub doesn't reach the riser (B1 here) can only be reached from a device on it."]),
    ("b", "B", "Circuit",
     "Every level is a bus off the riser, laid out zone by zone from the hub outwards. "
     "Its doors are gates on the line. You see the whole site at once, as wiring.",
     ["You hop from node to node along the bus instead of walking tiles: quick, and "
      "the route is the puzzle.",
      "A gate on the bus is a door on the floor. Crack it and you can go further down "
      "the line, and the door is open in the street.",
      "Because it doesn't show the floor, the panel draws a small plan of the floor under "
      "the node you're on, with the guards on it."]),
    ("c", "C", "Section",
     "The tower cut open from the side. Every floor is squashed to three rows (its back "
     "rooms, the wall with its doors, the front), and floors the lift passes are folded "
     "away. The lifts and the riser run through it all.",
     ["You see every level and how they stack, and you move left and right along a floor "
      "and up and down the riser.",
      "It reads like the building from the street: the firewall is its skin, B1 sits in "
      "the ground, and the air gap is a cut in the riser.",
      "A device keeps its place along the floor, so you can tell which end of the "
      "building it's at, but not how deep into the floor."]),
]

LEGEND = [
    ("▪", "#9fe6ff", "a device: camera, lift, gate, turret"),
    ("▯", "#9fe6ff", "a door with a lock"),
    ("◇", "#8c96ff", "a terminal, a way in"),
    ("◆", "#e6f0f5", "the building's server"),
    ("⌗", "#ffb000", "the data vault"),
    ("⬡", "#e6f0f5", "a floor's hub, on the riser"),
    ("◈", "#e6f0f5", "ICE (white, grey, black)"),
    ("▮", "#ff2848", "a firewall lock"),
    ("@", "#468cff", "you, on the Net"),
    ("▪", "#00ffc8", "green: yours (cracked, looped, held)"),
]


def screen(key):
    return ('<div class="screen" data-cols="120"><div class="fit">%s</div></div>'
            % PARTS["parts"][key])


def look(key, letter, name, idea, plays):
    items = "".join("<li>%s</li>" % p for p in plays)
    return f"""
<section class="look" id="look-{key}">
  <header class="look-head">
    <span class="tag">{letter}</span>
    <h2>{name}</h2>
  </header>
  <p class="idea">{idea}</p>
  {screen(key)}
  <ul class="plays">{items}</ul>
</section>"""


legend = "".join('<li><b style="color:%s">%s</b><span>%s</span></li>' % (c, g, t) for g, c, t in LEGEND)

page = f"""<title>A Site's Own Net</title>
<meta name="claude:viewport-fit-sides" content="cover">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=IBM+Plex+Sans:wght@400;500;600&display=swap">
<style>
{face(400, "FiraCode-Regular.ttf")}
{face(700, "FiraCode-Bold.ttf")}
/* A dark page around the game's own screens: one terminal world, so one theme. */
:root {{
  --ground: #06080c;
  --ink: #cdd4de;
  --muted: #8a93a3;
  --rule: #1c2a36;
  --signal: #00ffc8;
  --cyan: #00e5ff;
  --red: #ff2848;
  --mono: 'StrataFira', 'Fira Code', ui-monospace, Menlo, monospace;
  --sans: 'IBM Plex Sans', system-ui, -apple-system, 'Segoe UI', sans-serif;
  color-scheme: dark;
}}
body {{ background: var(--ground); color: var(--ink); font-family: var(--sans); font-size: 15px; line-height: 1.55; }}
main {{ max-width: 1240px; margin: 0 auto; padding-block: 28px 64px;
  padding-left: max(16px, env(safe-area-inset-left, 0px));
  padding-right: max(16px, env(safe-area-inset-right, 0px));
  display: grid; grid-template-columns: minmax(0, 1fr); gap: 44px; }}
.top {{ display: grid; gap: 12px; max-width: 70ch; min-width: 0; }}
.eyebrow {{ font-family: var(--mono); font-size: 12px; letter-spacing: .12em; text-transform: uppercase; color: var(--cyan); }}
h1 {{ font-family: var(--mono); font-weight: 700; font-size: clamp(26px, 4vw, 38px); line-height: 1.1; margin: 0; text-wrap: balance; color: #e6f0f5; }}
h2 {{ font-family: var(--mono); font-weight: 700; font-size: 20px; margin: 0; color: #e6f0f5; }}
p {{ margin: 0; }}
.lede {{ color: var(--ink); }}
.lede em {{ color: var(--signal); font-style: normal; }}
.bar {{ display: flex; flex-wrap: wrap; gap: 10px; align-items: center; }}
button {{ font: 600 13px var(--mono); color: var(--ground); background: var(--cyan); border: 0; border-radius: 3px; padding: 7px 12px; cursor: pointer; }}
button:focus-visible {{ outline: 2px solid var(--signal); outline-offset: 2px; }}
.bar span {{ color: var(--muted); font-size: 13px; }}
nav.jump {{ display: flex; flex-wrap: wrap; gap: 8px 18px; font-family: var(--mono); font-size: 13px; }}
nav.jump a {{ color: var(--cyan); text-decoration: none; border-bottom: 1px solid var(--rule); }}
.look, .gridsec {{ display: grid; grid-template-columns: minmax(0, 1fr); gap: 14px; min-width: 0; }}
.look-head {{ display: flex; align-items: baseline; gap: 12px; }}
.tag {{ font-family: var(--mono); font-weight: 700; color: var(--ground); background: var(--signal); padding: 1px 8px; border-radius: 2px; }}
.idea {{ max-width: 72ch; }}
.cap {{ max-width: 72ch; color: var(--muted); font-size: 14px; }}
.cap b {{ color: var(--signal); font-weight: 600; }}
.plays {{ margin: 0; padding-left: 1.2em; display: grid; gap: 6px; max-width: 78ch; color: var(--ink); }}
.plays li::marker {{ color: var(--signal); }}
.screen {{ overflow-x: auto; overflow-y: hidden; border-radius: 4px; box-shadow: 0 0 0 1px #1a2230, 0 10px 34px rgba(0,0,0,.55); background: #04060a; min-width: 0; }}
.fit {{ width: max-content; margin-inline: auto; }}
.tm {{ font-family: var(--mono); line-height: 1; font-size: var(--fs, 14px); }}
.tm .r {{ display: flex; height: 1.25em; }}
.tm i {{ display: inline-flex; align-items: center; justify-content: center; flex: none; width: .6em; height: 1.25em; white-space: pre; overflow: visible; font-style: normal; }}
.legend {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 6px 22px; margin: 0; padding: 0; list-style: none; font-size: 14px; }}
.legend li {{ display: flex; gap: 10px; align-items: baseline; min-width: 0; }}
.legend b {{ font-family: var(--mono); width: 1.2em; text-align: center; flex: none; }}
.note {{ color: var(--muted); font-size: 14px; max-width: 78ch; }}
hr {{ border: 0; border-top: 1px solid var(--rule); margin: 0; }}
{PARTS["css"]}
</style>
<main>
  <section class="top">
    <div class="eyebrow">#95 · round 2 · look D</div>
    <h1>A site's own Net</h1>
    <p class="lede">D puts C's section and A's floor plan together. One moment again: Vex walked into Meridian Clearing's lobby
    and jacked in at the visitor kiosk. On the building's Net she has cracked the
    turnstiles and the hub closet's door, looped the camera by the lifts and told the
    goods lift to stop at B1. Those are <em>green</em>, and each one is real in the
    street: the turnstiles let her through, the camera sees nothing. The ledger she's
    after is in the B1 vault, on a Net of its own, so next she jacks out and walks down.</p>
    <div class="bar"><button type="button" id="zoom">Show at full size</button><span id="zoomnote">Screens are shrunk to fit; full size scrolls sideways.</span></div>
    <nav class="jump"><a href="#look-d">D · Open tower</a><a href="#from-the-grid">From the city's Net</a><a href="#look-a">A · Floor plan</a><a href="#look-b">B · Circuit</a><a href="#look-c">C · Section</a></nav>
  </section>


  <section class="look" id="look-d">
    <header class="look-head"><span class="tag">D</span><h2>Open tower</h2></header>
    <p class="idea">The tower in section, as in C, with the floor you're on opened out to
    its full plan, as in A. The floors are bigger too: 58 by 15 instead of 50 by 13. Go up
    or down the riser and that floor opens; the one you left folds back into its three
    rows.</p>
    <p class="cap"><b>Now.</b> On L, the lobby open: the turnstiles and the closet door
    cracked, the cord running back to the kiosk where her body stands.</p>
    {screen("d1")}
    <p class="cap"><b>Later.</b> She has ridden the goods lift down and jacked in at B1's
    NOC terminal. B1 is open now, and the floors above are out of reach: B1's Net
    doesn't touch the riser. Black ICE sits on the vault.</p>
    {screen("d2")}
    <ul class="plays">
      <li>You always see the whole building and where you are in it, and the floor you're
      on at the size you'll walk it.</li>
      <li>Every column is the same place on every floor, so the lifts and the riser line up
      through the tower, and a node over your head is straight above you.</li>
      <li>A folded floor still shows its nodes, its ICE and what you hold there, so you can
      plan the next floor before you go up.</li>
      <li>The firewall's 3 locks sit on lanes of the city's grid. Crack them out there,
      from your rig, a backdoor or a jack box, and you never stand in the building; or
      crack them from inside, which is how you'd let a partner in or leave by the
      roof.</li>
      <li>The tower is taller than the map on a small terminal (100×30), so there it
      scrolls with you, the way the street does.</li>
    </ul>
  </section>

  <hr>
  <section class="top">
    <div class="eyebrow">round 1</div>
    <p class="note">The looks D comes from, and how the building looks from the city's Net
    (the same in every look).</p>
  </section>
  <section class="gridsec" id="from-the-grid">
    <header class="look-head"><span class="tag" style="background:var(--red)">▮</span><h2>From the city's Net</h2></header>
    <p class="idea">The same in every look. From outside, the building is a firewall
    with 3 locks, each on its own lane. Break all 3 and you're in, at the top of
    the riser. Every break adds trace, so you can do it from your rig or a backdoor,
    safe in body but loud, or walk in and use a kiosk, quiet but in the building.</p>
    {screen("grid")}
  </section>

  {"".join(look(*l) for l in LOOKS)}

  <hr>
  <section class="gridsec">
    <h2>Reading the screens</h2>
    <ul class="legend">{legend}</ul>
    <p class="note">The office is made up for the mock: four floors kept for the job (L,
    7F, 12F and B1), the rest passed by the lift. Sizes and counts are illustrative.
    Guards in the street don't move yet (#33), so the walk past them waits on that.</p>
  </section>
</main>
<script>
(function () {{
  var full = false;
  var btn = document.getElementById('zoom');
  var note = document.getElementById('zoomnote');
  function fit() {{
    document.querySelectorAll('.screen').forEach(function (s) {{
      var tm = s.querySelector('.tm');
      if (!tm) return;
      var w = s.clientWidth;
      var fs = full ? 14 : Math.min(16, (w - 1) / (120 * 0.6));
      tm.style.setProperty('--fs', fs.toFixed(3) + 'px');
    }});
  }}
  btn.addEventListener('click', function () {{
    full = !full;
    btn.textContent = full ? 'Fit to the page' : 'Show at full size';
    note.textContent = full ? 'Full size: scroll a screen sideways.' : 'Screens are shrunk to fit; full size scrolls sideways.';
    fit();
  }});
  window.addEventListener('resize', fit);
  if (document.fonts && document.fonts.ready) document.fonts.ready.then(fit);
  fit();
}})();
</script>
"""

open(OUT, "w").write(page)
print(OUT, len(page) // 1024, "KB")
