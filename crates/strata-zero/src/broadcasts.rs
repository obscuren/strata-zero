//! What the city says as you drop in: the broadcasts in `flavor/broadcasts.ron`, which
//! one plays, and its slots filled from the city being entered.

use citygen::rng::hash64;
use citygen::{City, Theme};
use ratatui::style::Color;
use serde::Deserialize;
use thiserror::Error;

const DEFAULT: &str = include_str!("../../../flavor/broadcasts.ron");

/// The slots a broadcast may use, and the district kind each theme slot names.
const SLOTS: [&str; 5] = ["city", "city.loud", "you", "home", "corp"];
const THEMES: [(&str, Theme); 8] = [
    ("market", Theme::NightMarket),
    ("zone", Theme::CombatZone),
    ("blocks", Theme::Megablocks),
    ("docks", Theme::Docks),
    ("core", Theme::CorpCore),
    ("strip", Theme::NeonStrip),
    ("industry", Theme::Industrial),
    ("oldtown", Theme::OldTown),
];
/// What `{corp}` says in a city without a named corp tower.
const NO_CORP: &str = "Orodyne";
/// What `{home}` says in a city without a home or a district to stand in for it.
const NO_HOME: &str = "the block";
/// What a district slot says in a city without districts.
const NO_DISTRICT: &str = "the Street";

/// When a broadcast plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum When {
    NewGame,
    LoadGame,
}

/// A colour as the file writes it, `(r, g, b)`.
pub type Rgb8 = (u8, u8, u8);
/// A span of the tag line and its colour; none is the text colour.
pub type TagSpan = (String, Option<Rgb8>);

/// One broadcast as the file has it: who is on air, what they shout, say and sign off
/// with, and when it plays.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Broadcast {
    pub id: String,
    pub when: When,
    /// The tag line's spans; a span without a colour is the text colour.
    pub tag: Vec<TagSpan>,
    pub shout: String,
    pub paras: Vec<String>,
    pub sign: String,
    pub sign_colour: Rgb8,
}

/// Every broadcast, in file order.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Broadcasts(pub Vec<Broadcast>);

#[derive(Clone, Debug, Error, PartialEq)]
pub enum BroadcastError {
    #[error("broadcasts parse error: {0}")]
    Parse(String),
    #[error("broadcasts: {0} is empty")]
    Empty(String),
    #[error("broadcasts: {id}: unknown slot `{{{slot}}}`")]
    UnknownSlot { id: String, slot: String },
    #[error("broadcasts: two broadcasts are `{0}`")]
    Duplicate(String),
    #[error("broadcasts: none plays on {0:?}")]
    Missing(When),
}

/// A broadcast with its slots filled, ready to draw.
#[derive(Clone, Debug, PartialEq)]
pub struct Aired {
    pub tag: Vec<(String, Option<Color>)>,
    pub shout: String,
    pub paras: Vec<String>,
    pub sign: String,
    pub sign_colour: Color,
}

impl Broadcasts {
    /// The shipped file.
    pub fn load() -> Result<Broadcasts, BroadcastError> {
        Broadcasts::parse(DEFAULT)
    }

    /// Any text in the file's shape, checked: unique ids, no empty text, known slots,
    /// and something to play on a new game and on a load.
    pub fn parse(src: &str) -> Result<Broadcasts, BroadcastError> {
        let all: Broadcasts =
            ron::from_str(src).map_err(|e| BroadcastError::Parse(e.to_string()))?;
        for (i, b) in all.0.iter().enumerate() {
            if b.id.is_empty() {
                return Err(BroadcastError::Empty(format!("broadcast {}'s id", i + 1)));
            }
            if all.0[..i].iter().any(|o| o.id == b.id) {
                return Err(BroadcastError::Duplicate(b.id.clone()));
            }
            let texts = b
                .tag
                .iter()
                .map(|(t, _)| ("tag", t))
                .chain([("shout", &b.shout), ("sign", &b.sign)])
                .chain(b.paras.iter().map(|p| ("paragraph", p)));
            for (what, text) in texts {
                if text.trim().is_empty() {
                    return Err(BroadcastError::Empty(format!("{}'s {what}", b.id)));
                }
                if let Some(slot) = slots(text).into_iter().find(|s| !known(s)) {
                    return Err(BroadcastError::UnknownSlot {
                        id: b.id.clone(),
                        slot,
                    });
                }
            }
            if b.paras.is_empty() {
                return Err(BroadcastError::Empty(format!("{}'s paragraphs", b.id)));
            }
        }
        for when in [When::NewGame, When::LoadGame] {
            if !all.0.iter().any(|b| b.when == when) {
                return Err(BroadcastError::Missing(when));
            }
        }
        Ok(all)
    }

    /// The broadcast for `when` in the city with `seed`: the same city always hears the
    /// same one.
    pub fn pick(&self, when: When, seed: u64) -> &Broadcast {
        let fits: Vec<&Broadcast> = self.0.iter().filter(|b| b.when == when).collect();
        fits[(hash64(seed, 0xB40A_DCA5) % fits.len() as u64) as usize]
    }
}

/// The `{slot}`s in a text, as written.
fn slots(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else { break };
        out.push(after[..close].to_string());
        rest = &after[close + 1..];
    }
    out
}

fn known(slot: &str) -> bool {
    let slot = slot.to_lowercase();
    SLOTS.contains(&slot.as_str()) || THEMES.iter().any(|(name, _)| *name == slot)
}

/// `b` with its slots filled for `city` and the runner called `handle`.
pub fn air(b: &Broadcast, city: &City, handle: &str) -> Aired {
    let fill = |t: &str| fill(t, city, handle);
    Aired {
        tag: b
            .tag
            .iter()
            .map(|(t, c)| (fill(t), c.map(|(r, g, b)| Color::Rgb(r, g, b))))
            .collect(),
        shout: fill(&b.shout),
        paras: b.paras.iter().map(|p| fill(p)).collect(),
        sign: fill(&b.sign),
        sign_colour: Color::Rgb(b.sign_colour.0, b.sign_colour.1, b.sign_colour.2),
    }
}

/// A text with its slots filled; a slot in capitals fills in capitals, and one the
/// file cannot have stays as written.
pub fn fill(text: &str, city: &City, handle: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else { break };
        out.push_str(&rest[..open]);
        let slot = &after[..close];
        match slot_text(&slot.to_lowercase(), city, handle) {
            Some(text) if slot.chars().any(char::is_uppercase) => {
                out.push_str(&text.to_uppercase())
            }
            Some(text) => out.push_str(&text),
            None => {
                out.push('{');
                out.push_str(slot);
                out.push('}');
            }
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

fn slot_text(slot: &str, city: &City, handle: &str) -> Option<String> {
    Some(match slot {
        "city" => city.name.clone(),
        "city.loud" => loud(&city.name),
        "you" => handle.to_string(),
        "home" => home(city),
        "corp" => corp(city),
        _ => {
            let (_, theme) = THEMES.iter().find(|(name, _)| *name == slot)?;
            district(city, *theme, slot)
        }
    })
}

/// A number for a slot's name, so each slot picks on its own from the city's seed.
fn salt(slot: &str) -> u64 {
    slot.bytes()
        .fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(u64::from(b)))
}

fn pick<T>(city: &City, slot: &str, from: &[T]) -> Option<usize> {
    (!from.is_empty()).then(|| (hash64(city.seed, salt(slot)) % from.len() as u64) as usize)
}

/// A district of `theme`'s name, else any district's.
fn district(city: &City, theme: Theme, slot: &str) -> String {
    let of: Vec<&str> = city
        .districts
        .iter()
        .filter(|d| d.theme == theme)
        .map(|d| d.name.as_str())
        .collect();
    let any: Vec<&str> = city.districts.iter().map(|d| d.name.as_str()).collect();
    let from = if of.is_empty() { any } else { of };
    pick(city, slot, &from).map_or_else(|| NO_DISTRICT.to_string(), |i| from[i].to_string())
}

/// The name of the building with your flat, else a district of blocks.
fn home(city: &City) -> String {
    citygen::home(city)
        .and_then(|h| city.building_opt(h.building))
        .and_then(|b| b.name.clone())
        .or_else(|| (!city.districts.is_empty()).then(|| district(city, Theme::Megablocks, "home")))
        .unwrap_or_else(|| NO_HOME.to_string())
}

/// A corp tower's name.
fn corp(city: &City) -> String {
    let corps: Vec<&str> = city
        .buildings
        .iter()
        .filter(|b| b.kind.is_corp())
        .filter_map(|b| b.name.as_deref())
        .collect();
    pick(city, "corp", &corps).map_or_else(|| NO_CORP.to_string(), |i| corps[i].to_string())
}

/// The name shouted: capitals, with its last two vowels that come before a consonant
/// held four times. HOLLOWSIDE is HOLLOOOOWSIIIIDE.
pub fn loud(name: &str) -> String {
    let chars: Vec<char> = name.to_uppercase().chars().collect();
    let vowel = |c: char| "AEIOU".contains(c);
    // The last letter of each vowel run that a consonant follows.
    let held: Vec<usize> = (0..chars.len())
        .filter(|&i| {
            vowel(chars[i])
                && chars
                    .get(i + 1)
                    .is_some_and(|&n| n.is_alphabetic() && !vowel(n))
        })
        .collect();
    let held = &held[held.len().saturating_sub(2)..];
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        out.push(c);
        if held.contains(&i) {
            out.extend([c; 3]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, SizePreset};

    fn small(seed: u64) -> City {
        Generator::run(GenParams {
            seed,
            size: SizePreset::Custom { w: 80, h: 40 },
            districts: 4,
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn the_shipped_broadcasts_load() {
        let all = Broadcasts::load().unwrap();
        let ids: Vec<&str> = all.0.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, ["static66", "nitewire", "block_pa"]);
        let mut loads = std::collections::BTreeSet::new();
        for seed in 1..20 {
            assert_eq!(all.pick(When::NewGame, seed).id, "static66");
            let id = &all.pick(When::LoadGame, seed).id;
            assert!(id == "nitewire" || id == "block_pa", "{id}");
            assert_eq!(&all.pick(When::LoadGame, seed).id, id, "a city hears one");
            loads.insert(id.clone());
        }
        assert_eq!(loads.len(), 2, "both loads play somewhere");
    }

    #[test]
    fn bad_books_are_refused() {
        let one = |id: &str, when: &str, shout: &str| {
            format!(
                r#"Broadcast(id: "{id}", when: {when}, tag: [("ON", None)], shout: "{shout}",
                   paras: ["words"], sign: "bye", sign_colour: (1, 2, 3))"#
            )
        };
        let book = |items: &[String]| format!("Broadcasts([{}])", items.join(","));
        let ok = [one("a", "NewGame", "hi"), one("b", "LoadGame", "hi")];
        assert!(Broadcasts::parse(&book(&ok)).is_ok());
        assert_eq!(
            Broadcasts::parse(&book(&[one("a", "NewGame", "hi {nope}"), ok[1].clone()])),
            Err(BroadcastError::UnknownSlot {
                id: "a".into(),
                slot: "nope".into()
            })
        );
        assert_eq!(
            Broadcasts::parse(&book(&[ok[0].clone(), one("a", "LoadGame", "hi")])),
            Err(BroadcastError::Duplicate("a".into()))
        );
        assert!(matches!(
            Broadcasts::parse(&book(&[one("a", "NewGame", " "), ok[1].clone()])),
            Err(BroadcastError::Empty(_))
        ));
        assert_eq!(
            Broadcasts::parse(&book(&[ok[0].clone()])),
            Err(BroadcastError::Missing(When::LoadGame))
        );
    }

    #[test]
    fn loud_holds_the_last_two_vowels_before_a_consonant() {
        assert_eq!(loud("Hollowside"), "HOLLOOOOWSIIIIDE");
        assert_eq!(loud("Newreach"), "NEEEEWREAAAACH");
        assert_eq!(loud("Ashreach"), "AAAASHREAAAACH");
        assert_eq!(loud("Greymarch"), "GREEEEYMAAAARCH");
    }

    #[test]
    fn slots_fill_from_the_city() {
        let city = small(2);
        assert_eq!(
            fill("{city} {CITY} {you}", &city, "Vex"),
            format!("{} {} Vex", city.name, city.name.to_uppercase())
        );
        let market = fill("{market}", &city, "Vex");
        assert!(
            city.districts.iter().any(|d| d.name == market),
            "{market} is one of the city's districts"
        );
        assert_eq!(
            fill("{market}", &city, "Vex"),
            market,
            "the same every time"
        );
        for b in &Broadcasts::load().unwrap().0 {
            let aired = air(b, &city, "Vex");
            let all = [aired.shout.clone(), aired.sign.clone()]
                .into_iter()
                .chain(aired.paras.clone())
                .chain(aired.tag.iter().map(|(t, _)| t.clone()));
            for text in all {
                assert!(!text.contains('{'), "{}: {text}", b.id);
            }
        }
    }

    #[test]
    fn slots_fall_back_when_the_city_lacks_them() {
        let lot = City::lot(1);
        for slot in SLOTS.iter().chain(THEMES.iter().map(|(n, _)| n)) {
            let text = fill(&format!("{{{slot}}}"), &lot, "Vex");
            assert!(!text.contains('{') && !text.is_empty(), "{slot}: {text}");
        }
        assert_eq!(fill("{corp}", &lot, "Vex"), NO_CORP);
        if lot.districts.is_empty() {
            assert_eq!(fill("{home}", &lot, "Vex"), NO_HOME);
            assert_eq!(fill("{zone}", &lot, "Vex"), NO_DISTRICT);
        }
    }
}
