# The ICE tree's last promises and the abilities on the Net — design

Date: 2026-10-17. Status: approved in chat.

## Goal

Every talent in the ICE tree does what its line says, and the active and
passive abilities do something once you are jacked in.

## Decisions taken with Jeffrey

Three talents promised things nothing read: Deep dive, Null route, The deep
end. Ping and Arc trace did nothing inside a construct; Signal ghost and
Low-light did nothing anywhere. Street sense stays as it is (the street has no
threats to sense yet).

## Talents (`net.rs`, `game`)

- **Deep dive** (`deep_dive` flag, kept with its +2 copy power): a successful
  turn of `copy` against a data vault makes no heat; a slip still adds
  `noise + 2`.
- **Null route** (`null_route` flag): once per construct per session, the turn
  that would dump the player at `DUMP_HEAT` routes away instead: the heat falls
  to `ALARM_HEAT`, the construct is remembered in `NetState.null_routed`, the
  log warns `null route · the trace loses you`, and the alarm still trips if it
  was not already alert. The next dump in that construct is real. All three
  places that add heat (a program turn, a white hit, a held mask) go through
  `NetState::add_heat(b, noise, now, null_route: bool) -> TraceEvent`.
- **The deep end** (`deep_end` flag): black ICE damage through the jack is
  halved after the deck's rating comes off (never below one); the failsafe
  pulls at an eighth of HP instead of a quarter; its hard dump cools
  `DEEP_END_COOLING = 10` minutes instead of `FAILSAFE_COOLING = 30`.

## Abilities while jacked in (`actions.rs`, `net.rs`, `render/fx.rs`)

- **Ping** inside a construct: its glow lights every node of the construct for
  the ability's glow ticks (`FxInstance.targets` carries them; the glow draws
  its targets when it has any, the strata's connectors otherwise), it adds
  `PING_NOISE = 2` heat through `add_heat`, and the log says
  `Ping: N nodes answer`.
- **Arc trace** inside a construct: the bolts run to every standing piece of
  ICE within the ability's radius; each is stunned for `STUN_TURNS = 2` turns
  (`Ice.stunned`): a stunned piece neither sees, moves nor hits, and the count
  falls by one at the end of each of its turns; it adds `TRACE_NOISE = 5` heat;
  the log says `Arc Trace: N pieces stunned` (`nothing to arc to` when none).
  The HERE line of a stunned piece reads `· stunned 2`.
- **Signal ghost** (`Character::has_ability`): inside a construct every awake
  piece is listed in HERE and drawn on the map whether or not it is in sight.
- **Low-light**: two more tiles of sight from 18:00 to 05:59. `AppState::
  sight_bonus()` is the character's bonus plus that, and every sight refresh
  uses it.

## Out of scope

Street sense, new talents, new abilities, the Net's look.

## Tests

A vault copy with the flag adds no heat on a success and `noise + 2` on a
slip; without the flag the usual. Null route: at heat 98 a crack turn leaves
heat 60, the warning, the alarm, no dump; the second time dumps; a white hit and
a held mask route the same way. Deep end: the black hit halves, the failsafe
line and cooling change. Ping in a construct: targets are every node, heat +2,
the log line; outside a construct unchanged. Arc trace in a construct: pieces
in range stunned for two turns, no hit or move while stunned, heat +5; the HERE
line. Signal ghost: an awake piece out of sight is listed and drawn; an asleep
one is not; without the ability neither. Low-light: the bonus at 22:00 and at
12:00. A smoke run through ping, arc trace and a null-routed dump.
