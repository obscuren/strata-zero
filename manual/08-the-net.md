# The Net

Over the city runs the Net: the systems behind every lock, lift, camera, lamp and screen. A runner who can get into them can open doors, blind cameras and put out the street lights. Out on the street a netrunner is easy prey; in the Net the city does what you tell it.

## Jacking in

You jack in from a terminal. On the street, stand on or next to a public terminal (`▣`) and press `space`, then `j Jack in`. At home, your rig does the same and is never locked down. Out on open ground, any terminal or vending machine with a signal will do.

You can jack in with nothing, but you can't run a program without a deck in your DECK slot. The log tells you when a port won't take you: it's locked down because the building next to it is on alert, the port is dead, your deck is still cooling off, or you're in a fight.

Your body stays where you jacked in. Cameras pay it no attention while you're away.

## Moving in the Net

The open Net lies over the city as a stratum of its own, the **grid**, with every tile in plain sight. Nodes sit where the systems are; lanes run between them.

| Mark | What it is |
|---|---|
| `◇` | a terminal |
| `◆` | a server |
| `⬡` | a hub |
| `⌗` | a data vault |
| `▪` | a lock, a camera, a lamp grid and the like |
| `◈` | ICE |

Every building has a **construct**: its own little Net behind a gate. Step onto the gate to go in. A building on alert keeps its gate closed. Security runs from Open, through Guarded and ICE, to Black; the higher it is, the harder everything inside is to crack, and the nastier the ICE.

With the **cord** on (SETTINGS), a line on the grid leads back the way you came.

## Programs

Load programs into your deck from its blueprint, before you jack in (The deck). A deck has only so many slots, and each program takes one or two.

| Program | Works on |
|---|---|
| Crack | any node, or ICE |
| Spoof | locks, lifts, cameras, ad boards, lamp grids, terminals |
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
- A blinded camera sees nothing.
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

## Trace

Everything you do in the Net makes **noise**, and noise raises the **trace** of the building or district you're working in. The footer shows it.

- At 60 the alarm goes off for an hour: the building locks down, its hacks end, its ICE wakes, its gate closes and the ports beside it lock.
- At 100 you're **dumped**: thrown back into your body.

Trace falls by itself, a little each minute, once you've left. A camera that sees you on the street raises its building's trace too: enough to set off the alarm, never enough to dump you.

## ICE

ICE guards constructs and walks the grid. It comes in three grades, and when it sees you the fight goes to turns.

- **White** ICE raises your trace when it hits.
- **Grey** ICE drains your FOCUS. At zero, you're dumped.
- **Black** ICE hurts your body.

Your deck's rating and your feedback resistance soften grey and black hits, and a shield soaks a whole hit. Attack breaks ICE; break the last one standing and the alarm ends.

Out on the grid, **sentinels** patrol near their hubs, but only bother you while their district is after you. When an alarm goes up from a run on the grid, a **tracer** comes after you. It follows your trail and dumps you if it catches you. Lose it at a hub, mask your trail, or jack out.

## Getting out

`J` jacks out, after asking. Jacking out stops any program you were running.

Being dumped is worse: your FOCUS is gone and your deck has to cool for ten minutes before you can jack in again.

> Your deck has a **failsafe**, on unless you turn it off. When black ICE brings you low, it dumps you rather than let you die, and the deck cools for half an hour. Turn it off on the deck's blueprint and black ICE can kill you.

## Backdoors

A backdoor planted on a server lasts three days. From your rig, `j` lists your backdoors as well as your own block: pick one and you land at that building's gate. From a server you hold, you can **hop** to another backdoor's gate. Your rig keeps up to three.

If you're dumped on a run that started at your rig, a trace follows your hops home, burning each backdoor it passes. If it reaches your door, your rig is seized for a day and every backdoor burns.

## The Net over the world

Jacked in, `W` lifts you over the Net of the whole country. Walk with the movement keys, `enter` drops you into the Net under your feet, and `W` takes you back. You can see other cities' nodes, but they're too far from your body to touch.
