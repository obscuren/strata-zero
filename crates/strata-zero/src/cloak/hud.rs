//! The cloak on the HUD (#36): the scrambler's charge on the footer, its CELL row while
//! it cloaks you, the footer's word where ENGAGED would be, the AMBUSH line, and the
//! ability slot that's on.

use super::{DRAIN_MOVING, Phase, Pool, cell, fade_ticks, hidden, pct};
use crate::app::AppState;
use crate::ui::chrome::{CYAN, DIM, MAGENTA};
pub use crate::ui::hud::CloakTag;
use game::{AbilityId, AbilityKind, Active, CopyId, FULL_CELL, ItemId, Source};
use ratatui::style::{Color, Style};
use ratatui::text::Span;

/// The item whose cell the footer reads while idle.
const SCRAMBLER: &str = "field_scrambler";

/// The burnout playing out, if one is: whether its cloak ran on a cell. A fade left over
/// once its time is up (the map wasn't drawn to drop it) is over.
fn burning_out(st: &AppState) -> Option<bool> {
    let f = st.veil.fade.filter(|f| f.phase == Phase::BurningOut)?;
    (st.tick.saturating_sub(f.since) < fade_ticks(f.phase)).then_some(f.cell)
}

/// Whether SIGNAL LOST draws pink this frame, else cyan.
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

/// The scrambler's name as the footer says it.
fn scrambler_name(st: &AppState) -> String {
    (st.content.item(&ItemId::new(SCRAMBLER)))
        .map_or_else(|| "field scrambler".into(), |i| i.name.clone())
}

/// The scrambler's charge on the footer, before the help key: the carried one with the
/// most, dim, or pink and burned out. Nothing without one, in a fight, or while a
/// cloak's word shows: SCRAMBLED with the CELL row, CLOAKED, or SIGNAL LOST say it then.
pub fn footer_charge(st: &AppState) -> Vec<Span<'static>> {
    if footer_tag(st).is_some() || crate::net::engaged_on_foot(st) {
        return Vec::new();
    }
    let Some((_, c)) = cell::best(st, &ItemId::new(SCRAMBLER)) else {
        return Vec::new();
    };
    let words = format!(
        "◌ {} {:>3}%",
        scrambler_name(st).to_lowercase(),
        pct(c.left)
    );
    if c.burned {
        vec![Span::styled(
            format!("{words} · burned out"),
            Style::default().fg(MAGENTA),
        )]
    } else {
        vec![Span::styled(words, Style::default().fg(DIM))]
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

/// Hidden with a standing foe in your sight that your gun or one of your strike
/// abilities reaches: the first hit is an ambush.
pub fn ambush_ready(st: &AppState) -> bool {
    if !hidden(st) {
        return false;
    }
    let gun = crate::combat::your_weapon(st).ok().map(|(_, w)| w.range);
    let strikes =
        (st.character.abilities(&st.content).into_iter()).filter_map(|(id, _)| {
            match st.character.ability(&st.content, &id)?.kind {
                AbilityKind::Active(Active::Strike { range, .. }) => Some(range),
                _ => None,
            }
        });
    let ranges: Vec<i32> = gun.into_iter().chain(strikes).collect();
    (crate::foes::in_sight(st).into_iter())
        .any(|i| (ranges.iter()).any(|&r| crate::combat::reach(st, i, r).is_ok()))
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
    use crate::ui::chrome::MAGENTA;
    use citygen::Pos;
    use game::{Charge, FULL_CELL};

    fn text(spans: &[Span<'static>]) -> String {
        spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn the_scramblers_charge_through_its_life() {
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
        assert_eq!(text(&footer_charge(&st)), "◌ field scrambler  62%");
        fire(&mut st, "scramble").unwrap();
        assert!(
            footer_charge(&st).is_empty(),
            "SCRAMBLED and the CELL row say it"
        );
        assert_eq!(footer_tag(&st), Some(CloakTag::Scrambled));
        set_charge(
            &mut st,
            k,
            Charge {
                left: 1,
                burned: false,
            },
        );
        frames(&mut st, 1);
        assert!(footer_charge(&st).is_empty(), "SIGNAL LOST says it");
        assert!(matches!(footer_tag(&st), Some(CloakTag::SignalLost { .. })));
        frames(&mut st, 72);
        let burned = footer_charge(&st);
        assert_eq!(text(&burned), "◌ field scrambler   0% · burned out");
        assert_eq!(burned[0].style.fg, Some(MAGENTA));
        assert_eq!(footer_tag(&st), None);
    }

    #[test]
    fn a_fight_hides_the_scramblers_charge() {
        let mut st = crate::foes::testutil::lot(Pos::new(10, 8));
        scrambler(&mut st);
        assert!(!footer_charge(&st).is_empty());
        let place = st.player.as_ref().unwrap().place;
        crate::net::engage(&mut st, place, true);
        assert!(crate::net::engaged_on_foot(&st));
        assert!(footer_charge(&st).is_empty(), "it won't go on in a fight");
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
    fn the_focus_cloak_hides_the_scramblers_charge_until_it_is_over() {
        let mut st = street();
        scrambler(&mut st);
        stealthy(&mut st);
        st.character.focus = 1;
        fire(&mut st, "cloak").unwrap();
        assert!(footer_charge(&st).is_empty());
        assert_eq!(footer_tag(&st), Some(CloakTag::Cloaked));
        assert!(
            slot_on(&st, &game::AbilityId::new("cloak"))
                && !slot_on(&st, &game::AbilityId::new("scramble"))
        );
        frames(&mut st, 30);
        assert!(matches!(footer_tag(&st), Some(CloakTag::SignalLost { .. })));
        assert!(footer_charge(&st).is_empty(), "SIGNAL LOST says it");
        frames(&mut st, 72);
        assert_eq!(text(&footer_charge(&st)), "◌ field scrambler 100%");
    }

    #[test]
    fn the_charge_reads_the_fullest_carried_copy() {
        let mut st = street();
        let low = scrambler(&mut st);
        let high = scrambler(&mut st);
        set_charge(
            &mut st,
            low,
            Charge {
                left: FULL_CELL / 5,
                burned: false,
            },
        );
        set_charge(
            &mut st,
            high,
            Charge {
                left: FULL_CELL / 2,
                burned: false,
            },
        );
        assert_eq!(text(&footer_charge(&st)), "◌ field scrambler  50%");
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

    #[test]
    fn ambush_reads_with_a_foe_in_a_strikes_reach() {
        let mut st = crate::foes::testutil::lot(Pos::new(10, 8));
        stealthy(&mut st);
        st.character.focus = 10;
        fire(&mut st, "cloak").unwrap();
        st.character.take_off(game::Slot::Primary);
        assert!(!ambush_ready(&st), "no gun and no strike");
        st.character.give(ItemId::new("neural_port"), 1);
        st.character
            .chrome
            .insert(game::Site::Neural, ItemId::new("neural_port"));
        assert!(
            crate::combat::your_weapon(&st).is_err(),
            "the strike alone, no gun"
        );
        assert!(
            ambush_ready(&st),
            "Static reaches 8 tiles, the goon stands 4 off"
        );
    }

    #[test]
    fn the_charge_is_never_bold() {
        let mut st = street();
        let k = scrambler(&mut st);
        let bold = |st: &AppState| {
            (footer_charge(st).iter()).any(|s| {
                s.style
                    .add_modifier
                    .contains(ratatui::style::Modifier::BOLD)
            })
        };
        assert!(!footer_charge(&st).is_empty() && !bold(&st), "idle");
        set_charge(
            &mut st,
            k,
            Charge {
                left: 0,
                burned: true,
            },
        );
        assert!(!footer_charge(&st).is_empty() && !bold(&st), "burned out");
    }
}
