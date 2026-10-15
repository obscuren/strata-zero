//! A character in progress: the wizard's choices and the rules that price them.

use crate::content::Content;
use crate::model::*;
use std::collections::BTreeSet;
use std::fmt;

/// The wizard's steps, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Name,
    Origin,
    Profession,
    Stats,
    Skills,
    Summary,
}

impl Step {
    pub const ALL: [Step; 6] = [
        Step::Origin,
        Step::Profession,
        Step::Stats,
        Step::Skills,
        Step::Name,
        Step::Summary,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Step::Name => "NAME",
            Step::Origin => "ORIGIN",
            Step::Profession => "PROFESSION",
            Step::Stats => "STATS",
            Step::Skills => "SKILLS",
            Step::Summary => "SUMMARY",
        }
    }

    pub fn tagline(self) -> &'static str {
        match self {
            Step::Name => "what they will call you",
            Step::Origin => "where you were made before the city got its hands on you",
            Step::Profession => "what you did before this",
            Step::Stats => "what the meat can do",
            Step::Skills => "what you know",
            Step::Summary => "the sheet",
        }
    }

    pub fn index(self) -> usize {
        Step::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn next(self) -> Option<Step> {
        Step::ALL.get(self.index() + 1).copied()
    }

    pub fn prev(self) -> Option<Step> {
        self.index().checked_sub(1).map(|i| Step::ALL[i])
    }
}

/// What a point in a stat costs, as a word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Affinity {
    Cheap,
    Normal,
    Dear,
}

impl Affinity {
    pub fn label(self) -> &'static str {
        match self {
            Affinity::Cheap => "cheap",
            Affinity::Normal => "normal",
            Affinity::Dear => "dear",
        }
    }
}

/// Why a build, or one move on it, is refused. Reads in voice on screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    NoName,
    NameTooLong(usize),
    BadName(char),
    NoOrigin,
    NoProfession,
    AtCap(Stat),
    AtBase(Stat),
    OverPoints(u32),
    NoPoints,
    OverSp(u32),
    NoSp,
    CategoryLocked { skill: SkillId, category: String },
    Unknown(String),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::NoName => write!(f, "no name on the sheet"),
            BuildError::NameTooLong(n) => write!(f, "a name is {n} characters at most"),
            BuildError::BadName(c) => write!(f, "the sheet will not take {c:?}"),
            BuildError::NoOrigin => write!(f, "pick where you were made"),
            BuildError::NoProfession => write!(f, "pick what you did"),
            BuildError::AtCap(s) => write!(f, "{} is maxed", s.name()),
            BuildError::AtBase(s) => write!(f, "nothing to refund on {}", s.name()),
            BuildError::OverPoints(n) => {
                write!(f, "over by {n} point{}", if *n == 1 { "" } else { "s" })
            }
            BuildError::NoPoints => write!(f, "no points left"),
            BuildError::OverSp(n) => write!(f, "over by {n} SP"),
            BuildError::NoSp => write!(f, "no SP left"),
            BuildError::CategoryLocked { category, .. } => write!(f, "unlock {category} first"),
            BuildError::Unknown(id) => write!(f, "the content does not know {id}"),
        }
    }
}

/// The choices so far. Costs are computed from the content every time, never stored,
/// so changing an origin after buying simply reprices what was bought.
#[derive(Clone, Debug, PartialEq)]
pub struct Build {
    pub handle: String,
    pub origin: Option<OriginId>,
    pub profession: Option<ProfessionId>,
    pub stats: Stats,
    pub categories: BTreeSet<CategoryId>,
    pub skills: BTreeSet<SkillId>,
    /// Extra kit on top of the profession's.
    pub kit: Vec<(ItemId, u32)>,
    pub template: Option<TemplateId>,
    /// Pool totals when they are not the creation defaults: a played character's
    /// spent points plus what it still holds.
    pub pools: Option<(u32, u32)>,
}

const NAME_MARKS: &str = "-_.' ";

impl Build {
    /// Nothing chosen, every stat at the base.
    pub fn blank(c: &Content) -> Build {
        let b = c.creation.stat_base;
        Build {
            handle: String::new(),
            origin: None,
            profession: None,
            stats: Stats {
                body: b,
                reflex: b,
                tech: b,
                cool: b,
                net: b,
            },
            categories: BTreeSet::new(),
            skills: BTreeSet::new(),
            kit: Vec::new(),
            template: None,
            pools: None,
        }
    }

    pub fn from_template(c: &Content, t: &Template) -> Build {
        let mut b = Build::blank(c);
        b.handle = t.handle.clone();
        b.origin = Some(t.origin.clone());
        b.profession = Some(t.profession.clone());
        b.stats = t.stats.clone();
        b.categories = t.categories.iter().cloned().collect();
        b.skills = t.skills.iter().cloned().collect();
        b.kit = t.kit.clone();
        b.template = Some(t.id.clone());
        b
    }

    /// A played character as a build, with pools sized so that what is left equals
    /// what the character holds.
    pub fn from_character(c: &Content, ch: &crate::character::Character) -> Build {
        let mut b = Build::blank(c);
        b.handle = ch.handle.clone();
        b.origin = Some(ch.origin.clone());
        b.profession = Some(ch.profession.clone());
        b.stats = ch.stats.clone();
        b.categories = ch.categories.clone();
        b.skills = ch.skills.clone();
        b.pools = Some((b.points_spent(c) + ch.stat_points, b.sp_spent(c) + ch.sp));
        b
    }

    /// The whole points pool this build draws on.
    pub fn points_total(&self, c: &Content) -> u32 {
        self.pools.map_or(c.creation.stat_points, |p| p.0)
    }

    /// The whole SP pool this build draws on.
    pub fn sp_total(&self, c: &Content) -> u32 {
        self.pools.map_or(c.creation.sp, |p| p.1)
    }

    fn shift(cheap: &[Stat], dear: &[Stat], s: Stat) -> i32 {
        if cheap.contains(&s) {
            -1
        } else if dear.contains(&s) {
            1
        } else {
            0
        }
    }

    /// The origin's shift for a stat: -1 cheap, 0, +1 dear.
    pub fn origin_shift(&self, c: &Content, s: Stat) -> i32 {
        self.origin
            .as_ref()
            .and_then(|id| c.origin(id))
            .map_or(0, |o| Self::shift(&o.cheap, &o.dear, s))
    }

    /// The profession's shift for a stat: -1 cheap, 0, +1 dear.
    pub fn profession_shift(&self, c: &Content, s: Stat) -> i32 {
        self.profession
            .as_ref()
            .and_then(|id| c.profession(id))
            .map_or(0, |p| Self::shift(&p.cheap, &p.dear, s))
    }

    /// What one point in a stat costs: the base cost plus both shifts, clamped to 1..=3.
    pub fn stat_cost(&self, c: &Content, s: Stat) -> u32 {
        (c.creation.point_cost as i32 + self.origin_shift(c, s) + self.profession_shift(c, s))
            .clamp(1, 3) as u32
    }

    pub fn affinity(&self, c: &Content, s: Stat) -> Affinity {
        match self.stat_cost(c, s) {
            1 => Affinity::Cheap,
            3 => Affinity::Dear,
            _ => Affinity::Normal,
        }
    }

    pub fn points_spent(&self, c: &Content) -> u32 {
        Stat::ALL
            .iter()
            .map(|&s| {
                let raised = self.stats.get(s).saturating_sub(c.creation.stat_base) as u32;
                raised * self.stat_cost(c, s)
            })
            .sum()
    }

    /// Points left in the pool; negative when a change of origin or profession
    /// repriced what was already bought.
    pub fn points_left(&self, c: &Content) -> i64 {
        self.points_total(c) as i64 - self.points_spent(c) as i64
    }

    fn stat_mut(&mut self, s: Stat) -> &mut u8 {
        match s {
            Stat::Body => &mut self.stats.body,
            Stat::Reflex => &mut self.stats.reflex,
            Stat::Tech => &mut self.stats.tech,
            Stat::Cool => &mut self.stats.cool,
            Stat::Net => &mut self.stats.net,
        }
    }

    pub fn buy_stat(&mut self, c: &Content, s: Stat) -> Result<(), BuildError> {
        if self.stats.get(s) >= c.creation.stat_cap {
            return Err(BuildError::AtCap(s));
        }
        if self.points_left(c) < self.stat_cost(c, s) as i64 {
            return Err(BuildError::NoPoints);
        }
        *self.stat_mut(s) += 1;
        Ok(())
    }

    pub fn refund_stat(&mut self, c: &Content, s: Stat) -> Result<(), BuildError> {
        if self.stats.get(s) <= c.creation.stat_base {
            return Err(BuildError::AtBase(s));
        }
        *self.stat_mut(s) -= 1;
        Ok(())
    }

    pub fn sp_spent(&self, c: &Content) -> u32 {
        let cats: u32 = self
            .categories
            .iter()
            .filter_map(|id| c.category(id))
            .map(|cat| cat.cost)
            .sum();
        let skills: u32 = self.skills.iter().filter_map(|id| c.skill_cost(id)).sum();
        cats + skills
    }

    pub fn sp_left(&self, c: &Content) -> i64 {
        self.sp_total(c) as i64 - self.sp_spent(c) as i64
    }

    /// The SP a skill costs, or `None` while its category is not owned.
    pub fn skill_cost(&self, c: &Content, id: &SkillId) -> Option<u32> {
        let cat = c.category_of(id)?;
        if !self.categories.contains(&cat.id) {
            return None;
        }
        c.skill_cost(id)
    }

    /// Buys an unowned category (its free skills come along) or refunds an owned one
    /// (its skills go with it). Returns whether it is owned afterwards.
    pub fn toggle_category(&mut self, c: &Content, id: &CategoryId) -> Result<bool, BuildError> {
        let cat = c
            .category(id)
            .ok_or_else(|| BuildError::Unknown(id.to_string()))?;
        if self.categories.remove(id) {
            for s in &cat.skills {
                self.skills.remove(&s.id);
            }
            return Ok(false);
        }
        if self.sp_left(c) < cat.cost as i64 {
            return Err(BuildError::NoSp);
        }
        self.categories.insert(id.clone());
        for s in cat.skills.iter().filter(|s| s.cost == 0) {
            self.skills.insert(s.id.clone());
        }
        Ok(true)
    }

    /// Buys an unowned skill or refunds an owned one; the category must be owned.
    pub fn toggle_skill(&mut self, c: &Content, id: &SkillId) -> Result<bool, BuildError> {
        let cat = c
            .category_of(id)
            .ok_or_else(|| BuildError::Unknown(id.to_string()))?;
        if !self.categories.contains(&cat.id) {
            return Err(BuildError::CategoryLocked {
                skill: id.clone(),
                category: cat.name.clone(),
            });
        }
        if self.skills.remove(id) {
            return Ok(false);
        }
        let cost = c.skill_cost(id).unwrap_or(0);
        if self.sp_left(c) < cost as i64 {
            return Err(BuildError::NoSp);
        }
        self.skills.insert(id.clone());
        Ok(true)
    }

    fn check_name(&self, c: &Content) -> Result<(), BuildError> {
        let name = self.handle.trim();
        if name.is_empty() {
            return Err(BuildError::NoName);
        }
        if name.chars().count() > c.creation.name_max {
            return Err(BuildError::NameTooLong(c.creation.name_max));
        }
        if let Some(bad) = name
            .chars()
            .find(|ch| !(ch.is_alphanumeric() || NAME_MARKS.contains(*ch)))
        {
            return Err(BuildError::BadName(bad));
        }
        Ok(())
    }

    fn check_pools(&self, c: &Content) -> Result<(), BuildError> {
        let left = self.points_left(c);
        if left < 0 {
            return Err(BuildError::OverPoints((-left) as u32));
        }
        let sp = self.sp_left(c);
        if sp < 0 {
            return Err(BuildError::OverSp((-sp) as u32));
        }
        Ok(())
    }

    /// Whether a step may be left. Stats and skills only refuse an overspend.
    pub fn ready(&self, c: &Content, step: Step) -> Result<(), BuildError> {
        match step {
            Step::Name => self.check_name(c),
            Step::Origin => self.origin.as_ref().map(|_| ()).ok_or(BuildError::NoOrigin),
            Step::Profession => self
                .profession
                .as_ref()
                .map(|_| ())
                .ok_or(BuildError::NoProfession),
            Step::Stats | Step::Skills => self.check_pools(c),
            Step::Summary => self.validate(c),
        }
    }

    /// The first reason this build cannot be played, or none.
    pub fn validate(&self, c: &Content) -> Result<(), BuildError> {
        self.check_name(c)?;
        let o = self.origin.as_ref().ok_or(BuildError::NoOrigin)?;
        c.origin(o)
            .ok_or_else(|| BuildError::Unknown(o.to_string()))?;
        let p = self.profession.as_ref().ok_or(BuildError::NoProfession)?;
        c.profession(p)
            .ok_or_else(|| BuildError::Unknown(p.to_string()))?;
        for s in Stat::ALL {
            if self.stats.get(s) > c.creation.stat_cap {
                return Err(BuildError::AtCap(s));
            }
            if self.stats.get(s) < c.creation.stat_base {
                return Err(BuildError::AtBase(s));
            }
        }
        for id in &self.categories {
            c.category(id)
                .ok_or_else(|| BuildError::Unknown(id.to_string()))?;
        }
        for id in &self.skills {
            let cat = c
                .category_of(id)
                .ok_or_else(|| BuildError::Unknown(id.to_string()))?;
            if !self.categories.contains(&cat.id) {
                return Err(BuildError::CategoryLocked {
                    skill: id.clone(),
                    category: cat.name.clone(),
                });
            }
        }
        self.check_pools(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vex(c: &Content) -> Build {
        Build::from_template(c, c.template(&TemplateId::new("vex")).unwrap())
    }

    #[test]
    fn costs_follow_origin_and_profession_and_clamp() {
        let c = Content::load().unwrap();
        let b = vex(&c);
        assert_eq!(b.stat_cost(&c, Stat::Body), 3, "dear twice clamps to 3");
        assert_eq!(b.stat_cost(&c, Stat::Net), 1, "cheap twice clamps to 1");
        assert_eq!(b.stat_cost(&c, Stat::Reflex), 2);
        assert_eq!(b.affinity(&c, Stat::Tech), Affinity::Cheap);
        for o in &c.origins {
            for p in &c.professions {
                let mut x = Build::blank(&c);
                x.origin = Some(o.id.clone());
                x.profession = Some(p.id.clone());
                for s in Stat::ALL {
                    assert!(
                        (1..=3).contains(&x.stat_cost(&c, s)),
                        "{} {} {s:?}",
                        o.id,
                        p.id
                    );
                }
            }
        }
        let blank = Build::blank(&c);
        assert_eq!(blank.stat_cost(&c, Stat::Body), 2, "no picks: base cost");
    }

    #[test]
    fn templates_spend_exactly_their_pools() {
        let c = Content::load().unwrap();
        for t in &c.templates {
            let b = Build::from_template(&c, t);
            assert_eq!((b.points_spent(&c), b.sp_spent(&c)), (20, 6), "{}", t.id);
            assert_eq!(b.validate(&c), Ok(()), "{}", t.id);
        }
    }

    #[test]
    fn buying_and_refunding_stats() {
        let c = Content::load().unwrap();
        let mut b = Build::blank(&c);
        b.origin = Some(OriginId::new("slab_rat"));
        b.profession = Some(ProfessionId::new("solo"));
        assert_eq!(b.points_left(&c), 20);
        b.buy_stat(&c, Stat::Body).unwrap();
        assert_eq!(
            (b.stats.body, b.points_left(&c)),
            (4, 19),
            "body is cheap twice: 1"
        );
        assert_eq!(
            b.refund_stat(&c, Stat::Tech),
            Err(BuildError::AtBase(Stat::Tech))
        );
        for _ in 0..4 {
            b.buy_stat(&c, Stat::Body).unwrap();
        }
        assert_eq!(
            b.buy_stat(&c, Stat::Body),
            Err(BuildError::AtCap(Stat::Body))
        );
        b.stats.tech = 8;
        b.stats.cool = 8;
        assert!(b.points_left(&c) < 0, "{}", b.points_left(&c));
        assert_eq!(b.buy_stat(&c, Stat::Reflex), Err(BuildError::NoPoints));
        assert!(matches!(
            b.ready(&c, Step::Stats),
            Err(BuildError::OverPoints(_))
        ));
    }

    #[test]
    fn changing_origin_can_overspend_a_template() {
        let c = Content::load().unwrap();
        let mut b = vex(&c);
        b.origin = Some(OriginId::new("stowaway"));
        assert!(b.points_left(&c) < 0);
        assert!(matches!(b.validate(&c), Err(BuildError::OverPoints(_))));
        assert!(
            b.validate(&c)
                .unwrap_err()
                .to_string()
                .starts_with("over by ")
        );
    }

    #[test]
    fn categories_gate_skills_and_refund_them() {
        let c = Content::load().unwrap();
        let mut b = Build::blank(&c);
        let net = CategoryId::new("netrunner");
        let ice = SkillId::new("ice_breaker");
        assert_eq!(
            b.toggle_skill(&c, &ice).unwrap_err().to_string(),
            "unlock NETRUNNER first"
        );
        assert_eq!(b.toggle_category(&c, &net), Ok(true));
        assert!(
            b.skills.contains(&SkillId::new("hacking")),
            "the free first skill comes with it"
        );
        assert_eq!(b.sp_left(&c), 4);
        assert_eq!(b.toggle_skill(&c, &ice), Ok(true));
        assert_eq!(b.sp_left(&c), 3);
        assert_eq!(b.skill_cost(&c, &SkillId::new("vault_run")), Some(2));
        assert_eq!(b.toggle_category(&c, &net), Ok(false));
        assert!(
            b.skills.is_empty() && b.sp_left(&c) == 6,
            "refunding the category drops its skills"
        );
        b.toggle_category(&c, &CategoryId::new("wrench")).unwrap();
        b.toggle_category(&c, &CategoryId::new("iron")).unwrap();
        b.toggle_category(&c, &CategoryId::new("edge")).unwrap();
        assert_eq!(
            b.toggle_category(&c, &CategoryId::new("ghost")),
            Err(BuildError::NoSp)
        );
        assert_eq!(
            b.toggle_skill(&c, &SkillId::new("scrapcraft")),
            Err(BuildError::NoSp),
            "0 left"
        );
    }

    #[test]
    fn readiness_and_names() {
        let c = Content::load().unwrap();
        let mut b = Build::blank(&c);
        assert_eq!(b.ready(&c, Step::Name), Err(BuildError::NoName));
        b.handle = "   ".into();
        assert_eq!(b.ready(&c, Step::Name), Err(BuildError::NoName));
        b.handle = "a".repeat(17);
        assert_eq!(b.ready(&c, Step::Name), Err(BuildError::NameTooLong(16)));
        b.handle = "Vex\u{7}".into();
        assert_eq!(b.ready(&c, Step::Name), Err(BuildError::BadName('\u{7}')));
        b.handle = " Mara-7 ".into();
        assert_eq!(b.ready(&c, Step::Name), Ok(()));
        assert_eq!(b.ready(&c, Step::Origin), Err(BuildError::NoOrigin));
        assert_eq!(b.validate(&c), Err(BuildError::NoOrigin));
        b.origin = Some(OriginId::new("creche"));
        assert_eq!(b.ready(&c, Step::Profession), Err(BuildError::NoProfession));
        b.profession = Some(ProfessionId::new("fixer"));
        assert_eq!(b.ready(&c, Step::Skills), Ok(()), "unspent is fine");
        assert_eq!(b.validate(&c), Ok(()));
        assert_eq!(BuildError::NoName.to_string(), "no name on the sheet");
        assert_eq!(BuildError::NoOrigin.to_string(), "pick where you were made");
        assert_eq!(BuildError::NoProfession.to_string(), "pick what you did");
        assert_eq!(BuildError::OverSp(1).to_string(), "over by 1 SP");
        assert_eq!(BuildError::OverPoints(1).to_string(), "over by 1 point");
        assert_eq!(BuildError::OverPoints(2).to_string(), "over by 2 points");
    }

    #[test]
    fn a_character_rebuilds_into_a_build_with_its_pools() {
        let c = Content::load().unwrap();
        let mut ch =
            crate::character::Character::from_template(&c, &TemplateId::new("vex")).unwrap();
        ch.stat_points = 3;
        ch.sp = 1;
        let b = Build::from_character(&c, &ch);
        assert_eq!((b.points_left(&c), b.sp_left(&c)), (3, 1));
        assert_eq!(b.stats, ch.stats);
        assert_eq!(b.categories, ch.categories);
        let mut b = b;
        b.buy_stat(&c, Stat::Body).unwrap();
        assert_eq!(b.points_left(&c), 0, "a point in BODY costs Vex 3");
    }
}
