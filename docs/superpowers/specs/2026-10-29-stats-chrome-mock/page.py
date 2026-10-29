"""The companion page: python3 page.py m.json out.html"""

import html
import json
import sys

d = json.load(open(sys.argv[1]))

# site -> (note, [(name, load, does, line, now)])
CHROME = [
    ("NEURAL", "the skull and the jack behind the ear", [
        ("Neural port", 1, "grants Ping, Static", "Standard jack behind the right ear. The seal itches when it rains.", True),
        ("Chipsocket", 1, "slot a skillchip: a skill you don't own, while it's in", "Somebody else's lessons, rented by the hour.", False),
        ("Pain editor", 2, "no wound penalties", "Turns the hurt down to a number you can read later.", False),
        ("Limbic damper", 2, "COOL +1, can't be intimidated", "Files the edges off fear. Also off joy, but who's counting.", False),
        ("Feedback fuse", 1, "FBK +25; the first black-ICE hit that would flatline you blows the fuse instead", "A cheap breaker between you and the Net. Pops once, smells of hair.", False),
        ("Ghost cache", 2, "every floor you've walked stays on your map", "Your memory, backed up to a chip the size of a fingernail. The chip remembers better.", False),
    ]),
    ("EYES", "a pair, one piece", [
        ("Nightglass optics", 2, "sight +4, grants Low-light", "Low-light lenses pulled from a security contractor's surplus.", True),
        ("Target lens", 2, "sight +1, grants Deadeye", "A lens with a box that follows whoever you are thinking about shooting.", True),
        ("Flicker iris", 1, "flashbangs don't blind you", "Shutters faster than a flashbang can say hello.", False),
        ("Heatsight", 2, "see warm bodies through walls within 6 tiles", "Paints the room in body heat. Ruins dinner parties.", False),
        ("Zoom barrels", 2, "sight +3; ranged hits hold up at long range", "Telescoping lenses. You look like a startled crab when they extend.", False),
        ("Secondhand eyes", 1, "sight +1, cheap", "Someone else's eyes. Their last view is still burned into the corner.", False),
    ]),
    ("EARS", "", [
        ("Bonephone", 1, "hear voices through walls and doors", "Picks up voices through concrete. Also the neighbours.", False),
        ("Damper cochlea", 1, "a blast's stun is a turn shorter", "Clamps down on anything loud. Conversations included.", False),
        ("Tripwire ear", 2, "never jumped: first in the order when a fight finds you", "Rings when somebody behind you holds their breath.", False),
    ]),
    ("JAW", "a jaw replacement, the lower face in steel", [
        ("Bulldog jaw", 2, "grants Bite: melee you can't be disarmed of", "A steel jaw on a hinge rated for car doors. Smile carefully.", False),
        ("Voicebox", 2, "plays back a voice you've heard: talk past voice locks", "Records a voice and plays it from your throat. Your mother would not know.", False),
        ("Rebreather", 1, "TOX +40; walk through gas", "A filter where your back teeth were. Gas clouds become a suggestion.", False),
        ("Grinder jaw", 1, "eat junk for 2 HP", "Grinds anything to paste. Bottle caps count as fibre.", False),
        ("Spit dart", 2, "grants Spit: silent, poisoned, once a rest", "A dart under the tongue. Kissing is off the table.", False),
    ]),
    ("SPINE", "", [
        ("Redline spine", 3, "REFLEX +1, MOVE +1", "A bus of fast wire down the back. The world slows down and so do your manners.", False),
        ("Hardback brace", 2, "carry +10, ARM +1", "A titanium brace from neck to tailbone. Standing up straight is no longer optional.", False),
        ("Overclock bus", 3, "+1 AP on your first turn of a fight, focus −10 after it", "Runs your nerves hot for one round, then sends the bill.", False),
    ]),
    ("BACK", "extra limbs mount here", [
        ("Spider rig", 4, "two thin arms: hold a second weapon, reload for free", "Two thin arms off the shoulder blades. They reload, they hold, they don't ask.", False),
        ("Shrike rig", 5, "bladed arms: a free strike on anyone who ends a move next to you", "Folded blades between the shoulders. They open like a bad idea.", False),
        ("Counterweight tail", 2, "no fall damage; climb without a ladder", "A tail for balance. Nobody makes the joke twice.", False),
        ("Drone cradle", 3, "grants Scout: a drone flies ahead and maps the floor", "A cradle for a drone the size of a sparrow. It comes back when it's bored.", False),
    ]),
    ("ORGANS", "under the ribs", [
        ("Adrenal pump", 2, "grants Redline", "A pump under the ribs that sells you the half second and bills the body later.", True),
        ("Second heart", 3, "HP +20; the first time you drop, you get back up at 1", "A spare heart behind the real one. It starts when the first one quits.", False),
        ("Filter liver", 1, "TOX +50; stims last twice as long", "You can't get drunk any more, and it shows.", False),
        ("Capacitor ribs", 2, "focus +10; shock attacks hit harder", "Ribs that hold a charge. Hugs are a liability.", False),
        ("Smuggler's gut", 1, "a 2 kg pocket that scanners don't see", "A sealed pocket where your appendix was. Customs never asks.", False),
    ]),
    ("SKIN", "", [
        ("Subdermal mesh", 2, "ARM +1, KIN +15", "Mesh under the skin. Knives go in a little way and think again.", False),
        ("Ceramic plating", 4, "ARM +3, KIN +30, THM +15, MOVE −1; needs BODY 6", "Plates in the chest and back. You knock on yourself for luck.", False),
        ("Grounding weave", 1, "SHK +40", "Copper weave under the skin. You leak lightning into the floor.", False),
        ("Chameleon skin", 3, "standing still, you can't be seen past 3 tiles", "Skin that takes the colour of the wall. Stand still and the guard looks through you.", False),
        ("Smart ink", 1, "COOL +1 with anyone who can see it", "Living tattoos. Yours show the odds of the next fight.", False),
    ]),
    ("ARM", "L and R apiece; a limb replaces the arm and has a hand and a forearm socket", [
        ("Grip arm", 3, "BODY +1, carry +12", "Hydraulic forearm with a clamp hand. Built for cargo, fine for doors.", True),
        ("Piston arm", 3, "BODY +1, grants Shove", "A piston in a sleeve, bolted on at the elbow. Good for arguments that have gone physical.", True),
        ("Hollow arm", 2, "the forearm socket is hidden from scanners", "A light arm with nothing where the bone was. Security wands just hum.", False),
        ("Surgeon arm", 3, "TECH +2", "Twelve joints a finger. Locks and wiring open like books.", False),
        ("Wrecker arm", 4, "BODY +2; punches through doors", "Demolition arm, filed down to street legal. Mostly.", False),
    ]),
    ("· hand", "socket on a chrome arm", [
        ("Arc hand", 1, "grants Jolt, SHK +10", "A hand that spits charge at whatever it touches. Keep it dry.", True),
        ("Toolfingers", 1, "TECH +1; open locks without a kit", "Each finger unscrews into a tool. Handshakes are an event.", False),
        ("Data spike", 1, "jack a terminal by touch, no port needed", "A spike out of the index finger into any port. Rude, but quick.", False),
        ("Razor nails", 1, "melee hits bleed", "Ten scalpel nails. You've stopped biting them.", False),
    ]),
    ("· forearm", "socket on a chrome arm", [
        ("Holdout pistol", 1, "a sidearm that draws free and passes scanners", "Comes out of the wrist with a sound like a cough.", False),
        ("Grapple line", 1, "grants Grapple: pull yourself, or them, 6 tiles", "A hooked line in the wrist. Ceilings are now a suggestion.", False),
        ("Launcher sleeve", 2, "throws reach 3 tiles further", "A spring tube for grenades along the forearm.", False),
        ("Monowire spool", 2, "grants Whip: reach 2, cuts through armour", "A wire too thin to see. Count your fingers after.", False),
        ("Fold shield", 1, "ARM +2 while you stand still", "A plate that fans out from the forearm. Cover you carry.", False),
    ]),
    ("LEGS", "a pair; knee and foot sockets", [
        ("Sprinter shanks", 3, "MOVE +1", "Carbon shins with a spring in them. Running is the easy part.", False),
        ("Dog legs", 4, "MOVE +2; no boots or leg armour fit", "Bent-back racing legs. Shoes are a memory.", False),
        ("Hydraulic knees", 1, "knee: no fall damage, grants Stomp", "You land like a dropped safe, and walk away from it.", False),
        ("Spring heels", 1, "foot: jump a gap of two tiles", "A coil in each heel. Gaps are a run-up now.", False),
        ("Silent pads", 1, "foot: footsteps make no noise", "Gel soles grown into the foot. Cats find you unsettling.", False),
    ]),
]

CONCEPT_NOTES = {
    "a": ["Every site visible at once, held or empty", "Drawing the body is the hard part; each piece needs a look on it",
          "Fits 100×30; at 120×40 the figure can grow"],
    "b": ["Closest to Caves of Qud and to the SKILLS cards", "Each piece needs card art, the way skills do",
          "A limb's sockets fold under it in the list"],
    "c": ["The cursor moves over the body itself", "You see what is meat and what is chrome at a glance",
          "The bay names what fits an empty socket"],
}

css = d["css"] + """
:root{color-scheme:dark}
body{background:#0b0d12;color:#cdd4de;margin:0;font:15px/1.5 -apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif}
main{max-width:1180px;margin:0 auto;padding:28px 16px 60px}
h1{font:700 22px 'JetBrains Mono',Menlo,monospace;color:#00e5ff;letter-spacing:.06em;margin:0 0 6px}
h2{font:700 15px 'JetBrains Mono',Menlo,monospace;color:#00e5ff;letter-spacing:.08em;margin:44px 0 10px;
   border-bottom:1px solid #283c4b;padding-bottom:6px}
p.lead{color:#a0aab9;max-width:820px;margin:0 0 8px}
.opt{display:grid;grid-template-columns:auto 1fr;gap:20px;align-items:start;background:#0f131a;border:1px solid #1d2733;
     border-radius:8px;padding:16px;margin:16px 0;cursor:pointer}
.opt.selected{border-color:#ff00c8;box-shadow:0 0 0 1px #ff00c8}
.opt h3{font:700 15px 'JetBrains Mono',Menlo,monospace;color:#cdd4de;margin:4px 0 8px}
.opt.selected h3{color:#ff00c8}
.opt p{color:#a0aab9;margin:0 0 10px}
.opt ul{margin:0;padding-left:18px;color:#a0aab9}
.opt li{margin:3px 0}
.scroll{overflow-x:auto;max-width:100%}
@media (max-width:1100px){.opt{grid-template-columns:1fr}}
.notes{display:grid;grid-template-columns:repeat(auto-fit,minmax(250px,1fr));gap:12px;margin-top:14px}
.note{background:#0f131a;border:1px solid #1d2733;border-radius:8px;padding:12px 14px;color:#a0aab9;font-size:14px}
.note b{font-family:'JetBrains Mono',Menlo,monospace;color:#cdd4de}
.site{margin:22px 0 6px;font:700 13px 'JetBrains Mono',Menlo,monospace;color:#cdd4de;letter-spacing:.06em}
.site span{color:#5c6673;font-weight:400;letter-spacing:0;margin-left:10px}
table{border-collapse:collapse;width:100%;font-size:14px}
td{padding:5px 10px 5px 0;vertical-align:top;border-top:1px solid #161d26}
td.n{font-family:'JetBrains Mono',Menlo,monospace;color:#cdd4de;white-space:nowrap;width:190px}
td.l{font-family:'JetBrains Mono',Menlo,monospace;color:#00c8e0;white-space:nowrap;width:44px}
td.d{color:#cdd4de;width:36%}
td.f{color:#7a8494;font-style:italic}
.now{font:11px 'JetBrains Mono',Menlo,monospace;color:#50e678;border:1px solid #2a5a3a;border-radius:3px;padding:0 4px;margin-left:6px}
@media (max-width:760px){td.f{display:none}td.n{white-space:normal;width:auto}}
"""

out = ["<!DOCTYPE html><html><head><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'>"
       "<title>STATS and chrome</title><style>%s</style></head><body><main>" % css]
out.append("<h1>STATS · YOUR NUMBERS AND YOUR CHROME</h1>")
out.append("<p class=lead>The deck's STATS tab at its smallest size, 100×30, on the game's own frame. "
           "The left pane is the same in every option: the five stats you buy, the secondary numbers that only gear and chrome move, "
           "and resistances. The right pane is the chrome, in three ways. Click the one that's closest.</p>")
out.append("<h2>THE CHROME PANE · THREE WAYS</h2><div class=options>")
for key, title, blurb, h in d["screens"]:
    notes = "".join("<li>%s</li>" % html.escape(n) for n in CONCEPT_NOTES[key])
    out.append("<div class=opt data-choice='%s' onclick='toggleSelect(this)'><div class=scroll>%s</div>"
               "<div><h3>%s</h3><p>%s</p><ul>%s</ul></div></div>"
               % (key, h, html.escape(title), html.escape(blurb), notes))
out.append("</div>")

out.append("<h2>THE LEFT PANE, FOCUSED</h2>")
out.append("<p class=lead>Arrows walk the tiles; the readout under them explains the one you're on. "
           "Here it's EMP, the price of going chrome.</p>")
out.append("<div class=scroll>%s</div>" % d["left"])
out.append("<div class=notes>"
           "<div class=note><b>STATS</b> · BODY, REFLEX, TECH, COOL, NET. The tile shows the total; the bottom edge shows what gear and chrome add. Space buys a point.</div>"
           "<div class=note><b>SECONDARY</b> · HP, focus, armour, sight, carry, move. Points never buy these; only gear and chrome move them. ARM and MOV are new.</div>"
           "<div class=note><b>RESISTANCES</b> · kinetic, thermal, shock, EMP, toxin, feedback. EMP drops 2 for every piece of chrome you carry. Feedback guards you against black ICE.</div>"
           "<div class=note><b>LOAD</b> · how much chrome the meat tolerates, from BODY and COOL. Every piece costs load; it's the budget the chrome pane spends.</div>"
           "</div>")

out.append("<h2>THE CHROME · %d PIECES OVER ELEVEN SITES</h2>" % sum(len(p) for _, _, p in CHROME))
out.append("<p class=lead>Eleven body sites, each holding one piece: neural, eyes, ears, jaw, spine, back, organs, skin, left arm, right arm, legs. "
           "A chrome limb replaces the arm or the legs and brings sockets for smaller pieces, the same sockets decks and guns already have. "
           "Extra limbs go on the back. <span class=now>now</span> marks a piece that's already in the game, moved to its site.</p>")
for site, note, pieces in CHROME:
    out.append("<div class=site>%s<span>%s</span></div><table>" % (html.escape(site), html.escape(note)))
    for name, load, does, line, now in pieces:
        out.append("<tr><td class=n>%s%s</td><td class=l>L%d</td><td class=d>%s</td><td class=f>%s</td></tr>"
                   % (html.escape(name), "<span class=now>now</span>" if now else "", load, html.escape(does), html.escape(line)))
    out.append("</table>")

out.append("<h2>WHAT IT MOVES UNDERNEATH</h2><div class=notes>"
           "<div class=note><b>Chrome leaves the GEAR rack.</b> NEURAL and ARMS go; EYES and TORSO keep only what you wear. Optics and shades, or a jacket and an adrenal pump, stop fighting over one slot.</div>"
           "<div class=note><b>Limbs carry sockets.</b> The Arc hand becomes a hand piece that fits any chrome arm. The blueprint's modding mode already fits hardware into a gun's or a deck's sockets.</div>"
           "<div class=note><b>Installing is a ripperdoc's job.</b> The pane shows what's in you and what would fit; a clinic puts it in. That's a later piece of work.</div>"
           "</div>")
out.append("</main></body></html>")
open(sys.argv[2], "w").write("".join(out))
