# HUD Chrome Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restyle the play HUD into one instrument panel: a four-row top band with column-wide bars and a tab row, a right column of title-less panes split by light rules, heavy band rules, and a two-row footer with labelled zones.

**Architecture:** `ui/hud.rs` keeps pure row builders (status, location, bars, carry, tabs, footer rows, rules) fed by plain values; `ui/sidebar.rs` keeps the sections and budget but loses its box and titles and gains separators, quest pips, the coordinate row and log stamps; `screens/explore.rs` lays the grid out and binds the F-keys. No data changes.

**Tech Stack:** Rust 2024, ratatui, crossterm; `TestBackend` buffer tests.

**Spec:** `docs/superpowers/specs/2026-10-13-hud-chrome-design.md`

## Global Constraints

- Right column `R` = 40 at every width (the spec's 36 under 120 columns was corrected during execution: the tab row needs 39 cells); map `L = W - R - 1`; the vertical rule `│` is at `x = L`.
- Rows: 4 band rows, heavy rule (`━` with `┳`), main, heavy rule (`━` with `┻`), 2 footer rows. Minimum 100 by 30 keeps 22 map rows.
- Bars: `" {label:<5} {num:>7} "` then `█`/`░` filling `width - 16` cells; count `round(cells * cur / max)`, 0 for max 0.
- Tabs: `[F1]Here*` on (cyan bold), `[F1]Here` off (dim); keys cyan, brackets dim.
- Panes have no titles; every pane after the first starts with a `─` row; LOG floor 3 rows; a pane under 2 rows gets none.
- Log stamps `HH:MM` dimmed from the entry's tick; wraps hang at 7 cells.
- Rules in `chrome::RULE`; bands and the right column on `chrome::PANEL_BG`.
- Attribution lines on every commit; `cargo fmt` and `cargo clippy --all-targets -- -D warnings` clean before each commit.

## Review Focus

1. A terminal exactly 100 wide: the main tab row must fit the 40-cell column without cutting `[F4]Log*` (Task 1 `tab_rows_fit_the_narrow_column`).
2. A handle longer than the status line leaves for it, with dev readouts on: the row must cut with `…` and never exceed the column (Task 1 `status_line_cuts_to_width`).
3. A location long enough to reach the clock on the 40-cell column: one cell must separate them (Task 1 `location_line_keeps_the_clock`).
4. The LOG pane as the first pane (HERE, QUEST and MAP off): no separator row and the first entry on the first row (Task 2 `first_pane_has_no_separator`).
5. F5 outside dev mode and F7 anywhere: no toggle, no panic, no log line (Task 3 `f_keys_toggle_sections`).

---

### Task 1: HUD row builders

**Files:**
- Modify: `crates/cyberpunk/src/ui/hud.rs` (replace `Header`/`header`, `Bars`/`bars`, `BarMode`/`action_bar`; keep `cut`, `bar_cells`, `hp_colour`, `Slot`, `SlotState`, `SLOTS`, `location`, colours)
- Modify: `crates/cyberpunk/src/app.rs` (`clock` delegates to `hud::clock_at`)

**Interfaces:**
- Produces:
  - `pub fn clock_at(tick: u64) -> (u8, u8)`; `pub fn clock_glyph(hour: u8) -> char` (`☾` for 18..=23 and 0..=5, else `☀`).
  - `pub struct Status<'a> { pub dev: bool, pub handle: &'a str, pub level: u8, pub credits: u32, pub seed: u64, pub fps: Option<f32> }`; `pub fn status_line(s: &Status, width: u16) -> Line<'static>`.
  - `pub fn location_line(location: &str, clock: (u8, u8), width: u16) -> Line<'static>`.
  - `pub fn bar_row(label: &str, cur: u32, max: u32, colour: Color, width: u16) -> Line<'static>`; `pub fn carry_row(weight: f32, cap: f32, width: u16) -> Line<'static>`.
  - `pub fn tab_rows(flags: &Sidebar, dev: bool) -> (Line<'static>, Line<'static>)` (dev row, main row).
  - `pub fn footer_status(status: Vec<Span<'static>>, here: &str, verbs: &[(&str, &str)], width: u16) -> Line<'static>`; `pub const WALK_VERBS` and `EXPLORE_VERBS: &[(&str, &str)]`.
  - `pub fn footer_abilities(slots: &[Slot], dev: bool, width: u16) -> Line<'static>`.
  - `pub fn rule_row(width: u16, cross: u16, glyph: char) -> Line<'static>` (heavy `━` with `glyph` at `cross`).
  - `pub fn cut_spans(spans: Vec<Span<'static>>, max: usize) -> Vec<Span<'static>>`.
  - `pub const RULE_STYLE: Style`.

- [ ] **Step 1: Replace the hud tests** with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::sidebar::{Section, Sidebar};
    use citygen::{GenParams, Generator, SizePreset};

    fn city() -> City {
        Generator::run(GenParams { seed: 6, size: SizePreset::Custom { w: 120, h: 60 }, districts: 5, ..Default::default() }).unwrap()
    }

    fn status() -> Status<'static> {
        Status { dev: false, handle: "Vex", level: 2, credits: 320, seed: 7, fps: Some(30.0) }
    }

    #[test]
    fn clock_glyph_and_clock_at() {
        assert_eq!(clock_at(0), (22, 0));
        assert_eq!(clock_at(90 * 120), (0, 0));
        assert_eq!(clock_glyph(17), '☀');
        assert_eq!(clock_glyph(18), '☾');
        assert_eq!(clock_glyph(5), '☾');
        assert_eq!(clock_glyph(6), '☀');
    }

    #[test]
    fn status_line_has_handle_level_credits_and_dev_readouts() {
        let s = status_line(&status(), 79).to_string();
        assert_eq!(s.trim_end(), " VEX :: LVL 2 :: ¤ 320", "{s:?}");
        let d = status_line(&Status { dev: true, ..status() }, 79).to_string();
        assert!(d.starts_with(" DEV ") || d.starts_with("  DEV"), "{d:?}");
        assert!(d.contains("VEX :: LVL 2 :: ¤ 320 :: seed 7 :: 30 fps"), "{d:?}");
        let no_fps = status_line(&Status { dev: true, fps: None, ..status() }, 79).to_string();
        assert!(no_fps.contains("seed 7") && !no_fps.contains("fps"), "{no_fps:?}");
    }

    #[test]
    fn status_line_cuts_to_width() {
        let long = "A".repeat(80);
        let s = status_line(&Status { dev: true, handle: &long, ..status() }, 59).to_string();
        assert!(s.chars().count() <= 59 && s.contains('…'), "{s:?}");
    }

    #[test]
    fn location_line_keeps_the_clock() {
        let s = location_line("Night Market · Street", (22, 41), 40).to_string();
        assert_eq!(s.chars().count(), 40, "{s:?}");
        assert!(s.starts_with(" Night Market · Street") && s.ends_with("☾ 22:41"), "{s:?}");
        let long = location_line(&"B".repeat(60), (9, 5), 36).to_string();
        assert_eq!(long.chars().count(), 36, "{long:?}");
        assert!(long.ends_with("… ☀ 09:05"), "{long:?}");
    }

    #[test]
    fn bar_rows_fill_the_column() {
        let s = bar_row("HP", 38, 45, GREEN, 79).to_string();
        assert_eq!(s.chars().count(), 79, "{s:?}");
        assert!(s.starts_with(" HP      38/45 "), "{s:?}");
        let cells = 79 - 16;
        assert_eq!(s.matches('█').count() + s.matches('░').count(), cells);
        assert_eq!(s.matches('█').count(), bar_cells_for(38, 45, cells));
        let x = bar_row("XP", 240, 600, AMBER, 59).to_string();
        assert!(x.starts_with(" XP    240/600 "), "{x:?}");
        let z = bar_row("FOCUS", 0, 0, CYAN, 59).to_string();
        assert_eq!(z.matches('█').count(), 0);
        let c = carry_row(11.6, 16.0, 40).to_string();
        assert_eq!(c.chars().count(), 40, "{c:?}");
        assert!(c.starts_with(" CARRY 11.6/16.0 kg ") && c.contains('█'), "{c:?}");
        let over = carry_row(20.0, 16.0, 40);
        assert!(over.spans.iter().any(|sp| sp.style.fg == Some(MAGENTA)));
    }

    #[test]
    fn tab_rows_fit_the_narrow_column() {
        let mut flags = Sidebar::default();
        flags.set(Section::Quest, false);
        let (dev, main) = tab_rows(&flags, true);
        let m = main.to_string();
        assert_eq!(m.trim_end(), " [F1]Here* [F2]Quest [F3]Map* [F4]Log*", "{m:?}");
        assert!(m.chars().count() <= 36);
        assert_eq!(dev.to_string().trim_end(), " [F5]Inspect [F6]Legend");
        let (dev, _) = tab_rows(&flags, false);
        assert_eq!(dev.to_string().trim(), "");
    }

    #[test]
    fn footer_rows_have_three_zones_and_slots() {
        let status = vec![Span::raw("on foot · Street")];
        let s = footer_status(status, "3 items · terminal", WALK_VERBS, 120).to_string();
        assert_eq!(s.chars().count(), 120, "{s:?}");
        assert!(s.starts_with(" STATUS: on foot · Street"), "{s:?}");
        assert_eq!(s.find("HERE: 3 items · terminal"), Some(48), "{s:?}");
        assert!(s.trim_end().ends_with("[space]act [enter]jack [v]iew [m]sgs [?]keys"), "{s:?}");
        let narrow = footer_status(vec![Span::raw("x".repeat(60))], &"y".repeat(60), EXPLORE_VERBS, 100).to_string();
        assert_eq!(narrow.chars().count(), 100, "{narrow:?}");
        assert!(narrow.trim_end().ends_with("[?]keys") && narrow.matches('…').count() == 2, "{narrow:?}");
        let slots = vec![Slot { name: "Netscan".into(), state: SlotState::Ready }, Slot { name: "Ping".into(), state: SlotState::Cooling(12) }];
        let a = footer_abilities(&slots, true, 120).to_string();
        assert!(a.starts_with(" ABILITIES: 1 netscan ●   2 ping 12s   3 ──   4 ──   5 ──"), "{a:?}");
        assert!(a.trim_end().ends_with("[:]console [tab]deck"), "{a:?}");
        let a = footer_abilities(&slots, false, 100).to_string();
        assert!(a.trim_end().ends_with("[tab]deck") && !a.contains("console"), "{a:?}");
    }

    #[test]
    fn rule_row_crosses_at_the_column() {
        let r = rule_row(100, 63, '┳').to_string();
        assert_eq!(r.chars().count(), 100);
        assert_eq!(r.chars().nth(63), Some('┳'));
        assert_eq!(r.matches('━').count(), 99);
    }

    #[test]
    fn location_names_building_district_and_net() {
        let c = city();
        let b = c.buildings.iter().find(|b| b.name.is_some()).unwrap();
        assert_eq!(location(&c, StratumKind::Street, b.footprint.center()), format!("{} · Street", b.name.clone().unwrap()));
        assert_eq!(location(&c, StratumKind::Street, Pos::new(-5, -5)), "the outskirts · Street");
        assert_eq!(location(&c, StratumKind::Net, Pos::new(-5, -5)), format!("{} · Net", c.name));
    }
}
```

`bar_cells_for(cur, max, cells)` is the new generalised `bar_cells`; keep `bar_cells(cur, max)` as `bar_cells_for(cur, max, BAR_CELLS)` only if anything still uses it, else delete both the constant and the old name.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p cyberpunk hud 2>&1 | grep -E "^error" | head`
Expected: unresolved names (`status_line`, `Status`, ...).

- [ ] **Step 3: Implement** (replacing `Header`, `header`, `Bars`, `bars`, `bar`, `BarMode`, `action_bar`):

```rust
use crate::ui::chrome::{CYAN, DIM, MAGENTA, RULE, TEXT};
use crate::ui::sidebar::{Section, Sidebar};

pub const RULE_STYLE: Style = Style::new().fg(RULE);
const TICKS_PER_MINUTE: u64 = 90;
const START_MINUTE: u64 = 22 * 60;

/// The clock at `tick`: 22:00 at tick 0, one minute per ninety ticks, wrapping at midnight.
pub fn clock_at(tick: u64) -> (u8, u8) {
    let m = (START_MINUTE + tick / TICKS_PER_MINUTE) % (24 * 60);
    ((m / 60) as u8, (m % 60) as u8)
}

pub fn clock_glyph(hour: u8) -> char {
    if (18..=23).contains(&hour) || hour <= 5 { '☾' } else { '☀' }
}

fn dim(s: impl Into<String>) -> Span<'static> { Span::styled(s.into(), Style::default().fg(DIM)) }
fn text(s: impl Into<String>) -> Span<'static> { Span::styled(s.into(), Style::default().fg(TEXT)) }
fn cyan(s: impl Into<String>) -> Span<'static> { Span::styled(s.into(), Style::default().fg(CYAN).add_modifier(Modifier::BOLD)) }
fn width_of(spans: &[Span<'static>]) -> usize { spans.iter().map(|s| s.width()).sum() }

/// Cuts a span list to `max` cells, ending in `…` when it was longer.
pub fn cut_spans(spans: Vec<Span<'static>>, max: usize) -> Vec<Span<'static>> {
    if width_of(&spans) <= max { return spans; }
    let mut out = Vec::new();
    let mut left = max.saturating_sub(1);
    for sp in spans {
        let w = sp.width();
        if w <= left { left -= w; out.push(sp); continue; }
        let kept: String = sp.content.chars().take(left).collect();
        out.push(Span::styled(kept, sp.style));
        out.push(Span::styled("…", sp.style));
        return out;
    }
    out
}

fn padded(mut spans: Vec<Span<'static>>, width: usize) -> Line<'static> {
    let w = width_of(&spans);
    if w < width { spans.push(Span::raw(" ".repeat(width - w))); }
    Line::from(spans)
}

pub struct Status<'a> { pub dev: bool, pub handle: &'a str, pub level: u8, pub credits: u32, pub seed: u64, pub fps: Option<f32> }

pub fn status_line(s: &Status, width: u16) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    if s.dev {
        spans.push(Span::styled(" DEV ", Style::default().fg(BADGE_FG).bg(AMBER).add_modifier(Modifier::BOLD)));
        spans.push(Span::raw(" "));
    }
    spans.push(cyan(s.handle.to_uppercase()));
    spans.push(dim(" :: LVL "));
    spans.push(text(s.level.to_string()));
    spans.push(dim(" :: ¤ "));
    spans.push(text(s.credits.to_string()));
    if s.dev {
        spans.push(dim(format!(" :: seed {}", s.seed)));
        if let Some(fps) = s.fps { spans.push(dim(format!(" :: {fps:.0} fps"))); }
    }
    padded(cut_spans(spans, width as usize), width as usize)
}

pub fn location_line(location: &str, clock: (u8, u8), width: u16) -> Line<'static> {
    let clock_text = format!("{} {:02}:{:02}", clock_glyph(clock.0), clock.1, clock.1);
```
(that last line is a typo trap: it must be `clock.0, clock.1`; write it as `format!("{} {:02}:{:02}", clock_glyph(clock.0), clock.0, clock.1)`.)
```rust
    let room = (width as usize).saturating_sub(clock_text.chars().count() + 2);
    let loc = cut(location, room);
    let gap = (width as usize).saturating_sub(1 + loc.chars().count() + clock_text.chars().count());
    Line::from(vec![Span::raw(" "), cyan(loc), Span::raw(" ".repeat(gap)), text(clock_text)])
}

pub fn bar_cells_for(cur: u32, max: u32, cells: usize) -> usize {
    if max == 0 { return 0; }
    ((cells as f64 * cur.min(max) as f64 / max as f64).round() as usize).min(cells)
}

fn bar_spans(filled: usize, cells: usize, colour: Color) -> Vec<Span<'static>> {
    vec![
        Span::styled("█".repeat(filled), Style::default().fg(colour)),
        Span::styled("░".repeat(cells - filled), Style::default().fg(colour).add_modifier(Modifier::DIM)),
    ]
}

/// ` {label:<5} {cur}/{max} ` then the bar to the column's edge but one.
pub fn bar_row(label: &str, cur: u32, max: u32, colour: Color, width: u16) -> Line<'static> {
    let cells = (width as usize).saturating_sub(16);
    let mut spans = vec![dim(format!(" {label:<5} ")), text(format!("{:>7} ", format!("{cur}/{max}")))];
    spans.extend(bar_spans(bar_cells_for(cur, max, cells), cells, colour));
    padded(spans, width as usize)
}

pub fn carry_row(weight: f32, cap: f32, width: u16) -> Line<'static> {
    let head = format!(" {weight:.1}/{cap:.1} kg ");
    let cells = (width as usize).saturating_sub(7 + head.chars().count() + 1);
    let colour = if weight > cap { MAGENTA } else { CYAN };
    let filled = if cap <= 0.0 { 0 } else { ((cells as f32 * (weight / cap).min(1.0)).round() as usize).min(cells) };
    let mut spans = vec![dim(" CARRY"), text(head)];
    spans.extend(bar_spans(filled, cells, colour));
    padded(spans, width as usize)
}

fn tab(key: &str, label: &str, on: bool) -> Vec<Span<'static>> {
    let word = if on { format!("{label}*") } else { label.to_string() };
    let style = if on { Style::default().fg(CYAN).add_modifier(Modifier::BOLD) } else { Style::default().fg(DIM) };
    vec![dim("["), Span::styled(key.to_string(), Style::default().fg(CYAN)), dim("]"), Span::styled(word, style)]
}

/// (dev row, main row); the dev row is blank outside dev mode.
pub fn tab_rows(flags: &Sidebar, dev: bool) -> (Line<'static>, Line<'static>) {
    let row = |sections: &[Section], first: usize| {
        let mut spans = vec![Span::raw(" ")];
        for (i, s) in sections.iter().enumerate() {
            if i > 0 { spans.push(Span::raw(" ")); }
            let label = { let mut c = s.label().chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() };
            spans.extend(tab(&format!("F{}", first + i), &label, flags.is_on(*s)));
        }
        Line::from(spans)
    };
    let main = row(&[Section::Here, Section::Quest, Section::Map, Section::Log], 1);
    let dev_row = if dev { row(&[Section::Inspect, Section::Legend], 5) } else { Line::from("") };
    (dev_row, main)
}

pub const WALK_VERBS: &[(&str, &str)] = &[("space", "act"), ("enter", "jack"), ("v", "iew"), ("m", "sgs"), ("?", "keys")];
pub const EXPLORE_VERBS: &[(&str, &str)] = &[("w", "alk"), ("r", "egen"), ("g", "configure"), ("v", "iew"), ("m", "sgs"), ("?", "keys")];

fn verb_spans(verbs: &[(&str, &str)]) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    for (i, (k, w)) in verbs.iter().enumerate() {
        if i > 0 { out.push(Span::raw(" ")); }
        out.push(dim("[")); out.push(Span::styled(k.to_string(), Style::default().fg(CYAN))); out.push(dim("]")); out.push(dim(w.to_string()));
    }
    out
}

/// ` STATUS: …` at the left, `HERE: …` at 40% of the width, the verbs right-aligned; zones are cut so they never touch.
pub fn footer_status(status: Vec<Span<'static>>, here: &str, verbs: &[(&str, &str)], width: u16) -> Line<'static> {
    let w = width as usize;
    let verbs = verb_spans(verbs);
    let verbs_x = w.saturating_sub(width_of(&verbs) + 1);
    let here_x = w * 40 / 100;
    let mut zone1 = vec![dim(" STATUS: ")]; zone1.extend(status);
    let zone1 = cut_spans(zone1, here_x.saturating_sub(1));
    let zone2 = cut_spans(vec![dim("HERE: "), text(here.to_string())], verbs_x.saturating_sub(here_x + 1));
    let mut spans = zone1;
    let w1 = width_of(&spans);
    spans.push(Span::raw(" ".repeat(here_x.saturating_sub(w1))));
    spans.extend(zone2);
    let w2 = width_of(&spans);
    spans.push(Span::raw(" ".repeat(verbs_x.saturating_sub(w2))));
    spans.extend(verbs);
    padded(spans, w)
}

pub fn footer_abilities(slots: &[Slot], dev: bool, width: u16) -> Line<'static> {
    let mut spans = vec![dim(" ABILITIES: ")];
    for i in 0..SLOTS {
        if i > 0 { spans.push(Span::raw("   ")); }
        match slots.get(i) {
            Some(slot) => {
                spans.push(cyan(format!("{} ", i + 1)));
                spans.push(text(cut(&slot.name.to_lowercase(), 10)));
                match slot.state {
                    SlotState::Ready => spans.push(Span::styled(" ●", Style::default().fg(CYAN))),
                    SlotState::Cooling(secs) => spans.push(dim(format!(" {secs}s"))),
                }
            }
            None => spans.push(dim(format!("{} ──", i + 1))),
        }
    }
    let right = if dev { verb_spans(&[(":", "console"), ("tab", "deck")]) } else { verb_spans(&[("tab", "deck")]) };
    let used = width_of(&spans);
    let gap = (width as usize).saturating_sub(used + width_of(&right) + 1);
    spans.push(Span::raw(" ".repeat(gap)));
    spans.extend(right);
    padded(spans, width as usize)
}

/// A heavy rule with `glyph` where the vertical rule crosses it.
pub fn rule_row(width: u16, cross: u16, glyph: char) -> Line<'static> {
    let s: String = (0..width).map(|x| if x == cross { glyph } else { '━' }).collect();
    Line::from(Span::styled(s, RULE_STYLE))
}
```

`app.rs`: `pub fn clock(&self) -> (u8, u8) { crate::ui::hud::clock_at(self.tick) }`, removing the two constants there (keep the test).

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p cyberpunk hud 2>&1 | tail -3`
Expected: 9 tests pass. `cargo build` will fail until Task 3 rewires `explore.rs`; that is expected — add `#![allow(dead_code)]` at the top of `hud.rs` for this task only if clippy needs it, and keep `explore.rs` compiling by stubbing: replace its `hud::header`/`hud::bars`/`hud::action_bar` calls with `Line::from("")` placeholders in this task (they are rewritten in Task 3).

- [ ] **Step 5: Commit**

```bash
git add crates/cyberpunk/src/ui/hud.rs crates/cyberpunk/src/app.rs crates/cyberpunk/src/screens/explore.rs
git commit -m "feat(ui): HUD chrome row builders"
```

---

### Task 2: Sidebar panes without titles

**Files:**
- Modify: `crates/cyberpunk/src/ui/sidebar.rs`

**Interfaces:**
- Produces: `pub fn width(total: u16) -> u16` (40 at ≥120 else 36); `pub fn quest_lines(&Character, &Content) -> Vec<Line<'static>>` (3 lines or 1); `pub fn here_summary(v: &SidebarView) -> String` for the footer (`3 items · terminal`, `nothing`); `pub fn status_text(...)` is NOT here (explore builds it); `draw` fills the area with `PANEL_BG`, draws separators, returns `bool` as today.
- Removes: `WIDTH`, `rule`, `list_line`.

- [ ] **Step 1: Update the sidebar tests** — replace `WIDTH` with `width(100)` (36) in `draw_to` calls; change `quest_picks_the_first_open_objective` to:

```rust
    #[test]
    fn quest_shows_title_objective_and_pips() {
        let st = state();
        let lines: Vec<String> = quest_lines(&st.character, &st.content).iter().map(|l| l.to_string()).collect();
        assert_eq!(lines, vec![" THE DROP · Mama Sol", " » Cross the corp core without a scan", " ▰▱▱ 1/3"]);
        let mut ch = st.character.clone();
        for q in &mut ch.quests { q.done.fill(true); }
        let lines: Vec<String> = quest_lines(&ch, &st.content).iter().map(|l| l.to_string()).collect();
        assert_eq!(lines, vec![" no active quest"]);
    }
```
add:
```rust
    #[test]
    fn panes_are_separated_by_light_rules_and_have_no_titles() {
        let mut st = state();
        st.dev = false;
        st.tick += 200;
        let (k, pos) = { let p = st.player.as_ref().unwrap(); (p.stratum, p.pos) };
        let text = draw_to(&view(&st, pos, k), &st.sidebar, width(120), 40);
        assert!(!text.contains("HERE") && !text.contains("LOG") && !text.contains("VEX") && !text.contains('╭'), "{text}");
        assert_eq!(text.matches(&"─".repeat(40)).count(), 3, "{text}");
        assert!(text.contains("· Street"), "coordinate row: {text}");
        assert_eq!(width(119), 36);
        assert_eq!(width(120), 40);
    }

    #[test]
    fn first_pane_has_no_separator() {
        let mut st = state();
        st.dev = false;
        st.say("hello");
        st.tick += 200;
        let (k, pos) = { let p = st.player.as_ref().unwrap(); (p.stratum, p.pos) };
        let mut flags = Sidebar::default();
        for s in [Section::Here, Section::Quest, Section::Map] { flags.set(s, false); }
        let text = draw_to(&view(&st, pos, k), &flags, width(100), 10);
        let first = text.lines().next().unwrap();
        assert!(first.contains("22:06 hello"), "{text}");
        assert!(!text.contains('─'), "{text}");
    }

    #[test]
    fn log_lines_carry_stamps_and_hang_wraps() {
        let mut st = state();
        st.dev = false;
        st.tick = 90 * 3;
        st.say("x".repeat(50));
        st.tick += 200;
        let (k, pos) = { let p = st.player.as_ref().unwrap(); (p.stratum, p.pos) };
        let mut flags = Sidebar::default();
        for s in [Section::Here, Section::Quest, Section::Map] { flags.set(s, false); }
        let text = draw_to(&view(&st, pos, k), &flags, width(100), 10);
        let rows: Vec<&str> = text.lines().filter(|l| l.contains('x')).collect();
        assert!(rows[0].starts_with(" 22:03 x"), "{text}");
        assert!(rows[1].starts_with("       x"), "{text}");
        assert_eq!(text.matches('x').count(), 50);
        assert_eq!(here_summary(&view(&st, pos, k)).is_empty(), false);
    }
```
and in `log_wraps_long_entries_whole` the row arithmetic changes: with no list row the budget is `1 rule? no: LOG first → no rule`; height 7 → 5 inner rows... rewrite its two draws as `draw_to(..., width(100), 6)` expecting 70 x's (4 rows + "short": 70 cells wrap at 29 = 3 rows + 1) and `draw_to(..., width(100), 4)` expecting 0 x's.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p cyberpunk sidebar 2>&1 | grep -E "^error" | head -5`
Expected: `width`, `here_summary` unresolved; `quest_lines` type mismatch.

- [ ] **Step 3: Implement**

```rust
pub fn width(total: u16) -> u16 { if total >= 120 { 40 } else { 36 } }
```
`quest_lines` returns `Vec<Line>`:
```rust
pub fn quest_lines(character: &Character, content: &Content) -> Vec<Line<'static>> {
    for q in &character.quests {
        if q.is_done() { continue; }
        let Some(quest) = content.quest(&q.quest) else { continue };
        let Some(i) = q.done.iter().position(|d| !*d) else { continue };
        let done = q.done.iter().filter(|d| **d).count();
        let total = q.done.len();
        let objective = quest.objectives.get(i).cloned().unwrap_or_default();
        return vec![
            Line::from(vec![Span::raw(" "), Span::styled(quest.title.to_uppercase(), Style::default().fg(CYAN).add_modifier(Modifier::BOLD)), Span::styled(format!(" · {}", quest.giver), Style::default().fg(TEXT))]),
            plain(format!(" » {objective}")),
            Line::from(vec![Span::raw(" "), Span::styled("▰".repeat(done), Style::default().fg(CYAN)), Span::styled("▱".repeat(total - done), Style::default().fg(DIM)), Span::styled(format!(" {done}/{total}"), Style::default().fg(DIM))]),
        ];
    }
    vec![dim(" no active quest")]
}
```
HERE and INSPECT lines get a leading space too (`plain(format!(" {icon} {name}{qty}"))` etc.) so every pane shares the one-cell gutter.

`here_summary`:
```rust
pub fn here_summary(v: &SidebarView) -> String {
    let mut parts = Vec::new();
    let n = v.ground.at(v.stratum, v.focus).len();
    if n == 1 { parts.push("1 item".to_string()); } else if n > 1 { parts.push(format!("{n} items")); }
    if v.stratum != StratumKind::Net && v.city.tile(v.stratum, v.focus).is_some_and(|t| t.feature == Some(Feature::Node(NodeKind::Terminal))) { parts.push("terminal".to_string()); }
    for c in v.city.connectors_at(v.focus).filter(|c| c.kind != ConnectorKind::Jack && c.serves.contains(&v.stratum)) { parts.push(c.kind.label().to_string()); }
    if parts.is_empty() { "nothing".to_string() } else { parts.join(" · ") }
}
```
Log entry lines with stamps:
```rust
fn entry_lines(e: &Entry, width: u16) -> Vec<Line<'static>> {
    let style = match e.kind { Kind::Info => Style::default().fg(TEXT), Kind::Warn => Style::default().fg(MAGENTA), Kind::Story => Style::default().fg(CYAN) };
    let (h, m) = crate::ui::hud::clock_at(e.tick);
    let stamp = format!(" {h:02}:{m:02} ");
    wrap_text(&e.marked(), (width as usize).saturating_sub(7)).into_iter().enumerate().map(|(i, s)| {
        if i == 0 { Line::from(vec![Span::styled(stamp.clone(), Style::default().fg(DIM)), Span::styled(s, style)]) }
        else { Line::from(vec![Span::raw("       "), Span::styled(s, style)]) }
    }).collect()
}
```
Map pane: content rows = `MINIMAP_H` for the minimap then one row `format!("{},{} · {}", v.focus.x, v.focus.y, v.stratum.label())` right-aligned in DIM with one trailing cell; need `MINIMAP_H + 1`.

`draw`: fill the area with `PANEL_BG` (`for each cell: set_bg(PANEL_BG)`), no block, no list. Needs include the separator: for the i-th active pane with i > 0, `need + 1`; the LOG need stays 0. Then for each budgeted `(s, rows)`: if it is not the first drawn pane, render `"─".repeat(width)` in `RULE_STYLE` at `y`, `y += 1`, `rows -= 1`; then its content. The `whole` rule in `budget` stays (a pane under 2 rows gets none), LOG floor 3 stays.

Delete `rule`, `list_line`, `WIDTH`; keep `MINIMAP_H`.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p cyberpunk sidebar 2>&1 | tail -3`
Expected: 9 tests pass (the explore tests stay broken until Task 3; run with `-- sidebar` only).

- [ ] **Step 5: Commit**

```bash
git add crates/cyberpunk/src/ui/sidebar.rs
git commit -m "feat(ui): sidebar panes without titles, stamps and pips"
```

---

### Task 3: The map screen draws the grid; F-keys

**Files:**
- Modify: `crates/cyberpunk/src/screens/explore.rs` (layout, draw, keys, tests), `crates/cyberpunk/src/screens/help.rs` (F-keys lines), `README.md`

**Interfaces:**
- Consumes everything from Tasks 1 and 2.
- Produces: `KeyCode::F(1..=6)` toggles; VIEW rows labelled `▣ here  F1`.

- [ ] **Step 1: Write the failing tests** (replace `frame_draws_header_bars_and_action_bar`, `frame_fits_at_minimum_with_every_section`, `minimap_lives_in_the_sidebar_map_section`, `action_bar_keeps_its_hints_in_dev_mode` and `hidden_log_shows_a_toast` assertions to the new text; add):

```rust
    #[test]
    fn grid_draws_bands_rules_and_the_column() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        for (w, h, r) in [(100u16, 30u16, 36u16), (120, 40, 40)] {
            let text = draw_screen(&s, &st, w, h);
            let lines: Vec<&str> = text.lines().collect();
            let l = (w - r - 1) as usize;
            assert!(lines[0].contains(" DEV ") && lines[0].contains("VEX :: LVL 1 :: ¤ 320"), "{text}");
            assert!(lines[0].contains("· Street") && lines[0].trim_end().ends_with("22:00"), "{text}");
            assert!(lines[1].starts_with(" HP ") && lines[1].contains("CARRY"), "{text}");
            assert!(lines[2].starts_with(" FOCUS") && lines[2].contains("[F5]Inspect [F6]Legend"), "{text}");
            assert!(lines[3].starts_with(" XP ") && lines[3].contains("[F1]Here* [F2]Quest* [F3]Map* [F4]Log*"), "{text}");
            assert_eq!(lines[4].chars().nth(l), Some('┳'), "{text}");
            assert_eq!(lines[h as usize - 3].chars().nth(l), Some('┻'), "{text}");
            for y in 5..h as usize - 3 { assert_eq!(lines[y].chars().nth(l), Some('│'), "row {y}: {text}"); }
            assert!(lines[h as usize - 2].starts_with(" STATUS: camera · Street"), "{text}");
            assert!(lines[h as usize - 2].contains("HERE: ") && lines[h as usize - 2].trim_end().ends_with("[?]keys"), "{text}");
            assert!(lines[h as usize - 1].starts_with(" ABILITIES: 1 netscan ●"), "{text}");
            assert!(!text.contains('╭') && !text.contains("── "), "{text}");
        }
        walking(&mut st, &mut s);
        let text = draw_screen(&s, &st, 100, 30);
        assert!(text.contains(" STATUS: on foot · Street") && text.contains("[space]act"), "{text}");
        st.dev = false;
        let text = draw_screen(&s, &st, 100, 30);
        assert!(!text.contains("[F5]") && !text.contains(" DEV ") && !text.contains("[:]console"), "{text}");
    }

    #[test]
    fn f_keys_toggle_sections() {
        let mut st = state();
        let mut s = MapScreen::explore(0);
        s.update(&mut st, &[]);
        s.update(&mut st, &[key(KeyCode::F(1))]);
        assert!(!st.sidebar.is_on(Section::Here));
        assert!(draw_screen(&s, &st, 100, 30).contains("[F1]Here [F2]Quest*"));
        s.update(&mut st, &[key(KeyCode::F(1)), key(KeyCode::F(5))]);
        assert!(st.sidebar.is_on(Section::Here) && st.sidebar.is_on(Section::Inspect));
        st.set_dev(false);
        s.update(&mut st, &[key(KeyCode::F(5)), key(KeyCode::F(7))]);
        assert!(!st.sidebar.is_on(Section::Inspect));
        assert!(st.log.is_empty() || st.log.latest().unwrap().text != "nothing in slot 7");
    }

    #[test]
    fn status_zone_follows_the_jack() {
        // reuse the jack-in setup from `enter_jacks_in_and_the_net_is_the_fifth_stratum`
        // and after jacking in assert the footer row contains "STATUS: ⚡ jacked in · Net".
    }
```
(Write `status_zone_follows_the_jack` fully by copying that test's setup; it ends with `assert!(draw_screen(&s, &st, 120, 40).contains("STATUS: ⚡ jacked in · Net"))`.)

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p cyberpunk explore 2>&1 | grep -E "^error|FAILED" | head -5`
Expected: compile errors / failures.

- [ ] **Step 3: Implement**

`layout`:
```rust
struct Regions { band_l: [Rect; 4], band_r: [Rect; 4], top_rule: Rect, map: Rect, side: Rect, vrule_x: u16, bottom_rule: Rect, status: Rect, abilities: Rect }

fn layout(area: Rect) -> Regions {
    let r = sidebar::width(area.width);
    let l = area.width.saturating_sub(r + 1);
    let x_rule = area.x + l;
    let row = |y: u16, x: u16, w: u16| Rect::new(x, area.y + y, w, 1);
    let main_y = area.y + 5;
    let main_h = area.height.saturating_sub(8);
    Regions {
        band_l: [row(0, area.x, l), row(1, area.x, l), row(2, area.x, l), row(3, area.x, l)],
        band_r: [row(0, x_rule + 1, r), row(1, x_rule + 1, r), row(2, x_rule + 1, r), row(3, x_rule + 1, r)],
        top_rule: row(4, area.x, area.width),
        map: Rect::new(area.x, main_y, l, main_h),
        side: Rect::new(x_rule + 1, main_y, r, main_h),
        vrule_x: x_rule,
        bottom_rule: row(area.height - 3, area.x, area.width),
        status: row(area.height - 2, area.x, area.width),
        abilities: row(area.height - 1, area.x, area.width),
    }
}
```
`draw`, after the map renders into `r.map`:
1. Fill the band rows, the right column and the two footer rows with `PANEL_BG` (helper `fill(buf, rect)`); draw `│` in `RULE_STYLE` at `vrule_x` on every row except the two rule rows.
2. Band: `hud::status_line`, `hud::location_line(&hud::location(...), st.clock(), r)`, three `hud::bar_row`s (`"HP"` with `hp_colour`, `"FOCUS"` with `CYAN`, `"XP"` with `DIM_AMBER` made `pub`), `hud::carry_row(st.character.weight(&st.content), st.character.carry_cap(&st.content), r)`, `hud::tab_rows(&st.sidebar, st.dev)` into `band_r[2]` and `band_r[3]`.
3. `hud::rule_row(width, vrule_x - area.x, '┳')` into `top_rule`, `'┻'` into `bottom_rule`.
4. `sidebar::draw` into `r.side` (returns `log_drawn`).
5. Footer status: status spans — Walk: `on foot · {stratum}`; jacked: `[cyan "⚡"] " jacked in · Net"`; Explore: `camera · {stratum}`; append `Span::styled(" · overburdened", magenta)` when `st.character.overburdened(&st.content)`. Here summary from `sidebar::here_summary(&view)`. Verbs `WALK_VERBS` / `EXPLORE_VERBS`.
6. Footer abilities: `hud::footer_abilities(&slots, st.dev, width)`, replaced by `console::line` when the console is open.
7. Toast as today when `!log_drawn`.

Keys, in `handle_common`:
```rust
            KeyCode::F(n @ 1..=6) => {
                let section = [Section::Here, Section::Quest, Section::Map, Section::Log, Section::Inspect, Section::Legend][n as usize - 1];
                if st.dev || !section.dev_only() { st.sidebar.toggle(section); }
            }
```
VIEW rows: `format!("{mark} {}  F{}", s.label(), i + 1)`.

`help.rs`: add `("F1-F4", "sidebar: here · quest · map · log")` to both lists and `("F5 F6", "inspect · legend (dev mode)")`.

Old tests: update the needles (`"── HERE"` → `"[F1]Here*"`, `"── MAP · street"` → `"· Street"` on the coordinate row, `"── INSPECT"` → the inspector's `pos` line, `"▢ here"` → `"[F1]Here "`, `"tab deck   ? keys"` → `"[tab]deck"`, `"w walk"` → `"[w]alk"`, `"1 netscan ●   2 ──"` stays, `"── LOG"` removed).

`README.md`: Explore and Walk lines gain `F1`-`F4` sidebar panes (`F5` `F6` dev); the HUD paragraph describes the band, the tab row, the panes and the footer zones.

- [ ] **Step 4: Run everything**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings 2>&1 | tail -2 && cargo test --workspace 2>&1 | grep -E "^test result|FAILED"`
Expected: clean, all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cyberpunk README.md
git commit -m "feat(explore): HUD chrome grid and F-key panes"
```

---

### Task 4: Smoke and screenshots

- [ ] Copy scratchpad `drive6.py` to `drive7.py`; change `expect("sidebar list", "▣ here")` to `expect("tabs", "[F1]Here*")`, `expect("quest section", "THE DROP")` stays, add `expect("carry", "CARRY")`, `expect("status zone", "STATUS:")`, `expect("abilities zone", "ABILITIES:")`; replace the `v`/`c` map-toggle pair with `send("\x1b[13~", 0.8); expect("map off", "[F3]Map ")` then `send("\x1b[13~", 0.8)`. Run: `cargo build --release -p cyberpunk && python3 -I drive7.py | tail -1` → all checks pass.
- [ ] `capture4.py` from `capture3.py`, snapshots `chrome_walk`, `chrome_messages`; render, convert, send with `SendUserFile`.
