# Strata Zero

A terminal roguelike in a full cyberpunk setting. You will play it on foot
through a multi-level city, and later from inside the Net as well.

The first subsystem is in place: a seeded, deterministic city generator and the
full-screen terminal app that lets you generate, explore and walk its cities.

The generator builds a city as walkable tile maps on four strata:

| Stratum | What lives there |
|---|---|
| Net | cyberspace: a dotted grid, a construct on every building, district hubs, link lanes, ICE at the gates |
| Heights | rooftops of the tall towers, helipads, bridges between them |
| Skyways | elevated walkways, mid-rise lobbies, platforms over intersections |
| Street | districts, arterials, blocks, buildings, markets, neon |
| Undercity | tunnels under the roads, sewers, a rail line, basements, black clinics |

Every building you see at street level is the same building you stand on at
Heights. Most of them are closed: the footprint is solid on the city maps and
the door opens an interior of its own, with real floors (a corp tower has
thirty-odd) reached by lift and stair, a lobby, corridors of locked flats or
offices, a bar with its counter and tables, a factory's machine rows, and exits
back onto the street, over a sky bridge onto the Skyways, onto the roof and
down into the Undercity. Stalls and small shops (noodle bars, markets, shops,
pawnbrokers, arcades) stay walk-in on the street. Interiors are generated from
the seed the first time you walk in, a floor at a time, at three times the
footprint's scale. Districts have themes (corp core, night market, megablocks, combat
zone, docks, industrial, neon strip, old town) that drive street layout,
building mix, palette and signage. A Net graph of subnets, nodes and links is
anchored to the physical tiles and can be shown as a digital overlay.

A city has up to two megablocks, in its megablocks districts where a block is
big enough: each a whole city block, hollow inside. From the street its name
runs in sign letters along the façade beside the main door, with a canopy and
lamps on the paving in front. A street terminal you can jack into usually claims
the forecourt; vending machines and a bench fill whatever room the pavement has
left. A food stall and parked bikes stand nearby, and round the back a service
door by the dumpsters lets you onto the ground plaza too. Its upper floors ring an open atrium with a railing and a walkway, locked
flats around them and the lifts and stairs in a lobby off the walkway; over the
railing you look down on the ground plaza, dim and static, dimmer the higher you
are. The plaza holds the reception inside the main door, stalls and tables under
the drop, a diner and shopfronts. High up in the city's first megablock is your
flat: a living room along the windows, a bedroom with the desk and your rig, a
shower room and a storage room. A new game wakes you beside the bed, and so does
loading a save; your front door opens onto the walkway with no change of map. A
city without a megablock starts you on the street as before.

Other people's flats are locked. They open for the resident's keycard, an item
one household per building has left at the lobby desk, or for the flat's
four-digit door code, which a server copy in the building's construct writes to
the JOURNAL tab beside the keycards you hold and the backdoors you have planted.
Your own door code and keycard are yours from the first minute. Every other
flat's door has a keypad: stand beside it, press `space` and interact to key a
code by hand; the worn keys show which digits it uses, a right code is written
to the journal, and three wrong ones lock the pad for half an hour.

Every terminal has a second way in. Beside a street terminal, `space` offers
`access terminal`: a typed session on a CRT over the map. Every machine wears its
district's theme: municipal gunmetal in the Megablocks, white and chrome in the
Corp Core with polite refusals, neon in the markets, and cracked
plastic with amber glass, dead cells and a flicker in the Combat Zone, Old Town,
the Docks and the industrial blocks. Keypads follow their building, corp towers
and offices in chrome; your own rig is a battered street box until you upgrade
it. Every one of them runs HELIX/OS, at whatever version its maker shipped:
Civitas kiosks and K-40 keypads, Halcyon nodes and SG-7s in the towers, Lotus
Marquees and Luxpads in the markets with their names in two neon colours, and
thirty-year-old Zenbo 64s and ZK-2s on the street.

HELIX/OS is a small Unix: a prompt like `guest@rust-flats:/$`, a filesystem with
owners and permissions, `ls`, `cd`, `cat`, `grep`, pipes and `>` redirects, `man`
and `help`, all in lower case as you type it. `ls /net` lists the district's
buildings, `ssh` (by name, a prefix or the number `scan` gives) logs you in to
one as whoever your standing there allows, and `exit` brings you back. On a
building the hardware is in `/dev`: `cat /dev/door` reads the locks, `echo
release > /dev/door` (or `door release`) opens them for an hour the way a Net
spoof does, and the same goes for the lifts, the cameras and the alarm.
`/var/log/auth.log` shows what has been done to the place and `plan` loads its
ground floor into your map. Hold one of the building's keycards or codes and you
log in as that resident, with their home and their notes, where a door code
may be waiting; a backdoor on the server, or a server you cracked within the hour,
makes you `root`; otherwise you are a
`guest` who may look but not touch. `backdoor`, run as a resident or root,
writes your rig's key into their `~/.ssh/authorized_keys`: for a couple of
hours you can `ssh` in from home, and the rig can jack you in at that
building's gate on the Net; the Backdoor program does the same for days, as
`root`. Your rig keeps three such lines (`ls /net`, `route`, `burn <host>`),
and a run from home that gets dumped sends a trace back along the backdoors you
came through, burning each one it passes; cut it with `burn`, or they kick your
door in and take the rig for a day. Your own rig is a HELIX box too, and its
files are yours: `cat ~/mail/001-a-favour.txt` opens the first mail, your first
job: a fixer's favour, a neighbour's door code and a shard to fetch from the
flat, and `nano ~/notes.txt` keeps whatever you write there.

The same graph is cyberspace. Jack in at any street terminal, or at the rig in
your flat (walk up to it, press `space`, then `j`): your body stays at the port and you
emerge in the Net at the same spot, in a wireframe world laid out one to one
with the city; from home you come out beside your megablock's gate. `J` jacks
out, after asking. In Explore mode `]` past Heights shows the Net.

Every building's construct has an inside. Step onto its gate from the grid and
you are in: a spine of lane with rooms off it holding the building's nodes
(door locks, cameras, lift controls, terminals, the server, the data vault
deepest), ICE standing at the gate from security 160 (white, grey from 200)
and at the vault room's door from 200. Jacked in, the number keys `1`-`5` are
the programs loaded in your deck, in slot order (the row under the map reads
PROGRAMS, and abilities move to the `a` menu): press one on or beside a node or
a piece of ICE and it runs at once against the one thing it applies to, or opens
a short list when several are in reach; `space` opens the full RUN list, every
program against every target, each with the chance a turn succeeds (a d10 plus
your power, twice Net plus the skill and the program's rating, against the
security over eight), the turns it takes and the noise it makes; a program that
cannot beat the construct reads `0%`, and you may still run it and pay the
turns and the heat. Every `.` or `space` is then a turn: a roll against the
security, heat on the construct's trace (more on a slip), the hack landing on
the last success; a move aborts it. While a run is in flight the status reads
`RUN crack 1/3 · heat 4`, `HUNTED` first when an awake piece is in sight. The HERE section shows the trace. At 60 heat the
alarm trips: the building locks down for an hour (cracked locks relock, lifts
stop, the ICE wakes, the gate and the ports beside it refuse); at 100, or at
the end of a turn beside awake ICE, you are dumped to your body with no focus
and a deck that will not jack in for ten minutes. What lands lasts an hour:
`crack` opens anything (the only program that takes ICE down) and a cracked or
`spoof`ed lock opens the building's private rooms, a spoofed lift control
keeps the lifts answering through a lockdown, `blind` silences a camera,
`copy` pulls a paydata shard off a server or a vault dump off a vault (once
each), and `backdoor` on the server keeps the gate open to you for good and
cools the trace twice as fast. Programs are items that take slots on your
deck; the deck's blueprint (Inspect it on GEAR & INVENTORY's rack) loads and unloads them; the deck is locked while you're jacked in.

ICE hunts. White ICE keeps its post; grey and black ICE walk the spine on a
beat, the gate guard the near half and the vault guard the far half. A piece
sees along the spine, in its row or column, four tiles for white, six for
grey, eight for black, and never into a room. Seen, it wakes and comes for you
one tile a turn by the shortest path; lose it for three turns and it goes back
to its beat. The alarm wakes every piece at once. Beside you a piece hits every
turn: white adds five heat, grey drains ten focus (less your deck's rating)
and dumps you at zero, black takes six hp through the jack, more over
security 230. `attack`, loaded like any program, takes two integrity a turn
off the piece you point it at (white 3, grey 6, black 10) and breaks it for an
hour; `shield` soaks the next hit; `mask` halves how far you are seen for six
turns at a heat a turn. The HERE section lists the ICE in sight with its mode
and integrity. With the deck's failsafe on (on its blueprint, on by default)
black ICE pulls you out at a quarter of your hp: a hard dump, thirty minutes
of cooling, the hp stays lost. Off, black ICE can take you to zero, and that
is the one way to die: FLATLINED, then the title.

The ICE tree's talents all land. Ice pick and Deep dive add to crack and copy;
Deep dive also copies a vault quietly (a successful turn makes no heat, a slip
is still loud). Null route turns the first dump in a construct away: the heat
falls back to the alarm line, the alarm trips if it had not, and the next one
is real. The deep end halves what black ICE takes through the jack, lets the
failsafe wait for an eighth of your hp, and cools ten minutes after it pulls.
Cold boot is focus; Wide band, Handshake and Arc welder reach and cool the
deck's pulses. Inside a construct those pulses have teeth: Netscan paints the
construct around you, Ping lights every node (two heat), Arc trace jumps to
every piece of ICE in range and stuns it for two turns (five heat), the
netrunner's way out of a hunt. Signal ghost, the netrunner's passive, feels
awake ICE through the walls: it is listed and drawn wherever it is. Low-light,
from the optics, sees two tiles further between 18:00 and 05:59.

Darkness limits how far you see; light does not. A lamp or a neon sign in your
line of sight is seen from across the dark with the pool it lights, and light
stops at walls. The city has a day: from 05:00 the Street, the Skyways and the
Heights brighten toward noon under a pale sky, the lamps go off at 06:00 and
sight runs to the edge of the screen; dusk comes amber at 17:00 and the lamps
come back at 18:00. The Undercity never sees the sun and the Net has none.

Same seed and parameters always produce a byte-identical city.

The city is lived in. Every building is staffed and every bed is slept in: the
`sim` crate seeds a population at jack-in, with professions of its own
(shopkeepers, noodle cooks, barkeeps, corp drones, line workers, couriers,
ripperdocs, sex workers, fixers, door muscle, monks, gangers) and a daily
routine for each, so a shop is open when its keeper is behind the counter and
the night shift at the factory relieves the day shift at ten. People walk to
work along the sidewalks, and the ones who own a car take it from the kerb
when the commute is long and park it near the door. The Neon Strip is the
red-light district: its workers hold the sidewalk in front of the clubs from
nine at night and take clients into the hotels. Off shift, people head for the
bar they are known at, queue at the noodle bars at the rush, and stop for a
word (`…`) when they meet. Nobody waits for you to be looking: the whole city
moves on the clock, and only the people inside the screen's bubble are drawn;
on foot you see the ones in your sight, and they see you by the same rule: the
HUD's `dim · 2 eyes` row says how lit your tile is and how many have you in
sight, which is what stealth will read. A parked car stays in memory like
a pile. Walk into someone and you swap places, so a crowd never blocks the
way; `space` lists whoever stands beside you, to talk. Inside a building the
people the simulation has there stand at their stations when you walk in: the
barkeep behind the bar, drones at their desks across the floors, line workers
at the machines, residents behind their locked doors; they are solid, and
`space` talks to them too. The clock is the city's
own: it runs while you walk and stops in the free camera and in every menu, so
the overview is a still of the city, and the inspector reads whoever is under
the cursor there: who they are, what they are doing, what comes next for them,
where they live and work, and where their car is.

The city has work. Fixers hold the corners of the Night Market and the Neon
Strip from six in the evening to four in the morning (a city without either
still has one somewhere), and drink in a bar after. Stand beside one and `space`
opens their rows: `w Work` offers a job rolled from the city around them, a
shard to fetch from a locked flat, a server or a vault to copy, a backdoor to
leave on a named host, a sealed package to put into a named person's hands at
their station. Take it and the brief lands as a mail on your rig with the
deadline; the JOBS pane shows the job, its next step and the clock, red under
an hour. Everything ends back at the fixer: `h Hand in` pays in credits, `d
Drop` gives it up. Miss the clock and the job fails and the fixer remembers:
each one keeps a standing with you that opens copy work at two and plant work
at four, and closes the door at minus three. The welcome mail is the first of
them.

## Install

Builds for macOS, Linux and Windows are on the
[releases page](https://github.com/obscuren/strata-zero/releases). Or, on
macOS and Linux:

```sh
curl -fsSL https://github.com/obscuren/strata-zero/raw/main/install.sh | sh
```

and on Windows, in PowerShell:

```powershell
irm https://github.com/obscuren/strata-zero/raw/main/install.ps1 | iex
```

Either puts `strata` in `~/.cargo/bin`. Both scripts (`install.sh`,
`install.ps1`) run the installer of the latest release.

## Run

`strata`, or from a checkout:

```sh
cargo run --release -p strata-zero
```

Headless generation, for scripts and tooling:

```sh
cargo run --release -p strata-zero -- gen --seed 42 --size medium --out cities/demo.city
cargo run --release -p strata-zero -- gen --seed 42 --size large --json --out /tmp/demo.json
```

Sizes: `small` (160x80), `medium` (240x120), `large` (320x160) or `WxH`.
Other flags: `--districts 4..12` (an upper bound; slivers merge into their
neighbours), `--water none|bay|river`, `--density 0..1`.

Needs a truecolor terminal of at least 100x30 cells (iTerm2, Ghostty, kitty,
WezTerm). Saved cities go to `./cities/*.city`.

## Keys

The game captures the mouse, so dragging over it does not select text (in iTerm2,
⌥-drag still selects). Mouse controls are not here yet; its events are dropped.

**Dev mode** is on by default for now (`DEV_DEFAULT` in `crates/strata-zero/src/app.rs`)
and shows a `DEV` badge in the header. It unlocks the free camera, stratum
browsing, the console and the inspect and legend sections. Press `` ` `` to flip it at runtime;
with it off you are always on foot.

**Title** `↑↓` select · `enter` confirm (new game opens the chooser; new game
(debug) drops D'SHAWN, maxed out, into a fresh city; new game (combat) puts him
on an empty lot at dusk with a goon twelve tiles off and a training dummy (`&`,
stands still, shrugs every round off) six north; load game opens the save
list) · `q` quit

**New game** picks a template (VEX, KESSLER) or a custom build, then walks six
steps: ORIGIN, PROFESSION, STATS, SKILLS, NAME, SUMMARY. `↑↓` choose · `enter`
next · `esc` back. On STATS `→` buys a point and `←` refunds one; a point's cost
follows your origin's and profession's affinity. On SKILLS `→` `←` unfold and
fold a category and `space` buys or refunds it or a skill with SP. SUMMARY ends
on `enter`, JACK IN, which builds the character, generates a city and wakes
you in your flat. Unspent points stay with you and can be spent from the deck.

**Configure** `↑↓` field · `←→` adjust · type digits into the seed · `enter`
on `RANDOM SEED` / `GENERATE` · `esc` back

**Generating** `space` pause · `+` `-` steps per frame · `enter` skip to the
end · `r` reseed after an error · `esc` back

**Explore** `hjkl` / arrows pan the cursor (shift: fast) · `[` `]` stratum down / up (Net on top) ·
`w` walk · `tab` deck · `F1`-`F4` panes (here, jobs, map, log; `F5` `F6` inspect and legend in dev mode) · `v` the same as a popup · `m` messages ·
`d` digital overlay · `s` save · `r` regenerate · `g` configure · `:` dev console (dev mode) · `?` keys · `esc` pause

**Walk** `hjkl` `yubn` / arrows move · walking onto a closed building's door
goes inside, onto an interior's door back out; a locked door says whose it is ·
`<` `>` take stairs, lift, ladder or manhole; on a lift bank inside they open the
Floors panel (every floor, the bridge and the roof marked), on a stairwell they
climb one floor · `space` opens the window of the thing in reach, named in its
header, with fixed verbs: `x` look, `t` talk, `i` interact (a terminal, a
keypad), `j` jack in or out, `g` take, `>` `<` go up or down; with several
things in reach it asks which way first, and a direction opens that one ·
`J` jacked in: jack out, after asking · `x` look: a cursor on the map (the walk
keys move it, shift by eight; `tab` and `shift-tab` step through what is worth a
look) and a window beside it that never covers it, saying what your eyes can tell
you; `space` opens the window of what it is on when that is in reach, `esc` or `x`
puts it away · `1`-`5` fire the ability in that slot ·
`a` the full ability list · walking into someone swaps places with them, and
they step back once you move on · `enter` walks the aim ·
`.` wait; in a construct it hurries the beat, in a fight it ends your turn ·
`e` jacked in: turns by hand, and back when nothing hunts you · `g` aim a
walk · `tab` deck ·
`F1`-`F4` panes · `v` view popup · `m` messages · `d` digital overlay · `s` save ·
`?` keys · `c` back to the free camera (dev mode) · `:` dev
console (dev mode: `spawn`, `drop`, `give`, `heal`, `xp`, `focus`, `sp`, `pts`, `talent`, `tpts`, `program`, `heat`, `alarm`, `wake`, `engage`, `calm`, `ice`, `failsafe`, `who`
counts the people out, inside and driving, `clock hh:mm` (or `time hh:mm`) jumps the day there,
`tp x y` teleports on the current map, `eyes` tints the walkers who see you,
`light` paints the map grey by its light, `job fetch|copy|plant|deliver` rolls and takes one from the nearest fixer,
`standing n` sets every fixer's opinion of you, `jobs` lists them, `target name` puts the free camera and the inspector on that person (their building's door when they are inside), `clear`, `help`, with Tab completion
and history) · `esc` pause

The play HUD frames the map on one grid. The top band carries the handle,
level, credits and dev readouts, then HP, FOCUS and XP bars that run the width
of the map column; on the right, where you are and the clock, the CARRY bar and
the pane tabs. The right column holds the panes you toggle with `F1`-`F4` (here,
jobs with its objective pips and the clock, the minimap with coordinates, the stamped log;
`F5` `F6` inspect and legend in dev mode), separated by thin rules. The footer
is four rows that never resize: where you are (`on foot`, `camera` or `⚡` with
the building and district; overburdened in magenta; jacked in, the TRACE heat
and `HUNTED`) with `[?]` the only hint left; a blank row; the fight row (`TURN n`
and the order, the `RUN` in flight with its beat bar in calm, the ICE in sight
at the right); and the `ABILITIES` or `PROGRAMS` slots, with `MOVE` and `AP` at
the right while engaged and hollow keys on an ICE turn. Every result and refusal
lands in the log, and `m` shows the whole history. With the log pane hidden the
newest line shows briefly over the map instead.

**Jacked in** a construct runs on a beat: every 45 ticks the program in flight
rolls once and asleep ICE walks a tile of its beat; `.` hurries the next one.
ICE that sees you, the alarm, or a program pointed at a piece puts the
construct on turns: an order rolled once (you by your Net, pieces by grade,
black before grey before white, ties to you), then `MOVE` 4 tiles and `AP` 2 a
turn. Your turn has two halves and the footer's second row names the one you
are in. MOVE first: the arrows aim a path from you, as far as your moves reach
(a red cross on a piece nothing reaches), `space` or `enter` walks it a tile
every 4 ticks with the keys waiting, `esc` drops the aim, and `space` with the
aim on your own tile stays put; from the
entry, aiming one tile off the map is the gate, and the walk leaves the
construct; the program keys draw hollow until you do. Then ACT: `1`-`5` pick a program and the map
marks what it can run against (nodes beside you; ICE beside you or in sight
within 6 tiles), `tab` or the arrows move the pick, `space` runs it for one AP
(the run stays armed, and `space` rolls it again), a pulse, a shield or a mask
cost one as well, and `.` ends your turn; spending the last move opens ACT by
itself. In calm the same pick works and costs nothing. An ICE turn plays out
one action every 8 ticks so you watch it come: white 0 tiles and 1 hit, grey 2
and 1, black 3 and 2. A stunned piece passes its turn. A round with nothing
awake calms the board again unless `e` asked for turns by hand; jacking out, a
dump, death and leaving end it too. Breaking the last standing piece clears the
alarm (the alert ends, the trace falls back under the line); escaping does not,
and a broken piece stays down for its hour whatever the alarm does. Every roll draws a bolt to its target
(white for an attack), a slip a magenta mark on it, every hit a bolt back to
you (red from black ICE), and a shield that absorbs a cyan mark on you.

Items lie where they are dropped: a pile shows its kind's icon in amber (`≡`
when mixed), dimmed once out of sight, and the HERE section lists what is on the
focused tile. A scan paints the map around you into memory; a ping lights the
terminals and stairs in range for a while.

**Street combat** a goon that sees you (eight tiles, a clear line) puts the
street on the same turns as the Net: `TURN 1  YOU › goon` in the footer,
`ENGAGED` at the right, `MOVE ▰▰▰▰ 4/4  AP ●● 2/2`. Your turn is two halves. In
the move half `hjkl` steps and spends a move, `g` aims a path over the street
and `enter` walks it, `space` stays. The act half opens the fire cursor on the
nearest goon: `hjkl` moves it, `t` jumps to the next one (`tab` stays the deck's,
so a stim is a key away), the readout over it
reads `goon · Scrap nine · 8 tiles · 90%`, `space` fires the worn gun for one
AP, every round rolled on its own (the Ratchet's three, the pistol's one); a hit
flashes the goon red, the number lifts off, a miss runs on and sparks where it
stops. `1`-`5` fire an ability: Static (from a neural port) costs both AP, hurts
and stuns for a turn, and tears the cells around the goon. A stim from the deck
costs an AP. `.` ends the turn; the goon closes in on the pace and shoots back.
`e` engages by hand with a goon in sight memory, and calms the street when
nothing hunts you; at zero HP the goon drops (`%`), you flatline.

**Pause** (`esc` or `q`) resume · save game · load game · settings (dev mode,
rain, fps) · quit to title · quit game

**Deck** (`tab`) `←→` or `1`-`8` tabs (`0` is talents too) · `↑↓` rows · `space` act on a row ·
`esc` close; it reopens where you left it. Tabs: skills (the category board:
`enter` folds and unfolds, `space` offers an unlock priced in SP), stats (the
five stats and your pools; `space` buys a point outright), gear & inventory
(below), workbench (WRENCH unlocks it; every recipe is a drawing: `←→` flips
them, `↑↓` walks the parts, `space` builds once every part is on hand; an item
whose drawing has parts is drawn exploded, the rest as a process), jobs (in
hand, on offer, done), journal (empty for now), map (what you remember of this
stratum) and talents (below). Every list wraps at both ends. GEAR & INVENTORY is
three panels. The rack is the nine slots as tiles. The pack sits beside it with
a row of kind icons on top: `[` and `]` filter it by kind, and the active icon
names its kind. The selected item's card sits beside its drawing at half size.
`←→` move between the rack and the pack, so `tab` and `shift-tab` switch tabs
here. `space` on a tile picks what fits from the pack, or unequips and inspects
what is worn; on a pack row it opens the verbs: Inspect, Equip, Use, Drop one,
Drop all. Jacked in, your gear and pack are locked: those verbs dim and only
Inspect works. In a popup the letter or `enter` fires a row and `esc` closes it;
a card a row shows closes the popup too.
Inspect on an item opens its blueprint in a window over the deck. The jacked
deck's blueprint is the deck drawn as hardware: arrows point at a slot, a ROM
strip, the failsafe or a program in your warez and the deck's glass reads it
out; `space` picks a program up and drops it in a slot, ejects one, or lifts
the failsafe's cover and throws it. A deck you carry shows its own blueprint
but can't be worked until you equip it; hardware and the Ratchet have blueprints
of their own, drawn large, and a sticker shows itself; anything else reads
MISSING BLUEPRINT for now. `esc` puts a carried program back, shuts a lifted
cover, then closes the window.

Some items have sockets: the pawn deck has two, one on each side, and the Ratchet
SMG has a muzzle and a rail. Press `m` in its blueprint for the modding mode. The
item turns grey and only the sockets stay in colour; arrows move between them and
`space` opens a HARDWARE tray of the pieces in your pack that fit the socket you
chose. Pick one to fit it, pick another to swap it, or pick empty to take it out;
`esc` backs out of a choice, then closes the window. You can only do this out of
the Net. Hardware only counts in what you wear. An empty socket shows just its
connector, the deck's gold contacts, the gun's gold threads or its bare rail, and
says what it takes; a fitted piece sits in its place on the item. Deck hardware:
Coldfin vI cuts the trace of each cast by 10%, Coldfin vII by 20%, and the Sidecar
co-processor adds 1 to program power. Weapon hardware: the Hushcan, a muzzle
piece, adds 1 to Cool, and the Redeye sight, for the rail, adds 1 to Reflex.

Stickers are cosmetic decals you find and carry in your pack. Anything you can
wear that has a drawing takes them: the pawn decks and the Ratchet. Press `s` in
its blueprint for the sticker mode: the item turns grey except where stickers may
go, and a tray of your stickers rises over its foot, each drawn as it looks.
`space` picks one and puts it on the item; the arrows move it, `r` turns it a
quarter clockwise (lettering stands up, its letters upright), `space` sticks it
and `esc` puts it back. `↑` from the tray points at the stickers already on;
`space` twice peels one, and it tears. Stickers never cover a working part, may
overlap (the newest on top) and change nothing but the look. Every pawn deck
comes with NO FUTURE and x‿x. You can only do this out of the Net.

On the rack and in the pack an item's marks follow its name: a deck shows its program
slots, then its sockets, as in `■■■■■ · ■□` (a filled box is a program in use or
a piece fitted, an empty one is free); on the rack the marks sit in the tile's bottom edge. The same marks tell two copies of one item
apart in the slot popup and on the ground. Skills and categories inspect as
cards. The readout shows hp and the carried weight over capacity (magenta when
over, and nothing more can be picked up). Items carry
perks while worn (stat, sight, carry, max hp) and can grant abilities;
consumables heal when used. The templates, origins, professions,
categories and every item, ability, skill, talent and recipe live in
`flavor/content.ron`.

**Talents** (deck tab `8` or `0`) are five trees, one per stat: MEAT, EDGE, WRENCH,
STREET and ICE, each a column of five tiers with two talents a tier and a
capstone on top. A talent has up to three ranks; each rank adds something you
can read on the sheet (a stat, max HP, focus, sight, carry), grants an
ability, sharpens one (reach, cooldown) or sets a named flag the combat and
shop systems will read later. A tier opens when the tree below it holds three
points a tier; a talent with a prerequisite wants it at its best first. You
start with one point and gain one a level. In the tab, `←→` walk a tier and
on into the next tree, `↑↓` move between tiers, `space` learns a rank; the
dossier at the bottom reads what the next rank brings or why it is locked.
The dev console learns by id with `talent <id>` and adds points with `tpts`.

## Layout

```
crates/citygen       world model + city generator + interiors (no terminal dependencies)
crates/game          characters, items, abilities, the build rules (no terminal dependencies)
crates/sim           the living city: people, places, paths and the world's minute
crates/strata-zero   the game: ratatui app, screens, renderer, walking and jacking in, the `gen` CLI
flavor/              names.ron (names, handles, signage glyphs) · style.ron (palettes, glyph sets) · content.ron (game content) · people.ron (professions and their lines)
docs/superpowers/specs   design specs
docs/superpowers/plans   implementation plans
```

## Develop

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p citygen --test seeds -- --ignored   # 250-seed sweep
```

## Release

Bump `version` in the workspace `Cargo.toml`, commit, tag `vX.Y.Z` and push
the tag. The GitHub mirror runs `.github/workflows/release.yml`, built by
[dist](https://opensource.axo.dev/cargo-dist/), which builds every target in
`dist-workspace.toml` and publishes them with the installers as a GitHub
Release. After changing `dist-workspace.toml`, run `dist generate`.

## License

MIT. See [LICENSE](LICENSE).
