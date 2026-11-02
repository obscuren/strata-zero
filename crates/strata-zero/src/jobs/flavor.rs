//! The fixers' voice: titles, briefs and lines, parsed from `flavor/jobs.ron`.

use serde::{Deserialize, Serialize};
use thiserror::Error;

const DEFAULT: &str = include_str!("../../../../flavor/jobs.ron");
const SLOTS: [&str; 12] = [
    "building",
    "floor",
    "number",
    "how",
    "host",
    "security",
    "name",
    "profession",
    "place",
    "hours",
    "district",
    "start",
];

/// A job's kind without its target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KindTag {
    Fetch,
    Copy,
    Plant,
    Deliver,
}

impl KindTag {
    pub const ALL: [KindTag; 4] = [
        KindTag::Fetch,
        KindTag::Copy,
        KindTag::Plant,
        KindTag::Deliver,
    ];

    pub fn label(self) -> &'static str {
        match self {
            KindTag::Fetch => "fetch",
            KindTag::Copy => "copy",
            KindTag::Plant => "plant",
            KindTag::Deliver => "deliver",
        }
    }
}

/// Which of the fixer's lines is wanted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Say {
    Offer,
    Taken,
    Handed,
    Dropped,
    Late,
    Refused,
    Nothing,
    Receiver,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum FlavorError {
    #[error("jobs parse error: {0}")]
    Parse(String),
    #[error("jobs: `{0}` is empty")]
    Empty(String),
    #[error("jobs: {where_}: unknown slot `{{{slot}}}`")]
    UnknownSlot { where_: String, slot: String },
    #[error("jobs: kind {0:?} has no titles or briefs")]
    MissingKind(KindTag),
}

/// How a Fetch's codes can be had, in the brief.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct How {
    pub card: String,
    pub server: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename = "Jobs")]
pub struct Flavor {
    pub titles: Vec<(KindTag, Vec<String>)>,
    pub briefs: Vec<(KindTag, Vec<Vec<String>>)>,
    pub how: How,
    #[serde(rename = "where")]
    pub where_: String,
    pub offer: Vec<String>,
    pub taken: Vec<String>,
    pub handed: Vec<String>,
    pub dropped: Vec<String>,
    pub late: Vec<String>,
    pub refused: Vec<String>,
    pub nothing: Vec<String>,
    pub receiver: Vec<String>,
}

/// Replaces every `{slot}` that has a value; unknown braces stay as written.
pub fn fill(template: &str, slots: &[(&str, String)]) -> String {
    let mut out = template.to_string();
    for (k, v) in slots {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

fn pick(list: &[String], roll: u64) -> &str {
    list.get((roll % list.len().max(1) as u64) as usize)
        .map_or("", String::as_str)
}

impl Flavor {
    pub fn load() -> Result<Flavor, FlavorError> {
        let f: Flavor = ron::from_str(DEFAULT).map_err(|e| FlavorError::Parse(e.to_string()))?;
        f.validate()?;
        Ok(f)
    }

    pub fn validate(&self) -> Result<(), FlavorError> {
        for (name, list) in [
            ("offer", &self.offer),
            ("taken", &self.taken),
            ("handed", &self.handed),
            ("dropped", &self.dropped),
            ("late", &self.late),
            ("refused", &self.refused),
            ("nothing", &self.nothing),
            ("receiver", &self.receiver),
        ] {
            if list.is_empty() {
                return Err(FlavorError::Empty(name.into()));
            }
        }
        for k in KindTag::ALL {
            let titles = self.titles.iter().find(|(t, _)| *t == k).map(|(_, l)| l);
            let briefs = self.briefs.iter().find(|(t, _)| *t == k).map(|(_, l)| l);
            match (titles, briefs) {
                (Some(t), Some(b)) if !t.is_empty() && !b.is_empty() => {
                    for s in t {
                        check_slots(&format!("title {k:?}"), s)?;
                    }
                    for lines in b {
                        for s in lines {
                            check_slots(&format!("brief {k:?}"), s)?;
                        }
                    }
                }
                _ => return Err(FlavorError::MissingKind(k)),
            }
        }
        check_slots("where", &self.where_)
    }

    pub fn title(&self, kind: KindTag, roll: u64, slots: &[(&str, String)]) -> String {
        let list = self
            .titles
            .iter()
            .find(|(t, _)| *t == kind)
            .map(|(_, l)| l.as_slice())
            .unwrap_or(&[]);
        fill(pick(list, roll), slots)
    }

    pub fn brief(&self, kind: KindTag, roll: u64, slots: &[(&str, String)]) -> Vec<String> {
        let list = self
            .briefs
            .iter()
            .find(|(t, _)| *t == kind)
            .map(|(_, l)| l.as_slice())
            .unwrap_or(&[]);
        list.get((roll % list.len().max(1) as u64) as usize)
            .map(|lines| lines.iter().map(|l| fill(l, slots)).collect())
            .unwrap_or_default()
    }

    pub fn how_card(&self) -> &str {
        &self.how.card
    }

    pub fn how_server(&self) -> &str {
        &self.how.server
    }

    pub fn where_line(&self, slots: &[(&str, String)]) -> String {
        fill(&self.where_, slots)
    }

    pub fn line(&self, which: Say, roll: u64) -> &str {
        let list = match which {
            Say::Offer => &self.offer,
            Say::Taken => &self.taken,
            Say::Handed => &self.handed,
            Say::Dropped => &self.dropped,
            Say::Late => &self.late,
            Say::Refused => &self.refused,
            Say::Nothing => &self.nothing,
            Say::Receiver => &self.receiver,
        };
        pick(list, roll)
    }
}

/// Every `{word}` in `s` must be a known slot.
fn check_slots(where_: &str, s: &str) -> Result<(), FlavorError> {
    let mut rest = s;
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}') else {
            break;
        };
        let slot = &rest[start + 1..start + end];
        if !SLOTS.contains(&slot) {
            return Err(FlavorError::UnknownSlot {
                where_: where_.into(),
                slot: slot.into(),
            });
        }
        rest = &rest[start + end + 1..];
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pack_parses_and_every_kind_has_titles_and_briefs() {
        let f = Flavor::load().unwrap();
        for k in KindTag::ALL {
            let t = f.title(
                k,
                0,
                &[
                    ("building", "Hollis Court".into()),
                    ("host", "meridian".into()),
                    ("name", "Ana".into()),
                    ("floor", "11F".into()),
                ],
            );
            assert!(!t.is_empty() && !t.contains('{'), "{k:?}: {t}");
            assert!(f.brief(k, 1, &[]).len() >= 2);
        }
        assert_eq!(f.line(Say::Refused, 0), "Lose this number.");
        assert_eq!(f.line(Say::Refused, 1), "I don't know you.");
        assert_eq!(f.line(Say::Refused, 2), "Lose this number.");
        assert_eq!(f.how_card(), "the lobby desk keeps a card");
        assert_eq!(
            f.where_line(&[("district", "Old Town".into()), ("start", "18:00".into())]),
            "I hold the corner on Old Town from 18:00."
        );
    }

    #[test]
    fn fill_replaces_known_slots_and_leaves_text_alone() {
        assert_eq!(
            fill(
                "{number} on {floor}",
                &[("number", "1104".into()), ("floor", "11F".into())]
            ),
            "1104 on 11F"
        );
        assert_eq!(fill("no slots", &[]), "no slots");
    }

    #[test]
    fn validation_refuses_an_unknown_slot_and_an_empty_list() {
        let mut f = Flavor::load().unwrap();
        f.offer.clear();
        assert!(matches!(f.validate(), Err(FlavorError::Empty(_))));
        let mut f = Flavor::load().unwrap();
        f.where_ = "see you at {moon}".into();
        assert!(matches!(f.validate(), Err(FlavorError::UnknownSlot { .. })));
        let mut f = Flavor::load().unwrap();
        f.titles.retain(|(k, _)| *k != KindTag::Plant);
        assert_eq!(f.validate(), Err(FlavorError::MissingKind(KindTag::Plant)));
    }
}
