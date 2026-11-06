//! The ad line (Coz's book spec §5.1): a role's line torn through a holo ad on the
//! street. Put up by `Effect::Ad`, it waits until you have walked off from where you
//! stood, then for a holo ad in view as the news cut finds one (`news::ad_in_view`),
//! and shows there at once: the ad glows magenta and an aside goes up over it. Its
//! showing ends its step; thirty game minutes with none shown end it unshown.

use super::asides::{self, Speaker};
use super::{effects, fill, news, visitors};
use crate::app::AppState;
use crate::runs::ids::StepRef;
use citygen::{Place, Pos, StratumKind};

/// Tiles (Chebyshev) from where you stood as the line went up that are walking off.
pub const WALK_OFF: i32 = 6;
/// Game minutes the line waits to show before it ends unshown.
pub const WAIT_MINUTES: u64 = 30;
const STREET: Place = Place::Stratum(StratumKind::Street);

/// A line waiting to tear through a holo ad.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AdLine {
    /// The role saying it.
    pub from: String,
    /// What it says, filled.
    pub text: String,
    /// The step that put it up: showing it, or its minutes running out, ends that step.
    pub story: StepRef,
    /// The game minute it went up.
    pub since: u64,
    /// Where you stood then.
    pub at: (Place, Pos),
}

/// Puts the role's line up for the step `story`, its text filled, from where you stand.
/// A line already waiting has its step ended first.
pub fn put_up(st: &mut AppState, from: &str, text: &str, story: StepRef) {
    if let Some(old) = st.missions.ad.take()
        && let Some(run) = st.missions.runs.get_mut(old.story.mission)
    {
        // Marked, not `end`ed: that would run the story inside the step putting this one
        // up. The story's next pass takes it.
        run.ended.insert(old.story.step);
    }
    // Nobody on the map: nothing can show it, so it waits out its minutes.
    let at = st
        .player
        .as_ref()
        .map_or((STREET, Pos::new(0, 0)), |p| (p.place, p.pos));
    st.missions.ad = Some(AdLine {
        from: from.to_string(),
        text: fill(st, text),
        story,
        since: st.net.now(st),
        at,
    });
}

/// The line's frame, in every explore update: its `WAIT_MINUTES` up, it ends unshown.
/// Before that, with the map in walk mode (`walking`), once you have walked off (onto
/// another place, or `WALK_OFF` tiles from where you stood) and `news::ad_in_view` finds
/// an ad, it shows over the ad, labelled as the role's holo line is, in their colour; the
/// ad glows while the aside is up, and the step ends.
pub fn tick(st: &mut AppState, walking: bool) {
    let Some(line) = st.missions.ad.as_ref() else {
        return;
    };
    if st.net.now(st).saturating_sub(line.since) >= WAIT_MINUTES {
        let story = line.story;
        st.missions.ad = None;
        super::end(st, story.mission, story.step);
        return;
    }
    let (place, pos) = line.at;
    let walked_off = st
        .player
        .as_ref()
        .is_some_and(|p| p.place != place || p.pos.chebyshev(pos) >= WALK_OFF);
    if !walking || !walked_off {
        return;
    }
    let Some(ad) = news::ad_in_view(st) else {
        return;
    };
    let Some(line) = st.missions.ad.take() else {
        return;
    };
    let label = effects::holo_label(st, &line.from);
    let colour = visitors::colour(&st.book, &line.from);
    let until = st.clock_tick() + asides::lasts(&line.text);
    asides::say(st, Speaker::Tile(STREET, ad), label, colour, line.text);
    st.missions.ad_cut = Some((ad, until));
    super::end(st, line.story.mission, line.story.step);
}

/// The street tile of the holo ad the line tore through, while it glows.
pub fn cut_in(st: &AppState) -> Option<Pos> {
    let now = st.clock_tick();
    st.missions
        .ad_cut
        .filter(|&(_, until)| now < until)
        .map(|(ad, _)| ad)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::jobs::testutil::state_home;
    use crate::missions::news::tests::{afters, by_an_ad, look_from};
    use crate::missions::{Effect, Mark, Mission, Step, Trigger, marks};
    use crate::runs::file::{read, write};
    use crate::runs::testutil::{Dir, running};
    use crate::walker::Player;
    use citygen::Rect;

    const LINE: &str = "He's gone quiet. Work for {coz}. I'll find you. —S";
    const SAID: &str = "He's gone quiet. Work for Coz. I'll find you. —S";
    /// The step that puts the line up.
    const ASK: StepRef = StepRef::new(0, 0);

    /// A book of one mission from Seren: a step putting her line up as the story begins,
    /// and one after it ends.
    pub(crate) fn ad_then_after(st: &mut AppState) {
        let step = |id: &str, when: Trigger, then: Effect| Step {
            id: id.into(),
            when,
            then: vec![then],
        };
        let ad = Effect::Ad {
            from: "seren".into(),
            text: LINE.into(),
        };
        st.book.missions = vec![Mission {
            id: "m".into(),
            title: "M".into(),
            from: "seren".into(),
            steps: vec![
                step("ask", Trigger::NewGame, ad),
                step(
                    "after",
                    Trigger::Ended("ask".into()),
                    Effect::Log("after".into()),
                ),
            ],
        }];
        st.book.validate().unwrap();
    }

    /// Seed 8 with its home and the book of one mission, the story not begun.
    fn home() -> AppState {
        let mut st = state_home();
        ad_then_after(&mut st);
        st
    }

    /// The story begins where you stand: the line goes up.
    fn begin(st: &mut AppState) {
        crate::missions::begin(st);
        assert!(st.missions.ad.is_some(), "the line waits");
    }

    /// You in your flat.
    fn in_the_flat(st: &mut AppState) {
        let (place, pos) = marks::resolve(st, &Mark::HomeInside(1), None).unwrap();
        st.player = Some(Player::at(place, pos));
    }

    /// You stand on a free street tile `d` tiles (Chebyshev) from `from`, looking round,
    /// a holo ad in view.
    fn off_by(st: &mut AppState, from: Pos, d: i32) {
        let ring: Vec<Pos> = Rect::new(from.x, from.y, 1, 1)
            .grow(d)
            .iter()
            .filter(|p| p.chebyshev(from) == d)
            .collect();
        for p in ring {
            let free = st.city.as_ref().unwrap().walkable(StratumKind::Street, p)
                && st.world.as_ref().unwrap().person_at(p).is_none();
            if !free {
                continue;
            }
            look_from(st, STREET, p);
            if news::ad_in_view(st).is_some() {
                return;
            }
        }
        panic!("no free tile {d} off {from:?} with an ad in view");
    }

    /// Runs the clock to tick `at`.
    fn run_to(st: &mut AppState, at: u64) {
        if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
            world.advance(city, at);
        }
    }

    /// Runs the world on `minutes` game minutes.
    fn advance(st: &mut AppState, minutes: u64) {
        let at = st.clock_tick() + minutes * sim::TICKS_PER_MINUTE;
        run_to(st, at);
    }

    /// The step that put the line up has ended.
    fn ask_ended(st: &AppState) -> bool {
        st.missions.runs[0].ended.contains(&ASK.step)
    }

    #[test]
    fn the_line_waits_until_you_walk_off() {
        let mut st = home();
        by_an_ad(&mut st);
        let start = st.player.as_ref().unwrap().pos;
        begin(&mut st);
        let line = st.missions.ad.clone().unwrap();
        let want = AdLine {
            from: "seren".into(),
            text: SAID.into(),
            story: ASK,
            since: st.net.now(&st),
            at: (STREET, start),
        };
        assert_eq!(line, want);
        // Where you stood, the ad in sight: it waits.
        tick(&mut st, true);
        assert!(st.missions.asides.is_empty());
        assert_eq!(st.missions.ad.as_ref(), Some(&line));
        // A tile short of walking off, an ad in view: still.
        off_by(&mut st, start, WALK_OFF - 1);
        tick(&mut st, true);
        assert!(st.missions.asides.is_empty() && st.missions.ad.is_some());
        // Walked off, an ad in view, but the map not on foot: still.
        off_by(&mut st, start, WALK_OFF);
        tick(&mut st, false);
        assert!(st.missions.asides.is_empty() && st.missions.ad.is_some());
        // On foot: torn through that ad, labelled as her holo line is, in her colour.
        let ad = news::ad_in_view(&st).unwrap();
        tick(&mut st, true);
        let [a] = st.missions.asides.as_slice() else {
            panic!("one aside up: {:?}", st.missions.asides)
        };
        assert_eq!(a.speaker, Speaker::Tile(STREET, ad));
        let channel = news::channel(st.city.as_ref().unwrap());
        let stem: String = channel.chars().take(channel.chars().count() - 3).collect();
        assert_eq!(a.label, format!("{stem}▚▞▚ S"));
        assert_eq!(a.text, SAID);
        assert_eq!(a.colour, visitors::colour(&st.book, "seren"));
        let until = a.until;
        assert_eq!(cut_in(&st), Some(ad));
        // Shown: no longer waiting, its step ended and the one after it run.
        assert_eq!(st.missions.ad, None);
        assert!(ask_ended(&st));
        assert_eq!(afters(&st), 1);
        // The ad glows as long as the aside is up.
        run_to(&mut st, until - 1);
        assert_eq!(cut_in(&st), Some(ad));
        run_to(&mut st, until);
        assert_eq!(cut_in(&st), None);
        // Once.
        st.missions.asides.clear();
        tick(&mut st, true);
        assert!(st.missions.asides.is_empty());
        assert_eq!(afters(&st), 1);
    }

    #[test]
    fn another_place_counts_as_walking_off() {
        let mut st = home();
        let ad = by_an_ad(&mut st);
        let spot = st.player.as_ref().unwrap().pos;
        // Put up on your flat's floor, on the tile that is by number the street tile you
        // come out on: only the place differs.
        let flat = st.home.unwrap().place();
        st.player = Some(Player::at(flat, spot));
        begin(&mut st);
        look_from(&mut st, STREET, spot);
        tick(&mut st, true);
        let speakers: Vec<Speaker> = st.missions.asides.iter().map(|a| a.speaker).collect();
        assert_eq!(speakers, [Speaker::Tile(STREET, ad)]);
        assert_eq!(st.missions.ad, None);
        assert!(ask_ended(&st));
    }

    #[test]
    fn the_line_ends_unseen_after_thirty_minutes() {
        let mut st = home();
        in_the_flat(&mut st);
        begin(&mut st);
        advance(&mut st, WAIT_MINUTES - 1);
        tick(&mut st, true);
        assert!(st.missions.ad.is_some(), "a minute short, it waits");
        assert!(!ask_ended(&st));
        // On the minute it ends, unshown, the map on foot or not.
        advance(&mut st, 1);
        tick(&mut st, false);
        assert_eq!(st.missions.ad, None);
        assert!(st.missions.asides.is_empty(), "never shown");
        assert_eq!(cut_in(&st), None);
        assert!(ask_ended(&st));
        assert_eq!(afters(&st), 1, "the story goes on");
    }

    #[test]
    fn a_new_line_ends_the_one_waiting() {
        let mut st = home();
        // A second line goes up at once over the first; the step after waits on the first.
        let steps = &mut st.book.missions[0].steps;
        steps.insert(
            1,
            Step {
                id: "again".into(),
                when: Trigger::After { minutes: 0 },
                then: vec![Effect::Ad {
                    from: "seren".into(),
                    text: "Again.".into(),
                }],
            },
        );
        st.book.validate().unwrap();
        in_the_flat(&mut st);
        begin(&mut st);
        let line = st.missions.ad.as_ref().unwrap();
        assert_eq!(
            (line.text.as_str(), line.story),
            ("Again.", StepRef::new(0, 1))
        );
        assert!(ask_ended(&st), "the first line's step ended");
        assert_eq!(afters(&st), 1, "and the story went on in the same pass");
    }

    /// The line waiting in your flat, the run saved to `dir`: the state and its file.
    fn saved_waiting(dir: &Dir) -> (AppState, std::path::PathBuf) {
        let mut st = home();
        in_the_flat(&mut st);
        begin(&mut st);
        let st = running(st);
        let path = write(&dir.0, &st).unwrap();
        (st, path)
    }

    #[test]
    fn a_waiting_line_survives_a_reopen() {
        let dir = Dir::new("ad-line");
        let (st, path) = saved_waiting(&dir);
        let (_, s, lost) = read(&path, &st.book).unwrap();
        assert!(lost.is_empty(), "{lost:?}");
        assert!(s.missions.ad.is_some());
        assert_eq!(s.missions.ad, st.missions.ad);
    }

    #[test]
    fn a_lost_step_drops_the_line() {
        let dir = Dir::new("ad-lost");
        let (st, path) = saved_waiting(&dir);
        let mut book = st.book.clone();
        book.missions[0].steps.remove(ASK.step);
        let (_, s, lost) = read(&path, &book).unwrap();
        assert_eq!(lost, ["a step of m (ask)"]);
        assert_eq!(s.missions.ad, None);
    }
}
