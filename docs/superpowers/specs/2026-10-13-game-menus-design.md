# Game menus: pause menu, the deck, settings, and the jack-in flash

**Date:** 2026-10-13
**Status:** approved in conversation (scope "shell + model + sample content")
**Builds on:** `2026-10-12-city-generator-design.md`, `2026-10-12-cyberspace-design.md`.

## 1. Intent

The game grows its own interface on top of the map: Esc opens a pause menu
instead of dropping to the title, Tab opens the player's deck with the tabs an
RPG needs (quests, gear, inventory, workbench, map, skills), and entering or
leaving cyberspace glitches like a screen change. Behind the deck sits a real
data model and a starter kit of content, so every tab shows something true.

No rules yet: items do not change stats, recipes do not craft, quests do not
progress, nothing is bought or sold, and the character is not saved to disk.
Those are their own specs.

### In scope

- A screen stack in the app runner: menus push over the live map and pop back;
  the map stays visible underneath, dimmed with a scanline wash, and does not
  update while a menu is up. A "flash" command runs a transition in place.
- Pause menu on Esc / `q`: Resume, Save, Load, Settings, Quit to title, Quit game.
- Settings screen: dev mode, rain, FPS counter. Kept in memory.
- The deck on Tab: QUESTS, GEAR, INVENTORY, WORKBENCH, MAP, SKILLS tabs with a
  list pane and a detail pane. The Map tab is the player's memory of the
  current stratum.
- A terminal-free `game` crate: content (items, recipes, skills, talents,
  quests, starter kit) loaded from `flavor/content.ron` and validated; a
  `Character` built from the starter kit.
- Glitch flash on jack in and jack out.
- Dev camera moves from Tab to `c`.

### Not in scope

Equipping with effects, crafting, quest state changes, trade, skill checks,
saving or loading the character, keyboard remapping, mouse.

## 2. Rulings taken while designing

- **Tab is the deck, `c` is the dev camera.** Tab is the RPG convention; the
  camera is a dev tool. Cost if wrong: two key constants.
- **The Map tab shows memory**, not the full minimap: only tiles the player has
  seen on the current stratum. The dev inspector keeps the full minimap. Cost
  if wrong: swap one renderer.
- **Three settings**: dev mode, rain, FPS. All three exist as behaviour today.
  Cost if wrong: rows in a form.
- **Save from the pause menu returns to the pause menu**, not the map; Load of
  a city replaces the whole stack with a fresh map, as today. Cost if wrong:
  one `Pop` count.
- **The character is not saved.** `.city` files stay city-only (format 2).
  Cost if wrong: none now; the profile save is a later spec.

## 3. Screen stack (`cyberpunk::app`)

```rust
pub enum ScreenCmd {
    Stay,
    /// Replace the whole stack with one screen.
    Switch { to: Box<dyn Screen>, fx: TransitionKind },
    /// Put a screen on top; the ones below keep drawing, dimmed, and stop updating.
    Push { to: Box<dyn Screen>, fx: TransitionKind },
    /// Remove the top screen. On the last screen it is ignored.
    Pop { fx: TransitionKind },
    /// Run a transition in place without changing screens (the jack-in glitch).
    Flash(TransitionKind),
    Quit,
}
```

`Runner` holds `screens: Vec<Box<dyn Screen>>` (never empty). Each frame: only
the top screen gets `update`. Drawing goes bottom-up; before every screen
except the bottom one, `ui::dim::wash(buf)` darkens the buffer (colours scaled
to 0.35, every odd row additionally scaled to 0.8 for scanlines). Transitions
snapshot the last frame as today and apply over the final composite. `Switch`
clears the stack. The `Files` screen loses its `back` hack: opened from the
pause menu or from `s`, it returns with `Pop`.

Key behaviour on the map screen, both modes:

| Key | Before | After |
|---|---|---|
| Esc | title | push pause menu |
| `q` | title | push pause menu |
| Tab | dev camera | push the deck |
| `c` | — | dev camera (dev mode only) |
| `s` | save screen (switch) | push save screen |
| Enter (walk) | jack in / out | same, plus `Flash(Glitch)` |

Title Esc still quits; Configure and Files Esc still go back.

## 4. Pause menu (`screens::pause`)

Centred boxed panel titled `PAUSED`, the existing `Menu` widget, items:
`RESUME`, `SAVE CITY`, `LOAD CITY`, `SETTINGS`, `QUIT TO TITLE`, `QUIT GAME`.
Esc or Resume → `Pop`. Save → `Push(Files::save)`. Load → `Push(Files::load)`
(a successful load does `Switch` to a fresh map as today). Settings →
`Push(Settings)`. Quit to title → `Switch(Title)`. Quit game → `Quit`.
Pushed with `TransitionKind::None`; the dim wash is the effect.

## 5. Settings (`screens::settings`)

Boxed panel titled `SETTINGS`, three rows, ↑↓ to move, ←→ / Enter to flip:

| Row | State | Effect |
|---|---|---|
| Dev mode | `AppState.dev` | as today |
| Rain | `AppState.rain` (default true) | the map screen includes the `Weather` overlay only when true |
| FPS counter | `AppState.show_fps` (default true) | `status_bar` takes `fps: Option<f32>` |

Esc → `Pop`.

## 6. The deck (`screens::deck`)

Pushed with `TransitionKind::Glitch`. Full-screen boxed panel titled with the
character handle. Row 1: the tab bar, `▌QUESTS▐  GEAR  INVENTORY  WORKBENCH  MAP  SKILLS`,
the active tab in bold cyan with the half-block brackets, others dim. Below: a
list pane (40% width, min 28) and a detail pane. Keys: ←→ or Tab / BackTab
cycle tabs, `1`..`6` jump, ↑↓ move the row, Esc closes (`Pop`). The detail pane
reveals its text with `typewriter` over 20 ticks from the last row change.
Palette: cyan `(0, 229, 255)` accents, magenta `(255, 0, 200)` for warnings
(missing ingredients, locked talents), dim grey `(120, 130, 145)` for inactive.

| Tab | List rows | Detail pane |
|---|---|---|
| QUESTS | active quests, then done ones (`✓`) | giver, location, description, objectives with `☐`/`☑` |
| GEAR | the nine slots: Head, Eyes, Neural, Arms, Torso, Legs, Primary, Secondary, Deck; `— empty —` when free | item name, kind, weight, value, description |
| INVENTORY | items grouped by kind, `qty× name`, footer `weight x.x kg · ¤ value` | item details; `equipped` marker when the item sits in a slot |
| WORKBENCH | recipes; magenta when the character lacks the skill level or an ingredient | skill and level needed, inputs with have/need counts, output, description |
| MAP | none; the pane is the map | the memory map (below) |
| SKILLS | eight skills `name  ▰▰▰▱▱ 3` | governing stat, description, then the skill's talents; unlocked ones cyan, locked ones dim with the level required |

**Chrome.** The deck must read as part of the city's interface, not a dialog:

- Frame: heavy top and bottom rules (`━`) in cyan `(0, 229, 255)`; light sides.
  When the terminal is at least 36 rows tall the handle is set in the title
  block font (`ui::logo`) across the top; otherwise it is a bold caption.
- Link line under the title: `▍ LINK · <district under the player or cursor> ·
  <stratum>`, pulsing with `pulse(tick, 40)` like the strata selector; on the
  right the readout `¤ <credits> · hp <hp>/<hp_max> · <weight> kg` in the
  status-bar style.
- Tab bar: the active tab in `▌…▐` half-block brackets, bold cyan with the menu
  glow; a magenta `(255, 0, 200)` segment under the active tab on the rule
  slides to the new tab over 6 ticks (`Tween`, `Easing::OutCubic`).
- Selected row: a bar with background `(0, 60, 70)` and the `▸` marker, glowing
  with `pulse(tick, 30)` like `MenuView`.
- Detail pane: a typewriter reveal over 20 ticks from the last change with a
  blinking magenta `▮` cursor at the end of the typed text while it types.
- Jitter: every 90 ticks a 3-tick burst of `glitch_cells` over one random frame
  row (the burst row and start from `hash64(tick / 90, 7)`), so the panel reads
  as a live signal. Opened with `Glitch`, closed (`Pop`) with `Glitch`.
- Icons by item kind, all glyphs the city already draws: weapon `/`, armor `▣`,
  cyberware `◈`, deck `⌗`, consumable `◍`, component `▪`, junk `·`. Skill bars
  `▰▱`, objectives `☑`/`☐`, done quests `✓`. Every glyph is one cell wide and
  covered by the width test.
- The map underneath keeps drawing (rain included) under the scanline wash.

**Memory map.** Draws `player.vis()` for the current stratum scaled to fit the
pane, one cell per block of tiles: unseen blocks are blank, remembered blocks
take the district accent scaled 0.35, visible blocks 0.7; the player's block is
`@` in white. Without a player: `no signal — walk the city first`.

## 7. The `game` crate (`crates/game`, no terminal dependencies)

```rust
pub struct ItemId(pub String);  pub struct SkillId(pub String);
pub struct TalentId(pub String); pub struct QuestId(pub String); pub struct RecipeId(pub String);

pub enum Slot { Head, Eyes, Neural, Arms, Torso, Legs, Primary, Secondary, Deck }
pub enum ItemKind { Weapon, Armor, Cyberware, Deck, Consumable, Component, Junk }
pub enum Stat { Body, Reflex, Tech, Cool, Net }

pub struct Item { pub id: ItemId, pub name: String, pub kind: ItemKind, pub slot: Option<Slot>,
                  pub weight: f32, pub value: u32, pub desc: String }
pub struct Skill { pub id: SkillId, pub name: String, pub stat: Stat, pub desc: String }
pub struct Talent { pub id: TalentId, pub name: String, pub skill: SkillId, pub level: u8, pub desc: String }
pub struct Quest { pub id: QuestId, pub title: String, pub giver: String, pub location: String,
                   pub desc: String, pub objectives: Vec<String> }
pub struct Recipe { pub id: RecipeId, pub name: String, pub skill: SkillId, pub level: u8,
                    pub inputs: Vec<(ItemId, u32)>, pub output: (ItemId, u32), pub desc: String }

pub struct Stats { pub body: u8, pub reflex: u8, pub tech: u8, pub cool: u8, pub net: u8 }
pub struct Starter { pub handle: String, pub credits: u32, pub hp: u32, pub stats: Stats,
                     pub inventory: Vec<(ItemId, u32)>, pub equipped: Vec<(Slot, ItemId)>,
                     pub skills: Vec<(SkillId, u8)>, pub talents: Vec<TalentId>,
                     pub quests: Vec<(QuestId, Vec<bool>)> }

pub struct Content { pub items: Vec<Item>, pub skills: Vec<Skill>, pub talents: Vec<Talent>,
                     pub quests: Vec<Quest>, pub recipes: Vec<Recipe>, pub starter: Starter }

impl Content {
    /// Parses the embedded `flavor/content.ron` and validates it.
    pub fn load() -> Result<Content, ContentError>;
    pub fn validate(&self) -> Result<(), ContentError>;
    pub fn item(&self, id: &ItemId) -> Option<&Item>;      // likewise skill, talent, quest, recipe
    pub fn talents_for(&self, skill: &SkillId) -> impl Iterator<Item = &Talent>;
}

pub struct Stack { pub item: ItemId, pub qty: u32 }
pub struct QuestLog { pub quest: QuestId, pub done: Vec<bool> }
pub struct Character { pub handle: String, pub credits: u32, pub hp: u32, pub hp_max: u32,
                       pub stats: Stats, pub skills: Vec<(SkillId, u8)>, pub talents: Vec<TalentId>,
                       pub equipment: BTreeMap<Slot, ItemId>, pub inventory: Vec<Stack>,
                       pub quests: Vec<QuestLog> }

impl Character {
    pub fn from_starter(c: &Content) -> Character;
    pub fn skill_level(&self, id: &SkillId) -> u8;
    pub fn count(&self, id: &ItemId) -> u32;                 // in inventory
    pub fn weight(&self, c: &Content) -> f32;                // inventory + equipped
    pub fn value(&self, c: &Content) -> u32;
    pub fn can_craft(&self, c: &Content, r: &Recipe) -> bool; // level and inputs, no crafting
    pub fn has_talent(&self, id: &TalentId) -> bool;
}
```

`ContentError` variants: `Parse(String)`, `UnknownItem { where_: String, id: String }`,
`UnknownSkill`, `UnknownTalent`, `UnknownQuest`, `SlotMismatch { item: String, slot: Slot }`
(an equipped item's `slot` must equal the slot it sits in), `DuplicateId(String)`,
`ObjectiveCount { quest: String }` (starter quest flags length ≠ objectives).

`AppState` gains `pub content: Content` and `pub character: Character`
(from the starter kit at startup) plus `pub rain: bool`, `pub show_fps: bool`.

## 8. Content: `flavor/content.ron` starter kit

Original names in the shared cyberpunk register. At least: 14 items covering
every kind and every slot at least once (e.g. *Kiroshi-pattern optics* is not
allowed; write our own: "Nightglass optics", "Ratchet SMG", "Pawnshop deck",
"Soy-noodle cup", "Scrap servo", "Burner shard"), 8 skills across the five
stats (e.g. Blades, Handguns, Stealth, Hacking, Electronics, Streetwise,
Athletics, Medtech), 8 talents, 4 quests (two active, one done, one not yet
started is fine), 6 recipes of which at least one is craftable with the starter
inventory and at least one is not. The starter has 2 to 4 items equipped and
one empty slot at least.

## 9. Jack-in flash

On a successful jack in or jack out the map screen returns
`ScreenCmd::Flash(TransitionKind::Glitch)`. The runner snapshots the last frame
and runs the glitch for its duration over the same screen. `Transition::new`
gets a `dur` of 10 ticks for a flash (vs 6 for a switch) via
`Transition::flash(kind, from, start)`.

## 10. Testing

`game`: content parses and validates; a broken copy (unknown item in a recipe,
slot mismatch, duplicate id, objective count) yields the right error; starter
character has the equipped items, counts and skill levels from the file;
`can_craft` true for the designed craftable recipe and false for the other;
weight and value sum correctly.

`cyberpunk`: runner pushes, dims the lower screen (a cell's colour is darker
than before), only updates the top, pops back, ignores a pop on the last
screen, `Switch` clears the stack, `Flash` starts a transition without
changing the screen; Esc on the map pushes the pause menu and Resume pops; Save
from the pause menu pushes Files and Esc pops back to the pause menu; Settings
flips rain and the map stops drawing rain; Tab opens the deck, `1`..`6` and ←→
switch tabs, every tab renders its list from the content, the detail pane names
the selected item, Esc closes; the Map tab shows `@` after a walk and "no
signal" before one; `c` toggles the camera only in dev mode; Enter beside a
terminal returns `Flash(Glitch)`; the pty smoke drives Esc → pause → settings →
back, Tab → every tab → Esc, and quits via the pause menu.
