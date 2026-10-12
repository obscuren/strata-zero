//! Static per-theme generation parameters.

use crate::tile::Hue;
use crate::world::{BuildingKind, HeightClass, Theme};

/// How a district carves its streets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreetStyle {
    Grid,
    Organic,
    Superblock,
    Rows,
    Shattered,
    Strip,
}

/// Tunables for one theme. Fractions are probabilities in 0..1.
#[derive(Clone, Debug)]
pub struct ThemeSpec {
    pub base_wealth: f32,
    pub base_decay: f32,
    pub street_style: StreetStyle,
    /// Target block size (w, h).
    pub block: (i32, i32),
    /// Minimum lot size (w, h).
    pub min_lot: (i32, i32),
    pub mix: &'static [(BuildingKind, u32)],
    pub tall: f32,
    pub mid: f32,
    pub basement: f32,
    pub plaza: f32,
    pub neon: f32,
    pub holo: f32,
    pub lamp_spacing: i32,
    pub security: u8,
    pub hues: &'static [Hue],
}

use BuildingKind as B;

static CORP_CORE: ThemeSpec = ThemeSpec {
    base_wealth: 0.9,
    base_decay: 0.1,
    street_style: StreetStyle::Grid,
    block: (24, 12),
    min_lot: (10, 7),
    mix: &[
        (B::CorpTower, 5),
        (B::Office, 4),
        (B::Hotel, 1),
        (B::Parking, 1),
    ],
    tall: 0.7,
    mid: 0.3,
    basement: 0.6,
    plaza: 0.25,
    neon: 0.3,
    holo: 0.5,
    lamp_spacing: 8,
    security: 200,
    hues: &[Hue::Cyan, Hue::White, Hue::Blue],
};
static NIGHT_MARKET: ThemeSpec = ThemeSpec {
    base_wealth: 0.4,
    base_decay: 0.5,
    street_style: StreetStyle::Organic,
    block: (16, 9),
    min_lot: (5, 4),
    mix: &[
        (B::Shop, 4),
        (B::NoodleBar, 4),
        (B::Bar, 2),
        (B::Market, 2),
        (B::Clinic, 1),
        (B::Pawn, 1),
        (B::Apartment, 2),
    ],
    tall: 0.05,
    mid: 0.3,
    basement: 0.1,
    plaza: 0.3,
    neon: 0.8,
    holo: 0.3,
    lamp_spacing: 12,
    security: 60,
    hues: &[Hue::Magenta, Hue::Orange, Hue::Yellow],
};
static MEGABLOCKS: ThemeSpec = ThemeSpec {
    base_wealth: 0.3,
    base_decay: 0.6,
    street_style: StreetStyle::Superblock,
    block: (40, 20),
    min_lot: (18, 10),
    mix: &[(B::Megablock, 6), (B::Apartment, 3), (B::Shop, 1)],
    tall: 0.9,
    mid: 0.1,
    basement: 0.5,
    plaza: 0.1,
    neon: 0.2,
    holo: 0.2,
    lamp_spacing: 10,
    security: 110,
    hues: &[Hue::Yellow, Hue::White, Hue::Orange],
};
static COMBAT_ZONE: ThemeSpec = ThemeSpec {
    base_wealth: 0.1,
    base_decay: 0.9,
    street_style: StreetStyle::Shattered,
    block: (16, 10),
    min_lot: (5, 4),
    mix: &[
        (B::Apartment, 3),
        (B::Shop, 2),
        (B::Bar, 2),
        (B::Pawn, 2),
        (B::Clinic, 1),
        (B::Garage, 1),
    ],
    tall: 0.05,
    mid: 0.2,
    basement: 0.1,
    plaza: 0.15,
    neon: 0.3,
    holo: 0.05,
    lamp_spacing: 16,
    security: 20,
    hues: &[Hue::Red, Hue::Yellow, Hue::Orange],
};
static DOCKS: ThemeSpec = ThemeSpec {
    base_wealth: 0.3,
    base_decay: 0.6,
    street_style: StreetStyle::Rows,
    block: (30, 14),
    min_lot: (12, 8),
    mix: &[
        (B::Warehouse, 6),
        (B::Depot, 2),
        (B::Bar, 1),
        (B::Garage, 1),
    ],
    tall: 0.0,
    mid: 0.2,
    basement: 0.2,
    plaza: 0.1,
    neon: 0.2,
    holo: 0.05,
    lamp_spacing: 14,
    security: 70,
    hues: &[Hue::Cyan, Hue::Green, Hue::Orange],
};
static INDUSTRIAL: ThemeSpec = ThemeSpec {
    base_wealth: 0.25,
    base_decay: 0.7,
    street_style: StreetStyle::Rows,
    block: (28, 14),
    min_lot: (12, 8),
    mix: &[
        (B::Factory, 5),
        (B::Warehouse, 3),
        (B::Depot, 2),
        (B::Garage, 1),
    ],
    tall: 0.05,
    mid: 0.3,
    basement: 0.4,
    plaza: 0.05,
    neon: 0.15,
    holo: 0.05,
    lamp_spacing: 14,
    security: 90,
    hues: &[Hue::Yellow, Hue::Green, Hue::White],
};
static NEON_STRIP: ThemeSpec = ThemeSpec {
    base_wealth: 0.6,
    base_decay: 0.3,
    street_style: StreetStyle::Strip,
    block: (18, 10),
    min_lot: (6, 4),
    mix: &[
        (B::Club, 4),
        (B::Bar, 3),
        (B::Arcade, 3),
        (B::Hotel, 2),
        (B::Shop, 1),
    ],
    tall: 0.2,
    mid: 0.5,
    basement: 0.2,
    plaza: 0.2,
    neon: 0.95,
    holo: 0.6,
    lamp_spacing: 6,
    security: 120,
    hues: &[Hue::Pink, Hue::Blue, Hue::Magenta],
};
static OLD_TOWN: ThemeSpec = ThemeSpec {
    base_wealth: 0.5,
    base_decay: 0.4,
    street_style: StreetStyle::Organic,
    block: (14, 9),
    min_lot: (5, 4),
    mix: &[
        (B::Apartment, 4),
        (B::Shop, 2),
        (B::Temple, 1),
        (B::NoodleBar, 2),
        (B::Bar, 1),
        (B::Clinic, 1),
    ],
    tall: 0.0,
    mid: 0.25,
    basement: 0.15,
    plaza: 0.25,
    neon: 0.4,
    holo: 0.1,
    lamp_spacing: 12,
    security: 80,
    hues: &[Hue::Orange, Hue::Green, Hue::White],
};

impl Theme {
    pub fn spec(self) -> &'static ThemeSpec {
        match self {
            Theme::CorpCore => &CORP_CORE,
            Theme::NightMarket => &NIGHT_MARKET,
            Theme::Megablocks => &MEGABLOCKS,
            Theme::CombatZone => &COMBAT_ZONE,
            Theme::Docks => &DOCKS,
            Theme::Industrial => &INDUSTRIAL,
            Theme::NeonStrip => &NEON_STRIP,
            Theme::OldTown => &OLD_TOWN,
        }
    }
}

impl BuildingKind {
    /// Height classes this kind may take, preferred first.
    pub fn height_classes(self) -> &'static [HeightClass] {
        match self {
            B::CorpTower | B::Megablock => &[HeightClass::Tall],
            B::Office | B::Hotel => &[HeightClass::Mid, HeightClass::Tall],
            B::Apartment => &[HeightClass::Mid, HeightClass::Low],
            B::Factory | B::Parking => &[HeightClass::Low, HeightClass::Mid],
            _ => &[HeightClass::Low],
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::world::{BuildingKind, HeightClass, Theme};

    #[test]
    fn theme_mix_weights_nonzero() {
        for t in Theme::ALL {
            let s = t.spec();
            assert!(s.mix.iter().map(|(_, w)| *w).sum::<u32>() > 0);
            assert!(s.min_lot.0 >= 4 && s.min_lot.1 >= 3);
            assert!(!s.hues.is_empty());
        }
        assert_eq!(
            BuildingKind::CorpTower.height_classes(),
            &[HeightClass::Tall]
        );
        assert_eq!(
            BuildingKind::Office.height_classes(),
            &[HeightClass::Mid, HeightClass::Tall]
        );
        assert_eq!(BuildingKind::Shop.height_classes(), &[HeightClass::Low]);
    }
}
