//! Flavor pack: names and signage glyphs, loaded from RON.

use crate::clinic::Front;
use crate::overworld::{PoiKind, RegionKind};
use crate::params::GenError;
use crate::world::{BuildingKind, Theme};
use rand::Rng;
use rand::seq::IndexedRandom;
use serde::{Deserialize, Serialize};

const DEFAULT_PACK: &str = include_str!("../../../flavor/names.ron");

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NameParts {
    pub prefixes: Vec<String>,
    pub suffixes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorpParts {
    pub first: Vec<String>,
    pub second: Vec<String>,
    pub suffix: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FlavorPack {
    pub city: NameParts,
    pub districts: Vec<(Theme, NameParts)>,
    pub corp: CorpParts,
    pub shops: Vec<(BuildingKind, Vec<String>)>,
    /// A clinic's name by its front, in place of a shop's.
    #[serde(default)]
    pub fronts: Vec<(Front, Vec<String>)>,
    pub landmarks: Vec<String>,
    pub signage: Vec<char>,
    /// Handles for the people of the city.
    #[serde(default)]
    pub people: Vec<String>,
    /// Names out in the world: regions by kind, POIs by kind.
    #[serde(default)]
    pub world: WorldNames,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WorldNames {
    #[serde(default)]
    pub regions: Vec<(RegionKind, Vec<String>)>,
    #[serde(default)]
    pub pois: Vec<(PoiKind, NameParts)>,
}

fn pick<'a>(rng: &mut impl Rng, items: &'a [String]) -> &'a str {
    items.choose(rng).map(String::as_str).unwrap_or("")
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

impl FlavorPack {
    /// Loads a named pack. Only `default` is built in.
    pub fn load(name: &str) -> Result<FlavorPack, GenError> {
        match name {
            "default" => FlavorPack::parse(DEFAULT_PACK),
            other => Err(GenError::Flavor(format!("unknown flavor pack `{other}`"))),
        }
    }

    pub fn parse(text: &str) -> Result<FlavorPack, GenError> {
        let pack: FlavorPack = ron::from_str(text).map_err(|e| GenError::Flavor(e.to_string()))?;
        if pack.signage.is_empty() || pack.city.prefixes.is_empty() || pack.city.suffixes.is_empty()
        {
            return Err(GenError::Flavor(
                "flavor pack is missing required lists".into(),
            ));
        }
        Ok(pack)
    }

    pub fn city_name(&self, rng: &mut impl Rng) -> String {
        format!(
            "{}{}",
            pick(rng, &self.city.prefixes),
            pick(rng, &self.city.suffixes)
        )
    }

    pub fn district_name(&self, theme: Theme, rng: &mut impl Rng) -> String {
        match self.districts.iter().find(|(t, _)| *t == theme) {
            Some((_, parts)) => format!(
                "{}{}",
                pick(rng, &parts.prefixes),
                pick(rng, &parts.suffixes)
            ),
            None => format!("{} {}", theme.label(), rng.random_range(1..100)),
        }
    }

    pub fn corp_name(&self, rng: &mut impl Rng) -> String {
        let stem = format!(
            "{}{}",
            pick(rng, &self.corp.first),
            pick(rng, &self.corp.second)
        );
        format!("{}{}", capitalize(&stem), pick(rng, &self.corp.suffix))
    }

    /// A two-syllable made-up word for shop names.
    fn syllable_word(&self, rng: &mut impl Rng) -> String {
        let stem = format!(
            "{}{}",
            pick(rng, &self.corp.first).to_lowercase(),
            pick(rng, &self.corp.second)
        );
        capitalize(&stem)
    }

    pub fn shop_name(&self, kind: BuildingKind, rng: &mut impl Rng) -> Option<String> {
        let (_, templates) = self.shops.iter().find(|(k, _)| *k == kind)?;
        let template = pick(rng, templates);
        let word = self.syllable_word(rng);
        Some(template.replace("{syl}", &word))
    }

    /// A clinic's name for its front: the same draws as `shop_name`, so the names after
    /// it come out the same.
    pub fn front_name(&self, front: Front, rng: &mut impl Rng) -> Option<String> {
        let (_, templates) = self.fronts.iter().find(|(f, _)| *f == front)?;
        let template = pick(rng, templates);
        let word = self.syllable_word(rng);
        Some(template.replace("{syl}", &word))
    }

    pub fn landmark_name(&self, corp: &str, district: &str, rng: &mut impl Rng) -> String {
        pick(rng, &self.landmarks)
            .replace("{corp}", corp)
            .replace("{district}", district)
    }

    pub fn sign_glyph(&self, rng: &mut impl Rng) -> char {
        *self.signage.choose(rng).unwrap_or(&'#')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::stage_rng;
    use crate::world::{BuildingKind, Theme};

    fn unicode_single_width(c: char) -> bool {
        let u = c as u32;
        u >= 0x20
            && !(0x1100..=0x115F).contains(&u)
            && !(0x2E80..=0xA4CF).contains(&u)
            && !(0xAC00..=0xD7A3).contains(&u)
            && !(0xF900..=0xFAFF).contains(&u)
            && !(0xFE30..=0xFE4F).contains(&u)
            && !(0xFF00..=0xFF60).contains(&u)
            && !(0xFFE0..=0xFFE6).contains(&u)
    }

    #[test]
    fn default_flavor_loads_and_names_are_deterministic() {
        let f = FlavorPack::load("default").unwrap();
        assert!(FlavorPack::load("nope").is_err());
        let mut a = stage_rng(7, 9);
        let mut b = stage_rng(7, 9);
        assert_eq!(f.city_name(&mut a), f.city_name(&mut b));
        for t in Theme::ALL {
            assert!(!f.district_name(t, &mut a).is_empty());
        }
        assert!(f.shop_name(BuildingKind::NoodleBar, &mut a).is_some());
        assert!(f.shop_name(BuildingKind::CorpTower, &mut a).is_none());
        assert!(!f.corp_name(&mut a).is_empty());
        assert!(f.landmark_name("Kirotek", "Apex Plaza", &mut a).len() > 5);
        assert!(f.signage.iter().all(|c| unicode_single_width(*c)));
        assert!(f.signage.len() >= 40);
        let _ = f.sign_glyph(&mut a);
    }

    #[test]
    fn default_flavor_has_people() {
        let f = FlavorPack::load("default").unwrap();
        assert!(f.people.len() >= 40, "{} handles", f.people.len());
        let mut seen = std::collections::HashSet::new();
        assert!(
            f.people.iter().all(|n| seen.insert(n.as_str())),
            "no duplicate handles"
        );
    }
}
