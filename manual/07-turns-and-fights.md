# Turns and fights

Most of the time the city runs in real time: the clock ticks, people walk, and you move when you like. When something sees you and means you harm, the map goes to **turns**. The log says `ENGAGED`, and a column at the top right of the map lists everyone in the fight in the order they act.

Who goes first depends on where you are: on foot your REFLEX, on the Net your NET. Ties go to you.

`e` goes to turns on your own, before anything has seen you, and `e` again goes back to real time once nothing hunts you. When the fight is over the log says `calm`.

## Fights indoors

A fight on a hotel's or office's floor works as it does on the street, a floor at a time: you step, shoot, push and throw the same. If something kills you there, what killed you names the building, not the street.

A guard who sees you says so, `guard spots you`, and on its next turn it radios it in: `guard radios it in · ALARM`, and the building's alarm goes off (The Net). Drop it or stun it before that turn comes and the call is never made. Each guard calls once, and none calls while the alarm is already on.

Guards hear what they can't see. A gunshot, yours or a guard's, or anything thrown going off wakes every guard within twelve tiles of it on that floor, through walls, and they come looking. A guard that only heard you has nothing to radio yet. A strike makes no sound.

On a floor, every guard hunting you takes a turn, seen or not. One you haven't seen stays out of the column, and its turn reads `FOOTSTEPS`. A fight started only by things you haven't seen, on a floor or on the street, opens with `ENGAGED · footsteps`.

Taking the stairs or a lift ends the fight, but the guards hunting you on that floor follow you up or down where their card lets them. You never land on a guard: you step out beside it, and when every tile at the landing is taken the log says `the landing is blocked`. No guard follows you out of the building; once you're out, they lose you.

## Your turn

Your turn has two halves.

1. **Move.** You have four moves, more with the right chrome. Step with the movement keys, or press `g`, aim with the arrows and press `enter` to walk the path in one go. `space` stays put and goes straight to acting.
2. **Act.** You have two action points (AP). Firing your gun, a program, most abilities, throwing something or using an item from your pack each costs one or two.

`.` ends your turn early. It also ends on its own a moment after you spend your last AP. The bottom line shows the moves and AP you have left.

While it isn't your turn, other keys wait; the game says `not your turn`. You can still look around, read the log or pause.

## Abilities

Abilities come from your skills, your gear, your chrome and your talents. The first five sit on `1` to `5`, and `a` lists them all with whether each is ready or cooling down. Cooldowns run in real time.

On the act half of a turn, `1` to `9` and `0` pick from everything you can do this turn: strikes, boosts, marks and Vanish first, then anything you can throw. A strike needs a target in range; a boost or a mark you use on yourself or on whoever the cursor is on. Deadeye locks on: brackets close in round the foe, `MARKED` and what it adds to your chance to hit read out beside them, and it keeps a `⌖` over it for the rest of the fight.

## The cloak

On foot and out of a fight, you can **cloak**. The Cloak ability comes with the Stealth skill (GHOST), and a field scrambler in your pack gives you Scramble on a cell of its own (The city and the world). Press its key on the ability bar and the log says `CLOAKED`, or `SCRAMBLED` for the scrambler; press it again and it switches off. Its slot reads `on` while it runs, and the map drifts in pink and cyan round you. Only one runs at a time: either key switches off the one that's on.

It won't go on in a fight (`not in a fight`), while you're jacked in (`not while jacked in`), on the Net (`not on the Net`), or with nothing left to run on (`no FOCUS left`, `the cell is burned out`).

While it holds, nobody sees you: no guard, foe, camera or walker. A guard doesn't spot you and nobody radios it in, a foe on the street doesn't start a fight, and a guard already hunting you walks to where it last saw you and gives up. Guards don't follow you up or down the stairs or a lift either. You are still there, though: a guard still stands in your way when you walk into it, and a gunshot is still heard.

The Cloak drains 1 FOCUS a second, rounded up when it ends, and Scramble drains the scrambler's cell; the Blind spot talent halves either while you stand still. Moving, doors, stairs and lifts, and walking into a building leave it on, and time on the deck or in a menu costs it nothing. It ends:

- with your first hit (The ambush, below);
- when it runs dry. It **burns out**: the map tears, the log says `the cloak burns out · you're in the open` (`the scrambler burns out · you're in the open` on a cell), and you're in the open from that moment, not when the picture settles;
- when a fight starts any other way, when you jack in, when you sleep or wait, and when you leave the city.

While it holds, the first bottom line reads `▒▒ CLOAKED ▒▒` or `▒▒ SCRAMBLED ▒▒` where `ENGAGED` would be, and through a burnout it flashes `▓▓ SIGNAL LOST ▓▓`.

## The ambush

`e` from the cloak goes to turns and keeps you cloaked until your first turn ends: foes that act before you don't see you. Your first shot or strike is an **ambush**. The log says `AMBUSH`, every round of it lands, and each does double damage. The cloak ends with it, and the fight goes on from there: the one you hit turns on where you stood, and a shot is heard as any shot is. A guard you drop with it never radios. A throw ends the cloak with no bonus, and so does ending that turn without an attack.

While you're cloaked with a foe in reach of your gun or one of your strikes, the second bottom line reads `AMBUSH · your first hit lands, ×2`.

## Vanish

The Shadow step talent (STREET) gives you **Vanish**, a way out of a fight. It's on the act half's list and costs no AP. The fight ends, every foe on your map and every guard of the building you're in, on any floor, loses you, and you're cloaked for 4 seconds for free to get away under. The log says `vanish · they lose you`. A guard that already radioed has radioed: an alarm stays on.

Vanish cools for 120 seconds, 90 with Shadow step at its best. Out of a fight it's refused (`only in a fight`); there the cloak is the tool.

## Your gun

The gun in your right hand fires; if that hand holds none, the gun in your other hand does. A gun in the other hand fires too, about one shot in four, free: it follows your shot at the same target if it can reach. If your first shot downs the target, it stays quiet. A shot from the cloak is an ambush for the first gun alone. On the act half the cursor comes up on your target: `t` jumps to the next one, and `space` fires the whole burst for one AP. The footer shows your target, the range and your chance to hit.

Your chance starts at 50%, gains 4% for every point of REFLEX and 10% if you have the weapon's skill, and drops the further past half its range the target stands. A target too far off, or with something in the way, can't be shot.

## Throwing

Grenades and the like go on the act half: pick one with its number, aim with the movement keys, `t` jumps to the next foe, and `space` throws for one AP. `esc` puts it back. You need to see where it lands. The blast hurts anyone in it, you included, less the further from the middle they stand.

| Throwable | What it does |
|---|---|
| Frag | damage, two tiles round |
| Flashbang | stuns, no damage |
| EMP puck | hurts chrome only, and stuns |
| Choke can | toxic gas |
| Satchel charge | a bigger blast, as your next turn begins |

## Getting hurt

- **Armour** gives each hit a chance to turn aside: 10% for every point, up to 60%.
- **Resistances** cut each kind of harm by their percentage: kinetic, thermal, shock, EMP, toxin and feedback. Every piece of chrome in you lowers your EMP resistance.
- **Stun** costs whoever is stunned their next action points.

When your HP reaches zero, you flatline. Saving and dying covers what happens next.

## Healing

- A stim patch or something to eat, used from your pack (`u` on the deck's GEAR & INVENTORY tab). In a fight that costs an AP.
- Sleep in your own bed. Six hours heals you completely (Time and rest).

You don't heal by waiting.
