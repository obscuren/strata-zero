//! The cloak on the HUD (#36): the scrambler's tag top right, its CELL row while it
//! cloaks you, the footer's word where ENGAGED would be, the AMBUSH line, and the
//! ability slot that's on.

use super::{DRAIN_MOVING, Phase, Pool, cell, fade_ticks, hidden, pct};
use crate::app::AppState;
use crate::ui::chrome::{CYAN, DIM, MAGENTA};
pub use crate::ui::hud::CloakTag;
use game::{AbilityId, AbilityKind, Active, CopyId, FULL_CELL, ItemId, Source};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

/// The item whose cell the tag reads while idle.
const SCRAMBLER: &str = "field_scrambler";

/// The burnout playing out, if one is: whether its cloak ran on a cell. A fade left over
/// once its time is up (the map wasn't drawn to drop it) is over.
fn burning_out(st: &AppState) -> Option<bool> {
    let f = st.veil.fade.filter(|f| f.phase == Phase::BurningOut)?;
    (st.tick.saturating_sub(f.since) < fade_ticks(f.phase)).then_some(f.cell)
}

/// Whether a blinking word draws pink this frame, else cyan.
fn blink(st: &AppState) -> bool {
    (st.tick / 4).is_multiple_of(2)
}

/// Pink below a quarter of the cell, else cyan.
fn charge_colour(left: u32) -> Color {
    if left < FULL_CELL / 4 { MAGENTA } else { CYAN }
}

/// The cell a cloak runs on, if it runs on one.
fn running_cell(st: &AppState) -> Option<CopyId> {
    match st.cloak.as_ref()?.pool {
        Pool::Cell(id) => Some(id),
        Pool::Focus { .. } | Pool::Free { .. } => None,
    }
}

/// The scrambler's name as the tag says it.
fn scrambler_name(st: &AppState) -> String {
    (st.content.item(&ItemId::new(SCRAMBLER)))
        .map_or_else(|| "field scrambler".into(), |i| i.name.clone())
}

/// The tag top right, before the clock: the scrambler's charge while it cloaks you (the
/// copy that runs) or idle (the carried one with the most), FAILING through a burnout
/// (the scrambler's, or the FOCUS cloak's), and nothing with neither.
pub fn tag(st: &AppState) -> Vec<Span<'static>> {
    let name = scrambler_name(st);
    let styled = |s: String, fg: Color| Span::styled(s, Style::default().fg(fg));
    if let Some(id) = running_cell(st) {
        let left = cell::charge(st, id).map_or(0, |c| c.left);
        let bold = |s: String, fg: Color| {
            Span::styled(s, Style::default().fg(fg).add_modifier(Modifier::BOLD))
        };
        return vec![
            bold(format!("◌ {} ", name.to_uppercase()), CYAN),
            bold(format!("{:>3}%", pct(left)), charge_colour(left)),
        ];
    }
    if let Some(on_cell) = burning_out(st) {
        let what = if on_cell {
            name.to_uppercase()
        } else {
            "CLOAK".into()
        };
        let fg = if blink(st) { MAGENTA } else { CYAN };
        return vec![Span::styled(
            format!("◌ {what} ▓▓ FAILING"),
            Style::default().fg(fg).add_modifier(Modifier::BOLD),
        )];
    }
    let Some((_, c)) = cell::best(st, &ItemId::new(SCRAMBLER)) else {
        return Vec::new();
    };
    let words = format!("◌ {} {:>3}%", name.to_lowercase(), pct(c.left));
    if c.burned {
        vec![styled(format!("{words} · burned out"), MAGENTA)]
    } else {
        vec![styled(words, DIM)]
    }
}

/// Cloaked on a scrambler's cell: what's left, the seconds that lasts moving, and the
/// bar's colour.
pub fn cell(st: &AppState) -> Option<(u32, u64, Color)> {
    let left = cell::charge(st, running_cell(st)?)?.left;
    let per_second = u64::from(DRAIN_MOVING * cell::rate(st)) * 30;
    Some((
        left,
        u64::from(left).div_ceil(per_second),
        charge_colour(left),
    ))
}

/// The footer's word: SCRAMBLED or CLOAKED while hidden, SIGNAL LOST through a burnout.
pub fn footer_tag(st: &AppState) -> Option<CloakTag> {
    if hidden(st) {
        return Some(if running_cell(st).is_some() {
            CloakTag::Scrambled
        } else {
            CloakTag::Cloaked
        });
    }
    burning_out(st).map(|_| CloakTag::SignalLost { blink: blink(st) })
}

/// Hidden with a standing foe in your sight that your weapon reaches: the first hit is
/// an ambush.
pub fn ambush_ready(st: &AppState) -> bool {
    if !hidden(st) {
        return false;
    }
    let Ok((_, w)) = crate::combat::your_weapon(st) else {
        return false;
    };
    (crate::foes::in_sight(st).into_iter()).any(|i| crate::combat::reach(st, i, w.range).is_ok())
}

/// Whether `id` is the cloak ability running: on a cell, the one its item grants; on
/// FOCUS, a cloak ability of yours no cell grants. Vanish's free cloak lights none.
pub fn slot_on(st: &AppState, id: &AbilityId) -> bool {
    let Some(c) = st.cloak.as_ref() else {
        return false;
    };
    let is_cloak = st
        .content
        .ability(id)
        .is_some_and(|a| a.kind == AbilityKind::Active(Active::Cloak));
    if !is_cloak {
        return false;
    }
    let source = (st.character.abilities(&st.content).into_iter())
        .find(|(have, _)| have == id)
        .map(|(_, source)| source);
    let from_cell = |item: &ItemId| st.content.item(item).is_some_and(|i| i.cell);
    match (&c.pool, source) {
        (Pool::Cell(copy), Some(Source::Item(item))) => {
            from_cell(&item)
                && st
                    .character
                    .copy_in_pack(*copy)
                    .is_some_and(|s| s.item == item)
        }
        (Pool::Cell(_), _) => false,
        (Pool::Focus { .. }, Some(Source::Item(item))) => !from_cell(&item),
        (Pool::Focus { .. }, Some(_)) => true,
        (Pool::Focus { .. } | Pool::Free { .. }, None) | (Pool::Free { .. }, Some(_)) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloak::testutil::{armed, fire, frames, scrambler, set_charge, stealthy, street};
    use crate::cloak::{Cloak, Pool};
    use crate::ui::chrome::MAGENTA;
    use citygen::Pos;
    use game::{Charge, FULL_CELL};

    fn text(spans: &[Span<'static>]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn the_scramblers_tag_through_its_life() {
        let mut st = street();
        let k = scrambler(&mut st);
        set_charge(
            &mut st,
            k,
            Charge {
                left: FULL_CELL * 62 / 100,
                burned: false,
            },
        );
        assert_eq!(text(&tag(&st)), "◌ field scrambler  62%");
        fire(&mut st, "scramble").unwrap();
        assert_eq!(text(&tag(&st)), "◌ FIELD SCRAMBLER  62%");
        assert_eq!(footer_tag(&st), Some(CloakTag::Scrambled));
        set_charge(
            &mut st,
            k,
            Charge {
                left: FULL_CELL / 5,
                burned: false,
            },
        );
        assert_eq!(
            tag(&st).last().unwrap().style.fg,
            Some(MAGENTA),
            "pink below a quarter"
        );
        set_charge(
            &mut st,
            k,
            Charge {
                left: 1,
                burned: false,
            },
        );
        frames(&mut st, 1);
        assert_eq!(text(&tag(&st)), "◌ FIELD SCRAMBLER ▓▓ FAILING");
        assert!(matches!(footer_tag(&st), Some(CloakTag::SignalLost { .. })));
        frames(&mut st, 72);
        assert_eq!(text(&tag(&st)), "◌ field scrambler   0% · burned out");
        assert_eq!(footer_tag(&st), None);
    }

    #[test]
    fn the_cell_row_counts_down_while_cloaked() {
        let mut st = street();
        scrambler(&mut st);
        assert_eq!(cell(&st), None);
        fire(&mut st, "scramble").unwrap();
        frames(&mut st, 30);
        assert_eq!(cell(&st).map(|c| c.1), Some(19));
    }

    #[test]
    fn the_focus_cloak_has_no_tag_until_it_fails() {
        let mut st = street();
        stealthy(&mut st);
        st.character.focus = 1;
        fire(&mut st, "cloak").unwrap();
        assert!(tag(&st).is_empty());
        assert_eq!(footer_tag(&st), Some(CloakTag::Cloaked));
        assert!(
            slot_on(&st, &game::AbilityId::new("cloak"))
                && !slot_on(&st, &game::AbilityId::new("scramble"))
        );
        frames(&mut st, 30);
        assert_eq!(text(&tag(&st)), "◌ CLOAK ▓▓ FAILING");
    }

    #[test]
    fn the_tag_reads_the_copy_that_runs_and_a_focus_burnout_isnt_the_scramblers() {
        let mut st = street();
        let spare = scrambler(&mut st);
        let running = scrambler(&mut st);
        set_charge(
            &mut st,
            spare,
            Charge {
                left: FULL_CELL,
                burned: false,
            },
        );
        set_charge(
            &mut st,
            running,
            Charge {
                left: FULL_CELL / 2,
                burned: false,
            },
        );
        st.cloak = None;
        // Switch on from the copy at half: `best` would pick the spare, so run it by hand.
        st.cloak = Some(Cloak {
            pool: Pool::Cell(running),
            since: st.tick,
            drained_to: st.tick,
            at: Pos::new(6, 8),
            moved: st.tick,
            waiting: false,
        });
        assert_eq!(
            text(&tag(&st)),
            "◌ FIELD SCRAMBLER  50%",
            "the running copy, not the spare's 100%"
        );
        st.cloak = None;
        stealthy(&mut st);
        st.character.focus = 1;
        fire(&mut st, "cloak").unwrap();
        frames(&mut st, 30);
        assert_eq!(
            text(&tag(&st)),
            "◌ CLOAK ▓▓ FAILING",
            "a FOCUS burnout with scramblers carried"
        );
    }

    #[test]
    fn ambush_reads_with_a_foe_in_your_weapons_reach() {
        let mut st = crate::foes::testutil::lot(Pos::new(10, 8));
        armed(&mut st);
        stealthy(&mut st);
        st.character.focus = 10;
        assert!(!ambush_ready(&st), "not cloaked");
        fire(&mut st, "cloak").unwrap();
        assert!(ambush_ready(&st));
        st.foes[0].down = true;
        assert!(!ambush_ready(&st), "nobody standing in reach");
    }
}
