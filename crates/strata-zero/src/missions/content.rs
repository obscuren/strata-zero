//! The book: `flavor/missions.ron` parsed and checked at load.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const DEFAULT: &str = include_str!("../../../../flavor/missions.ron");

/// What a `{role.field}` slot may ask of a role.
pub const SLOT_FIELDS: [&str; 5] = ["title", "spaced", "place", "district", "hours"];
/// What a `{role.field}` slot may ask of a visitor, who works nowhere.
pub const VISITOR_FIELDS: [&str; 2] = ["title", "spaced"];
/// The fields that ask where a person is, so they need one in the city.
const PLACE_FIELDS: [&str; 3] = ["place", "district", "hours"];

/// The shade letters a face row may hold.
const SHADES: &str = ".dmlhbeX";
/// A face is this many pixels wide.
const FACE_WIDTH: usize = 16;
/// The face a role without one calls with.
const SILHOUETTE: &str = "silhouette";
/// What a news still's rows may hold: the camera's greens dark to light, the bracket's red.
const STILL_PIXELS: &str = "0123456r";
/// A news still is this many pixels a side.
const STILL_SIDE: usize = 18;
/// The player, as a beat's speaker or walker and as the `{you}` slot. No cast role takes it.
pub const YOU: &str = "you";
/// The `{code}` slot in a mail: the code it carries. No cast role takes it.
pub const CODE: &str = "code";
/// What a save says a mission waits on once it is past its last step. No step takes it.
pub const END: &str = "end";

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
    #[error("missions: {place}: unknown scene `{scene}`")]
    UnknownScene { place: String, scene: String },
    #[error("missions: {place}: `{role}` is not a visitor")]
    NotAVisitor { place: String, role: String },
    #[error("missions: {place}: {what}")]
    BadNumber { place: String, what: String },
    #[error("missions: {place}: {why}")]
    BadFits { place: String, why: String },
    #[error("missions: no role may be called `{0}`")]
    ReservedRole(String),
    #[error("missions: {mission}: no step may be called `{step}`")]
    ReservedStep { mission: String, step: String },
    #[error("missions: scene `{0}` has no beats")]
    EmptyScene(String),
    #[error("missions: scene `{0}` comes back to itself through `next`")]
    SceneLoop(String),
    #[error("missions: {place}: unknown news `{news}`")]
    UnknownNews { place: String, news: String },
    #[error("missions: news `{news}`: still {why}")]
    BadStill { news: String, why: String },
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename = "Story")]
pub struct Book {
    pub cast: BTreeMap<String, Role>,
    pub faces: BTreeMap<String, Face>,
    pub missions: Vec<Mission>,
    #[serde(default)]
    pub scenes: BTreeMap<String, Scene>,
    #[serde(default)]
    pub news: BTreeMap<String, News>,
    /// What who you are finds in an item, by item id (doctor spec §3).
    #[serde(default)]
    pub readings: BTreeMap<String, Vec<Reading>>,
}

/// Where an item is read: at your rig, at your bench, or by its Look.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum ReadAt {
    Rig,
    Bench,
    Look,
}

/// A reading of an item at one place, for whoever its need holds for. A `Look` reading
/// is only `text`, read in place of the item's description; at the rig and the bench it
/// is offered as `verb` and shown as a card of `rows` under `head`. Every text takes
/// the usual slots.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Reading {
    pub at: ReadAt,
    pub need: Need,
    #[serde(default)]
    pub verb: String,
    #[serde(default)]
    pub head: String,
    #[serde(default)]
    pub rows: Vec<Row>,
    /// The label after `space`.
    #[serde(default)]
    pub close: String,
    /// Showing it records `Need::Read` of the item.
    #[serde(default)]
    pub tells: bool,
    #[serde(default)]
    pub text: String,
}

/// One row of a reading's card. Backticked spans in a field's text and in art stand out.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Row {
    /// A label in the gutter, then its text.
    Field(String, String),
    /// A line in alarm.
    Hot(String),
    /// Somebody's mark, led by `▚▞`.
    Tag(String),
    /// A drawing.
    Art(String),
    /// What you make of it.
    You(String),
    /// `shown` entries of a ledger of `total`, seeded by the city and the item.
    Ledger(u32, u32),
    Blank,
}

impl Reading {
    /// Every piece of text it shows, for slots.
    fn texts(&self) -> Vec<&str> {
        let mut out = vec![
            self.verb.as_str(),
            self.head.as_str(),
            self.close.as_str(),
            self.text.as_str(),
        ];
        for row in &self.rows {
            match row {
                Row::Field(label, text) => out.extend([label.as_str(), text.as_str()]),
                Row::Hot(t) | Row::Tag(t) | Row::Art(t) | Row::You(t) => out.push(t),
                Row::Ledger(..) | Row::Blank => {}
            }
        }
        out
    }
}

/// A news item: `flavor/missions.ron` `news`, put out by `Effect::News`. Its texts take
/// the usual slots.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct News {
    /// The role whose body the camera saw.
    pub about: String,
    pub headline: String,
    /// The broadcast's paragraphs.
    pub report: Vec<String>,
    /// What the set in your flat says, in turn.
    pub reads: Vec<String>,
    /// The ticker along the broadcast's foot.
    pub crawl: Vec<String>,
    /// What people in the street say of it, in turn.
    pub overheard: Vec<String>,
    /// Who is sought, and what they carried.
    pub sought: String,
    pub carrying: String,
    /// The camera that took the still.
    pub camera: String,
    /// `STILL_SIDE` rows of `STILL_SIDE` pixels from `STILL_PIXELS`.
    pub still: Vec<String>,
    /// The label after `space` in the broadcast.
    pub close: String,
    /// Game minutes it stays current.
    pub fresh: u64,
}

impl News {
    /// Every piece of text the item shows, for slots.
    fn texts(&self) -> impl Iterator<Item = &str> {
        [
            &self.headline,
            &self.sought,
            &self.carrying,
            &self.camera,
            &self.close,
        ]
        .into_iter()
        .chain(&self.report)
        .chain(&self.reads)
        .chain(&self.crawl)
        .chain(&self.overheard)
        .map(String::as_str)
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Role {
    pub name: String,
    pub title: String,
    pub pick: Pick,
    pub face: Option<String>,
    /// A speaker's colour, and a visitor's `@`.
    #[serde(default)]
    pub tint: Option<(u8, u8, u8)>,
    /// What look mode says of a visitor.
    #[serde(default)]
    pub look: Option<String>,
}

/// How a scene is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Look {
    Film,
    Bubbles,
}

/// A scene's timing, all data: seconds, or characters per second for typing.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Pace {
    /// Characters per second a line types at.
    #[serde(rename = "type")]
    pub chars: f32,
    /// Seconds per tile a walk takes.
    pub walk: f32,
    /// Seconds a typed line stays before the scene moves on; `None` waits for space.
    pub hold: Option<f32>,
}

/// A scripted cinematic: `flavor/missions.ron` `scenes`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Scene {
    pub title: String,
    pub look: Look,
    pub pace: Pace,
    pub beats: Vec<Beat>,
    /// The scene that plays straight after this one, for the same step: the step
    /// ends with the last.
    #[serde(default)]
    pub next: Option<String>,
}

/// Where a beat points, resolved when it runs.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Mark {
    You,
    Role(String),
    HomeDoor,
    HomeOutside,
    HomeInside(u32),
    HomeRig,
    HomeBed,
    HomeFurniture(citygen::Furniture),
    Beside(Box<Mark>),
    /// The operating chair in the role's work building, on its floor.
    Chair(String),
}

/// The code a scripted keypad types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum PadCode {
    Journal,
    Code(u16),
}

/// What the scripted keypad's display ends on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum PadResult {
    Denied,
    Open,
}

fn yes() -> bool {
    true
}

/// Who you are, as the story asks it (doctor spec §2): `missions::holds` answers.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Need {
    /// You have the skill.
    Skill(String),
    /// A reading of the item that tells has been shown this story.
    Read(String),
    /// You carry at least one.
    Carries(String),
    /// The story has unsealed the item.
    Unsealed(String),
    /// The jacket the camera saw is known and isn't what you wear now.
    Changed,
    /// The jacket the camera saw is known and is what you wear now.
    Unchanged,
    /// Now is past the first time the clock showed this minute of the day after the
    /// mission began.
    Late(u16),
    /// The build you chose: your profession's id.
    Build(String),
    /// The mission is done.
    Done(String),
    Not(Box<Need>),
    All(Vec<Need>),
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Beat {
    Say {
        who: String,
        text: String,
        #[serde(default, rename = "as")]
        label: Option<String>,
        #[serde(default)]
        at: Option<Mark>,
        #[serde(default)]
        hold: Option<f32>,
        /// Said only when it holds; else skipped at once.
        #[serde(default)]
        need: Option<Need>,
    },
    Stage(String),
    Walk {
        who: String,
        to: Mark,
        #[serde(default)]
        pace: Option<f32>,
        #[serde(default = "yes")]
        wait: bool,
    },
    Wait(f32),
    Keypad {
        door: Mark,
        code: PadCode,
        result: PadResult,
        keys: f32,
        hold: f32,
    },
    Door {
        at: Mark,
        open: bool,
    },
    Camera(Mark),
    Arrive {
        role: String,
        at: Mark,
    },
    Leave(String),
    Lights(f32),
    /// Leaves a line under the Look of the mark's tile; a newer one replaces it.
    Note {
        at: Mark,
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Pick {
    NearestFixer,
    /// The one of this profession (an id) whose work tile is nearest the home door.
    Nearest(String),
    Worker {
        professions: Vec<String>,
        at: Vec<citygen::BuildingKind>,
        range: (i32, i32),
    },
    /// Never cast from the sim: a scene or an effect brings them.
    Visitor,
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
    Sees {
        role: String,
        within: i32,
    },
    After {
        minutes: u64,
    },
    At(Where),
    Talk(String),
    MissionDone {
        mission: String,
        minutes: u64,
    },
    Chromed(String),
    /// Fires on any pass once the need holds.
    Holds(Need),
    /// Never fires: the story stands here until a later version gives the step a real
    /// trigger.
    Later,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum Where {
    Home,
    Work {
        role: String,
        within: i32,
    },
    /// Home: in your flat, or outside it within one tile of its door (the door included).
    HomeDoor,
    /// Within `within` tiles (Manhattan) of the role's person on the same place: the
    /// street where the sim has them, or the floor that draws them.
    By {
        role: String,
        within: i32,
    },
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
    /// Lets the pack open the item.
    Unseal(String),
    Dead {
        role: String,
        look: String,
    },
    Objective(String),
    /// Pushes the text of the first fit whose need holds, as `Objective` does.
    ObjectiveFit(Vec<Fit>),
    /// Records `Need::Read(item)` as if a telling reading had been shown: someone told
    /// you whose it is.
    Told(String),
    Mail {
        from: String,
        subject: String,
        body: Vec<String>,
        /// A code the mail carries: learned as it lands, and its `{code}` slot.
        #[serde(default)]
        code: Option<CodeOf>,
    },
    Log(String),
    /// Plays the scene; its end marks the step `Ended`, as a feed's does.
    Scene(String),
    Arrive {
        role: String,
        at: Mark,
    },
    Leave(String),
    Lights(f32),
    /// The role's line, said through your flat's holo set while you are home.
    Holo {
        from: String,
        text: String,
    },
    /// The role's line, torn through a holo ad on the street once you have walked off;
    /// its showing marks the step `Ended`, as a feed's close does, and thirty game
    /// minutes with none shown end it unshown.
    Ad {
        from: String,
        text: String,
    },
    /// Leaves a line under the Look of the mark's tile; a newer one replaces it.
    Note {
        at: Mark,
        text: String,
    },
    /// Puts out the news item; its end marks the step `Ended`, as a feed's does.
    News(String),
    /// The screen swims for this many seconds.
    Daze(f32),
    /// Keeps the role's person at work, against their hours.
    Hold(String),
    /// Lets the role's person go, the first minute you are out of their work building.
    Release(String),
    /// Keeps the role's person where they are now, against their hours, until `Release`.
    Stay(String),
    /// Adds credits, silently.
    Pay(u32),
    /// Adds `by` to your standing with the role's person, a fixer.
    Standing {
        role: String,
        by: i32,
    },
    /// The `from` role holds out the first set whose need holds: one piece to take, on
    /// the `tab` role's tab, which pays for it in `from`'s chair.
    Tray {
        from: String,
        tab: String,
        title: String,
        line: String,
        sets: Vec<TraySet>,
    },
}

/// An objective's text for who you are: shown when its need holds.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Fit {
    pub need: Need,
    pub text: String,
}

/// The pieces a tray holds when its need does.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct TraySet {
    pub need: Need,
    pub items: Vec<String>,
}

/// A tray's rows take the keys 1 to 9.
const TRAY_MAX: usize = 9;

/// What a mail's code opens.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub enum CodeOf {
    /// The street doors of the role's work building.
    Clinic(String),
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
            Effect::ObjectiveFit(fits) => fits.iter().map(|f| f.text.as_str()).collect(),
            Effect::Holo { text, .. } | Effect::Ad { text, .. } | Effect::Note { text, .. } => {
                vec![text]
            }
            Effect::Tray { title, line, .. } => vec![title, line],
            Effect::Mail { subject, body, .. } => body
                .iter()
                .map(String::as_str)
                .chain([subject.as_str()])
                .collect(),
            Effect::Give { .. }
            | Effect::Take { .. }
            | Effect::Unseal(_)
            | Effect::Told(_)
            | Effect::Scene(_)
            | Effect::Arrive { .. }
            | Effect::Leave(_)
            | Effect::Lights(_)
            | Effect::News(_)
            | Effect::Daze(_)
            | Effect::Hold(_)
            | Effect::Release(_)
            | Effect::Stay(_)
            | Effect::Pay(_)
            | Effect::Standing { .. } => vec![],
        }
    }

    /// The roles named outside text: who speaks, who dies, who arrives (but you), who is
    /// held or let go, whose building a mailed code opens, and who holds out a tray on
    /// whose tab.
    fn named_roles(&self) -> Vec<&str> {
        match self {
            Effect::Mail {
                from,
                code: Some(CodeOf::Clinic(role)),
                ..
            } => vec![from, role],
            Effect::Tray { from, tab, .. } => vec![from, tab],
            Effect::Feed { from, .. }
            | Effect::Call { from, .. }
            | Effect::Mail { from, .. }
            | Effect::Holo { from, .. }
            | Effect::Ad { from, .. } => {
                vec![from]
            }
            Effect::Arrive { role, .. } if role == YOU => vec![],
            Effect::Dead { role, .. }
            | Effect::Arrive { role, .. }
            | Effect::Leave(role)
            | Effect::Hold(role)
            | Effect::Release(role)
            | Effect::Stay(role)
            | Effect::Standing { role, .. } => vec![role],
            _ => vec![],
        }
    }

    fn presents(&self) -> bool {
        matches!(
            self,
            Effect::Feed { .. }
                | Effect::Call { .. }
                | Effect::Scene(_)
                | Effect::News(_)
                | Effect::Mail { .. }
                | Effect::Ad { .. }
        )
    }
}

impl Trigger {
    fn named_roles(&self) -> Vec<&str> {
        match self {
            Trigger::Sees { role, .. }
            | Trigger::At(Where::Work { role, .. } | Where::By { role, .. })
            | Trigger::Talk(role)
            | Trigger::Chromed(role) => vec![role],
            _ => vec![],
        }
    }
}

impl Beat {
    /// The marks the beat points at.
    fn marks(&self) -> Vec<&Mark> {
        match self {
            Beat::Say { at, .. } => at.iter().collect(),
            Beat::Walk { to, .. } => vec![to],
            Beat::Keypad { door, .. } => vec![door],
            Beat::Door { at, .. }
            | Beat::Camera(at)
            | Beat::Arrive { at, .. }
            | Beat::Note { at, .. } => vec![at],
            Beat::Stage(_) | Beat::Wait(_) | Beat::Leave(_) | Beat::Lights(_) => vec![],
        }
    }
}

impl Mark {
    /// The role the mark needs a person in the city for: a chair's doc.
    fn cast_role(&self) -> Option<&str> {
        match self {
            Mark::Chair(role) => Some(role),
            Mark::Beside(inner) => inner.cast_role(),
            _ => None,
        }
    }
}

impl Mission {
    /// Every role the mission names: its `from`, triggers, effects and slots.
    #[cfg(test)]
    pub fn roles(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        out.insert(self.from.clone());
        for step in &self.steps {
            out.extend(step.when.named_roles().into_iter().map(String::from));
            for e in &step.then {
                out.extend(e.named_roles().into_iter().map(String::from));
                for t in e.texts() {
                    out.extend(
                        slots(t)
                            .into_iter()
                            .map(|(role, _)| role)
                            .filter(|r| r != YOU && r != CODE),
                    );
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

/// `{code}`, as a mail's text says it.
pub fn code_slot() -> String {
    format!("{{{CODE}}}")
}

/// The roles a text asks the place, district or hours of.
fn placed(text: &str) -> impl Iterator<Item = String> {
    slots(text)
        .into_iter()
        .filter(|(_, f)| f.as_deref().is_some_and(|f| PLACE_FIELDS.contains(&f)))
        .map(|(role, _)| role)
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
        if let Some(reserved) = [YOU, CODE].into_iter().find(|r| self.cast.contains_key(*r)) {
            return Err(BookError::ReservedRole(reserved.into()));
        }
        for (id, role) in &self.cast {
            if let Some(face) = role.face.as_ref().filter(|f| !self.faces.contains_key(*f)) {
                return Err(BookError::BadFace {
                    face: face.clone(),
                    why: format!("role `{id}` names a face the book lacks"),
                });
            }
        }
        for (id, scene) in &self.scenes {
            self.check_scene(id, scene)?;
        }
        for (id, news) in &self.news {
            self.check_news(id, news)?;
        }
        for m in &self.missions {
            self.check_mission(m)?;
        }
        for (item, readings) in &self.readings {
            for (i, r) in readings.iter().enumerate() {
                self.check_reading(&format!("reading {} of {item}", i + 1), r)?;
            }
        }
        Ok(())
    }

    /// A visitor is never cast from the sim.
    pub fn is_visitor(&self, role: &str) -> bool {
        self.cast
            .get(role)
            .is_some_and(|r| matches!(r.pick, Pick::Visitor))
    }

    /// The roles a mission needs the sim to cast: those a trigger waits on, an effect acts
    /// on, a mark lies by, a scene's speaker or a news item is about, and any whose
    /// place, district or hours a text asks. A bare name needs nobody. Visitors and `you`
    /// are never cast.
    pub fn needs_cast(&self, m: &Mission) -> BTreeSet<String> {
        let mut roles = BTreeSet::new();
        for step in &m.steps {
            roles.extend(step.when.named_roles().into_iter().map(String::from));
            for e in &step.then {
                for t in e.texts() {
                    roles.extend(placed(t));
                }
                // One arm per effect that acts on a person or points at a mark.
                match e {
                    Effect::Dead { role, .. }
                    | Effect::Hold(role)
                    | Effect::Release(role)
                    | Effect::Stay(role)
                    | Effect::Standing { role, .. }
                    | Effect::Tray { from: role, .. }
                    | Effect::Mail {
                        code: Some(CodeOf::Clinic(role)),
                        ..
                    } => {
                        roles.insert(role.clone());
                    }
                    Effect::Arrive { at, .. } | Effect::Note { at, .. } => {
                        roles.extend(at.cast_role().map(String::from));
                    }
                    Effect::Scene(id) => self.scene_needs(id, &mut roles),
                    Effect::News(id) => {
                        if let Some(news) = self.news.get(id) {
                            roles.insert(news.about.clone());
                            for t in news.texts() {
                                roles.extend(placed(t));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        roles
            .into_iter()
            .filter(|r| r != YOU && !self.is_visitor(r))
            .collect()
    }

    /// The roles a scene and the scenes after it need: speakers, marks and placed slots.
    fn scene_needs(&self, id: &str, roles: &mut BTreeSet<String>) {
        let mut next = Some(id);
        // A `next` chain is checked to end, but a bound costs nothing.
        for _ in 0..=self.scenes.len() {
            let Some(scene) = next.and_then(|n| self.scenes.get(n)) else {
                return;
            };
            for beat in &scene.beats {
                roles.extend(
                    beat.marks()
                        .into_iter()
                        .filter_map(Mark::cast_role)
                        .map(String::from),
                );
                match beat {
                    Beat::Say {
                        who, text, label, ..
                    } => {
                        roles.insert(who.clone());
                        roles.extend(placed(text));
                        roles.extend(label.iter().flat_map(|l| placed(l)));
                    }
                    Beat::Stage(text) | Beat::Note { text, .. } => roles.extend(placed(text)),
                    _ => {}
                }
            }
            next = scene.next.as_deref();
        }
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
            if role == YOU {
                // The player's handle takes no field.
                match field {
                    Some(f) => {
                        return Err(BookError::UnknownSlot {
                            place: place.into(),
                            slot: format!("{role}.{f}"),
                        });
                    }
                    None => continue,
                }
            }
            self.check_role(place, &role)?;
            let fields: &[&str] = if self.is_visitor(&role) {
                &VISITOR_FIELDS
            } else {
                &SLOT_FIELDS
            };
            if let Some(field) = field.filter(|f| !fields.contains(&f.as_str())) {
                return Err(BookError::UnknownSlot {
                    place: place.into(),
                    slot: format!("{role}.{field}"),
                });
            }
        }
        Ok(())
    }

    /// A role that must be a visitor.
    fn check_visitor(&self, place: &str, role: &str) -> Result<(), BookError> {
        self.check_role(place, role)?;
        if self.is_visitor(role) {
            Ok(())
        } else {
            Err(BookError::NotAVisitor {
                place: place.into(),
                role: role.into(),
            })
        }
    }

    /// Who an `Arrive` puts on the map: a visitor, or you (a cut).
    fn check_arriving(&self, place: &str, role: &str) -> Result<(), BookError> {
        if role == YOU {
            Ok(())
        } else {
            self.check_visitor(place, role)
        }
    }

    /// A mark: its roles are visitors (a chair's is in the cast), `HomeInside` goes in at
    /// least a tile.
    fn check_mark(&self, place: &str, mark: &Mark) -> Result<(), BookError> {
        match mark {
            Mark::Role(role) => self.check_visitor(place, role),
            Mark::Chair(role) => self.check_role(place, role),
            Mark::HomeInside(0) => Err(BookError::BadNumber {
                place: place.into(),
                what: "HomeInside(0) is the door itself".into(),
            }),
            Mark::Beside(inner) => self.check_mark(place, inner),
            _ => Ok(()),
        }
    }

    fn check_scene(&self, id: &str, scene: &Scene) -> Result<(), BookError> {
        let at = format!("scene {id}");
        if scene.beats.is_empty() {
            return Err(BookError::EmptyScene(id.into()));
        }
        // A title is printed raw in the film's bar and the bubbles' footer: no slots.
        if scene.title.contains('{') {
            return Err(BookError::UnknownSlot {
                place: format!("title of {at}"),
                slot: scene.title.clone(),
            });
        }
        positive(&at, "type", scene.pace.chars)?;
        positive(&at, "walk", scene.pace.walk)?;
        if let Some(h) = scene.pace.hold {
            positive(&at, "hold", h)?;
        }
        for (i, beat) in scene.beats.iter().enumerate() {
            let place = format!("{at} beat {}", i + 1);
            self.check_beat(&place, beat)?;
        }
        // Each `next` is a scene, and following them ends.
        let mut next = scene.next.as_deref();
        for _ in 0..self.scenes.len() {
            let Some(n) = next else {
                return Ok(());
            };
            if n == id {
                return Err(BookError::SceneLoop(id.into()));
            }
            let Some(s) = self.scenes.get(n) else {
                return Err(BookError::UnknownScene {
                    place: format!("next of {at}"),
                    scene: n.into(),
                });
            };
            next = s.next.as_deref();
        }
        Ok(())
    }

    fn check_beat(&self, place: &str, beat: &Beat) -> Result<(), BookError> {
        match beat {
            Beat::Say {
                who,
                text,
                label,
                at,
                hold,
                need,
            } => {
                // `you` may speak; every other speaker is in the cast.
                if who != YOU {
                    self.check_role(place, who)?;
                }
                self.check_text(place, text)?;
                if let Some(label) = label {
                    self.check_text(place, label)?;
                }
                if let Some(mark) = at {
                    self.check_mark(place, mark)?;
                }
                if let Some(h) = hold {
                    positive(place, "hold", *h)?;
                }
                if let Some(need) = need {
                    self.check_need(place, need)?;
                }
            }
            Beat::Stage(text) => self.check_text(place, text)?,
            Beat::Walk { who, to, pace, .. } => {
                if who != YOU {
                    self.check_visitor(place, who)?;
                }
                self.check_mark(place, to)?;
                if let Some(p) = pace {
                    positive(place, "pace", *p)?;
                }
            }
            Beat::Wait(s) => positive(place, "wait", *s)?,
            Beat::Keypad {
                door, keys, hold, ..
            } => {
                self.check_mark(place, door)?;
                positive(place, "keys", *keys)?;
                positive(place, "hold", *hold)?;
            }
            Beat::Door { at, .. } | Beat::Camera(at) => self.check_mark(place, at)?,
            Beat::Arrive { role, at } => {
                self.check_arriving(place, role)?;
                self.check_mark(place, at)?;
            }
            Beat::Leave(role) => self.check_visitor(place, role)?,
            Beat::Lights(level) => check_level(place, *level)?,
            Beat::Note { at, text } => {
                self.check_text(place, text)?;
                self.check_mark(place, at)?;
            }
        }
        Ok(())
    }

    fn check_news(&self, id: &str, news: &News) -> Result<(), BookError> {
        let at = format!("news `{id}`");
        self.check_role(&at, &news.about)?;
        let empty = [
            ("headline", news.headline.is_empty()),
            ("report", news.report.is_empty()),
            ("reads", news.reads.is_empty()),
            ("overheard", news.overheard.is_empty()),
            ("fresh", news.fresh == 0),
        ];
        if let Some((what, _)) = empty.iter().find(|(_, is)| *is) {
            return Err(BookError::Empty(format!("{what} in {at}")));
        }
        for t in news.texts() {
            self.check_text(&at, t)?;
        }
        check_still(id, &news.still)
    }

    /// A reading: its need, its slots, the verb that offers it at the rig or the bench or
    /// the text a Look reads, and ledgers that show no more than they hold.
    fn check_reading(&self, place: &str, r: &Reading) -> Result<(), BookError> {
        self.check_need(place, &r.need)?;
        let (what, said) = match r.at {
            ReadAt::Look => ("text", &r.text),
            ReadAt::Rig | ReadAt::Bench => ("verb", &r.verb),
        };
        if said.is_empty() {
            return Err(BookError::Empty(format!("{what} in {place}")));
        }
        for t in r.texts() {
            self.check_text(place, t)?;
        }
        for row in &r.rows {
            if let Row::Ledger(shown, total) = row
                && shown > total
            {
                return Err(BookError::BadNumber {
                    place: place.into(),
                    what: format!("Ledger({shown}, {total}) shows more than it holds"),
                });
            }
        }
        Ok(())
    }

    /// A tray holds a set, each set's need checked, and a set from one piece to one a key.
    fn check_tray(&self, place: &str, sets: &[TraySet]) -> Result<(), BookError> {
        if sets.is_empty() {
            return Err(BookError::Empty(format!("tray in {place}")));
        }
        for set in sets {
            self.check_need(place, &set.need)?;
            if set.items.is_empty() {
                return Err(BookError::Empty(format!("a tray's set in {place}")));
            }
            if set.items.len() > TRAY_MAX {
                return Err(BookError::BadNumber {
                    place: place.into(),
                    what: format!(
                        "a tray's set of {}, wanted {TRAY_MAX} at most",
                        set.items.len()
                    ),
                });
            }
        }
        Ok(())
    }

    /// An objective's fits: at least one, each need checked, and the last always holds, as
    /// the tray's sets are checked.
    fn check_fits(&self, place: &str, fits: &[Fit]) -> Result<(), BookError> {
        let Some(last) = fits.last() else {
            return Err(BookError::Empty(format!("objective fits in {place}")));
        };
        for fit in fits {
            self.check_need(place, &fit.need)?;
        }
        if last.need != Need::All(vec![]) {
            return Err(BookError::BadFits {
                place: place.into(),
                why: "the last fit's need is not All([]), so a player could be left with no objective"
                    .into(),
            });
        }
        Ok(())
    }

    /// A `Late` minute is a minute of the day, inside `Not` and `All` too.
    fn check_need(&self, place: &str, need: &Need) -> Result<(), BookError> {
        match need {
            Need::Late(minute) if *minute >= sim::DAY => Err(BookError::BadNumber {
                place: place.into(),
                what: format!("Late({minute}), wanted 0 to {}", sim::DAY - 1),
            }),
            Need::Not(inner) => self.check_need(place, inner),
            Need::All(needs) => needs.iter().try_for_each(|n| self.check_need(place, n)),
            Need::Done(mission) if !self.missions.iter().any(|o| &o.id == mission) => {
                Err(BookError::UnknownMission(mission.clone()))
            }
            _ => Ok(()),
        }
    }

    fn check_mission(&self, m: &Mission) -> Result<(), BookError> {
        if m.steps.is_empty() {
            return Err(BookError::Empty(format!("mission `{}`", m.id)));
        }
        self.check_role(&format!("mission `{}`", m.id), &m.from)?;
        // A title is printed raw in the pane, the tab and the dossier: no slots.
        if m.title.contains('{') {
            return Err(BookError::UnknownSlot {
                place: format!("title of mission `{}`", m.id),
                slot: m.title.clone(),
            });
        }
        let mut seen: Vec<&Step> = Vec::new();
        for step in &m.steps {
            let place = format!("{}/{}", m.id, step.id);
            if seen.iter().any(|s| s.id == step.id) {
                return Err(BookError::DuplicateStep {
                    mission: m.id.clone(),
                    step: step.id.clone(),
                });
            }
            if step.id == END {
                return Err(BookError::ReservedStep {
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
                Trigger::Holds(need) => self.check_need(&place, need)?,
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
                // A mail may say the code it carries.
                let mail = matches!(e, Effect::Mail { .. });
                for t in e.texts() {
                    if mail {
                        self.check_text(&place, &t.replace(&code_slot(), ""))?;
                    } else {
                        self.check_text(&place, t)?;
                    }
                }
                let at = format!("mission {} step {}", m.id, step.id);
                match e {
                    Effect::Scene(id) if !self.scenes.contains_key(id) => {
                        return Err(BookError::UnknownScene {
                            place: at,
                            scene: id.clone(),
                        });
                    }
                    Effect::Arrive { role, at: mark } => {
                        self.check_arriving(&at, role)?;
                        self.check_mark(&at, mark)?;
                    }
                    Effect::Leave(role) => self.check_visitor(&at, role)?,
                    Effect::Lights(level) => check_level(&at, *level)?,
                    Effect::Note { at: mark, .. } => self.check_mark(&at, mark)?,
                    Effect::News(id) if !self.news.contains_key(id) => {
                        return Err(BookError::UnknownNews {
                            place: at,
                            news: id.clone(),
                        });
                    }
                    Effect::Daze(seconds) => positive(&at, "daze", *seconds)?,
                    Effect::Feed { text, .. } if text.is_empty() => {
                        return Err(BookError::Empty(format!("feed in {place}")));
                    }
                    Effect::Call { lines, .. } if lines.is_empty() => {
                        return Err(BookError::Empty(format!("call in {place}")));
                    }
                    Effect::Tray { sets, .. } => self.check_tray(&place, sets)?,
                    Effect::ObjectiveFit(fits) => self.check_fits(&place, fits)?,
                    _ => {}
                }
            }
            seen.push(step);
        }
        Ok(())
    }

    /// Every item a `Give` effect puts in the pack, and what those items open into.
    pub fn gives(&self, content: &game::Content) -> Vec<game::ItemId> {
        let mut out = BTreeSet::new();
        for e in self
            .missions
            .iter()
            .flat_map(|m| &m.steps)
            .flat_map(|s| &s.then)
        {
            if let Effect::Give { item, .. } = e {
                let id = game::ItemId::new(item);
                if let Some(it) = content.item(&id) {
                    out.extend(it.opens.iter().map(|(o, _)| o.clone()));
                }
                out.insert(id);
            }
        }
        out.into_iter().collect()
    }

    /// Every item id a mission's steps name: in a trigger's need or in an effect (`Give`,
    /// `Take`, `Unseal`, `Told`, and the needs of an `ObjectiveFit`).
    pub fn items_named(&self, m: &Mission) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for step in &m.steps {
            if let Trigger::Holds(need) = &step.when {
                items_in(need, &mut out);
            }
            for e in &step.then {
                match e {
                    Effect::ObjectiveFit(fits) => {
                        fits.iter().for_each(|f| items_in(&f.need, &mut out));
                    }
                    Effect::Give { item, .. }
                    | Effect::Take { item, .. }
                    | Effect::Unseal(item)
                    | Effect::Told(item) => {
                        out.insert(item.clone());
                    }
                    _ => {}
                }
            }
        }
        out
    }

    /// Every item a `Give`, `Take` or `Unseal` names, a tray holds or a reading reads, for
    /// the test that has the content.
    #[cfg(test)]
    pub fn items(&self) -> Vec<&str> {
        let mut out = BTreeSet::new();
        for step in self.missions.iter().flat_map(|m| &m.steps) {
            if let Trigger::Holds(need) = &step.when {
                items_in(need, &mut out);
            }
        }
        for e in self
            .missions
            .iter()
            .flat_map(|m| &m.steps)
            .flat_map(|s| &s.then)
        {
            match e {
                Effect::ObjectiveFit(fits) => {
                    fits.iter().for_each(|f| items_in(&f.need, &mut out));
                }
                Effect::Give { item, .. }
                | Effect::Take { item, .. }
                | Effect::Unseal(item)
                | Effect::Told(item) => {
                    out.insert(item.as_str());
                }
                Effect::Tray { sets, .. } => {
                    out.extend(sets.iter().flat_map(|s| &s.items).map(String::as_str));
                }
                _ => {}
            }
        }
        out.extend(self.readings.keys().map(String::as_str));
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

/// The items a need names, inside `Not` and `All` too.
fn items_in<'a, S: From<&'a str> + Ord>(need: &'a Need, out: &mut BTreeSet<S>) {
    match need {
        Need::Carries(i) | Need::Read(i) | Need::Unsealed(i) => {
            out.insert(S::from(i.as_str()));
        }
        Need::Not(inner) => items_in(inner, out),
        Need::All(needs) => needs.iter().for_each(|n| items_in(n, out)),
        _ => {}
    }
}

/// Seconds and rates are finite and above zero.
fn positive(place: &str, what: &str, v: f32) -> Result<(), BookError> {
    if v.is_finite() && v > 0.0 {
        Ok(())
    } else {
        Err(BookError::BadNumber {
            place: place.into(),
            what: format!("{what} is {v}, wanted more than 0"),
        })
    }
}

/// A light level runs from black (0) to normal (1).
fn check_level(place: &str, level: f32) -> Result<(), BookError> {
    if (0.0..=1.0).contains(&level) {
        Ok(())
    } else {
        Err(BookError::BadNumber {
            place: place.into(),
            what: format!("light level {level}, wanted 0 to 1"),
        })
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

fn check_still(news: &str, still: &[String]) -> Result<(), BookError> {
    let bad = |why: String| BookError::BadStill {
        news: news.into(),
        why,
    };
    if still.len() != STILL_SIDE {
        return Err(bad(format!("{} rows, wanted {STILL_SIDE}", still.len())));
    }
    for (i, row) in still.iter().enumerate() {
        let width = row.chars().count();
        if width != STILL_SIDE {
            return Err(bad(format!("row {i} is {width} wide, wanted {STILL_SIDE}")));
        }
        if let Some(c) = row.chars().find(|c| !STILL_PIXELS.contains(*c)) {
            return Err(bad(format!("row {i} has unknown pixel `{c}`")));
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
        assert_eq!(b.missions.len(), 6);
        assert_eq!(b.missions[0].id, "the_drop");
        assert_eq!(b.missions[1].id, "house_call");
        assert_eq!(b.missions[2].id, "morning_after");
        assert_eq!(b.scenes.len(), 6);
        assert_eq!(b.missions[0].steps.len(), 6);
        assert!(b.cast.contains_key("mezz") && b.cast.contains_key("marlo"));
        assert_eq!(b.face_of("mezz").eye, (255, 0, 200));
        assert_eq!(b.face_of("marlo"), &b.faces["silhouette"]);
    }

    /// The skills a need asks for, inside `Not` and `All` too.
    fn skills_in(need: &Need, out: &mut Vec<String>) {
        match need {
            Need::Skill(id) => out.push(id.clone()),
            Need::Not(inner) => skills_in(inner, out),
            Need::All(needs) => needs.iter().for_each(|n| skills_in(n, out)),
            _ => {}
        }
    }

    #[test]
    fn the_shipped_book_loads_the_doctor() {
        let b = book();
        let ids: Vec<&str> = b.missions.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "the_drop",
                "house_call",
                "morning_after",
                "doctor",
                "ledger",
                "the_three"
            ]
        );
        let (doctor, ledger, three) = (&b.missions[3], &b.missions[4], &b.missions[5]);
        assert_eq!(doctor.title, "Doctor's in by ten");
        let steps: Vec<&str> = doctor.steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(steps, ["word", "in", "tray", "out", "free"]);
        // The doc must be in the city, and the cook whose district the news named; the
        // fixer she names needn't be, so only the hook waits on him. The hook sends whoever
        // can't read the shard to her, so it waits on her too.
        let needs = |m: &Mission| b.needs_cast(m).into_iter().collect::<Vec<_>>();
        assert_eq!(needs(doctor), ["marlo", "odile"]);
        assert_eq!(needs(ledger), ["coz", "odile"]);
        let steps: Vec<&str> = ledger.steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(steps, ["whose", "know", "sell", "paid"]);
        // The act's end needs nobody in the city: in one without Coz it never opens.
        assert_eq!(three.title, "The three");
        let steps: Vec<&str> = three.steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(steps, ["ask", "quiet", "later"]);
        assert!(needs(three).is_empty(), "{:?}", needs(three));
        let morning = &b.missions[2];
        let steps: Vec<&str> = morning.steps.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(steps, ["wake", "news", "saw", "word", "open"]);
        assert!(b.needs_cast(morning).iter().all(|r| r != "odile"));
        assert_eq!(b.cast["odile"].pick, Pick::Nearest("ripperdoc".into()));
        assert!(matches!(
            &b.cast["coz"].pick,
            Pick::Worker { professions, .. } if professions == &["fixer"]
        ));
        for id in ["odile", "odile_out", "coz"] {
            assert_eq!(b.scenes[id].look, Look::Bubbles, "{id}");
        }
        // Coz's scene is the doctor's pace; what it stages, in order.
        assert_eq!(b.scenes["coz"].title, "The ledger");
        assert_eq!(b.scenes["coz"].pace, b.scenes["odile"].pace);
        let staged: Vec<&str> = b.scenes["coz"]
            .beats
            .iter()
            .filter_map(|beat| match beat {
                Beat::Stage(text) => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            staged,
            [
                "He pulls you into a doorway.",
                "He turns it under the light.",
                "A long breath.",
                "He peels off a roll, and doesn't look at you while he counts.",
            ]
        );
        let at = |item: &str| b.readings[item].iter().map(|r| r.at).collect::<Vec<_>>();
        assert_eq!(
            at("ledger_shard"),
            [ReadAt::Rig, ReadAt::Bench, ReadAt::Look]
        );
        assert_eq!(at("taped_package"), [ReadAt::Look]);
        // A set of three for each category's free skill, the last for anyone.
        let sets = doctor
            .steps
            .iter()
            .flat_map(|s| &s.then)
            .find_map(|e| match e {
                Effect::Tray { sets, .. } => Some(sets),
                _ => None,
            })
            .expect("her tray");
        assert_eq!(sets.len(), 9);
        assert!(sets.iter().all(|s| s.items.len() == 3));
        assert_eq!(sets.last().unwrap().need, Need::All(vec![]));
        // Every skill a need asks for is one the content has.
        let mut skills = Vec::new();
        for need in sets.iter().map(|s| &s.need) {
            skills_in(need, &mut skills);
        }
        for r in b.readings.values().flatten() {
            skills_in(&r.need, &mut skills);
        }
        for step in b.missions.iter().flat_map(|m| &m.steps) {
            if let Trigger::Holds(n) = &step.when {
                skills_in(n, &mut skills);
            }
            for e in &step.then {
                if let Effect::ObjectiveFit(fits) = e {
                    fits.iter().for_each(|f| skills_in(&f.need, &mut skills));
                }
            }
        }
        for beat in b.scenes.values().flat_map(|s| &s.beats) {
            if let Beat::Say { need: Some(n), .. } = beat {
                skills_in(n, &mut skills);
            }
        }
        assert_eq!(skills.len(), 17, "{skills:?}");
        let c = Content::load().unwrap();
        for id in skills {
            assert!(
                c.skill(&game::SkillId::new(&id)).is_some(),
                "no skill `{id}`"
            );
        }
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
    fn the_drop_needs_mezz_and_marlo() {
        let want: BTreeSet<String> = ["mezz", "marlo"].map(String::from).into();
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
    fn a_slot_in_a_mission_title_is_refused() {
        let mut b = book();
        b.missions[0].title = "{marlo}'s drop".into();
        assert!(matches!(
            b.validate(),
            Err(BookError::UnknownSlot { ref place, .. }) if place.contains("title")
        ));
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

    /// A small book with a runner, a visitor, and the steps and scenes of the test's making.
    fn tiny(cast: &str, steps: &str, scenes: &str) -> Result<Book, BookError> {
        let src = r#"Story(
            cast: {
                "marlo": Role(name: "Marlo", title: "cook",
                    pick: Worker(professions: ["noodle_cook"], at: [], range: (0, 9)), face: None),
                "seren": Role(name: "Seren", title: "doc", pick: Visitor, face: None,
                    tint: Some((1, 2, 3)), look: Some("a doctor")),
                CAST
            },
            faces: { "silhouette": Face(rows: ["................", "................"], eye: (0, 0, 0)) },
            missions: [Mission(id: "m", title: "M", from: "seren", steps: [STEPS])],
            scenes: { SCENES },
        )"#;
        Book::parse(
            &src.replace("CAST", cast)
                .replace("STEPS", steps)
                .replace("SCENES", scenes),
        )
    }

    const STEP: &str = r#"Step(id: "a", when: NewGame, then: [Log("hi")])"#;

    /// A scene "s" of the given beats under a pace that passes.
    fn scene_with(pace: &str, beats: &str) -> String {
        format!(r#""s": Scene(title: "S", look: Film, pace: {pace}, beats: [{beats}])"#)
    }

    fn scened(beats: &str) -> Result<Book, BookError> {
        let pace = "Pace(type: 45, walk: 0.3, hold: Some(2.5))";
        tiny("", STEP, &scene_with(pace, beats))
    }

    fn with_steps(steps: &str) -> Result<Book, BookError> {
        tiny(
            "",
            steps,
            &scene_with("Pace(type: 45, walk: 0.3, hold: None)", "Wait(1.0)"),
        )
    }

    #[test]
    fn scenes_parse_with_every_beat() {
        let b = scened(
            r#"Say(who: "seren", text: "hello {you}", as: Some("A voice"), at: Some(HomeDoor), hold: Some(1.0)),
            Stage("INSIDE"),
            Walk(who: "seren", to: Beside(You)),
            Wait(1.0),
            Keypad(door: HomeDoor, code: Journal, result: Denied, keys: 0.2, hold: 1.0),
            Door(at: HomeDoor, open: true),
            Camera(HomeFurniture(Couch)),
            Arrive(role: "seren", at: HomeOutside),
            Leave("seren"),
            Lights(0.5)"#,
        )
        .unwrap();
        let s = &b.scenes["s"];
        assert_eq!(s.beats.len(), 10);
        assert_eq!(s.look, Look::Film);
        assert_eq!(s.pace.hold, Some(2.5));
        assert!(matches!(
            &s.beats[2],
            Beat::Walk {
                wait: true,
                pace: None,
                ..
            }
        ));
        assert!(matches!(
            &s.beats[0],
            Beat::Say { label: Some(l), .. } if l == "A voice"
        ));
    }

    #[test]
    fn an_unknown_scene_is_refused() {
        let steps = r#"Step(id: "a", when: NewGame, then: [Scene("nope")])"#;
        assert!(matches!(
            with_steps(steps),
            Err(BookError::UnknownScene { .. })
        ));
    }

    #[test]
    fn a_visitor_takes_no_place_slot() {
        let steps = r#"Step(id: "a", when: NewGame, then: [Log("{seren.place}")])"#;
        assert!(matches!(
            with_steps(steps),
            Err(BookError::UnknownSlot { .. })
        ));
        let ok = r#"Step(id: "a", when: NewGame, then: [Log("{seren} the {seren.title}")])"#;
        assert!(with_steps(ok).is_ok());
    }

    #[test]
    fn no_role_is_called_you() {
        let cast = r#""you": Role(name: "Y", title: "t", pick: Visitor, face: None),"#;
        let pace = "Pace(type: 45, walk: 0.3, hold: None)";
        assert!(matches!(
            tiny(cast, STEP, &scene_with(pace, "Wait(1.0)")),
            Err(BookError::ReservedRole(_))
        ));
    }

    #[test]
    fn no_step_is_called_end() {
        let step = r#"Step(id: "end", when: NewGame, then: [Log("hi")])"#;
        assert_eq!(
            tiny("", step, ""),
            Err(BookError::ReservedStep {
                mission: "m".into(),
                step: END.into()
            })
        );
    }

    #[test]
    fn walks_and_arrivals_need_visitors() {
        for beat in [
            r#"Arrive(role: "marlo", at: HomeDoor)"#,
            r#"Walk(who: "marlo", to: HomeDoor)"#,
            r#"Leave("marlo")"#,
            r#"Camera(Role("marlo"))"#,
        ] {
            assert!(
                matches!(scened(beat), Err(BookError::NotAVisitor { .. })),
                "{beat}"
            );
        }
        assert!(matches!(
            scened(r#"Leave("ghost")"#),
            Err(BookError::UnknownRole { .. })
        ));
        assert!(scened(r#"Walk(who: "you", to: Beside(Role("seren")))"#).is_ok());
    }

    #[test]
    fn you_may_arrive() {
        assert!(scened(r#"Arrive(role: "you", at: HomeOutside)"#).is_ok());
        let steps = r#"Step(id: "a", when: NewGame, then: [Arrive(role: "you", at: HomeBed)])"#;
        let b = with_steps(steps).unwrap();
        assert!(!b.missions[0].roles().contains(YOU));
        // Only arriving: you never leave the map.
        assert!(matches!(
            scened(r#"Leave("you")"#),
            Err(BookError::UnknownRole { .. })
        ));
    }

    #[test]
    fn numbers_must_be_positive() {
        let bad = |r: Result<Book, BookError>| {
            assert!(matches!(r, Err(BookError::BadNumber { .. })), "{r:?}")
        };
        let paced = |pace: &str| tiny("", STEP, &scene_with(pace, "Wait(1.0)"));
        bad(paced("Pace(type: 0, walk: 0.3, hold: None)"));
        bad(paced("Pace(type: 45, walk: -1.0, hold: None)"));
        bad(paced("Pace(type: 45, walk: 0.3, hold: Some(0.0))"));
        bad(scened("Wait(-1.0)"));
        bad(scened("Lights(1.5)"));
        bad(scened(r#"Walk(who: "you", to: HomeDoor, pace: Some(0.0))"#));
        bad(scened(
            "Keypad(door: HomeDoor, code: Code(1234), result: Open, keys: 0.0, hold: 1.0)",
        ));
        bad(scened(r#"Say(who: "you", text: "x", hold: Some(-2.0))"#));
        bad(scened(
            r#"Say(who: "you", text: "x", need: Some(Not(All([Late(1440)]))))"#,
        ));
        assert!(scened(r#"Say(who: "you", text: "x", need: Some(Late(1439)))"#).is_ok());
        bad(scened("Camera(HomeInside(0))"));
        bad(with_steps(
            r#"Step(id: "a", when: NewGame, then: [Daze(0.0)])"#,
        ));
        let steps = r#"Step(id: "a", when: NewGame, then: [Lights(2.0)])"#;
        assert!(
            matches!(with_steps(steps), Err(BookError::BadNumber { ref place, .. }) if place.contains("mission m step a"))
        );
    }

    #[test]
    fn an_empty_scene_is_refused() {
        assert!(matches!(scened(""), Err(BookError::EmptyScene(_))));
    }

    #[test]
    fn a_next_scene_exists_and_comes_back_to_none() {
        let pace = "Pace(type: 45, walk: 0.3, hold: None)";
        let two = |s_next: &str, t_next: &str| {
            let scenes = format!(
                r#""s": Scene(title: "S", look: Film, pace: {pace}, beats: [Wait(1.0)], next: {s_next}),
                "t": Scene(title: "T", look: Bubbles, pace: {pace}, beats: [Wait(1.0)], next: {t_next})"#
            );
            tiny("", STEP, &scenes)
        };
        let b = two(r#"Some("t")"#, "None").unwrap();
        assert_eq!(b.scenes["s"].next.as_deref(), Some("t"));
        assert_eq!(b.scenes["t"].next, None);
        assert!(matches!(
            two(r#"Some("nope")"#, "None"),
            Err(BookError::UnknownScene { .. })
        ));
        assert!(matches!(
            two(r#"Some("t")"#, r#"Some("s")"#),
            Err(BookError::SceneLoop(_))
        ));
        assert!(matches!(
            two(r#"Some("s")"#, "None"),
            Err(BookError::SceneLoop(_))
        ));
    }

    #[test]
    fn ended_may_name_a_scene_step() {
        let steps = r#"Step(id: "a", when: NewGame, then: [Scene("s")]),
            Step(id: "b", when: Ended("a"), then: [Log("after")])"#;
        assert!(with_steps(steps).is_ok());
    }

    #[test]
    fn you_is_a_slot() {
        let ok = r#"Step(id: "a", when: NewGame, then: [Log("{you}")])"#;
        assert!(with_steps(ok).is_ok());
        let bad = r#"Step(id: "a", when: NewGame, then: [Log("{you.place}")])"#;
        assert!(matches!(
            with_steps(bad),
            Err(BookError::UnknownSlot { .. })
        ));
    }

    #[test]
    fn visitors_need_no_cast() {
        let steps =
            r#"Step(id: "a", when: NewGame, then: [Log("{you} and {marlo} at {marlo.place}")])"#;
        let b = with_steps(steps).unwrap();
        let m = &b.missions[0];
        assert_eq!(m.from, "seren");
        assert!(m.roles().contains("seren") && !m.roles().contains("you"));
        let want: BTreeSet<String> = ["marlo"].map(String::from).into();
        assert_eq!(b.needs_cast(m), want);
        assert!(b.is_visitor("seren") && !b.is_visitor("marlo") && !b.is_visitor(YOU));
    }

    #[test]
    fn a_scene_title_takes_no_slots() {
        let src = r#""s": Scene(title: "{you}", look: Bubbles,
            pace: Pace(type: 45, walk: 0.3, hold: None), beats: [Wait(1.0)])"#;
        assert!(matches!(
            tiny("", STEP, src),
            Err(BookError::UnknownSlot { .. })
        ));
    }

    #[test]
    fn an_empty_feed_is_refused() {
        let mut b = book();
        if let Effect::Feed { text, .. } = &mut step_mut(&mut b, "welcome").then[0] {
            text.clear();
        }
        assert!(matches!(b.validate(), Err(BookError::Empty(_))));
    }

    /// The shipped book with its news item changed.
    fn news_changed(change: impl FnOnce(&mut News)) -> Result<(), BookError> {
        let mut b = book();
        change(b.news.get_mut("marlo").unwrap());
        b.validate()
    }

    /// A small book with a news item "x" about `about` whose headline is `headline`,
    /// and these steps.
    fn with_news(about: &str, headline: &str, steps: &str) -> Result<Book, BookError> {
        let still = vec![format!("\"{}\"", "0".repeat(18)); 18].join(", ");
        let src = format!(
            r#"Story(
                cast: {{
                    "marlo": Role(name: "Marlo", title: "cook",
                        pick: Worker(professions: ["noodle_cook"], at: [], range: (0, 9)), face: None),
                    "seren": Role(name: "Seren", title: "doc", pick: Visitor, face: None),
                }},
                faces: {{ "silhouette": Face(rows: ["................", "................"], eye: (0, 0, 0)) }},
                missions: [Mission(id: "m", title: "M", from: "seren", steps: [{steps}])],
                news: {{
                    "x": News(about: "{about}", headline: "{headline}", report: ["A report."],
                        reads: ["A read."], crawl: ["A crawl."], overheard: ["Overheard."],
                        sought: "someone", carrying: "something", camera: "CAM 1",
                        still: [{still}], close: "off", fresh: 60),
                }},
            )"#
        );
        Book::parse(&src)
    }

    #[test]
    fn the_shipped_book_has_its_news() {
        let b = book();
        let n = &b.news["marlo"];
        assert_eq!((n.still.len(), n.still[0].chars().count()), (18, 18));
        assert_eq!(n.fresh, 1440);
    }

    #[test]
    fn an_unknown_news_is_refused() {
        let mut b = book();
        step_mut(&mut b, "body").then[0] = Effect::News("nope".into());
        assert!(matches!(
            b.validate(),
            Err(BookError::UnknownNews { ref news, .. }) if news == "nope"
        ));
    }

    #[test]
    fn a_news_about_nobody_is_refused() {
        assert!(matches!(
            news_changed(|n| n.about = "nobody".into()),
            Err(BookError::UnknownRole { ref role, .. }) if role == "nobody"
        ));
        assert!(matches!(
            news_changed(|n| n.reads[0] = "{nobody} did it".into()),
            Err(BookError::UnknownRole { .. })
        ));
    }

    #[test]
    fn an_empty_headline_is_refused() {
        assert!(matches!(
            news_changed(|n| n.headline.clear()),
            Err(BookError::Empty(ref what)) if what.contains("news `marlo`")
        ));
        assert!(matches!(
            news_changed(|n| n.report.clear()),
            Err(BookError::Empty(_))
        ));
        assert!(matches!(
            news_changed(|n| n.reads.clear()),
            Err(BookError::Empty(_))
        ));
    }

    #[test]
    fn an_empty_overheard_is_refused() {
        assert!(matches!(
            news_changed(|n| n.overheard.clear()),
            Err(BookError::Empty(ref what)) if what.contains("news `marlo`")
        ));
    }

    #[test]
    fn a_bad_still_is_refused() {
        let bad_still = |r: Result<(), BookError>| {
            assert!(matches!(r, Err(BookError::BadStill { .. })), "{r:?}")
        };
        bad_still(news_changed(|n| {
            n.still.pop();
        }));
        bad_still(news_changed(|n| n.still[3].push('0')));
        bad_still(news_changed(|n| {
            n.still[5] = format!("x{}", "0".repeat(17))
        }));
    }

    #[test]
    fn a_stale_at_once_news_is_refused() {
        assert!(matches!(
            news_changed(|n| n.fresh = 0),
            Err(BookError::Empty(ref what)) if what.contains("news `marlo`")
        ));
    }

    #[test]
    fn news_is_a_presentation() {
        let steps = r#"Step(id: "n", when: NewGame, then: [News("x")]),
            Step(id: "after", when: Ended("n"), then: [Log("after")])"#;
        assert!(with_news("marlo", "Headline", steps).is_ok());
    }

    /// The roles `steps` need cast, with a fixer `zed` in the cast and scenes `s` and `t`.
    fn needed(steps: &str) -> BTreeSet<String> {
        let zed = r#""zed": Role(name: "Zed", title: "fixer", pick: NearestFixer, face: None),"#;
        let pace = "Pace(type: 45, walk: 0.3, hold: None)";
        let scenes = format!(
            r#""s": Scene(title: "S", look: Film, pace: {pace}, next: Some("t"),
                beats: [Say(who: "seren", text: "hi")]),
            "t": Scene(title: "T", look: Film, pace: {pace},
                beats: [Say(who: "zed", text: "hey")])"#
        );
        let b = tiny(zed, steps, &scenes).unwrap();
        b.needs_cast(&b.missions[0])
    }

    #[test]
    fn needs_cast_skips_bare_names_and_counts_places() {
        let none = BTreeSet::new();
        let zed: BTreeSet<String> = ["zed"].map(String::from).into();
        let log = |t: &str| format!(r#"Step(id: "a", when: NewGame, then: [Log("{t}")])"#);
        assert_eq!(needed(&log("{zed} {zed.spaced} {zed.title}")), none);
        assert_eq!(needed(&log("at {zed.place}")), zed);
        assert_eq!(needed(&log("in {zed.district}")), zed);
        assert_eq!(needed(&log("from {zed.hours}")), zed);
        let talk = r#"Step(id: "a", when: Talk("zed"), then: [Log("x")])"#;
        assert_eq!(needed(talk), zed);
        let dead = r#"Step(id: "a", when: NewGame, then: [Dead(role: "zed", look: "x")])"#;
        assert_eq!(needed(dead), zed);
        // A scene's speaker, found down its `next` chain.
        let scene = r#"Step(id: "a", when: NewGame, then: [Scene("s")])"#;
        assert_eq!(needed(scene), zed);
        // A visitor speaking needs nobody.
        let seren =
            r#"Step(id: "a", when: NewGame, then: [Feed(from: "zed", text: ["x"], close: "y")])"#;
        assert_eq!(needed(seren), none);
    }

    #[test]
    fn a_fit_placing_a_role_needs_it_cast() {
        let zed: BTreeSet<String> = ["zed"].map(String::from).into();
        let fit = r#"Step(id: "a", when: NewGame, then: [ObjectiveFit([
            Fit(need: Skill("hacking"), text: "read it"),
            Fit(need: All([]), text: "ask the doctor at {zed.place}"),
        ])])"#;
        assert_eq!(needed(fit), zed);
        let bare = fit.replace("at {zed.place}", "");
        assert_eq!(needed(&bare), BTreeSet::new());
    }

    #[test]
    fn objective_fit_must_end_on_all() {
        let fit =
            |fits: &str| format!(r#"Step(id: "a", when: NewGame, then: [ObjectiveFit([{fits}])])"#);
        let ok = fit(r#"Fit(need: Skill("hacking"), text: "a"), Fit(need: All([]), text: "b")"#);
        assert!(with_steps(&ok).is_ok());
        for bad in [
            r#"Fit(need: Skill("hacking"), text: "a")"#,
            r#"Fit(need: All([]), text: "a"), Fit(need: Skill("hacking"), text: "b")"#,
            "",
        ] {
            assert!(with_steps(&fit(bad)).is_err(), "{bad}");
        }
        // Its words name the cast, and its needs are checked.
        let nobody = fit(r#"Fit(need: All([]), text: "{nobody}")"#);
        assert!(matches!(
            with_steps(&nobody).unwrap_err(),
            BookError::UnknownRole { .. }
        ));
        let late = fit(r#"Fit(need: Late(2000), text: "a"), Fit(need: All([]), text: "b")"#);
        assert!(with_steps(&late).is_err());
    }

    #[test]
    fn a_holds_step_has_its_need_checked() {
        let holds = |need: &str| format!(r#"Step(id: "a", when: Holds({need}), then: [Log("x")])"#);
        assert!(with_steps(&holds(r#"Carries("x")"#)).is_ok());
        assert!(with_steps(&holds("Late(2000)")).is_err());
    }

    #[test]
    fn the_book_names_the_items_its_needs_and_told_name() {
        let b = with_steps(
            r#"Step(id: "a", when: Holds(Not(Carries("c"))), then: [
                Told("t"),
                ObjectiveFit([
                    Fit(need: All([Read("r"), Unsealed("u")]), text: "a"),
                    Fit(need: All([]), text: "b"),
                ]),
            ])"#,
        )
        .unwrap();
        assert_eq!(b.items(), ["c", "r", "t", "u"]);
    }

    #[test]
    fn beside_someone_and_their_chair_need_them_cast() {
        let zed: BTreeSet<String> = ["zed"].map(String::from).into();
        let by = r#"Step(id: "a", when: At(By(role: "zed", within: 4)), then: [Log("x")])"#;
        assert_eq!(needed(by), zed);
        // A mark in an effect, inside `Beside` too.
        let arrive = r#"Step(id: "a", when: NewGame,
            then: [Arrive(role: "seren", at: Beside(Chair("zed")))])"#;
        assert_eq!(needed(arrive), zed);
        // A mark in a scene's beat.
        let doc =
            r#""zed": Role(name: "Zed", title: "doc", pick: Nearest("ripperdoc"), face: None),"#;
        let pace = "Pace(type: 45, walk: 0.3, hold: None)";
        let b = tiny(
            doc,
            r#"Step(id: "a", when: NewGame, then: [Scene("s")])"#,
            &scene_with(
                pace,
                r#"Say(who: "seren", text: "hi", at: Some(Chair("zed")))"#,
            ),
        )
        .unwrap();
        assert_eq!(b.needs_cast(&b.missions[0]), zed);
    }

    #[test]
    fn beside_and_the_chair_name_a_known_role() {
        let by = r#"Step(id: "a", when: At(By(role: "nobody", within: 4)), then: [Log("x")])"#;
        assert!(matches!(with_steps(by), Err(BookError::UnknownRole { .. })));
        assert!(matches!(
            scened(r#"Camera(Chair("nobody"))"#),
            Err(BookError::UnknownRole { .. })
        ));
        assert!(scened(r#"Camera(Chair("marlo"))"#).is_ok());
    }

    #[test]
    fn the_news_roles_must_be_cast() {
        let steps = r#"Step(id: "n", when: NewGame, then: [News("x")])"#;
        let want: BTreeSet<String> = ["marlo"].map(String::from).into();
        // Named in a text, the item about a visitor.
        let b = with_news("seren", "{marlo} in {marlo.district}", steps).unwrap();
        assert!(!b.missions[0].roles().contains("marlo"));
        assert_eq!(b.needs_cast(&b.missions[0]), want);
        // The role it is about.
        let b = with_news("marlo", "Headline", steps).unwrap();
        assert_eq!(b.needs_cast(&b.missions[0]), want);
        // A mission that puts out no news needs none of its roles.
        let b = with_news(
            "marlo",
            "{marlo}",
            r#"Step(id: "a", when: NewGame, then: [Log("hi")])"#,
        )
        .unwrap();
        assert!(b.needs_cast(&b.missions[0]).is_empty());
    }

    #[test]
    fn holds_releases_and_a_mailed_code_need_the_role_cast() {
        let zed: BTreeSet<String> = ["zed"].map(String::from).into();
        let step = |e: &str| format!(r#"Step(id: "a", when: NewGame, then: [{e}])"#);
        assert_eq!(needed(&step(r#"Hold("zed")"#)), zed);
        assert_eq!(needed(&step(r#"Release("zed")"#)), zed);
        let mail = r#"Mail(from: "seren", subject: "ten", body: ["Code's {code}."],
            code: Some(Clinic("zed")))"#;
        assert_eq!(needed(&step(mail)), zed);
        // A mail with no code needs nobody but its slots.
        let plain = r#"Mail(from: "zed", subject: "ten", body: ["hi"])"#;
        assert_eq!(needed(&step(plain)), BTreeSet::new());
    }

    #[test]
    fn holds_and_mailed_codes_name_a_known_role() {
        let step = |e: &str| format!(r#"Step(id: "a", when: NewGame, then: [{e}])"#);
        assert!(with_steps(&step(r#"Hold("marlo")"#)).is_ok());
        assert!(with_steps(&step(r#"Release("marlo")"#)).is_ok());
        for e in [
            r#"Hold("nobody")"#,
            r#"Release("nobody")"#,
            r#"Mail(from: "seren", subject: "s", body: ["x"], code: Some(Clinic("nobody")))"#,
        ] {
            assert!(
                matches!(with_steps(&step(e)), Err(BookError::UnknownRole { .. })),
                "{e}"
            );
        }
    }

    #[test]
    fn stay_and_standing_need_their_role() {
        let zed: BTreeSet<String> = ["zed"].map(String::from).into();
        let cast = r#""zed": Role(name: "Zed", title: "fixer", pick: NearestFixer, face: None),
            "kay": Role(name: "Kay", title: "fixer", pick: NearestFixer, face: None),"#;
        let step = |e: &str| format!(r#"Step(id: "a", when: NewGame, then: [{e}])"#);
        assert_eq!(needed(&step(r#"Stay("zed")"#)), zed);
        assert_eq!(
            needed(&step(r#"Standing(role: "zed", by: 1)"#)),
            zed,
            "standing needs the fixer cast"
        );
        // Both held in one step: each role is needed.
        let both = step(r#"Stay("zed"), Standing(role: "kay", by: 1)"#);
        let b = tiny(cast, &both, "").unwrap();
        let want: BTreeSet<String> = ["kay", "zed"].map(String::from).into();
        assert_eq!(b.needs_cast(&b.missions[0]), want);
        for e in [r#"Stay("nobody")"#, r#"Standing(role: "nobody", by: 1)"#] {
            assert!(
                matches!(with_steps(&step(e)), Err(BookError::UnknownRole { .. })),
                "{e}"
            );
        }
        // Pay names nobody.
        assert_eq!(needed(&step("Pay(800)")), BTreeSet::new());
    }

    #[test]
    fn done_names_a_mission() {
        let holds = |need: &str| {
            with_steps(&format!(
                r#"Step(id: "a", when: Holds({need}), then: [Log("hi")])"#
            ))
        };
        assert!(matches!(
            holds(r#"Done("nope")"#),
            Err(BookError::UnknownMission(m)) if m == "nope"
        ));
        assert!(holds(r#"Done("m")"#).is_ok());
        // Inside Not and All too.
        assert!(matches!(
            holds(r#"All([Not(Done("nope"))])"#),
            Err(BookError::UnknownMission(_))
        ));
    }

    #[test]
    fn ad_presents_and_names_its_speaker() {
        let step = |e: &str| format!(r#"Step(id: "a", when: NewGame, then: [{e}])"#);
        let ad = r#"Ad(from: "seren", text: "Work for {marlo}.")"#;
        let b = with_steps(&format!(
            r#"{}, Step(id: "after", when: Ended("a"), then: [Log("x")])"#,
            step(ad)
        ))
        .unwrap();
        assert_eq!(
            b.missions[0].steps[0].then[0],
            Effect::Ad {
                from: "seren".into(),
                text: "Work for {marlo}.".into()
            }
        );
        // From a role nobody plays, or saying one: refused, as a holo line is.
        for e in [
            r#"Ad(from: "nobody", text: "x")"#,
            r#"Holo(from: "nobody", text: "x")"#,
            r#"Ad(from: "seren", text: "{nobody}")"#,
        ] {
            assert!(
                matches!(with_steps(&step(e)), Err(BookError::UnknownRole { .. })),
                "{e}"
            );
        }
        // Whoever says it needn't be cast.
        assert_eq!(
            needed(&step(r#"Ad(from: "zed", text: "x")"#)),
            BTreeSet::new()
        );
    }

    #[test]
    fn later_parses() {
        let b = with_steps(
            r#"Step(id: "a", when: NewGame, then: [Log("hi")]),
            Step(id: "later", when: Later, then: [Log("more")])"#,
        )
        .unwrap();
        assert_eq!(b.missions[0].steps[1].when, Trigger::Later);
    }

    /// The builds a book's needs ask for, inside `Not` and `All` too.
    fn builds_in(need: &Need, out: &mut Vec<String>) {
        match need {
            Need::Build(id) => out.push(id.clone()),
            Need::Not(inner) => builds_in(inner, out),
            Need::All(needs) => needs.iter().for_each(|n| builds_in(n, out)),
            _ => {}
        }
    }

    /// Every `Build` a book's needs ask for that `content` has no profession for.
    fn unknown_builds(b: &Book, content: &Content) -> Vec<String> {
        let mut builds = Vec::new();
        for step in b.missions.iter().flat_map(|m| &m.steps) {
            if let Trigger::Holds(n) = &step.when {
                builds_in(n, &mut builds);
            }
            for e in &step.then {
                match e {
                    Effect::ObjectiveFit(fits) => {
                        fits.iter().for_each(|f| builds_in(&f.need, &mut builds));
                    }
                    Effect::Tray { sets, .. } => {
                        sets.iter().for_each(|s| builds_in(&s.need, &mut builds));
                    }
                    _ => {}
                }
            }
        }
        for r in b.readings.values().flatten() {
            builds_in(&r.need, &mut builds);
        }
        for beat in b.scenes.values().flat_map(|s| &s.beats) {
            if let Beat::Say { need: Some(n), .. } = beat {
                builds_in(n, &mut builds);
            }
        }
        builds
            .into_iter()
            .filter(|id| content.profession(&game::ProfessionId::new(id)).is_none())
            .collect()
    }

    #[test]
    fn every_build_need_names_a_build() {
        let c = Content::load().unwrap();
        assert!(unknown_builds(&book(), &c).is_empty());
        // The same check catches a build the content lacks.
        let chef = with_steps(
            r#"Step(id: "a", when: Holds(All([Build("solo"), Build("chef")])), then: [Log("hi")])"#,
        )
        .unwrap();
        assert_eq!(unknown_builds(&chef, &c), ["chef"]);
    }

    #[test]
    fn the_code_slot_is_a_mails_alone() {
        let step = |e: &str| format!(r#"Step(id: "a", when: NewGame, then: [{e}])"#);
        let b = with_steps(&step(
            r#"Mail(from: "seren", subject: "ten", body: ["Code's {code}."],
                code: Some(Clinic("marlo")))"#,
        ))
        .unwrap();
        let Effect::Mail { code, .. } = &b.missions[0].steps[0].then[0] else {
            panic!("a mail")
        };
        assert_eq!(code, &Some(CodeOf::Clinic("marlo".into())));
        assert!(!b.missions[0].roles().contains(CODE));
        // A mail without a code may still say `{code}`: it fills as nothing.
        assert!(
            with_steps(&step(
                r#"Mail(from: "seren", subject: "s", body: ["{code}"])"#
            ))
            .is_ok()
        );
        // Anywhere else it is a role nobody plays.
        assert!(matches!(
            with_steps(&step(r#"Log("{code}")"#)),
            Err(BookError::UnknownRole { .. })
        ));
        // And no role may take the slot's name.
        let code_role = r#""code": Role(name: "C", title: "x", pick: NearestFixer, face: None),"#;
        assert_eq!(
            tiny(code_role, STEP, ""),
            Err(BookError::ReservedRole(CODE.into()))
        );
    }

    /// A step holding out a tray from `from` on `tab`'s tab, with these sets.
    fn tray_step(from: &str, tab: &str, sets: &str) -> String {
        format!(
            r#"Step(id: "a", when: NewGame, then: [Tray(from: "{from}", tab: "{tab}",
                title: "her tray", line: "One piece.", sets: [{sets}])])"#
        )
    }

    const ONE_SET: &str = r#"TraySet(need: All([]), items: ["neural_port"])"#;

    #[test]
    fn a_tray_and_chromed_need_their_doc_cast() {
        let zed: BTreeSet<String> = ["zed"].map(String::from).into();
        // The tab's payer is a name on a line: nobody need play them.
        assert_eq!(needed(&tray_step("zed", "marlo", ONE_SET)), zed);
        let chromed = r#"Step(id: "a", when: Chromed("zed"), then: [Log("x")])"#;
        assert_eq!(needed(chromed), zed);
    }

    #[test]
    fn a_tray_names_known_roles_and_one_to_nine_pieces_a_set() {
        let b = with_steps(&tray_step(
            "marlo",
            "seren",
            r#"TraySet(need: Skill("hacking"), items: ["neural_port", "feedback_fuse"]),
                TraySet(need: All([]), items: ["subdermal_mesh"])"#,
        ))
        .unwrap();
        let Effect::Tray {
            from, tab, sets, ..
        } = &b.missions[0].steps[0].then[0]
        else {
            panic!("a tray")
        };
        assert_eq!((from.as_str(), tab.as_str()), ("marlo", "seren"));
        assert_eq!(sets[0].need, Need::Skill("hacking".into()));
        assert_eq!(sets[1].items, ["subdermal_mesh"]);
        let refused = |steps: &str| with_steps(steps).unwrap_err();
        for (from, tab) in [("nobody", "seren"), ("marlo", "nobody")] {
            assert!(
                matches!(
                    refused(&tray_step(from, tab, ONE_SET)),
                    BookError::UnknownRole { .. }
                ),
                "{from} {tab}"
            );
        }
        let chromed = r#"Step(id: "a", when: Chromed("nobody"), then: [Log("x")])"#;
        assert!(matches!(refused(chromed), BookError::UnknownRole { .. }));
        // Its words name the cast too.
        assert!(matches!(
            refused(&tray_step("marlo", "seren", ONE_SET).replace("One piece.", "{nobody}")),
            BookError::UnknownRole { .. }
        ));
        // A tray holds a set; a set holds a piece, and no more than the keys 1 to 9.
        assert!(matches!(
            refused(&tray_step("marlo", "seren", "")),
            BookError::Empty(_)
        ));
        let empty = r#"TraySet(need: All([]), items: [])"#;
        assert!(matches!(
            refused(&tray_step("marlo", "seren", empty)),
            BookError::Empty(_)
        ));
        let ten = format!(
            "TraySet(need: All([]), items: [{}])",
            [r#""neural_port""#; 10].join(", ")
        );
        assert!(matches!(
            refused(&tray_step("marlo", "seren", &ten)),
            BookError::BadNumber { .. }
        ));
        let late = r#"TraySet(need: Late(1440), items: ["neural_port"])"#;
        assert!(matches!(
            refused(&tray_step("marlo", "seren", late)),
            BookError::BadNumber { .. }
        ));
    }

    /// The tiny book with these readings.
    fn with_readings(readings: &str) -> Result<Book, BookError> {
        tiny("", STEP, &format!("}}, readings: {{ {readings}"))
    }

    #[test]
    fn readings_parse_and_check_their_texts() {
        let b = with_readings(
            r#""ledger_shard": [
                Reading(at: Rig, need: Skill("hacking"), verb: "Read", head: "h · {marlo}",
                    close: "pull it", tells: true, rows: [
                    Field("owner", "`{marlo}` · cook"), Hot("open"), Tag("{seren}"),
                    Art("`{marlo.spaced}`"), You("hm"), Ledger(4, 214), Blank,
                ]),
                Reading(at: Look, need: Not(Read("ledger_shard")), text: "Tape. {marlo}."),
            ]"#,
        )
        .unwrap();
        let shard = &b.readings["ledger_shard"];
        assert_eq!((shard[0].at, shard[1].at), (ReadAt::Rig, ReadAt::Look));
        assert_eq!(shard[0].rows[5], Row::Ledger(4, 214));
        assert!(shard[0].tells && !shard[1].tells && shard[1].verb.is_empty());
        // A book without readings has none.
        assert!(tiny("", STEP, "").unwrap().readings.is_empty());
        let refused = |r: &str| with_readings(&format!(r#""ledger_shard": [{r}]"#));
        // Every text's slots name the cast.
        assert!(matches!(
            refused(r#"Reading(at: Look, need: Skill("x"), text: "{nobody}")"#),
            Err(BookError::UnknownRole { .. })
        ));
        assert!(matches!(
            refused(
                r#"Reading(at: Bench, need: Skill("x"), verb: "Open", rows: [Field("{nobody}", "x")])"#
            ),
            Err(BookError::UnknownRole { .. })
        ));
        // A rig or bench reading is offered by its verb; a Look reading is its text.
        assert!(matches!(
            refused(r#"Reading(at: Rig, need: Skill("x"), head: "h")"#),
            Err(BookError::Empty(_))
        ));
        assert!(matches!(
            refused(r#"Reading(at: Look, need: Skill("x"))"#),
            Err(BookError::Empty(_))
        ));
        // A ledger shows no more than it holds, and a need's minute is a minute of the day.
        assert!(matches!(
            refused(r#"Reading(at: Rig, need: Skill("x"), verb: "Read", rows: [Ledger(5, 4)])"#),
            Err(BookError::BadNumber { .. })
        ));
        assert!(matches!(
            refused(r#"Reading(at: Look, need: Late(1440), text: "x")"#),
            Err(BookError::BadNumber { .. })
        ));
    }

    #[test]
    fn items_named_reads_needs_and_effects() {
        let b = book();
        let named = |id: &str| b.items_named(b.missions.iter().find(|m| m.id == id).unwrap());
        assert!(named("ledger").contains("ledger_shard"));
        assert!(named("the_drop").contains("taped_package"));
    }
}
