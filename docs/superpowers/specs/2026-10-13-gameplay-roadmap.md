# Gameplay interface roadmap

Three specs, built in this order. Each one is its own design, plan and branch,
and each leaves the game playable on its own.

## 1. Play HUD

What the player looks at every second of play. A persistent panel beside the
map with the character's vitals, abilities and cooldowns, the active quest
objective, a scrolling message log, and what lies on the current tile. The
dev inspector becomes a dev-only view. Shares the deck's chrome.

Spec: `2026-10-13-play-hud-design.md` (next).

## 2. Flow and framing

How a session starts, saves and ends. Title goes straight into a new game
rather than a city generator; the generator becomes a dev tool. Saves carry
the character and the ground, not only the city. Pause and settings are
tidied, and one key scheme holds across every screen.

Spec: to follow after the HUD ships.

## 3. The core loop

Time. Actions take turns, the city has a clock, cooldowns and effects count
in turns rather than ticks, and the HUD shows it. This is the largest of the
three and reshapes parts of the other two, which is why it comes last: it
builds on a stable HUD and a stable session flow.

Spec: to follow after flow and framing ships.

## Out of scope for all three

Combat numbers, loot generation, NPCs and dialogue. Those are their own
roadmap once the interface and the loop exist to carry them.
