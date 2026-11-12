# The Net

Over the city runs the Net: the systems behind every lock, lift, camera, lamp and screen. A runner who can get into them can open doors, blind cameras and put out the street lights. Out on the street a netrunner is easy prey; in the Net the city does what you tell it.

## Jacking in

You jack in from a terminal. On the street, stand on or next to a public terminal (`▣`) and press `space`, then `j Jack in`. At home, your rig does the same and is never locked down. Out on open ground, any terminal or vending machine with a signal will do.

Inside a hotel or office, the terminals are set in the walls (`◇`): a kiosk in the lobby, and desk terminals in some of the staff's rooms. The switch box of a line of turnstiles (`▪`) takes you too. Stand beside one, on its own side of the wall, and press `space`, then `j Jack in`. You start on its node, on the building's own Net (A building's own Net, below).

You can jack in with nothing, but you can't run a program without a deck in your DECK slot. The log tells you when a port won't take you: it's locked down because its building, or the one next to it, is on alert, the port is dead, your deck is still cooling off, or you're in a fight.

Your body stays where you jacked in. Cameras pay it no attention while you're away. Jacked in on a hotel's or office's floor, the `BODY` line in the HERE pane says where your body stands and who is near it: `nobody on L`, or how many guards are on that floor and how close the nearest one is.

## Moving in the Net

The open Net lies over the city as a stratum of its own, the **grid**, with every tile in plain sight. Nodes sit where the systems are; lanes run between them.

| Mark | What it is |
|---|---|
| `◇` | a terminal |
| `◆` | a server |
| `⬡` | a hub |
| `⌗` | a data vault |
| `▪` | a lock, a camera, a lamp grid and the like |
| `▮` | a lock of a firewall; `▯` once it's broken |
| `◈` | ICE |

Every building has a **construct**: its own little Net behind a gate. Step onto the gate to go in. A building on alert keeps its gate closed. Security runs from Open, through Guarded and ICE, to Black; the higher it is, the harder everything inside is to crack, and the nastier the ICE. A hotel or an office has more than a construct: a Net of its own, behind a firewall (A building's own Net, below).

With the **cord** on (SETTINGS), a line on the grid leads back the way you came.

## Programs

Load programs into your deck from its blueprint, before you jack in (The deck). A deck has only so many slots, and each program takes one or two.

| Program | Works on |
|---|---|
| Crack | any node but a hub, or ICE |
| Spoof | locks, lifts, cameras, ad boards, lamp grids, terminals, gate switches (the switch box of a line of turnstiles) |
| Blind | a camera |
| Copy | a server or a data vault |
| Backdoor | a server |
| Attack | ICE |
| Shield | you: soaks one hit |
| Mask | you: harder to see |

### Running one

1. Stand on or next to the node (ICE can be further off, if nothing is in the way).
2. Press the program's number, `1` to `5`. The targets in reach are listed nearest first; `tab` or the arrows pick one.
3. `space` runs it. `esc` changes your mind.

You can also press `space` on a node: its window lists every program you can run on it, with your odds, how many turns it takes and how much noise it makes.

A program takes several turns. Each turn it rolls your power against the node's security: your NET twice over, the program's rating, the right skill and any hardware you've fitted. Moving away stops it. While nothing hunts you, the game rolls a turn every beat; `.` or `space` hurries it along.

### What a landed program does

- A cracked or spoofed lock opens the building's doors for an hour. A lift control makes the lifts answer you.
- In a hotel or office, each node works its own device and no other. A door's lock opens that one door; in an alarm, a crack you land after it went off opens it again, until the alarm ends. A gate switch opens its line of turnstiles. A spoofed lift stops at every floor it serves without asking for a card, and one you spoof during an alarm answers you. A turret or a container's lock stays cracked for the hour, and the `HELD` list shows it.
- A blinded camera sees nothing.
- A cracked server makes you root on its host for the hour (The rig and HELIX).
- Copying a server gives you a paydata shard, and the building's door codes go into your journal. Copying a vault gives you its dump.
- A backdoor makes the server's gate yours.

Hacks last an hour, or until the building's alarm goes off.

## Holding the city

Crack or spoof a camera, an ad board, a lamp grid or a terminal and you **hold** it for an hour. While no one hunts you, these keys act on the nearest node you hold:

| Key | On | What it does |
|---|---|---|
| `f` | a camera | tap its feed |
| `o` | a camera | loop it |
| `o` | a lamp grid | lights out |
| `i` | an ad board | hijack the screen |
| `z` | an ad board | lure the ICE |
| `r` | a terminal | open a shell |

A looped camera sees nothing until the hold ends. Lights out kills the district's street lamps for twenty minutes, at night. A hijacked board can smear a corp, tag your name, or put up an offer that draws people over to look.

A hotel's or office's cameras and terminals are held the same way, from its own Net, and the keys work on the ones on your level. `r` on one of its terminals opens a shell on the building's own host.

## Trace

Everything you do in the Net makes **noise**, and noise raises the **trace** of the building or district you're working in. The footer shows it.

- At 60 the alarm goes off for an hour: the building locks down, its hacks end, its ICE wakes, its gate closes and its ports lock, on its floors and beside it. A firewall's broken locks close again; break them all again and its gate lets you through, until the alarm ends.
- At 100 you're **dumped**: thrown back into your body.

Trace falls by itself, a little each minute, once you've left. A camera that sees you, on the street or on a hotel's or office's floor, raises its building's trace too: enough to set off the alarm, never enough to dump you.

## ICE

ICE guards constructs and walks the grid. It comes in three grades, and when it sees you the fight goes to turns.

- **White** ICE raises your trace when it hits.
- **Grey** ICE drains your FOCUS. At zero, you're dumped.
- **Black** ICE hurts your body.

Your deck's rating and your feedback resistance soften grey and black hits, and a shield soaks a whole hit. Attack breaks ICE; break the last one standing and the alarm ends.

Out on the grid, **sentinels** patrol near their hubs, but only bother you while their district is after you. When an alarm goes up from a run on the grid, a **tracer** comes after you. It follows your trail and dumps you if it catches you. Lose it at a hub, mask your trail, or jack out.

## Getting out

`J` jacks out, after asking, from anywhere on the Net. Jacking out stops any program you were running.

Being dumped is worse: your FOCUS is gone and your deck has to cool for ten minutes before you can jack in again.

> Your deck has a **failsafe**, on unless you turn it off. When black ICE brings you low, it dumps you rather than let you die, and the deck cools for half an hour. Turn it off on the deck's blueprint and black ICE can kill you.

## Backdoors

A backdoor planted on a server lasts three days. From your rig, `j` lists your backdoors as well as your own block: pick one and you land at that building's gate. From a server you hold, you can **hop** to another backdoor's gate. Your rig keeps up to three.

A backdoor on a hotel's or office's server lands you on its `L` hub instead, past the firewall and beside the server, and so does a hop to it.

If you're dumped on a run that started at your rig, a trace follows your hops home, burning each backdoor it passes. If it reaches your door, your rig is seized for a day and every backdoor burns.

## A building's own Net

Hotels and offices have a Net of their own, made from the building itself. It has a **level** for each of the building's floors, and each level is its floor's plan, tile for tile: the same rooms, the same walls, the same doors. What you learn there is the floor you'll walk.

Every device on a floor is a node on its level: each door's lock and each private room's keypad, the switch of each line of turnstiles, and the terminals. Each lift has one too, at its stop on the lowest floor it serves that the riser reaches. Each level has a hub (`⬡`) beside the riser (below), and `L` has the building's server (`◆`) beside its hub. On a job, the building's cameras, turrets and containers are nodes too, and a data vault (`⌗`) may hold what you came for.

A node works its own device and no other. Crack a door's lock and that door opens, on the floor and on the Net, while every other door stays locked. On the Net a locked door stops you as it does on foot, and so does a line of turnstiles, until you crack or spoof its node (a line's is its switch). From a door's free side you pass, as you do on the floor. So the zones on the Net are the zones on the floor.

ICE keeps the levels, more of it the higher the building's security: white ICE beside `L`'s hub from Guarded, patrols up the levels from ICE, and black ICE beside the data vault at Black. Only the ICE on your level can see you or take turns with you.

## The firewall

From the city's Net, a hotel or office stands behind a **firewall**. Its locks stand in the building's own walls, spread round it, one of them the gate: `▮` while whole, `▯` once broken. How many it has goes by the building's security:

| Security | SEC | Locks |
|---|---|---|
| Open | under 100 | 1 |
| Guarded | 100 to 159 | 2 |
| ICE | 160 to 229 | 3 |
| Black | 230 and up | 4 |

Crack each lock standing beside it on the grid, as you would any node. You can do it on a run from anywhere, your rig included, but every break is heard: each one adds trace to the building, and a miss adds more. From inside the building's Net the same locks stand on `L`'s outer wall, and a crack there counts the same.

A broken lock stays broken for an hour, or until an alarm goes off: the alarm closes them all. On the grid, HERE's card for the building shows how it stands, a mark a lock: `FIREWALL ▯▮▮ · 1/3 broken`.

While a lock is whole, the gate turns you away: `FIREWALL · 3 locks · 1/3 broken`. With every lock broken, step onto the gate and you land on `L`'s hub, beside the server. From inside, while every lock is still broken, step onto the gate on `L` to go back out onto the grid. An alarm closes the locks behind you: crack them again from inside, and the gate lets you out. Jacking out always works, wherever you are: the firewall only stands between the building's Net and the city's.

The building's server, data vault and lift controls are behind the firewall, out of reach from the grid. The cameras and terminals on its face are still the street's.

## The riser and air gaps

The levels are joined by the **riser**, which follows the shaft of the lift that serves the most floors. Stand on a level's hub and press `<` to ride it down to the next level on it, or `>` to ride it up. You land on that level's hub. You can't ride it in a fight.

On a job in a building of ICE security or higher (`SEC 160` and up), the job's floor is cut off from the riser by an **air gap**, unless it is `L`. Nothing else on the building's Net reaches that level's nodes. From the riser's levels it is dimmed and marked `AIR GAP`, and on its own hub `<` and `>` say `AIR GAP · no riser here`. The `JOB` line says so too: `B1 has its own Net: jack in there`.

The only way onto it is on foot. Get to that floor (a spoofed lift that stops there will take you), find its terminal near where the lifts and stairs come in, and jack in there. That level opens, and you can work its doors and its vault from its own Net; the rest of the building is out of reach from it in turn.

## Reading the tower

On a hotel's or office's Net, the map shows the building in section: the **tower**.

- The level you're on is open at its full plan, and scrolls with you.
- Every other level is folded to 3 rows: its back rooms, the wall with its doors, and its front. A folded level still shows its nodes, its ICE and what you hold there.
- A slab lies between each pair of levels. Floors in between that you can't get off at are folded into it, with a label such as `8F–11F passed`.
- The street runs across under `L`, with the earth (`░`) below it.
- The riser runs through the hubs, cut `╳` where an air gap leaves a level off it.
- The firewall is a red double line round the levels on the riser, with `FIREWALL · 3 locks` on its top edge and its locks on `L`'s outer wall.
- A level you can't reach from where you stand is dimmed and says `out of reach`. One across an air gap says `AIR GAP`.
- Each level's name stands on its left. The open level's also says how many nodes it has, `11 ▪`, and how many of them you hold, `4 held`. When the map is too narrow for them, they show on one line in its top-left corner.

On the tower the marks are:

| Mark | What it is |
|---|---|
| `▯` | a door's lock |
| `◇` | a terminal |
| `◆` | the server |
| `⌗` | a data vault |
| `⬡` | a hub |
| `▪` | a lift, a gate switch, a camera, a turret or a container |
| `◈` | ICE |

What you hold or have cracked is green. With the cord on, it runs over the open level from the node you jacked in at to you.

On a building's Net the HERE pane is its panel:

- The building's name, its security, and its firewall: `FIREWALL ▮▮▮ 0/3`.
- `JOB`: on a job, what you're after and on which floor.
- `LEVELS`: a row a level, with its nodes, its ICE, and `◆` for the server, `⌗` for the vault and `@` for you; or why it's out of reach.
- `HELD`: each node you hold or have hacked, and what you did to it.
- `BODY`: where your body stands, and who is near it.

## The Net over the world

Jacked in, `W` lifts you over the Net of the whole country. Walk with the movement keys, `enter` drops you into the Net under your feet, and `W` takes you back. You can see other cities' nodes, but they're too far from your body to touch.
