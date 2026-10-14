# Character creation

A new game starts with a character you built, not with Vex. NEW GAME opens a
chooser (two templates or a custom build), then a six-step wizard: name,
origin, profession, stats, skills, summary. The result is a full character
with stats bought under affinity, skill categories unlocked with SP, a kit from
the profession, and leftover points kept for later. The deck grows a STATS tab
and turns its SKILLS tab into the same folding category board the wizard uses.

Everything here is content-driven: origins, professions, categories, templates
and the numbers live in `flavor/content.ron` and are validated at load.

## Brief

What Jeffrey asked for, in order:

- Full build with two templates (pre-filled) or custom. Templates fill the
  sheet and can be adjusted.
- Name, origin, profession, stats, skills. Talents come in a later spec with a
  WoW-style tree and 1 point; the model leaves room, the screens do not show
  them.
- Origin sets affinity per stat, and affinity lowers what a point costs.
  Profession also shifts affinity and sets starting gear.
- A pool of stat points to spend. Stats are the five existing ones, still
  called stats.
- Skills are on or off and live in categories. A category costs SP to unlock,
  each skill under it costs SP, some first skills are free, some categories
  grant an ability (passive or active), WRENCH unlocks the workbench. Higher
  tiers of a skill are further skills in the same category, not ranks.
- A wizard, one step per screen. It has to be pretty, and everything must
  speak cyberpunk.
- The deck's categories fold and unfold and look as good as the wizard.

Assumptions carried in: one spec for model, content and screens; Vex becomes a
template; unspent points stay on the character and can be spent from the deck
under the same rules; nothing reads the new skills or the new passive yet.

## Voice

Every name, tagline and description in the content file reads like the Arc
Trace description: second person, short, street. No rulebook words on screen.
Labels are lower case except category and step names, which are set in caps
like the deck's tabs. The confirm is `JACK IN`.

## Flow

NEW GAME opens the chooser:

```
   VEX          crèche kid behind the ICE
   KESSLER      came down in a freight can, gets paid when it goes loud
 ▸ CUSTOM BUILD start from nothing
```

A template fills every step and jumps to SUMMARY; `esc` from there walks back
through the filled steps. CUSTOM starts at NAME with a blank build. `esc` on the
chooser returns to the title.

Six steps, one screen each, in this order because each depends on the ones
before:

1. **NAME** — a text field. Tagline: *what they will call you*.
2. **ORIGIN** — a list; the dossier on the right shows the line, the story and
   the affinity table. Tagline: *where you were made before the city got its
   hands on you*.
3. **PROFESSION** — same shape; the dossier adds the kit and credits. Tagline:
   *what you did before this*.
4. **STATS** — five rows with value, bar and the cost of the next point under
   the chosen origin and profession; the pool as a meter above. `→` buys, `←`
   refunds. Tagline: *what the meat can do*.
5. **SKILLS** — the category board. `→` unfolds, `←` folds, `space` buys or
   refunds the selected category or skill. The SP pool as a meter above.
   Tagline: *what you know*.
6. **SUMMARY** — the whole sheet; unspent points in magenta; `enter` is
   `JACK IN`. Tagline: *the sheet*.

Keys on every step: `↑↓` move, `enter` next, `esc` back. `esc` on NAME returns
to the chooser. `enter` refuses to leave a step that is not ready and shows the
reason in the footer for a few seconds: *no name on the sheet*, *pick where you
were made*, *pick what you did*, *over by N points*, *over by N SP*. Unspent
points pass.

The in-progress `Build` lives in `AppState`, not on a screen, so stepping back
and forth never loses a choice and a template's jump to SUMMARY is the same
screen in a different step.

JACK IN turns the build into the character, clears the build, and switches to
Generating with a fresh seed exactly as NEW GAME does today.

## Look

One frame for every step: a step strip, the caption in the logo's block font
with a tagline under it, a left list pane, a right dossier pane, a heavy rule
above and below, and the HUD's `[key]verb` footer with `step N of 6` at the
right. The title's dark backdrop runs behind it. Steps change with the glitch
transition; the chooser and wizard enter with the slide-up.

Step strip: the current step in the deck's `▌2 ORIGIN▐` handle in cyan, the
others dim, `▸` between them. The top right of the strip shows the build so
far: handle, then origin and profession as they are chosen, in dim text.

Below 36 rows the block-font caption becomes one bold line, as the deck does
with the handle. Everything else keeps its place down to 100×30.

ORIGIN at 120×40:

```
  1 NAME ▸ ▌2 ORIGIN▐ ▸ 3 PROFESSION ▸ 4 STATS ▸ 5 SKILLS ▸ 6 SUMMARY                              VEX · custom build
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  ▄▀▀▄ █▀▀▄ ▀█▀ ▄▀▀▄ ▀█▀ █▄  █
  █  █ █▄▄▀  █  █ ▄▄  █  █ ▀▄█          where you were made before the city got its hands on you
  ▀▄▄▀ █  █ ▄█▄ ▀▄▄▀ ▄█▄ █  ▀█

                                                      ┌─ DOSSIER ──────────────────────────────────────────────┐
  ▸ SLAB RAT          born between the slabs          │   ░▒▓█▓▒░                                              │
    CRÈCHE            badge revoked at sixteen        │   ▒█ ◉ █▒    SLAB RAT                                  │
    STOWAWAY          came down in a freight can      │   ░▒▓█▓▒░                                              │
    VAT-GROWN         decanted, not born              │                                                        │
                                                      │   You learned the alleys before you learned to read.   │
                                                      │   Nothing in the Slab surprises you, and you can run   │
                                                      │   on nothing for days.▮                                │
                                                      │                                                        │
                                                      │   AFFINITY                                             │
                                                      │   BODY     ▰▰▰▰▱   cheap                               │
                                                      │   REFLEX   ▰▰▰▱▱   normal                              │
                                                      │   TECH     ▰▰▱▱▱   dear                                │
                                                      │   COOL     ▰▰▰▰▱   cheap                               │
                                                      │   NET      ▰▰▰▱▱   normal                              │
                                                      └────────────────────────────────────────────────────────┘
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  [↑↓]choose  [enter]next  [esc]back                                                              step 2 of 6
```

The list slides in row by row like the title menu. The dossier types itself
out when the selection changes (the typewriter effect over 30 ticks). The
emblem is three rows of seven glyphs per origin and profession, in the content
file. Affinity bars: cheap `▰▰▰▰▱`, normal `▰▰▰▱▱`, dear `▰▰▱▱▱`.

STATS: the pool is a meter (`POINTS ▮▮▮▮▮▯▯▯▯▯ 5 left`), each stat has a
colour (BODY green, REFLEX cyan, TECH amber, COOL magenta, NET blue, the same
values the HUD bars use where they overlap), the selected bar pulses, a bought
segment flashes bright for six ticks like a fired ability slot, and the row
shows `next 2 pts` plus `cheap` or `dear` when the cost is not the base. The
dossier shows the stat's blurb, the skills it feeds, the origin's and the
profession's shift and the resulting cost. Over budget, the meter turns
magenta and reads `over by 2`.

SKILLS, the board, drawn the same way in the wizard and in the deck:

```
  SP  ▮▮▮▮▮▮▯▯  6 left

  ◆ NETRUNNER      owned          ┐
    ├─ ◆ Hacking        free      │  ⚡ Signal ghost
    ├─ ◇ ICE breaker    1 SP      │
    └─ ◇ Vault run      2 SP      ┘
  ◇ WRENCH         2 SP          ⚙ unlocks the workbench
  ◇ IRON           2 SP
  ◇ EDGE           2 SP
  ◇ GHOST          2 SP
  ◇ STREET         1 SP          first skill free
  ◇ RIPPER         2 SP
  ◇ MEAT           2 SP
```

Owned `◆` in cyan, affordable `◇` in text colour, unaffordable `◇` dim, a
locked skill (category not owned) dim with its cost. Folded categories show one
row; an unfolded one shows its skills with `├─` and `└─` connectors and a `⚡`
or `⚙` note for what it grants or unlocks. Buying plays a six-tick glitch over
the row. Every category starts folded except the ones you own.

SUMMARY: the handle in block font across the top, two columns under it, stats
bars and owned skills on the left, origin, profession, kit and credits on the
right, unspent points in magenta, and a pulsing `[enter] JACK IN` at the
bottom centre.

## Content

New ids: `OriginId`, `ProfessionId`, `CategoryId`, `TemplateId`. The
`starter` block goes away; `creation`, `origins`, `professions`, `categories`
and `templates` arrive.

```ron
creation: Creation(
    stat_base: 3, stat_cap: 8, stat_points: 20, point_cost: 2,
    sp: 6, name_max: 16, hp: 40, focus: 30,
    quests: [("the_drop", [true, false, false]), ("noodle_debt", [false]), ("first_jack", [true, true])],
),
```

Origins, each with two cheap stats, two dear and one normal:

| id | name | line | cheap | dear |
|---|---|---|---|---|
| slab_rat | SLAB RAT | born between the slabs, knew the alleys before the alphabet | Body, Cool | Tech, Net |
| creche | CRÈCHE | raised in a tower nursery, badge revoked at sixteen | Tech, Net | Body, Cool |
| stowaway | STOWAWAY | came down the gravity well in a freight can | Body, Reflex | Net, Tech |
| vat_grown | VAT-GROWN | decanted, not born; the lab lost the paperwork | Net, Tech | Reflex, Cool |

Professions, one cheap stat, one dear, a kit and credits. Kit items with a
slot are equipped when the slot is free, the rest are packed:

| id | name | line | cheap | dear | kit | ¤ |
|---|---|---|---|---|---|---|
| netrunner | NETRUNNER | you live behind the ICE | Net | Body | pawnshop_deck, salvage_jacket, dead_phone, copper_coil ×3, burner_shard | 320 |
| fixer | FIXER | you know who owes whom | Cool | Tech | ratchet_smg, salvage_jacket, mirrorshades, bottle_caps ×20, soy_noodle_cup ×2 | 600 |
| solo | SOLO | you get paid when it goes loud | Body | Net | ratchet_smg, ballistic_weave, shiv_blade, stim_patch ×2 | 250 |
| techie | TECHIE | you make broken things bite | Tech | Cool | salvage_jacket, cargo_pants, scrappers_roll, scrap_servo ×3, copper_coil ×4, burner_schematic | 300 |

Three new items: `ballistic_weave` (armor, torso, HpMax 8, 3.0 kg, ¤650, *"Layered weave that stops the first burst and remembers the second."*),
`scrappers_roll` (component, 1.5 kg, ¤140, *"Drivers, picks, a soldering iron that runs off a stolen cell."*),
`burner_schematic` (component, 0.05 kg, ¤90, *"One-shot plans etched on a shard that will not survive the read."*).

Categories. The eight existing skill ids are each category's free first skill
so recipes and talents keep their references. Second skills cost 1 SP, third
cost 2:

| id | name | cost | skills | line | extra |
|---|---|---|---|---|---|
| netrunner | NETRUNNER | 2 | hacking, ice_breaker, vault_run | the Net is a place and you live there | grants `signal_ghost` |
| wrench | WRENCH | 2 | electronics, scrapcraft, overclock | nothing stays broken around you | unlocks Workbench |
| iron | IRON | 2 | handguns, longarm, snapshot | you are the one holding it | |
| edge | EDGE | 2 | blades, close_work, riposte | inside arm's reach is your reach | |
| ghost | GHOST | 2 | stealth, lockpick, vanish | the cameras remember a smear | |
| street | STREET | 1 | streetwise, haggle, fixers_ear | everyone owes somebody | grants `street_sense` |
| ripper | RIPPER | 2 | medtech, field_patch, stim_cocktail | blood goes back in | |
| meat | MEAT | 2 | athletics, freerun, second_wind | the body is the first machine | |

Sixteen new skills with one-line descriptions in voice, each with the stat of
its category's first skill. New passive `signal_ghost`: *"You feel the Net's
weather change before the ICE does."* `street_sense` leaves the innate list
and is granted by STREET.

Templates are complete builds. `kit` is extra on top of the profession's:

```ron
Template(id: "vex", handle: "VEX", line: "crèche kid behind the ICE",
    origin: "creche", profession: "netrunner",
    stats: Stats(body: 4, reflex: 6, tech: 5, cool: 5, net: 6),
    categories: ["netrunner", "wrench", "street"], skills: ["hacking", "ice_breaker", "electronics", "streetwise"],
    talents: ["ice_pick", "solder_hand"],
    kit: [("ratchet_smg", 1), ("nightglass_optics", 1), ("scrap_servo", 2), ("soy_noodle_cup", 2), ("stim_patch", 1), ("bottle_caps", 12)]),
Template(id: "kessler", handle: "KESSLER", line: "came down in a freight can, gets paid when it goes loud",
    origin: "stowaway", profession: "solo",
    stats: Stats(body: 8, reflex: 7, tech: 4, cool: 7, net: 3),
    categories: ["iron", "edge", "meat"], skills: ["handguns", "blades", "athletics"],
    talents: ["quick_draw", "backstab"], kit: [("cargo_pants", 1)]),
```

Vex costs exactly 20 points (crèche + netrunner: Body 3, Reflex 2, Tech 1,
Cool 3, Net 1 per point) and 6 SP. Kessler costs exactly 20 (stowaway + solo:
Body 1, Reflex 1, Tech 3, Cool 2, Net 3) and 6 SP. Vex's pack and gear equal
today's starter. Templates carry talents as flavour until the talent spec;
custom builds start with none.

Talents and recipes lose `level` and require holding the skill. `Perk::Skill`
goes away: `shiv_blade` gets `Stat(Reflex, 1)`, `pawnshop_deck` gets
`Stat(Net, 1)`.

Validation adds: every skill belongs to exactly one category; category skills,
grants, profession kit items, template ids and quest flags resolve; a
category's first skill costs 0; both templates pass `Build::validate`.

## Rules

- A stat starts at `stat_base` (3) and may not pass `stat_cap` (8) at
  creation.
- A point in a stat costs `point_cost` (2) plus the origin's shift plus the
  profession's shift, where a stat listed as cheap shifts −1 and as dear +1,
  clamped to 1..=3. Cost 1 reads *cheap*, 2 *normal*, 3 *dear*.
- Points spent is the sum over stats of (value − base) × current cost. It is
  recomputed, never stored, so changing origin or profession after buying can
  push a build over budget; the STATS step and SUMMARY say *over by N* and
  SUMMARY refuses.
- A category costs its listed SP. A skill costs its listed SP and needs its
  category owned. Refunding a category refunds its skills too. SP spent is
  recomputed the same way.
- Name: 1 to `name_max` characters after trimming, from letters, digits,
  space, `-`, `_`, `.` and `'`.
- A build is valid when it has a name, an origin, a profession, every stat in
  base..=cap, points and SP not overspent, and every owned skill's category
  owned. Unspent points go to the character's `stat_points` and `sp`.

## Model

### game crate

`model.rs` adds `OriginId`, `ProfessionId`, `CategoryId`, `TemplateId` and:

```rust
pub struct Origin { pub id: OriginId, pub name: String, pub line: String, pub desc: String,
    pub cheap: Vec<Stat>, pub dear: Vec<Stat>, pub emblem: Vec<String> }
pub struct Profession { pub id: ProfessionId, pub name: String, pub line: String, pub desc: String,
    pub cheap: Vec<Stat>, pub dear: Vec<Stat>, pub kit: Vec<(ItemId, u32)>, pub credits: u32, pub emblem: Vec<String> }
pub struct CategorySkill { pub id: SkillId, pub cost: u32 }
pub enum Unlock { Workbench }
pub struct Category { pub id: CategoryId, pub name: String, pub line: String, pub cost: u32,
    pub skills: Vec<CategorySkill>, #[serde(default)] pub grants: Vec<AbilityId>, #[serde(default)] pub unlocks: Vec<Unlock> }
pub struct Template { pub id: TemplateId, pub handle: String, pub line: String, pub origin: OriginId,
    pub profession: ProfessionId, pub stats: Stats, pub categories: Vec<CategoryId>, pub skills: Vec<SkillId>,
    #[serde(default)] pub talents: Vec<TalentId>, #[serde(default)] pub kit: Vec<(ItemId, u32)> }
pub struct Creation { pub stat_base: u8, pub stat_cap: u8, pub stat_points: u32, pub point_cost: u32,
    pub sp: u32, pub name_max: usize, pub hp: u32, pub focus: u32, pub quests: Vec<(QuestId, Vec<bool>)> }
```

`Stat` gains `ALL`, `name()` (`BODY`), and `blurb()` lives in content as
`stat_blurbs: Vec<(Stat, String)>`. `Talent` and `Recipe` drop `level`.
`Perk::Skill` is removed. `Starter` is removed.

`Content` replaces `starter` with `creation`, `origins`, `professions`,
`categories`, `templates`, `stat_blurbs`, with lookups `origin`, `profession`,
`category`, `template`, `category_of(skill)`, `skill_cost(skill)`.

`Character` changes: `skills: BTreeSet<SkillId>`, `categories:
BTreeSet<CategoryId>`, `origin: OriginId`, `profession: ProfessionId`,
`stat_points: u32`, `sp: u32`. `skill_base` and `skill_level` go;
`has_skill`, `has_category`, `has_workbench(c)` arrive. `abilities()` lists
innate, then category grants as `Source::Category(id)`, then item grants.
`can_craft` checks `has_skill`. `from_starter` becomes
`from_build(c, &Build)` and `from_template(c, &TemplateId) -> Option<Character>`.
Kit handling: for each kit entry in profession then template order, equip when
the item has a slot that is free, else pack.

### `game::build`

```rust
pub struct Build { pub handle: String, pub origin: Option<OriginId>, pub profession: Option<ProfessionId>,
    pub stats: Stats, pub categories: BTreeSet<CategoryId>, pub skills: BTreeSet<SkillId>,
    pub talents: Vec<TalentId>, pub kit: Vec<(ItemId, u32)>, pub template: Option<TemplateId> }
pub enum Step { Name, Origin, Profession, Stats, Skills, Summary }
pub enum Affinity { Cheap, Normal, Dear }
pub enum BuildError { NoName, NameTooLong(usize), BadName(char), NoOrigin, NoProfession,
    AtCap(Stat), AtBase(Stat), OverPoints(u32), NoPoints, OverSp(u32), NoSp,
    CategoryLocked(SkillId), AlreadyOwned, NotOwned, Unknown(String) }
impl Build {
    pub fn blank(c: &Content) -> Build;
    pub fn from_template(c: &Content, t: &Template) -> Build;
    pub fn stat_cost(&self, c: &Content, s: Stat) -> u32;
    pub fn affinity(&self, c: &Content, s: Stat) -> Affinity;
    pub fn points_spent(&self, c: &Content) -> u32;
    pub fn points_left(&self, c: &Content) -> i64;
    pub fn buy_stat(&mut self, c: &Content, s: Stat) -> Result<(), BuildError>;
    pub fn refund_stat(&mut self, c: &Content, s: Stat) -> Result<(), BuildError>;
    pub fn sp_spent(&self, c: &Content) -> u32;
    pub fn sp_left(&self, c: &Content) -> i64;
    pub fn skill_cost(&self, c: &Content, id: &SkillId) -> Option<u32>;
    pub fn toggle_category(&mut self, c: &Content, id: &CategoryId) -> Result<bool, BuildError>;
    pub fn toggle_skill(&mut self, c: &Content, id: &SkillId) -> Result<bool, BuildError>;
    pub fn ready(&self, c: &Content, step: Step) -> Result<(), BuildError>;
    pub fn validate(&self, c: &Content) -> Result<(), BuildError>;
}
```

`BuildError: Display` in voice (*no name on the sheet*, *pick where you were
made*, *pick what you did*, *over by 3 points*, *over by 1 SP*, *BODY is
maxed*, *nothing to refund*, *unlock NETRUNNER first*). `toggle_*` buys when
not owned and refunds when owned, returning the new state; buying needs the
pool to cover it, refunding a category drops its skills. `ready(Name)` checks
the name, `ready(Origin)`/`ready(Profession)` the picks, `ready(Stats)` and
`ready(Skills)` only that nothing is overspent, `ready(Summary)` is `validate`.

### app crate

- `AppState.build: Option<game::Build>`. `AppState::new` builds the default
  character from the `vex` template.
- `screens/create/mod.rs`: `Create { step: Step, opened: u64, cursor: [usize; 6],
  name: Form, open: BTreeSet<CategoryId>, flash: Option<(usize, u64)>,
  notice: Option<(String, u64)> }` and the shared frame: `strip`, caption,
  tagline, panes, footer. One file per step for its keys, list and dossier:
  `name.rs`, `origin.rs`, `profession.rs`, `stats.rs`, `skills.rs`,
  `summary.rs`. `chooser.rs` holds the template menu.
- `ui/board.rs`: `BoardRow { text: Line, kind: RowKind::Category(CategoryId) | RowKind::Skill(SkillId), owned, affordable }`
  and `board_rows(c, owned_categories, owned_skills, sp_left, open) -> Vec<BoardRow>`;
  `glyph` rules and connectors live here so the wizard and the deck draw the
  same rows. `ui/meter.rs`: `meter(label, used, total, colour)` for the two
  pools.
- Title: NEW GAME pushes the chooser with the slide-up. The chooser's `enter`
  sets `st.build` and switches to `Create` at SUMMARY for a template or NAME
  for custom; `esc` pops.
- `Create::update`: JACK IN runs `validate`, then
  `st.character = Character::from_build(..)`, `st.build = None`, and switches
  to `Generating::new(Title::fresh_params(..))`.
- Deck tabs: SKILLS (the board, `→` `←` fold, `space` buys from `sp`), STATS
  (five rows, pools, `space` buys from `stat_points`), GEAR, INVENTORY,
  WORKBENCH (locked rows *no bench. WRENCH unlocks it.* when
  `!has_workbench`), CYBERDECK, QUESTS, JOURNAL, MAP. `Detail::Skill(usize)`
  becomes `Detail::Category(CategoryId)`, `Detail::Skill(SkillId)` and
  `Detail::Stat(Stat)`. The abilities list that SKILLS showed moves into each
  category's rows (`⚡ name` lines) and stays on CYBERDECK.
- Console: `sp <n>` and `pts <n>` add to the pools.
- The dev camera's `g` path and Configure are untouched; they run on the
  default character.
- Help and README: the chooser and wizard keys, the deck's new tabs.

Saves hold only the city, so nothing migrates.

## Testing

- Content: validation catches a skill in two categories, a skill in none, a
  dangling category skill, a first skill that costs SP, a template over
  budget, a kit item that does not exist; both templates load and cost
  exactly 20 points and 6 SP.
- Rules: cost shifts and clamp for every origin × profession pair stay within
  1..=3; overspend after an origin change; refunding a category drops its
  skills; every `BuildError` reason reachable; `from_build` on the Vex
  template gives today's stats, gear and pack.
- Character: `has_workbench` true only with WRENCH; `abilities()` lists a
  category grant with `Source::Category`; `can_craft` with and without the
  skill.
- Screens through the test backend: strip and caption at 40 rows, plain
  caption at 30; refusal notices; template lands on SUMMARY; `esc` on NAME
  returns to the chooser; `esc` on the chooser returns to the title; JACK IN
  produces the character and a Generating screen; typing a name; buying and
  refunding a stat; folding and buying on the board.
- Board: rows for folded and unfolded categories, glyph states, connectors.
- Deck: STATS and SKILLS tabs render, workbench locked rows, spending from
  both tabs.
- Smoke: `drive10.py` walks a custom build end to end and a template jump.

## Out of scope

The talent tree and talent points; what a level-up grants; mechanics for the
sixteen new skills and for Signal ghost; saving the character; mouse input.

## Revisions

2026-10-14, after Jeffrey played the first build:

- Steps run ORIGIN, PROFESSION, STATS, SKILLS, NAME, SUMMARY. The name comes
  last so the sheet beside it has something on it: NAME's right pane is THE
  SHEET so far, with the name on top as you type it.
- SUMMARY has no caption of its own: the character's name is set in the
  block font where the step name was, with *the sheet* as the tagline. The
  sheet lists what each owned category grants (`◈ Signal ghost`).
- The block font knows the name marks `' - _ .` and folds accented Latin
  letters onto their base glyph; it falls back to a plain line only for
  scripts it has no glyphs for. Wizard captions do not flicker.
- ORIGIN and PROFESSION rows take two lines: the name with its cheap and dear
  stats at a glance, then the whole tagline. The list pane is 55 percent.
- On the board a price the pool cannot cover reads `no SP`; a skill behind an
  unowned category keeps its price.
- In the deck, `space` on a stat buys a point outright; the refusal lands in
  the status strip.
- The step glitch stays.

