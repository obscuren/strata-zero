# Talent tree — design

Date: 2026-10-15. Status: approved in chat.

## Goal

Five talent trees, one per stat, in the old-school style: columns of tiers, a
few talents per tier with ranks, the next tier opening as points pile up in
that tree. Every tree is open to every character. One talent point at
creation, one more per level. Talents do real things on the sheet today and
carry named flags for the systems still to come. The deck gets a TALENTS tab
that draws the trees and spends the points.

## Decisions taken with Jeffrey

- **Five trees, one per stat**: MEAT (Body), EDGE (Reflex), WRENCH (Tech),
  STREET (Cool), ICE (Net). Classic tier gating; the skills a character owns
  colour the flavour only.
- **Effects**: perks the sheet already understands (stat, sight, carry, max
  HP, focus), ability grants, ability sharpening (radius, cooldown), and named
  flags the combat and shop systems will read later.
- **Points**: one at creation, one per level-up.
- **Screen**: a TALENTS tab in the deck on `0`, drawn across the whole pane.

Assumptions carried in: no respec in this version (a ripperdoc service later);
talents never come from items; the eight flavour talents that templates hand
out today fold into the trees and the templates stop handing them out.

## Content

In `flavor/content.ron`:

```ron
trees: [
    Tree(stat: Body, name: "MEAT", line: "the body is the first machine",
         tiers: [["thick_skin", "long_haul"], ["second_lung", "iron_grip"], ...]),
    ...
],
talents: [
    Talent(id: "thick_skin", name: "Thick skin", icon: '▲', ranks: 3, skill: Some("athletics"),
           effects: [[Perk(HpMax(3))], [Perk(HpMax(3))], [Perk(HpMax(4))]],
           desc: "Bruises are a kind of armour."),
    Talent(id: "second_lung", name: "Second lung", icon: '◌', ranks: 2, requires: Some("thick_skin"),
           effects: [[Perk(Focus(2))], [Perk(Focus(3))]], desc: "..."),
    ...
],
creation: Creation(..., talent_points: 1, tier_gate: 3),
```

- `Tree { stat, name, line, tiers: Vec<Vec<TalentId>> }`: five tiers, two
  talents per tier, one on the fifth (the capstone). A talent's column is its
  index in its tier row.
- `Talent { id, name, icon: char, ranks: u8 (1..=3), requires: Option<TalentId>,
  effects: Vec<Vec<Effect>> (one list per rank: what that rank adds),
  skill: Option<SkillId> (flavour), desc }`.
- `Effect::{ Perk(Perk), Grant(AbilityId), Sharpen { ability, radius: i32,
  cooldown: i64 }, Flag(String) }`. `Perk` gains `Focus(i32)` (max focus).
- `Creation` gains `talent_points` (1) and `tier_gate` (3): tier `t` (from 0)
  opens when the tree holds at least `tier_gate * t` points.
- Validation: every talent in exactly one tree tier; `effects.len() == ranks`;
  ranks 1..=3; `requires` names a talent in the same tree, a lower tier and the
  same column (the connector draws straight down); abilities and skills named
  exist; no tier empty; the capstone has one rank.
- About sixty talents, with the existing eight folded in: quick draw, double
  tap, backstab in EDGE; ghost in STREET; ice pick, deep dive in ICE; solder
  hand, field medic in WRENCH. `Template.talents` goes.

## Rules (`game`)

- `Character.talents: BTreeMap<TalentId, u8>` (ranks) and
  `Character.talent_points: u32`. `from_build` starts with
  `creation.talent_points`.
- `talent_rank(id)`, `tree_points(c, stat)` (ranks summed over the tree),
  `can_learn(c, id) -> Result<(), String>` with the reasons in order: no point
  left; tier closed ("3 more points in EDGE"); prerequisite not at max rank
  ("needs Quick draw 3/3"); already at max; `learn(c, id) -> Result<u8, String>`
  spends the point and returns the new rank.
- `talent_effects(c)`: every effect of every owned rank. `perks(c)` is the
  equipment's perks followed by the talents'; `focus_max` reads `Perk::Focus`.
  `abilities(c)` adds `Source::Talent(id)` for grants, first source wins.
  `ability(c, id) -> Option<Ability>`: the content's ability with every
  `Sharpen` for it applied (radius added, cooldown added then clamped at 0).
  `flags(c) -> BTreeSet<String>`.
- `gain_xp` adds one talent point per level gained.
- `Action::Learn(TalentId)` runs `learn` and reports "Quick draw 2/3".
- The app fires and lists abilities through `Character::ability`, so a
  sharpened ping reaches further and cools sooner.

## The TALENTS tab

Tab ten, on `0`; `Tab::ALL` grows to ten. The tab draws across the whole deck
pane, no row list:

- Header: `TALENTS · 2 points` on the left, then `MEAT 3 · EDGE 0 · WRENCH 1
  · STREET 0 · ICE 2` with each name in its stat colour.
- Five columns, 23 cells each, one per tree, the tree's name and line at the
  top. Tiers run top to bottom. A tier holds two boxes of 11 cells, two rows
  each: `⚡ Quick draw` (icon, name cut to fit) over `▰▰▱ 2/3` (the rank
  meter). The gap row below a tier carries a `│` under a box that a talent in
  the next tier requires, and on a closed tier a dim caption `── 3 more ──`.
- Colour: affordable (a point left, tier open, prerequisite met, not maxed)
  cyan; owned ranks fill the meter in cyan; maxed magenta; out of reach dim.
  The cursor's box is inverted.
- Keys in this tab: `←` `→` (`h` `l`) move along a tier and on past its end
  into the next tree; `↑` `↓` (`j` `k`) move between tiers of the same tree,
  landing on the nearest column; `space` or `enter` learns a rank with the
  deck's flash on the box; `tab`, `backtab` and the digits still change tabs.
- Dossier strip at the bottom, four rows: `QUICK DRAW · EDGE · tier 2 · rank
  2/3`; `next: +1 Reflex · ping reaches 4 further`; `now: +1 Reflex` (or
  `nothing yet`); the description. A talent out of reach says why on the
  second row (`locked · 3 more points in EDGE`, `needs Quick draw 3/3`,
  `no points left`). Flags read as `later: quick_draw`.
- Effects read in words: `+1 Reflex`, `+3 max HP`, `+2 focus`, `sight +2`,
  `carry +4 kg`, `grants Ping`, `ping reaches 4 further`, `ping cools 60 ticks
  sooner`, `later: quick_draw`.

## Elsewhere

- Dev console: `talent <id>` learns under the rules; `tpts <n>` adds points.
- The creation SUMMARY step's unspent line adds `1 talent point`.
- A level-up's log line reads `level 3 · a talent point`.
- README: the deck's tenth tab and its keys.

## Out of scope

Respec, talents from items, the combat and shop effects behind the flags.

## Tests

game: content validation (wrong tree, wrong column, bad rank count, unknown
ability, empty tier); learning rules in order; perks, focus, abilities and
flags from ranks; a sharpened ability's radius and cooldown, cooldown clamped
at zero; points on level-up; the action. app: the tab draws five trees and the
header; cursor moves along, across and down; learning with space fills the
meter and spends the point; closed tiers dim with their caption; the dossier's
reasons; the console commands; the summary's talent point; the deck at its
minimum size. A smoke run and a screenshot of the tree.
