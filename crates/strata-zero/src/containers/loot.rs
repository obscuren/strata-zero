//! The loot tables (`flavor/loot.ron`) and the pure roll from a seed.

use std::sync::OnceLock;

use citygen::rng::hash64;
use citygen::tile::ContainerKind;
use game::ItemId;
use serde::Deserialize;

use super::state::Loot;

/// A security band: low below 86, mid below 170, high from 170.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Band {
    Low,
    Mid,
    High,
}

pub fn band(security: u8) -> Band {
    if security < 86 {
        Band::Low
    } else if security < 170 {
        Band::Mid
    } else {
        Band::High
    }
}

/// What a line hands out.
#[derive(Clone, Debug, Deserialize)]
pub enum What {
    Cash((u32, u32)),
    /// A sum range and the percent of chips that are locked.
    Chip((u32, u32), u8),
    Item(String, (u32, u32)),
    Card,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Line {
    pub weight: u32,
    /// Empty: every band.
    #[serde(default)]
    pub bands: Vec<Band>,
    pub what: What,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Table {
    pub kind: ContainerKind,
    pub site: Option<String>,
    pub guarded: bool,
    /// Percent of these containers that hold nothing.
    pub empty: u32,
    pub count: (u32, u32),
    pub lines: Vec<Line>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename = "Loot")]
pub struct Tables {
    /// Percent a cash or chip sum is scaled by in each band.
    pub scale: (u32, u32, u32),
    pub tables: Vec<Table>,
}

#[derive(Debug, thiserror::Error)]
pub enum LootError {
    #[error("loot parse error: {0}")]
    Parse(String),
}

/// Which table to roll.
pub struct TableKey<'a> {
    pub kind: ContainerKind,
    /// A venue's `type_id`, or `"plain"`.
    pub site: &'a str,
    /// A job's layer spot.
    pub guarded: bool,
    /// The zone a card line hands out; None: card lines roll nothing.
    pub deeper: Option<u8>,
}

impl Tables {
    pub fn parse(src: &str) -> Result<Tables, LootError> {
        ron::from_str(src).map_err(|e| LootError::Parse(e.to_string()))
    }

    pub fn shipped() -> &'static Tables {
        static T: OnceLock<Tables> = OnceLock::new();
        T.get_or_init(|| {
            Tables::parse(include_str!("../../../../flavor/loot.ron"))
                .expect("embedded loot tables are valid")
        })
    }

    fn find(&self, kind: ContainerKind, site: Option<&str>, guarded: bool) -> Option<&Table> {
        self.tables
            .iter()
            .find(|t| t.kind == kind && t.site.as_deref() == site && t.guarded == guarded)
    }

    fn table_for(&self, key: &TableKey) -> Option<&Table> {
        self.find(key.kind, Some(key.site), key.guarded)
            .or_else(|| self.find(key.kind, None, key.guarded))
            .or_else(|| self.find(key.kind, Some(key.site), false))
            .or_else(|| self.find(key.kind, None, false))
    }

    /// The tables against the content: known items, sane ranges, a fallback per kind.
    pub fn check(&self, content: &game::Content) -> Result<(), String> {
        if self.scale.0 == 0 || self.scale.1 == 0 || self.scale.2 == 0 {
            return Err("a band scales sums to nothing".into());
        }
        for kind in ContainerKind::ALL {
            for guarded in [false, true] {
                if self.find(kind, None, guarded).is_none() {
                    return Err(format!(
                        "no fallback table for {kind:?} (guarded: {guarded})"
                    ));
                }
            }
        }
        for t in &self.tables {
            let who = format!("{:?} {:?} guarded {}", t.kind, t.site, t.guarded);
            if t.empty > 100 {
                return Err(format!("{who}: empty is a percent"));
            }
            if t.count.0 > t.count.1 {
                return Err(format!("{who}: count range runs backwards"));
            }
            if t.count.1 > 0 && t.lines.is_empty() {
                return Err(format!("{who}: counts items but has no lines"));
            }
            for l in &t.lines {
                if l.weight == 0 {
                    return Err(format!("{who}: a line has no weight"));
                }
                let range = match &l.what {
                    What::Cash(r) | What::Chip(r, _) | What::Item(_, r) => Some(*r),
                    What::Card => None,
                };
                if let Some((lo, hi)) = range
                    && lo > hi
                {
                    return Err(format!("{who}: range ({lo}, {hi}) runs backwards"));
                }
                if let What::Chip(_, pct) = &l.what
                    && *pct > 100
                {
                    return Err(format!("{who}: locked is a percent"));
                }
                if let What::Item(id, _) = &l.what
                    && content.item(&ItemId::new(id)).is_none()
                {
                    return Err(format!("{who}: no such item {id}"));
                }
            }
        }
        Ok(())
    }
}

/// What a container holds, rolled from `seed`; pure.
pub fn roll(tables: &Tables, key: &TableKey, security: u8, seed: u64) -> Vec<Loot> {
    let Some(table) = tables.table_for(key) else {
        return Vec::new();
    };
    let mut n = 0u64;
    let mut next = || {
        n += 1;
        hash64(seed, n)
    };
    if next() % 100 < table.empty as u64 {
        return Vec::new();
    }
    let b = band(security);
    let scale = match b {
        Band::Low => tables.scale.0,
        Band::Mid => tables.scale.1,
        Band::High => tables.scale.2,
    } as u64;
    let (clo, chi) = table.count;
    let count = clo + (next() % (chi - clo + 1) as u64) as u32;
    let lines: Vec<&Line> = table
        .lines
        .iter()
        .filter(|l| l.bands.is_empty() || l.bands.contains(&b))
        .collect();
    let total: u64 = lines.iter().map(|l| l.weight as u64).sum();
    let mut out = Vec::new();
    if total == 0 {
        return out;
    }
    let scaled = |base: u64| (base * scale / 100) as u32;
    for _ in 0..count {
        let mut pick = next() % total;
        let Some(line) = lines.iter().find(|l| {
            if pick < l.weight as u64 {
                true
            } else {
                pick -= l.weight as u64;
                false
            }
        }) else {
            continue;
        };
        match &line.what {
            What::Cash((lo, hi)) => {
                let base = *lo as u64 + next() % (*hi - *lo + 1) as u64;
                out.push(Loot::Cash(scaled(base)));
            }
            What::Chip((lo, hi), pct) => {
                let base = *lo as u64 + next() % (*hi - *lo + 1) as u64;
                let locked = next() % 100 < *pct as u64;
                out.push(Loot::Chip {
                    credits: scaled(base),
                    locked,
                });
            }
            What::Item(id, (lo, hi)) => {
                let qty = lo + (next() % (*hi - *lo + 1) as u64) as u32;
                out.push(Loot::Item {
                    item: ItemId::new(id),
                    qty,
                });
            }
            What::Card => {
                if let Some(z) = key.deeper {
                    out.push(Loot::Card(z));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(table: &str) -> Tables {
        Tables::parse(&format!("Loot(scale: (100, 150, 220), tables: [{table}])")).unwrap()
    }

    fn key(deeper: Option<u8>) -> TableKey<'static> {
        TableKey {
            kind: ContainerKind::Locker,
            site: "plain",
            guarded: false,
            deeper,
        }
    }

    /// Fallbacks (plain and guarded) for every kind, leaving out the Crate when asked.
    fn fallbacks(with_crate: bool) -> String {
        let mut s = String::new();
        for k in ["Locker", "Safe", "Cabinet", "Drawer", "Crate"] {
            if k == "Crate" && !with_crate {
                continue;
            }
            for g in ["false", "true"] {
                s += &format!(
                    "Table(kind: {k}, site: None, guarded: {g}, empty: 0, count: (1, 1), lines: [Line(weight: 1, what: Cash((1, 2)))]),"
                );
            }
        }
        s
    }

    #[test]
    fn the_shipped_tables_hold_up() {
        let c = game::Content::load().unwrap();
        Tables::shipped().check(&c).unwrap();
    }

    #[test]
    fn check_finds_an_unknown_item_a_bad_range_and_a_missing_fallback() {
        let c = game::Content::load().unwrap();
        one(&fallbacks(true)).check(&c).unwrap();

        let err = one(&fallbacks(false)).check(&c).unwrap_err();
        assert!(err.contains("Crate"), "{err}");

        let extra = |what: &str| {
            one(&format!(
                "{}Table(kind: Locker, site: Some(\"x\"), guarded: false, empty: 0, count: (1, 1), lines: [Line(weight: 1, what: {what})]),",
                fallbacks(true)
            ))
        };
        let err = extra("Item(\"no_such_item\", (1, 1))")
            .check(&c)
            .unwrap_err();
        assert!(err.contains("no_such_item"), "{err}");
        let err = extra("Cash((9, 3))").check(&c).unwrap_err();
        assert!(err.contains("(9, 3)"), "{err}");
    }

    #[test]
    fn a_roll_is_the_same_twice_and_another_seed_rolls_otherwise() {
        let key = TableKey {
            kind: ContainerKind::Safe,
            site: "office",
            guarded: true,
            deeper: Some(3),
        };
        let t = Tables::shipped();
        assert_eq!(roll(t, &key, 200, 7), roll(t, &key, 200, 7));
        assert!((0..20).any(|s| roll(t, &key, 200, s) != roll(t, &key, 200, 7)));
    }

    #[test]
    fn bands_scale_sums_and_pick_lines() {
        let t = one(
            "Table(kind: Locker, site: None, guarded: false, empty: 0, count: (1, 1), lines: [Line(weight: 1, what: Cash((100, 100)))])",
        );
        assert_eq!(roll(&t, &key(None), 10, 1), vec![Loot::Cash(100)]);
        assert_eq!(roll(&t, &key(None), 100, 1), vec![Loot::Cash(150)]);
        assert_eq!(roll(&t, &key(None), 200, 1), vec![Loot::Cash(220)]);

        let t = one(
            "Table(kind: Locker, site: None, guarded: false, empty: 0, count: (1, 1), lines: [Line(weight: 1, bands: [High], what: Cash((5, 5)))])",
        );
        for s in 0..30 {
            assert!(roll(&t, &key(None), 10, s).is_empty());
        }
        assert!(!roll(&t, &key(None), 200, 1).is_empty());
    }

    #[test]
    fn a_card_line_needs_a_deeper_zone() {
        let t = one(
            "Table(kind: Locker, site: None, guarded: false, empty: 0, count: (1, 1), lines: [Line(weight: 1, what: Card)])",
        );
        assert_eq!(roll(&t, &key(Some(2)), 10, 1), vec![Loot::Card(2)]);
        assert!(roll(&t, &key(None), 10, 1).is_empty());
    }

    #[test]
    fn band_edges() {
        assert_eq!(band(85), Band::Low);
        assert_eq!(band(86), Band::Mid);
        assert_eq!(band(169), Band::Mid);
        assert_eq!(band(170), Band::High);
    }
}
