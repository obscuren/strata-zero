# Your rig's own Net (#104)

Mock: `2026-11-18-rig-construct-mock/`. `python3 -I gen.py` in that folder writes the
current round (`rig-construct.html`); the earlier rounds are kept beside it
(`rig-construct-round1.html` … `round4.html`). The shots it draws over are in `shots/`.

## Why

- Jacking in at home today drops you on the city's grid at your block, or, from a list,
  at a backdoor's gate. The rig you've built up (the rack, its sleds, the firewall, the
  backdoors your relay holds, the jobs in its core) is a desk screen and a shell; on the
  Net it isn't anywhere.
- Containers (#96) hand out locked credchips, and nothing can crack one: using one in the
  pack says `locked · crack it at your rig`.
- #104 asks for a Net area of your own, built from what you own (your firewall, the
  hardware in the rack, a credchip you've put in the rig), where a locked credchip gets
  cracked.

## Decided

The look and its rules were settled over five rounds of the mock.

- **Look A, the room.** Your rig's Net is one floor plan, and each part of the rig is a
  room of it. Nothing physical is named in it: no chair, no desk.
- **The firewall** is the wall round the whole room, red, titled with its tier
  (`FIREWALL · sandbag ▮▯▯`). A rig with no firewall sled has no ring.
- **PORTS** (the relay) is a room down the west side with a door in its outer wall for
  each line. A held backdoor's door is lit, named for its building with the time it has
  left, and its lane runs out through the firewall. A free line's door is dark.
- **Backdoors are exits.** Walk through a held backdoor's door and you come out at that
  building's gate on the grid, as picking it from today's landing list does (a hotel's or
  office's lands you on its `L` hub). The landing list goes.
- **LINE** (the line sled) is the trunk out of the bottom of the room to the grid. Take
  it and you come out at your own block.
- **SEATS** (the core) is a room down the east side with a seat for each of the core's
  seats. A running job sits in one, drawn with its name, what it works on and its state;
  a free seat is dim. A job that works on a backdoor (`sniff`, `keepalive`) draws a
  dotted line from its seat across the room to that backdoor's door, and a held
  backdoor's door says `held · keepalive` while keepalive holds it.
- **The READER** sits in the hall between them: a slot for each of the rack's reader
  slots, holding what you put in at the desk. Stand at one and `space` summons what's in
  it (below).
- **The terminal** is a `◇` in the SEATS room, drawn the way the grid draws a terminal
  you hold: green, `TERMINAL` beside it, `[r] shell` under it. `r` opens it from anywhere
  in the room, and `space` at it does too. It opens the rig's own HELIX shell, logged in
  as you, in the JACK window every Net terminal's shell uses (`◇ JACK · ZENBO`, you on
  the right, no HELD clock). Whatever the shell changes (`run`, `kill`, `burn`) shows in
  the room as soon as it's done, because the room is drawn from the rig's state.
- **The panel** on the right is the rig's: its name and rack, the firewall, the LINES with
  what each holds, the READER's slots, the SEATS and their jobs, then what's under you,
  then `BODY at home`.
- **Summon** (`space` at a slot) does what the thing in it holds:

  | In the slot | Summon |
  |---|---|
  | a credchip | its sum comes to you, and the chip is gone, as cashing it from the pack does |
  | a locked credchip | nothing until it's cracked (below); then as a credchip |
  | a black credchip | nothing until MASTIFF is broken (below); then as a credchip |
  | a rig program shard (`sniff`, `keepalive`) | installs it, as sitting down with it does today, and the shard is spent; with a seat free, the rig asks which backdoor it should work on and runs it there |
  | a paydata shard or a vault dump | runs `sift` on it in a free seat |
  | a shard with a reading at the rig (Coz's ledger) | opens that reading's HOLO card over the map; the shard stays |

- **Cracking a locked credchip.** Stand at it and `space` runs your loaded Crack on its
  locks, one lock at a time, each a run as a lock on the Net is. The panel shows the
  locks left and your chance. Inside your own firewall nothing raises trace, and no ICE
  answers. When the last lock breaks the chip is an open credchip in its slot.
- **A black credchip and MASTIFF.** Some locked chips carry black ICE: MASTIFF, coiled
  round the chip in its slot, its reach tinted red round it. Asleep it does nothing.
  `space` at the chip asks first (`Wake MASTIFF?`). Awake, the fight goes to turns:
  - **The leash.** MASTIFF can't leave the red ground round its slot. Off it, you're out
    of its reach, but for every turn you stay off, a plate of its coat grows back.
  - **The coat.** Eight plates round a core, drawn on the board and shown as `COAT 5/8`
    in the panel. Each Attack that lands breaks a plate on the side you hit from; once
    the coat is off, the core takes the last hits.
  - **The bite.** Every few turns it marks the tiles it will bite on its next turn (`×`).
    Step off them or Shield. It's black ICE: a bite hurts your body, and the deck's
    failsafe dumps you before it kills you.
  - **Your rig fights with you.** A free core seat runs your loaded Attack at MASTIFF on
    its own turn, after yours, shown in the turn order (`TURN 4  YOU › seat 2 ›
    MASTIFF`) with its beam coming through the SEATS wall. A seat busy with a job sits
    the fight out, so a second core is a second fighter.
  - **No trace.** The fight is inside your firewall: TRACE stays at 0 and no tracer
    comes.
  - **Losing.** Dumped, or jacked out, and MASTIFF coils back onto the chip with its coat
    whole; the chip waits for another go.
  - **Winning.** The core breaks, MASTIFF goes to grit, and the chip opens: an open
    credchip, summoned as any other.

## What happens today

- **Jacking in at home.** At the rig, `j Jack in` (`interact::gather`) goes through
  `popup::Popup::fire`: while you hold a backdoor, `net::wants_landing_list` swaps in the
  JACK IN list (`net::landing_popup`: `a your block`, then a row per standing backdoor).
  `your block` runs `net::jack_in_here` → `Player::jack_in` → `Place::Stratum(Net)` at
  `HomeRig.landing`; a backdoor row runs `net::land_at(st, b)`, which checks the
  backdoor, `Landing::of` it (a site's `L` hub or the city gate), spends it, jacks you in
  there and starts the route home with that backdoor as its first relay
  (`after_jack_in(st, from, Some(b))`).
- **Getting out.** `J` → `net::jack_out`; being dumped → `net::dump_for`, which starts the
  home trace along the route.
- **Places.** `citygen::Place` is `Stratum`, `Floor`, `Construct(b)` and `SiteNet`. A
  construct's map is a `citygen::Construct` (tiles, light, entry, nodes, guards) in
  `st.constructs` by building; `MapRef::for_place` turns the place into the map that
  walking, FOV and drawing read. The Net overlay (`render/netlook`) draws the grid, a
  construct inside or a site's tower; the HERE card is picked by place in
  `explore.rs` (`sitenet::panel_lines`, `net::card_lines`).
- **The rig.** `rig::of(st)` reads the sleds: seats, speed, lines, hold days, firewall
  tier. The rack is `Character.frame` (Zenbo, Civitas, Lotus, Halcyon), the sleds are
  `Character.rig`'s fitted stacks. The desk (`screens/desk.rs`) fits them: the FRAME
  row, then a row per bay.
- **Jobs.** `st.backdoors.jobs` (`rig/jobs.rs`): `start`, `kill`, `running`; kinds
  `Sniff(b)`, `Keepalive(b)`, `Sift{shard}`, `Rent`. `sniff` and `keepalive` are
  installed from their shards in the pack when the rig's terminal boots
  (`terminal::rig_news`: `INSTALLED sniff FROM SHARD`). `run sift` takes a paydata shard
  or vault dump from the pack.
- **Shells on the Net.** `r` on a terminal you hold (`holds::use_verb`) opens
  `Terminal::open(st, host, at)`; jacked in, a terminal wears `NET_LOOK`, the JACK
  window, titled `◇ JACK · {place_label(host)}`.
- **Readings at the rig.** `interact::rig_reads` offers a row per carried item with a
  `ReadAt::Rig` reading; it opens `missions::reading::ReadCard` (HOLO).
- **Credchips.** `credchip` and `credchip_locked` (`Use::Cash{locked}`), their sum on the
  copy (`ItemCopy.credits`). Cashing one from the pack is `Action::UseCopy`; a locked one
  refuses with `locked · crack it at your rig`. Safes and lockers hand them out
  (`flavor/loot.ron`, `Chip((lo, hi), locked%)`).
- **Fights on turns.** `net::engage` rolls an order of `Actor`s (`You`, `Ice(i)`,
  `Foe(i)`, `Bot(site, n)`), with pools per actor (`pools_for`), turns driven by `pace`
  and `end_turn`. ICE hits by grade (`net::hit_by`): black takes HP, and the failsafe
  dumps you first (`FAILSAFE_COOLING`). Attack runs at a construct's ICE through
  `combat::attack_ice`.

## Goals

- Jacking in at the rig lands you in your rig's own Net, built from the rig as it stands,
  and every way out of it (a backdoor's door, the line to your block) works as today's
  landing list did.
- What you put in the rig's reader shows in the room and summons by what it holds.
- A locked credchip can be cracked there, and a black one fought for.
- The rig's shell opens from the room, and what it changes shows in the room at once.

## Not in this spec

- Coming back into the room from the grid without jacking in again.
- Defending the room: a trace or a corp counter-op reaching into it (#74's part C).
- Deck programs on chips or shards (summoned into the PROGRAMS slots); the reader takes
  only the kinds in the summon table.
- Sticks or shards guarded by anything but MASTIFF; more bosses later.
- A meat-side picture of the reader on the desk's rack drawing beyond its rows.

## What would make it a failure

- Jacking in at home is slower or more confusing than the list it replaces: the doors
  have to be right there, named, and one walk from where you land.
- The room disagrees with the rig: a door for a backdoor that's burned, a job that isn't
  running, a seat count that isn't the core's.
- The fight is a slog: MASTIFF must read on the board (its coat, its leash, the bite it
  marks), and lose in a handful of turns to a deck that has Attack loaded.
- A save made inside the room doesn't reopen there.

## The design

### Approaches

- **A · a place of its own (chosen).** A new `Place::Rig`. The room is a tile grid built
  from the rig every time you go in (it holds nothing that isn't in the rig, so it isn't
  saved), walked and drawn like a construct through `MapRef::Construct` with a construct
  value of its own, with its own renderer, panel and interactions. MASTIFF and a helping
  seat join the existing turn system as two new actors.
- **B · the room as the home building's construct** in `st.constructs`, reusing every
  building-keyed path (ICE, reach, attack chance). Rejected: the home block has its own
  construct under the same id on the grid, and the building-keyed code would treat the
  rig as a building (its security, gate, name and alarm).
- **C · a screen over the map** with a cursor on the rig's parts. Rejected: it loses
  walking, doors you walk out through, and the fight on turns.

### The place

- `citygen::Place::Rig`: inside your rig's own Net. `stratum()` and `construct()` are
  `None` for it, so building-keyed code leaves it alone; every `match` on `Place` gets an
  arm (the survey list is in "Left to the builder").
- Jacked in there, `player.body` is your body at the rig as today; `Player::jack_out`
  brings you back to it.
- The room is fully seen (as the grid is): no fog.
- The HUD's place line and the footer's first row read `zenbo · your rig`; the side
  panel's top reads `construct · unseen`; TRACE shows and stays 0 while you're inside.
- A save made in the room reopens in it: `after_restore` rebuilds the room when the
  player's place is `Place::Rig`. `runs::describe` says `in your rig's Net`.

### The room

Built by `rigroom::build(st) -> RigRoom` from `rig::of(st)`, the rack's reader slots and
the core's seats, as the mock draws it:

- **The firewall ring** two cells out from the floor's wall, double-lined red, titled
  `FIREWALL · <tier> ▮▯▯` (filled to the tier). None without a firewall sled.
- **PORTS** down the west side, titled with the relay (`PORTS`, `Switchback · 5`). A door
  per line in its outer wall, four rows apart (three if that doesn't fit), in the order
  `route` lists your backdoors, held ones first. A held door: `▯` in green, the building's
  name beside it and its time left under that (`2d left`, `6h left`), its lane running
  out through the firewall (`╬` where it crosses) to the room's edge. A free line: a dim
  `▯` and `free line`.
- **The hall** in the middle: `READER · N slots` at its top, the slots in a row under it
  (a box each, the thing's glyph inside, its name and state under), and the `LINE` trunk
  at its foot: `LINE <sled> · <days>d`, the trunk through the wall and the firewall, and
  `▸ the grid · your block` past it.
- **SEATS** down the east side, titled with the core (`SEATS`, `Tinman 2X · 2`): a box
  per seat. A job's box shows `◉ <job>`, what it works on and its state (`▰▰▰▱▱ 4h`
  for sift, `noise 3 of 36` for sniff and keepalive, `¤6/h` for rent); a free seat is a
  dim box saying `free seat`. Under the seats, the terminal.
- **Lines from seats to doors.** A job on a backdoor draws a dotted line (`┈ ┊` and
  corners, a pulse `●` on it) from its seat's box through the hall and the PORTS wall to
  that backdoor's door, ending in `◂`. keepalive's door reads `held · keepalive` in place
  of its time left.
- **The layout grows** with the counts: the rows needed are the most of PORTS'
  (`5 + 4 × lines`), SEATS' (`5 + 6 × seats + 3` for the terminal) and the hall's; it's
  never shorter than the mock's. A room taller than the map pane scrolls with you as any
  map does.
- **You land** in the hall, under the reader, facing the slots.

Everything that changes while you're in the room (a door held or burned, a job started
or killed, a slot filled, a chip cracked) is read from the game's state when it's drawn;
only the layout is built on entry.

### Walking out

- **A held backdoor's door.** Step onto it and you leave the room as `net::land_at` left
  the list today: the same checks, the same spend (`COST_LAND`, `COST_LAND_NET`), the
  same honeypot, `Landing::of` (a site's `L` hub or the city gate), and the route home
  starts with that backdoor as its first relay. A refused door says why in the log and
  you stay. A free line's door is a wall.
- **The LINE trunk.** Step onto its end and you come out at your block on the grid
  (`HomeRig.landing`), as `your block` did.
- **Jacking in at home** always lands you in the room; the JACK IN list goes
  (`wants_landing_list` is false at home). Street terminals are unchanged.
- **Jacking out** (`J`) from the room, and being dumped in it, work as anywhere on the
  Net; a dump in the room starts no trace (there's no route yet).

### The reader

- **Slots by rack** (a field of the rack's `FrameSpec`, my call): Zenbo half-rack 1,
  Lotus tower 2, Civitas cabinet 3, Halcyon server frame 4.
- **Held on the character**: `Character.reader: Vec<Option<Stack>>` (copies kept, so a
  chip keeps its sum), saved with the character. A change of rack to fewer slots sends
  the extras to the pack, as extra sleds go.
- **At the desk** a `READER` row follows the bays, one row per slot. `space` on a slot
  lists what in your pack the reader takes; picking one puts it in. `x` pulls it into
  your pack. Neither takes any time. The side card says what the thing in the slot will
  do when summoned.
- **What it takes:** credchips of every kind, `prog_sniff` and `prog_keepalive`,
  `paydata` and `vault_data` (not sifted ones), and any item with a `ReadAt::Rig`
  reading. Anything else isn't listed.
- **Glyphs**, kept clear of the Net's marks (`◇ ◆ ⬡ ⌗ ▪ ▮ ▯ ◈`) except where the mock
  settled them: a credchip `¤` amber, a locked one `▣` ice blue, a black one `◆` red, a
  rig program shard in CORE cyan, a data shard and a story shard in the builder's pick
  (the mock's `◈` clashes with ICE and must change; any new glyph needs a font that has
  it).

### Summon

`space` within a step of a slot's box opens a popup headed with the thing, whose first
row is its summon (and `x Leave it`):

- **A credchip** (`¤`): `Summon · +¤340`. Its sum goes to your credits and the chip is
  gone; the log says `summoned ¤340 · you carry ¤660`, the HUD's ¤ updates, and a `+¤340`
  number rises off you (the existing number fx).
- **A rig program shard**: `Install keepalive`. It's added to the rig's programs
  (`INSTALLED keepalive FROM SHARD`) and the shard is spent. If the program wants a host
  and a seat is free, a second popup lists your held Net backdoors (keys refused as
  `run` refuses them): pick one and it runs there (`rig::jobs::start`), else it's only
  installed. Already installed: the shard isn't spent and the row is `Run keepalive`.
- **Paydata or a vault dump**: `Sift it · 6h` (10 for a vault). It runs `sift` on it in a
  free seat, taking it from the slot; it comes back sifted to your pack as `sift` returns
  it today. No free seat: the row is greyed, `every seat is busy`.
- **A story shard**: its reading's verb (`Read`). It opens the HOLO card over the map
  pane (not the middle of the screen, so the rig's panel stays readable) and counts as
  read at the rig. The shard stays.
- **A locked credchip**: `Crack a lock · 58%` (below).
- **A black credchip**: `Wake MASTIFF` (below).

### Cracking a locked credchip

- A locked chip has locks by its sum: under ¤500 one, under ¤2,000 two, else three. The
  copy keeps how many are cracked (`ItemCopy.cracked`, default 0).
- `Crack a lock` runs your loaded Crack at it: a minute of game time, one roll at the
  chance `game::chance(power(crack, lock), difficulty(tier))` where the tier rises with
  the locks (1, 2, 3 for one, two, three locks). A hit cracks a lock (`lock 1 of 2
  cracked`), a miss says `the lock holds`. No Crack loaded, or no deck: refused as a run
  is on the Net.
- No trace, no ICE, no alarm. The last lock cracked turns the chip into a `credchip` with
  its sum, in its slot.
- The panel's HERE lines show the chip, its sum, `2 locks · crack 58% · space`.

### MASTIFF

- **The chip.** `credchip_black`, "Black credchip", `Use::Cash{locked: true}`: a bearer
  chip whose sum is under black ICE. Using it in the pack says what a locked one says.
  Safes guarded on a job hand one out now and then (`flavor/loot.ron`: a `BlackChip`
  line, sums ¤2,000 to ¤6,000 before the band's scale, weight 1 in guarded office and
  hotel safes).
- **Asleep.** MASTIFF is drawn coiled round the chip in its slot (`▛▀▜ ▌◆▐ ▙▄▟` in red,
  the core `◆` white), its leash tinted red round the slot. It does nothing.
- **Waking.** `Wake MASTIFF` (from the chip's popup) puts the room on turns: the order is
  you, each free seat in seat order, then MASTIFF. It leaves the slot to the tile in
  front of it. One MASTIFF awake at a time; a second chip's row is refused while a fight
  is on.
- **The board.** MASTIFF is three cells square: eight plates round its core, a broken
  plate drawn as `░` in a dim red. Its leash is a rectangle round its slot (six cells
  out each way, clipped to the hall), tinted, with a dashed edge on its open sides.
- **Its turn.** Two moves, toward you inside the leash, then: if no bite is marked, it
  marks one (the 3×3 tiles beside it on your side, drawn `×` in bright red) and the
  prompt says `MASTIFF bites the × tiles next turn`; if one is marked, it bites them:
  standing on one, you're hit as black ICE hits (Shield soaks it), and the marks clear.
- **The leash.** While you're off the leash at the end of your turn, MASTIFF grows a
  plate back (up to eight) and can't reach you.
- **Your Attack.** One AP, your loaded Attack at MASTIFF within its reach (the Net's
  attack reach), one roll at `game::chance(power(attack, ice), difficulty(3))`. A hit
  breaks the plate nearest you; with no plate left it hits the core, which takes two.
  Shield and the rest of the deck work as on the Net.
- **A seat's turn.** A free core seat runs your loaded Attack at MASTIFF at the same
  chance, scaled by the core's speed (`chance × speed / 100`, capped at 95), and its beam
  draws from its box through the SEATS wall for the turn. The turn order shows it as
  `seat 2`. No Attack loaded: the seats sit out.
- **Winning.** The core's second hit: MASTIFF goes to grit (a few `░ ▖ ▗ ·` where it
  stood, gone when you leave the room), the turns end, and the chip turns into a
  `credchip` with its sum.
- **Losing.** Dumped (the failsafe, or FOCUS gone) or jacked out: MASTIFF is asleep and
  whole again next time.
- **Inside your firewall**: TRACE stays 0, nothing calls a tracer, no alarm.
- The fight is not saved: a run reopened mid-fight finds MASTIFF asleep and whole.
- The footer's fight rows are the Net's: `ENGAGED`, the prompt and keys, `TURN 4  YOU ›
  seat 2 › MASTIFF`, the ICE list (`▣ MASTIFF · black ICE · coat 5/8`), the PROGRAMS and
  the pools. The panel's HERE lines: `◆ MASTIFF · black ICE`, `COAT ▮▮▮▮▮▯▯▯ 5/8`, `CORE
  sealed under the coat` (or `CORE 2 hits`), `BITE next turn · the × tiles` when marked,
  `LEASH the red ground`.

### The terminal

- A `◇` node in the SEATS room under the seats, drawn as a held terminal on the grid
  (green on its dark green ground, bold), `TERMINAL` beside it in green bold, `[r] shell`
  under in the label grey.
- `r` anywhere in the room, or `space` within a step of it, opens
  `Terminal::open(st, HostId::Rig, (Place::Rig, its tile))`: the JACK window, titled
  `◇ JACK · ZENBO` (the rig's place label in the room, not the home block's name), the
  rig's notice, mail and news (installs from the pack happen here as at the desk), with
  you on the right edge and no HELD clock. `esc · back to your rig` on its bottom edge.
- Nothing in the room needs refreshing after it closes: jobs, doors and slots are drawn
  from the state the shell changed.

### The panel

`rigroom::panel_lines(st)` in place of the HERE card while you're in the room, as the
mock draws it (round 5's frame 1):

- `ZENBO · your rig`, the rack (`Civitas cabinet · 4 bays`), `FIREWALL ▮▯▯ sandbag`;
- `LINES 3/5 · hold 5 days`, a row per line (`▯ Kiroflux Mart  net · 2d`, `key · 6h`,
  `net · held` under keepalive; `free line` dim), then `▸ the grid  your block`;
- `READER 2/3`, a row per slot (`¤ Credchip  ¤340`, `▣ Locked credchip  2 locks`,
  `◆ Black credchip  MASTIFF`, `· free slot`);
- `SEATS 2/2 · Tinman 2X`, a row per job (`◉ sift · Paydata shard  4h`);
- what's under you (a slot's thing and its summon, the terminal, a door and where it
  goes, MASTIFF in a fight);
- `BODY at home` at the foot.

### Saving

- `Place::Rig` is a new variant of a saved enum: old saves read as before.
- `Character.reader` and `ItemCopy.cracked` are `#[serde(default)]`.
- The room (`st.rig_room`) isn't saved: `snapshot` binds it `_` (rebuilt), and
  `after_restore` builds it when the player stands in `Place::Rig`. MASTIFF's fight state
  lives with the room and goes with it.
- The fixtures in `runs/fixtures/` keep opening.

### The manual

- `08-the-net.md`: Jacking in (at home you land in your rig's Net), Backdoors (the doors
  replace the list), and a new section, "Your rig's Net": the room, its parts, walking
  out, the reader and summon, cracking a chip, MASTIFF, the terminal; its marks in the
  table style the chapter uses.
- `09-the-rig-and-helix.md`: the desk's READER rows and reader slots per rack; `r` from
  the room.
- Wherever the manual says a locked credchip can't be cracked yet, it now points to your
  rig's Net.

### Checking

- Unit tests: the room's layout for each rack and for 1-8 lines and 1-6 seats (doors,
  slots, seats and the terminal on their tiles, the room walkable from where you land to
  every door, slot, seat front and the trunk); jacking in at home lands in `Place::Rig`;
  each door leaves as `land_at` did (a site's hub, a gate, a refused key); the trunk
  lands on `HomeRig.landing`; summon for each kind; crack odds and lock counts; MASTIFF's
  turn (mark, then bite), the leash regrowth, plates broken from your side, the core's two
  hits, a seat's attack, losing resets; the terminal opens the JACK window on the rig;
  `run`/`kill` in it show in the room's drawing; a save inside reopens inside.
- Shots (`screens/shots.rs`): the room as the mock's round 5 frame 1, a summon, the
  locked chip, the fight at turn 4 with a bite marked, after it, and the shell over the
  room; through `every_drawn_character_has_a_glyph`.
- Both front ends' five checks from `CLAUDE.md`, and the Windows and Linux `cargo check`.

## Calls I made

1. Reader slots per rack: Zenbo 1, Lotus 2, Civitas 3, Halcyon 4, in the rack's
   `FrameSpec`.
2. The reader is filled and emptied at the desk, at no time cost.
3. The reader takes credchips, the two rig program shards, unsifted paydata and vault
   dumps, and anything with a reading at the rig; nothing else.
4. A summoned credchip is used up, as cashing it from the pack does (the mock's empty
   stick staying in the reader gave way to #96's rule).
5. A locked credchip's locks come from its sum (1, 2 or 3), each cracked by one roll of
   Crack per minute; the cracked count rides on the chip's copy.
6. The black credchip is a new item from guarded office and hotel safes; MASTIFF guards
   every black chip, and nothing else.
7. MASTIFF alternates marking a bite and biting; its leash reaches six cells round its
   slot; its core takes two hits; plates grow back one a turn while you're off the
   leash.
8. A seat attacks at your Attack's chance scaled by the core's speed, capped at 95.
9. The JACK IN list goes; the doors and the trunk replace it.
10. The room isn't saved; a fight isn't either, and a run reopened mid-fight finds
    MASTIFF asleep and whole.
11. No coming back into the room from the grid without jacking in again.
12. The HOLO card over the map pane rather than the screen's middle in the room.
13. The JACK window in the room is titled `ZENBO`, not the home block's name.

## Left to the builder

- Every `match` on `Place` that needs a `Place::Rig` arm: `ui/hud.rs::location`,
  `walker/mod.rs` (`radius`, `sees_all`), `net::scene_of` and the turn code in `net.rs`,
  `holds::node_pos`, `look.rs`, `track/`, `screens/deck.rs`, `explore.rs` (stepping,
  `door_ahead`, `gateway`, the map, the overlay, the card, `cam_bounds`),
  `runs/describe.rs`, and `citygen::world`.
- How the room's tiles are typed for walking (a construct's `Tile` kinds, or a small set
  of the room's own read through `MapRef`), as long as walls, slot boxes, seat boxes and
  the terminal block and doors and the trunk are steps.
- The data and story shard glyphs.
- The exact rows of the desk's READER card.
- How MASTIFF's and a seat's actions plug into `pace` and `end_turn`, and the beam's
  picture.

## What done looks like

Jack in at your rig: you're in the room, your rig's parts round you, every backdoor a
door that takes you to its building and the trunk to your block. Put a credchip, a
locked one, a black one and a story shard in the reader at the desk, jack in, and summon
the cash, crack the lock, wake and break MASTIFF with a seat beside you, read the shard;
open the shell with `r`, `kill` and `run` a job, and watch its seat and its line change.
Save inside and it reopens inside. The manual says all of it, and both front ends draw
it.
