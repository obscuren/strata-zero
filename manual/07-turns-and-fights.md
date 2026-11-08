# Turns and fights

Most of the time the city runs in real time: the clock ticks, people walk, and you move when you like. When something sees you and means you harm, the map goes to **turns**. The log says `ENGAGED`, and a column at the top right of the map lists everyone in the fight in the order they act.

Who goes first depends on where you are: on the street your REFLEX, on the Net your NET. Ties go to you.

`e` goes to turns on your own, before anything has seen you, and `e` again goes back to real time once nothing hunts you. When the fight is over the log says `calm`.

## Your turn

Your turn has two halves.

1. **Move.** You have four moves, more with the right chrome. Step with the movement keys, or press `g`, aim with the arrows and press `enter` to walk the path in one go. `space` stays put and goes straight to acting.
2. **Act.** You have two action points (AP). Firing your gun, a program, most abilities, throwing something or using an item from your pack each costs one or two.

`.` ends your turn early. It also ends on its own a moment after you spend your last AP. The bottom line shows the moves and AP you have left.

While it isn't your turn, other keys wait; the game says `not your turn`. You can still look around, read the log or pause.

## Abilities

Abilities come from your skills, your gear, your chrome and your talents. The first five sit on `1` to `5`, and `a` lists them all with whether each is ready or cooling down. Cooldowns run in real time.

On the act half of a turn, `1` to `9` and `0` pick from everything you can do this turn: strikes, boosts and marks first, then anything you can throw. A strike needs a target in range; a boost or a mark you use on yourself or on whoever the cursor is on. Deadeye locks on: brackets close in round the foe, `MARKED` and what it adds to your chance to hit read out beside them, and it keeps a `⌖` over it for the rest of the fight.

## Your gun

Only the weapon in your PRIMARY slot fires. On the act half the cursor comes up on your target: `t` jumps to the next one, and `space` fires the whole burst for one AP. The footer shows your target, the range and your chance to hit.

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
