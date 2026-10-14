//! The professions: who works where, when, and what they say. Parsed from
//! `flavor/people.ron` and validated.

use citygen::BuildingKind;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

const DEFAULT_PEOPLE: &str = include_str!("../../../flavor/people.ron");

/// Minutes in a day; hours are stored as minutes of the day.
pub const DAY: u16 = 24 * 60;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum ContentError {
    #[error("people parse error: {0}")]
    Parse(String),
    #[error("duplicate profession `{0}`")]
    DuplicateId(String),
    #[error("profession `{id}` needs at least two lines")]
    TooFewLines { id: String },
    #[error("profession `{id}`: hours must be minutes of a day")]
    BadHours { id: String },
    #[error("profession `{id}`: staff range {lo}..={hi} is backwards")]
    BadStaff { id: String, lo: u8, hi: u8 },
    #[error("profession `{id}`: leisure weight {w} is not within 0..=1")]
    BadWeight { id: String, w: f32 },
}

/// When a profession works. `close` before `open` wraps past midnight.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Hours {
    Day { open: u16, close: u16 },
    Shifts(Vec<(u16, u16)>),
    Beat { from: u16, to: u16 },
}

impl Hours {
    /// Every (start, end) span this profession may keep.
    pub fn spans(&self) -> Vec<(u16, u16)> {
        match self {
            Hours::Day { open, close } => vec![(*open, *close)],
            Hours::Shifts(s) => s.clone(),
            Hours::Beat { from, to } => vec![(*from, *to)],
        }
    }
}

/// Where a leisure trip goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Leisure {
    Bar,
    NoodleBar,
    Strip,
}

/// The colour a profession wears on the map; most wear none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tint {
    Magenta,
    Amber,
    Red,
    Green,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Profession {
    pub id: String,
    pub label: String,
    /// Building kinds that employ it; empty for spot and roaming jobs.
    #[serde(default)]
    pub works_at: Vec<BuildingKind>,
    pub hours: Hours,
    /// Staff per workplace building, inclusive; spot jobs read it per district.
    pub staff: (u8, u8),
    #[serde(default)]
    pub leisure: Vec<(Leisure, f32)>,
    pub lines: Vec<String>,
    #[serde(default)]
    pub night_line: Option<String>,
    #[serde(default)]
    pub tint: Option<Tint>,
    /// Added to the car-ownership roll: positive for the salaried, negative for the broke.
    #[serde(default)]
    pub car_nudge: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct People {
    pub professions: Vec<Profession>,
}

impl People {
    /// Parses the embedded `flavor/people.ron` and validates it.
    pub fn load() -> Result<People, ContentError> {
        Self::parse(DEFAULT_PEOPLE)
    }

    pub fn parse(text: &str) -> Result<People, ContentError> {
        let p: People = ron::from_str(text).map_err(|e| ContentError::Parse(e.to_string()))?;
        p.validate()?;
        Ok(p)
    }

    pub fn validate(&self) -> Result<(), ContentError> {
        let mut seen: HashSet<&str> = HashSet::new();
        for pr in &self.professions {
            if !seen.insert(pr.id.as_str()) {
                return Err(ContentError::DuplicateId(pr.id.clone()));
            }
            if pr.lines.len() < 2 {
                return Err(ContentError::TooFewLines { id: pr.id.clone() });
            }
            if pr.hours.spans().iter().any(|(a, b)| *a >= DAY || *b >= DAY) {
                return Err(ContentError::BadHours { id: pr.id.clone() });
            }
            if pr.staff.0 > pr.staff.1 {
                return Err(ContentError::BadStaff {
                    id: pr.id.clone(),
                    lo: pr.staff.0,
                    hi: pr.staff.1,
                });
            }
            if let Some((_, w)) = pr.leisure.iter().find(|(_, w)| !(0.0..=1.0).contains(w)) {
                return Err(ContentError::BadWeight {
                    id: pr.id.clone(),
                    w: *w,
                });
            }
        }
        Ok(())
    }

    pub fn profession(&self, id: &str) -> Option<&Profession> {
        self.professions.iter().find(|p| p.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn people_content_loads() {
        let p = People::load().unwrap();
        assert_eq!(p.professions.len(), 12);
        let sw = p.profession("sex_worker").unwrap();
        assert_eq!(
            sw.hours,
            Hours::Beat {
                from: 21 * 60,
                to: 5 * 60
            }
        );
        assert_eq!(sw.label, "sex worker");
        assert_eq!(sw.tint, Some(Tint::Magenta));
        let lw = p.profession("line_worker").unwrap();
        assert!(matches!(&lw.hours, Hours::Shifts(s) if s.len() == 3));
        let cd = p.profession("corp_drone").unwrap();
        assert!(
            cd.leisure
                .iter()
                .any(|(l, w)| *l == Leisure::Strip && (*w - 0.15).abs() < 1e-6)
        );
        assert!(
            cd.works_at.contains(&BuildingKind::Office)
                && cd.works_at.contains(&BuildingKind::CorpTower)
        );
        assert!(p.professions.iter().all(|pr| pr.lines.len() >= 2));
        assert!(p.profession("door_muscle").unwrap().night_line.is_some());
        assert!(
            p.profession("shopkeeper")
                .unwrap()
                .works_at
                .contains(&BuildingKind::Pawn)
        );
        assert_eq!(
            p.profession("monk").unwrap().hours,
            Hours::Day {
                open: 6 * 60,
                close: 19 * 60
            }
        );
    }

    #[test]
    fn validation_catches_broken_people() {
        let good = People::load().unwrap();
        let mut c = good.clone();
        c.professions[0].lines.truncate(1);
        assert!(matches!(
            c.validate(),
            Err(ContentError::TooFewLines { .. })
        ));
        let mut c = good.clone();
        c.professions[0].hours = Hours::Day {
            open: 1500,
            close: 10,
        };
        assert!(matches!(c.validate(), Err(ContentError::BadHours { .. })));
        let mut c = good.clone();
        c.professions[0].staff = (3, 1);
        assert!(matches!(c.validate(), Err(ContentError::BadStaff { .. })));
        let mut c = good.clone();
        let dup = c.professions[0].clone();
        c.professions.push(dup);
        assert!(matches!(c.validate(), Err(ContentError::DuplicateId(_))));
        assert!(matches!(
            People::parse("not ron"),
            Err(ContentError::Parse(_))
        ));
    }
}
