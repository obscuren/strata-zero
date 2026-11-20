//! What the rig's reader takes: chips it reads or cracks, program shards it seats,
//! data it sifts and story shards it opens. What it does with each is told by
//! `summon_word`.

use crate::app::AppState;
use crate::missions::ReadAt;
use game::{ItemId, Stack};
use ratatui::style::Color;

/// What the reader makes of an item it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holds {
    /// An open credchip: its sum comes off.
    Chip,
    /// A locked credchip, cracked at the reader.
    Locked,
    /// The black chip: only the rig opens it.
    Black,
    /// A program shard, `sniff` or `keepalive`.
    Program(&'static str),
    /// Paydata or a vault dump, sifted by the rig.
    Data,
    /// A shard with a reading at the rig.
    Story,
}

/// What the reader makes of `item`, or none when it won't take it.
pub fn holds(st: &AppState, item: &ItemId) -> Option<Holds> {
    match item.0.as_str() {
        "credchip" => Some(Holds::Chip),
        "credchip_locked" => Some(Holds::Locked),
        "credchip_black" => Some(Holds::Black),
        "prog_sniff" => Some(Holds::Program("sniff")),
        "prog_keepalive" => Some(Holds::Program("keepalive")),
        "paydata" | "vault_data" => Some(Holds::Data),
        id => st
            .book
            .readings
            .get(id)
            .is_some_and(|rs| rs.iter().any(|r| r.at == ReadAt::Rig))
            .then_some(Holds::Story),
    }
}

/// A slot's mark: the kind in a glyph and a colour, none of them the Net's own marks
/// (`◇ ⬡ ⌗ ▪ ▮ ▯ ◈`), and `◆` only for the black chip.
pub fn glyph(h: Holds) -> (char, Color) {
    match h {
        Holds::Chip => ('¤', Color::Rgb(0xff, 0xb0, 0x00)),
        Holds::Locked => ('▣', Color::Rgb(0x9f, 0xe6, 0xff)),
        Holds::Black => ('◆', Color::Rgb(0xff, 0x28, 0x48)),
        Holds::Program(_) => ('λ', Color::Rgb(0x00, 0xe5, 0xff)),
        Holds::Data => ('≡', Color::Rgb(0xc8, 0x9b, 0xff)),
        Holds::Story => ('¶', Color::Rgb(0xe6, 0xf0, 0xf5)),
    }
}

/// How many locks a chip of `credits` has: a bigger sum is better held.
pub fn locks(credits: u32) -> u8 {
    match credits {
        0..500 => 1,
        500..2000 => 2,
        _ => 3,
    }
}

/// What the reader would do with `stack`, in a few words: `+¤340`, `install keepalive`,
/// `sift · 6h`, `read`, `2 locks`, `MASTIFF`.
pub fn summon_word(st: &AppState, stack: &Stack) -> String {
    let credits = stack.copy.as_ref().map_or(0, |k| k.credits);
    match holds(st, &stack.item) {
        None => String::new(),
        Some(Holds::Chip) => format!("+¤{}", crate::ui::commas(credits)),
        Some(Holds::Locked) => {
            let cracked = stack.copy.as_ref().map_or(0, |k| k.cracked);
            let left = locks(credits).saturating_sub(cracked);
            format!("{left} {}", if left == 1 { "lock" } else { "locks" })
        }
        Some(Holds::Black) => "MASTIFF".into(),
        Some(Holds::Program(p)) => {
            let verb = if st.backdoors.jobs.programs.contains(p) {
                "run"
            } else {
                "install"
            };
            format!("{verb} {p}")
        }
        Some(Holds::Data) if stack.item.0 == "vault_data" => "sift · 10h".into(),
        Some(Holds::Data) => "sift · 6h".into(),
        Some(Holds::Story) => {
            let verb = crate::missions::reading(st, &stack.item.0, ReadAt::Rig)
                .map(|r| crate::missions::fill(st, &r.verb))
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "Read".into());
            verb.to_lowercase()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game::ItemId;

    #[test]
    fn the_reader_takes_its_kinds() {
        let st = crate::app::AppState::new();
        let h = |id: &str| holds(&st, &ItemId::new(id));
        assert_eq!(h("credchip"), Some(Holds::Chip));
        assert_eq!(h("credchip_locked"), Some(Holds::Locked));
        assert_eq!(h("credchip_black"), Some(Holds::Black));
        assert_eq!(h("prog_keepalive"), Some(Holds::Program("keepalive")));
        assert_eq!(h("prog_sniff"), Some(Holds::Program("sniff")));
        assert_eq!(h("paydata"), Some(Holds::Data));
        assert_eq!(h("vault_data"), Some(Holds::Data));
        assert_eq!(h("paydata_sifted"), None);
        assert_eq!(h("stim_patch"), None);
        assert_eq!(h("sled_tinman"), None);
        assert_eq!(h("ledger_shard"), Some(Holds::Story));
    }

    #[test]
    fn locks_come_from_the_sum() {
        for (sum, n) in [(0, 1), (499, 1), (500, 2), (1999, 2), (2000, 3)] {
            assert_eq!(locks(sum), n, "{sum}");
        }
    }

    #[test]
    fn glyphs_stay_off_the_nets_marks() {
        let all = [
            Holds::Chip,
            Holds::Locked,
            Holds::Black,
            Holds::Program("sniff"),
            Holds::Data,
            Holds::Story,
        ];
        let colours = [
            (0xff, 0xb0, 0x00),
            (0x9f, 0xe6, 0xff),
            (0xff, 0x28, 0x48),
            (0x00, 0xe5, 0xff),
            (0xc8, 0x9b, 0xff),
            (0xe6, 0xf0, 0xf5),
        ];
        for (h, (r, g, b)) in all.into_iter().zip(colours) {
            assert_eq!(glyph(h).1, Color::Rgb(r, g, b), "{h:?}");
            let c = glyph(h).0;
            assert!(!"◇⬡⌗▪▮▯◈".contains(c), "{h:?}");
            assert!(c != '◆' || h == Holds::Black, "{h:?}");
        }
        assert_eq!(glyph(Holds::Chip).0, '¤');
    }

    #[test]
    fn summon_words_say_what_the_reader_does() {
        let mut st = crate::app::AppState::new();
        let c = st.content.clone();
        let give = |st: &mut crate::app::AppState, id: &str, credits: u32, cracked: u8| {
            st.character.give_item(&c, ItemId::new(id), 1);
            let mut s = st
                .character
                .inventory
                .iter()
                .rev()
                .find(|s| s.item.0 == id)
                .cloned()
                .unwrap();
            if let Some(k) = s.copy.as_mut() {
                k.credits = credits;
                k.cracked = cracked;
            }
            s
        };
        let chip = give(&mut st, "credchip", 340, 0);
        let locked = give(&mut st, "credchip_locked", 800, 1);
        let black = give(&mut st, "credchip_black", 3000, 0);
        let keep = give(&mut st, "prog_keepalive", 0, 0);
        let pay = give(&mut st, "paydata", 0, 0);
        let vault = give(&mut st, "vault_data", 0, 0);
        let shard = give(&mut st, "ledger_shard", 0, 0);
        assert_eq!(summon_word(&st, &chip), "+¤340");
        assert_eq!(summon_word(&st, &locked), "1 lock");
        assert_eq!(summon_word(&st, &black), "MASTIFF");
        assert_eq!(summon_word(&st, &keep), "install keepalive");
        st.backdoors.jobs.programs.insert("keepalive".into());
        assert_eq!(summon_word(&st, &keep), "run keepalive");
        assert_eq!(summon_word(&st, &pay), "sift · 6h");
        assert_eq!(summon_word(&st, &vault), "sift · 10h");
        assert_eq!(summon_word(&st, &shard), "read");
    }
}
