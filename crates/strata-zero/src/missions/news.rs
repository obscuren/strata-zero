//! The news (morning news spec §2): the item the story put out last, its texts filled.
//! One is current at a time, for its `fresh` game minutes; every outlet shows it.

use super::fill;
use crate::app::AppState;

/// The channel's name in a city with no named corp.
const NO_CORP: &str = "CITY";

/// The news item put out last, its texts filled.
#[derive(Clone, Debug, PartialEq)]
pub struct NewsRun {
    /// The book's id for it.
    pub id: String,
    /// The mission and step that put it out; `usize::MAX` from the console.
    pub mission: usize,
    pub step: usize,
    /// The game minute it was put out.
    pub started: u64,
    /// Game minutes it stays current.
    pub fresh: u64,
    /// `VELFORM NEWSLINE`: one of the city's corps.
    pub channel: String,
    pub headline: String,
    pub report: Vec<String>,
    pub reads: Vec<String>,
    pub crawl: Vec<String>,
    pub overheard: Vec<String>,
    /// The role whose body the camera saw.
    pub about: String,
    /// The holo set in your flat is on.
    pub on: bool,
    /// The step that put it out has ended.
    pub ended: bool,
}

/// Puts out the book's news item `id` for step `step` of mission `mission`: its texts
/// filled, the set switched on, current in place of any earlier one.
pub fn put_out(st: &mut AppState, id: &str, mission: usize, step: usize) {
    let Some(item) = st.book.news.get(id) else {
        if st.dev {
            st.warn(format!("story · no news {id}"));
        }
        return;
    };
    let all = |texts: &[String]| texts.iter().map(|t| fill(st, t)).collect();
    let run = NewsRun {
        id: id.to_string(),
        mission,
        step,
        started: st.net.now(st),
        fresh: item.fresh,
        channel: st
            .city
            .as_ref()
            .map_or_else(|| format!("{NO_CORP} NEWSLINE"), channel),
        headline: fill(st, &item.headline),
        report: all(&item.report),
        reads: all(&item.reads),
        crawl: all(&item.crawl),
        overheard: all(&item.overheard),
        about: item.about.clone(),
        on: true,
        ended: false,
    };
    st.missions.news = Some(run);
}

/// The channel: the first word of the first named corp building's name, in capitals,
/// then `NEWSLINE`.
pub fn channel(city: &citygen::City) -> String {
    let corp = city
        .buildings
        .iter()
        .filter(|b| b.kind.is_corp())
        .find_map(|b| b.name.as_deref()?.split_whitespace().next())
        .map_or_else(|| NO_CORP.to_string(), str::to_uppercase);
    format!("{corp} NEWSLINE")
}

/// The item while it is fresh.
pub fn current(st: &AppState) -> Option<&NewsRun> {
    let now = st.net.now(st);
    st.missions
        .news
        .as_ref()
        .filter(|n| now.saturating_sub(n.started) < n.fresh)
}

#[allow(dead_code)] // the set's Watch asks it from the next tasks
/// Whether an item is fresh.
pub fn fresh(st: &AppState) -> bool {
    current(st).is_some()
}

#[allow(dead_code)] // the broadcast's `space` calls it from the next tasks
/// Switches the set off and ends the item's step, once.
pub fn switch_off(st: &mut AppState) {
    if let Some(n) = st.missions.news.as_mut() {
        n.on = false;
    }
    end_step(st);
}

/// Ends the step that put the item out, once; the set stays as it is.
pub fn end_step(st: &mut AppState) {
    let Some(n) = st.missions.news.as_mut().filter(|n| !n.ended) else {
        return;
    };
    n.ended = true;
    let (mission, step) = (n.mission, n.step);
    super::end(st, mission, step);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::testutil::state_home;
    use crate::missions::{Effect, Mission, RunState, Step, Trigger};

    /// Seed 8 with its home, the story begun.
    fn begun() -> AppState {
        let mut st = state_home();
        crate::missions::begin(&mut st);
        st
    }

    #[test]
    fn putting_out_fills_the_texts_and_names_the_channel() {
        let mut st = begun();
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        let n = current(&st).unwrap();
        assert!(n.on && !n.ended);
        assert_eq!(n.id, "marlo");
        assert!(
            n.headline.starts_with("Courier sought in "),
            "{}",
            n.headline
        );
        for t in n
            .report
            .iter()
            .chain(&n.reads)
            .chain(&n.crawl)
            .chain(&n.overheard)
            .chain([&n.headline])
        {
            assert!(!t.contains('{'), "{t}");
        }
        assert!(n.channel.ends_with(" NEWSLINE") && n.channel == n.channel.to_uppercase());
        assert_eq!(n.channel, channel(st.city.as_ref().unwrap()));
        assert_eq!(n.started, st.net.now(&st));
        assert_eq!(n.fresh, 1440);
        assert!(fresh(&st));
    }

    #[test]
    fn the_channel_is_a_corps_first_word() {
        let mut city = state_home().city.unwrap();
        let corps: Vec<usize> = (0..city.buildings.len())
            .filter(|&i| city.buildings[i].kind.is_corp())
            .collect();
        assert!(corps.len() >= 2, "seed 8 has corps");
        for &i in &corps {
            city.buildings[i].name = None;
        }
        // The first corp with a name; an unnamed one before it and other kinds don't count.
        city.buildings[corps[1]].name = Some("Velform Group Plaza".into());
        city.buildings[corps[1] + 1..]
            .iter_mut()
            .filter(|b| b.kind.is_corp())
            .for_each(|b| b.name = Some("Miracorp Tower".into()));
        assert_eq!(channel(&city), "VELFORM NEWSLINE");
        for &i in &corps {
            city.buildings[i].name = None;
        }
        assert_eq!(channel(&city), "CITY NEWSLINE");
    }

    /// Runs the world on `minutes` game minutes.
    fn advance(st: &mut AppState, minutes: u64) {
        if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
            world.advance(city, world.tick() + minutes * sim::TICKS_PER_MINUTE);
        }
    }

    #[test]
    fn news_goes_stale() {
        let mut st = begun();
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        let fresh_for = current(&st).unwrap().fresh;
        advance(&mut st, fresh_for - 1);
        assert!(fresh(&st) && current(&st).is_some());
        advance(&mut st, 1);
        assert!(!fresh(&st));
        assert!(current(&st).is_none());
        // Still kept, so its step can be ended.
        assert!(st.missions.news.is_some());
    }

    #[test]
    fn a_new_item_replaces_the_old() {
        let mut st = begun();
        let mut other = st.book.news["marlo"].clone();
        other.headline = "Lift nine out again".into();
        st.book.news.insert("lift".into(), other);
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        switch_off(&mut st);
        put_out(&mut st, "lift", 2, 3);
        let n = current(&st).unwrap();
        assert_eq!(
            (n.id.as_str(), n.headline.as_str(), n.mission, n.step),
            ("lift", "Lift nine out again", 2, 3)
        );
        assert!(n.on && !n.ended);
        // An id the book lacks changes nothing.
        put_out(&mut st, "nope", 0, 0);
        assert_eq!(current(&st).unwrap().id, "lift");
    }

    #[test]
    fn switching_off_ends_the_step_once() {
        let mut st = state_home();
        let step = |id: &str, when: Trigger, then: Effect| Step {
            id: id.into(),
            when,
            then: vec![then],
        };
        st.book.missions = vec![Mission {
            id: "m".into(),
            title: "M".into(),
            from: "mezz".into(),
            steps: vec![
                step("n", Trigger::NewGame, Effect::News("marlo".into())),
                step(
                    "after",
                    Trigger::Ended("n".into()),
                    Effect::Log("after".into()),
                ),
            ],
        }];
        st.book.validate().unwrap();
        crate::missions::begin(&mut st);
        let n = current(&st).expect("the step put it out");
        assert_eq!((n.mission, n.step), (0, 0));
        assert_eq!(st.missions.runs[0].state, RunState::Open);
        switch_off(&mut st);
        switch_off(&mut st);
        let afters = st.log.iter().filter(|e| e.text == "after").count();
        assert_eq!(afters, 1);
        assert!(matches!(st.missions.runs[0].state, RunState::Done { .. }));
        let n = st.missions.news.as_ref().unwrap();
        assert!(!n.on && n.ended);
    }

    #[test]
    fn ending_the_step_leaves_the_set_on() {
        let mut st = begun();
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        end_step(&mut st);
        let n = current(&st).unwrap();
        assert!(n.on && n.ended);
    }

    #[test]
    fn putting_out_without_a_home_or_city_does_not_panic() {
        let mut st = AppState::new();
        put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        assert_eq!(current(&st).unwrap().channel, "CITY NEWSLINE");
        switch_off(&mut st);
        assert!(!current(&st).unwrap().on);
    }
}
