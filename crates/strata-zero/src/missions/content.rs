//! The book: `flavor/missions.ron` parsed and checked at load.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const DEFAULT: &str = include_str!("../../../../flavor/missions.ron");

/// What a `{role.field}` slot may ask of a role.
pub const SLOT_FIELDS: [&str; 4] = ["title", "place", "district", "hours"];

/// The shade letters a face row may hold.
const SHADES: &str = ".dmlhbeX";
/// A face is this many pixels wide.
const FACE_WIDTH: usize = 16;
/// The face a role without one calls with.
const SILHOUETTE: &str = "silhouette";

#[derive(Clone, Debug, Error, PartialEq)]
pub enum BookError {
    #[error("missions parse error: {0}")]
    Parse(String),
    #[error("missions: {0} is empty")]
    Empty(String),
    #[error("missions: {place}: unknown role `{role}`")]
    UnknownRole { place: String, role: String },
    #[error("missions: {place}: unknown slot `{{{slot}}}`")]
    UnknownSlot { place: String, slot: String },
    #[error("missions: {mission}: `Ended({step})` names no earlier step")]
    UnknownStep { mission: String, step: String },
    #[error("missions: {mission}: `Ended({step})` names a step that opens no feed or call")]
    NotAPresentation { mission: String, step: String },
    #[error("missions: no mission `{0}`")]
    UnknownMission(String),
    #[error("missions: {mission}: step `{step}` appears twice")]
    DuplicateStep { mission: String, step: String },
    #[error("missions: face `{face}`: {why}")]
    BadFace { face: String, why: String },
    #[error("missions: the book has no `silhouette` face")]
    NoSilhouette,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename = "Story")]
pub struct Book {
    pub cast: BTreeMap<String, Role>,
    pub faces: BTreeMap<String, Face>,
    pub missions: Vec<Mission>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Role {
    pub name: String,
    pub title: String,
    pub pick: Pick,
    pub face: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Pick {
    NearestFixer,
    Worker {
        professions: Vec<String>,
        at: Vec<citygen::BuildingKind>,
        range: (i32, i32),
    },
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Face {
    pub rows: Vec<String>,
    pub eye: (u8, u8, u8),
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Mission {
    pub id: String,
    pub title: String,
    pub from: String,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Step {
    pub id: String,
    pub when: Trigger,
    pub then: Vec<Effect>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Trigger {
    NewGame,
    Ended(String),
    Sees { role: String, within: i32 },
    After { minutes: u64 },
    At(Where),
    Talk(String),
    MissionDone { mission: String, minutes: u64 },
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Where {
    Home,
    Work { role: String, within: i32 },
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Effect {
    Feed {
        from: String,
        text: Vec<String>,
        close: String,
    },
    Call {
        from: String,
        lines: Vec<String>,
    },
    Give {
        item: String,
        count: u32,
    },
    Take {
        item: String,
        count: u32,
    },
    Dead {
        role: String,
        look: String,
    },
    Objective(String),
    Mail {
        from: String,
        subject: String,
        body: Vec<String>,
    },
    Log(String),
}

impl Effect {
    /// Every piece of text the effect shows, for slots.
    fn texts(&self) -> Vec<&str> {
        match self {
            Effect::Feed { text, close, .. } => text
                .iter()
                .map(String::as_str)
                .chain([close.as_str()])
                .collect(),
            Effect::Call { lines, .. } => lines.iter().map(String::as_str).collect(),
            Effect::Dead { look, .. } => vec![look],
            Effect::Objective(s) | Effect::Log(s) => vec![s],
            Effect::Mail { subject, body, .. } => body
                .iter()
                .map(String::as_str)
                .chain([subject.as_str()])
                .collect(),
            Effect::Give { .. } | Effect::Take { .. } => vec![],
        }
    }

    /// The roles named outside text: who speaks, who dies.
    fn named_roles(&self) -> Vec<&str> {
        match self {
            Effect::Feed { from, .. } | Effect::Call { from, .. } | Effect::Mail { from, .. } => {
                vec![from]
            }
            Effect::Dead { role, .. } => vec![role],
            _ => vec![],
        }
    }

    fn presents(&self) -> bool {
        matches!(self, Effect::Feed { .. } | Effect::Call { .. })
    }
}

impl Trigger {
    fn named_roles(&self) -> Vec<&str> {
        match self {
            Trigger::Sees { role, .. }
            | Trigger::At(Where::Work { role, .. })
            | Trigger::Talk(role) => vec![role],
            _ => vec![],
        }
    }
}

impl Mission {
    /// Every role the mission names: its `from`, triggers, effects and slots.
    pub fn roles(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        out.insert(self.from.clone());
        for step in &self.steps {
            out.extend(step.when.named_roles().into_iter().map(String::from));
            for e in &step.then {
                out.extend(e.named_roles().into_iter().map(String::from));
                for t in e.texts() {
                    out.extend(slots(t).into_iter().map(|(role, _)| role));
                }
            }
        }
        out
    }
}

/// The `{role}` and `{role.field}` slots in a text, in order.
pub fn slots(text: &str) -> Vec<(String, Option<String>)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else { break };
        let slot = &after[..close];
        match slot.split_once('.') {
            Some((role, field)) => out.push((role.to_string(), Some(field.to_string()))),
            None => out.push((slot.to_string(), None)),
        }
        rest = &after[close + 1..];
    }
    out
}

impl Book {
    /// The shipped file.
    pub fn load() -> Result<Book, BookError> {
        Book::parse(DEFAULT)
    }

    /// Any text in the book's shape, checked.
    pub fn parse(src: &str) -> Result<Book, BookError> {
        let book: Book = ron::from_str(src).map_err(|e| BookError::Parse(e.to_string()))?;
        book.validate()?;
        Ok(book)
    }

    pub fn validate(&self) -> Result<(), BookError> {
        if self.cast.is_empty() {
            return Err(BookError::Empty("cast".into()));
        }
        if self.missions.is_empty() {
            return Err(BookError::Empty("missions".into()));
        }
        for (name, face) in &self.faces {
            check_face(name, face)?;
        }
        if !self.faces.contains_key(SILHOUETTE) {
            return Err(BookError::NoSilhouette);
        }
        for (id, role) in &self.cast {
            if let Some(face) = role.face.as_ref().filter(|f| !self.faces.contains_key(*f)) {
                return Err(BookError::BadFace {
                    face: face.clone(),
                    why: format!("role `{id}` names a face the book lacks"),
                });
            }
        }
        for m in &self.missions {
            self.check_mission(m)?;
        }
        Ok(())
    }

    fn check_role(&self, place: &str, role: &str) -> Result<(), BookError> {
        if self.cast.contains_key(role) {
            Ok(())
        } else {
            Err(BookError::UnknownRole {
                place: place.into(),
                role: role.into(),
            })
        }
    }

    fn check_text(&self, place: &str, text: &str) -> Result<(), BookError> {
        for (role, field) in slots(text) {
            self.check_role(place, &role)?;
            if let Some(field) = field.filter(|f| !SLOT_FIELDS.contains(&f.as_str())) {
                return Err(BookError::UnknownSlot {
                    place: place.into(),
                    slot: format!("{role}.{field}"),
                });
            }
        }
        Ok(())
    }

    fn check_mission(&self, m: &Mission) -> Result<(), BookError> {
        if m.steps.is_empty() {
            return Err(BookError::Empty(format!("mission `{}`", m.id)));
        }
        self.check_role(&format!("mission `{}`", m.id), &m.from)?;
        self.check_text(&format!("mission `{}` title", m.id), &m.title)?;
        let mut seen: Vec<&Step> = Vec::new();
        for step in &m.steps {
            let place = format!("{}/{}", m.id, step.id);
            if seen.iter().any(|s| s.id == step.id) {
                return Err(BookError::DuplicateStep {
                    mission: m.id.clone(),
                    step: step.id.clone(),
                });
            }
            match &step.when {
                Trigger::Ended(prev) => match seen.iter().find(|s| &s.id == prev) {
                    None => {
                        return Err(BookError::UnknownStep {
                            mission: m.id.clone(),
                            step: prev.clone(),
                        });
                    }
                    Some(s) if !s.then.iter().any(Effect::presents) => {
                        return Err(BookError::NotAPresentation {
                            mission: m.id.clone(),
                            step: prev.clone(),
                        });
                    }
                    Some(_) => {}
                },
                Trigger::MissionDone { mission, .. } => {
                    if !self.missions.iter().any(|o| &o.id == mission) {
                        return Err(BookError::UnknownMission(mission.clone()));
                    }
                }
                other => {
                    for role in other.named_roles() {
                        self.check_role(&place, role)?;
                    }
                }
            }
            for e in &step.then {
                for role in e.named_roles() {
                    self.check_role(&place, role)?;
                }
                for t in e.texts() {
                    self.check_text(&place, t)?;
                }
                match e {
                    Effect::Feed { text, .. } if text.is_empty() => {
                        return Err(BookError::Empty(format!("feed in {place}")));
                    }
                    Effect::Call { lines, .. } if lines.is_empty() => {
                        return Err(BookError::Empty(format!("call in {place}")));
                    }
                    _ => {}
                }
            }
            seen.push(step);
        }
        Ok(())
    }

    /// Every item a `Give` or `Take` names, for the test that has the content.
    #[cfg(test)]
    pub fn items(&self) -> Vec<&str> {
        let mut out = BTreeSet::new();
        for e in self
            .missions
            .iter()
            .flat_map(|m| &m.steps)
            .flat_map(|s| &s.then)
        {
            if let Effect::Give { item, .. } | Effect::Take { item, .. } = e {
                out.insert(item.as_str());
            }
        }
        out.into_iter().collect()
    }

    /// The role's face, or the silhouette.
    pub fn face_of(&self, role: &str) -> &Face {
        self.cast
            .get(role)
            .and_then(|r| r.face.as_deref())
            .and_then(|f| self.faces.get(f))
            .unwrap_or_else(|| &self.faces[SILHOUETTE])
    }
}

fn check_face(name: &str, face: &Face) -> Result<(), BookError> {
    let bad = |why: String| BookError::BadFace {
        face: name.into(),
        why,
    };
    if face.rows.is_empty() || !face.rows.len().is_multiple_of(2) {
        return Err(bad(format!(
            "{} rows, wanted an even number",
            face.rows.len()
        )));
    }
    for (i, row) in face.rows.iter().enumerate() {
        let width = row.chars().count();
        if width != FACE_WIDTH {
            return Err(bad(format!("row {i} is {width} wide, wanted {FACE_WIDTH}")));
        }
        if let Some(c) = row.chars().find(|c| !SHADES.contains(*c)) {
            return Err(bad(format!("row {i} has unknown shade `{c}`")));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use game::Content;

    fn book() -> Book {
        Book::load().unwrap()
    }

    fn step_mut<'a>(b: &'a mut Book, id: &str) -> &'a mut Step {
        b.missions[0].steps.iter_mut().find(|s| s.id == id).unwrap()
    }

    #[test]
    fn the_shipped_book_parses_and_validates() {
        let b = book();
        assert_eq!(b.missions.len(), 1);
        assert_eq!(b.missions[0].id, "the_drop");
        assert_eq!(b.missions[0].steps.len(), 6);
        assert!(b.cast.contains_key("dez") && b.cast.contains_key("marlo"));
        assert_eq!(b.face_of("dez").eye, (255, 0, 200));
        assert_eq!(b.face_of("marlo"), &b.faces["silhouette"]);
    }

    #[test]
    fn every_item_the_book_names_exists() {
        let c = Content::load().unwrap();
        let b = book();
        assert!(!b.items().is_empty());
        for item in b.items() {
            assert!(
                c.item(&game::ItemId::new(item)).is_some(),
                "no item `{item}`"
            );
        }
    }

    #[test]
    fn slots_split_role_and_field() {
        assert_eq!(
            slots("to {marlo}, at {marlo.place}"),
            vec![
                ("marlo".to_string(), None),
                ("marlo".to_string(), Some("place".to_string()))
            ]
        );
    }

    #[test]
    fn the_drop_needs_dez_and_marlo() {
        let want: BTreeSet<String> = ["dez", "marlo"].map(String::from).into();
        assert_eq!(book().missions[0].roles(), want);
    }

    #[test]
    fn a_feed_from_nobody_is_refused() {
        let mut b = book();
        if let Effect::Feed { from, .. } = &mut step_mut(&mut b, "welcome").then[0] {
            *from = "nobody".into();
        }
        assert!(matches!(b.validate(), Err(BookError::UnknownRole { .. })));
    }

    #[test]
    fn a_slot_for_nobody_is_refused() {
        let mut b = book();
        step_mut(&mut b, "body").then[0] = Effect::Log("{nobody} is dead".into());
        assert!(matches!(b.validate(), Err(BookError::UnknownRole { .. })));
    }

    #[test]
    fn an_unknown_field_is_refused() {
        let mut b = book();
        step_mut(&mut b, "body").then[0] = Effect::Log("{marlo.shoe}".into());
        assert!(matches!(b.validate(), Err(BookError::UnknownSlot { .. })));
    }

    #[test]
    fn ended_must_name_a_step_that_presents() {
        let mut b = book();
        step_mut(&mut b, "ring").when = Trigger::Ended("body".into());
        assert!(matches!(
            b.validate(),
            Err(BookError::NotAPresentation { .. })
        ));
    }

    #[test]
    fn ended_must_name_an_earlier_step() {
        let mut b = book();
        step_mut(&mut b, "ring").when = Trigger::Ended("later".into());
        assert!(matches!(b.validate(), Err(BookError::UnknownStep { .. })));
        let mut b = book();
        step_mut(&mut b, "ring").when = Trigger::Ended("run".into());
        assert!(matches!(b.validate(), Err(BookError::UnknownStep { .. })));
    }

    #[test]
    fn mission_done_must_name_a_mission() {
        let mut b = book();
        step_mut(&mut b, "ring").when = Trigger::MissionDone {
            mission: "nope".into(),
            minutes: 5,
        };
        assert_eq!(b.validate(), Err(BookError::UnknownMission("nope".into())));
    }

    #[test]
    fn step_ids_are_unique() {
        let mut b = book();
        step_mut(&mut b, "run").id = "ring".into();
        assert!(matches!(b.validate(), Err(BookError::DuplicateStep { .. })));
    }

    #[test]
    fn a_short_face_row_is_refused() {
        let mut b = book();
        b.faces.get_mut("hustler").unwrap().rows[3] = "...............".into();
        assert!(matches!(b.validate(), Err(BookError::BadFace { .. })));
    }

    #[test]
    fn the_silhouette_is_required() {
        let mut b = book();
        b.faces.remove("silhouette");
        assert_eq!(b.validate(), Err(BookError::NoSilhouette));
    }

    #[test]
    fn an_empty_feed_is_refused() {
        let mut b = book();
        if let Effect::Feed { text, .. } = &mut step_mut(&mut b, "welcome").then[0] {
            text.clear();
        }
        assert!(matches!(b.validate(), Err(BookError::Empty(_))));
    }
}
