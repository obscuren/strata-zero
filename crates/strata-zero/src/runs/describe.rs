//! What a run's header says of it, and the short forms the run lists print.

use super::{Death, RunHeader, RunId};
use crate::app::{AppState, FPS};
use citygen::{FloorPlan, Place, StratumKind};

/// The wall clock in Unix seconds.
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The header for the run in `st` as it stands, saved now, with how it ended if it has.
pub fn header(st: &AppState, death: Option<Death>) -> RunHeader {
    let ch = &st.character;
    let content = &st.content;
    let profession = content
        .profession(&ch.profession)
        .map_or_else(|| ch.profession.to_string(), |p| p.name.clone());
    let chrome = ch
        .chrome
        .values()
        .map(|id| {
            content
                .item(id)
                .map_or_else(|| id.to_string(), |i| i.name.clone())
        })
        .collect();
    let (mission, objective) = story(st);
    let city = st.city.as_ref();
    RunHeader {
        id: st.run.map_or(RunId(0), |r| r.id),
        handle: ch.handle.clone(),
        profession,
        level: ch.level,
        city: city.map_or_else(String::new, |c| c.name.clone()),
        seed: city.map_or(0, |c| c.seed),
        day: crate::ui::hud::day_at(st.clock_tick()),
        minute: st.minute_of_day(),
        place: place_line(st),
        mission,
        objective,
        perma: st.perma_death,
        credits: ch.credits,
        chrome,
        talents: ch.talents.values().filter(|r| **r > 0).count() as u32,
        played: st
            .run
            .map_or(0, |r| st.tick.saturating_sub(r.started) / FPS),
        saved: unix_now(),
        death,
    }
}

/// Where you stand, as the run's card says it: `{building} {floor} · at home` in the
/// flat, `{building} {floor}` on another floor, the district on the street, `on the
/// Net` jacked in. Empty before you stand anywhere.
pub fn place_line(st: &AppState) -> String {
    let (Some(city), Some(p)) = (st.city.as_ref(), st.player.as_ref()) else {
        return String::new();
    };
    if p.body.is_some() {
        return "on the Net".to_string();
    }
    match st.outside.as_ref().map(|o| o.at) {
        Some(crate::outside::At::Overworld(_)) => return "overworld".to_string(),
        Some(crate::outside::At::Ground(_)) => return "open ground".to_string(),
        _ => {}
    }
    match p.place {
        Place::Floor { building, level } => {
            let at = format!(
                "{} {}",
                crate::net::building_name(st, building),
                FloorPlan::label(level)
            );
            let home = st
                .home
                .is_some_and(|r| r.place() == p.place && r.home.rect.contains(p.pos));
            if home { format!("{at} · at home") } else { at }
        }
        Place::Construct(_) | Place::Stratum(StratumKind::Net) => "on the Net".to_string(),
        Place::Stratum(k) => city
            .tile(k, p.pos)
            .and_then(|t| t.district)
            .or_else(|| city.tile(StratumKind::Street, p.pos)?.district)
            .and_then(|d| city.district_opt(d))
            .map_or_else(|| city.name.clone(), |d| d.name.clone()),
    }
}

/// The title and the current objective of the first mission in book order that is
/// under way.
pub fn story(st: &AppState) -> (Option<String>, Option<String>) {
    // away in another city, the home story is set aside with the home city
    let missions = st.away.as_ref().map_or(&st.missions, |a| &a.missions);
    missions
        .runs
        .iter()
        .zip(&st.book.missions)
        .find(|(run, _)| run.state == crate::missions::RunState::Open)
        .map_or((None, None), |(run, m)| {
            (Some(m.title.clone()), run.objectives.last().cloned())
        })
}

/// Time played: `3h 12m`, `40m`.
pub fn played(secs: u64) -> String {
    let (h, m) = (secs / 3600, secs % 3600 / 60);
    if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

/// City time a run lasted, in its largest whole unit: `3 days`, `19 hours`, `40 min`.
pub fn lasted(minutes: u64) -> String {
    let plural = |n: u64, one: &str| {
        if n == 1 {
            format!("1 {one}")
        } else {
            format!("{n} {one}s")
        }
    };
    match minutes {
        m if m >= 24 * 60 => plural(m / (24 * 60), "day"),
        m if m >= 60 => plural(m / 60, "hour"),
        m => format!("{m} min"),
    }
}

/// Money: `¤ 1,240`.
pub fn credits(n: u32) -> String {
    let digits = n.to_string();
    let mut out = String::from("¤ ");
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// When a run was saved, against `now` (both Unix seconds): `today 10:44` on the same
/// UTC date, else `2026-11-04 10:44`.
pub fn saved_at(unix: u64, now: u64) -> String {
    let full = crate::screens::files::fmt_unix(unix);
    if unix / 86_400 == now / 86_400 {
        let (_, clock) = full.split_once(' ').unwrap_or(("", &full));
        format!("today {clock}")
    } else {
        full
    }
}

/// The UTC date of a Unix time: `2026-11-02`.
pub fn date(unix: u64) -> String {
    let full = crate::screens::files::fmt_unix(unix);
    match full.split_once(' ') {
        Some((day, _)) => day.to_string(),
        None => full,
    }
}

/// The city's clock in a run: `day 2 07:40`.
pub fn clock(day: u32, minute: u16) -> String {
    format!("day {day} {:02}:{:02}", minute / 60, minute % 60)
}

/// An objective up to its first ` · `: `Bring it to Coz`.
pub fn first_part(objective: &str) -> &str {
    objective
        .split_once(" · ")
        .map_or(objective, |(first, _)| first)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use citygen::{FloorPlan, Place, Pos, StratumKind};

    /// 2026-11-04 10:44 UTC.
    const AT: u64 = 1_793_789_040;

    #[test]
    fn a_run_saved_out_in_the_world_says_where() {
        let mut st = crate::outside::moves::tests::on_the_world();
        crate::outside::ensure(&mut st).unwrap().at = crate::outside::At::Overworld(Pos::new(3, 3));
        assert_eq!(place_line(&st), "overworld");
        st.outside.as_mut().unwrap().at = crate::outside::At::Ground(Pos::new(30, 30));
        assert_eq!(place_line(&st), "open ground");
    }

    #[test]
    fn time_played_reads_in_hours_and_minutes() {
        assert_eq!(played(3 * 3600 + 12 * 60 + 59), "3h 12m");
        assert_eq!(played(40 * 60), "40m");
        assert_eq!(played(59), "0m");
        assert_eq!(played(2 * 3600), "2h 0m");
    }

    #[test]
    fn a_life_lasts_days_hours_or_minutes() {
        assert_eq!(lasted(3 * 1440 + 5 * 60), "3 days");
        assert_eq!(lasted(19 * 60 + 30), "19 hours");
        assert_eq!(lasted(40), "40 min");
        assert_eq!(lasted(1440), "1 day");
        assert_eq!(lasted(60), "1 hour");
        assert_eq!(lasted(0), "0 min");
    }

    #[test]
    fn money_reads_in_thousands() {
        assert_eq!(credits(1240), "¤ 1,240");
        assert_eq!(credits(2340), "¤ 2,340");
        assert_eq!(credits(999), "¤ 999");
        assert_eq!(credits(0), "¤ 0");
        assert_eq!(credits(1_000_000), "¤ 1,000,000");
    }

    #[test]
    fn a_save_from_today_says_today() {
        assert_eq!(saved_at(AT, AT + 3600), "today 10:44");
        assert_eq!(saved_at(AT, AT + 86_400), "2026-11-04 10:44");
        // Past midnight is another day.
        let late = AT - 10 * 3600 - 44 * 60 + 23 * 3600 + 59 * 60;
        assert_eq!(saved_at(late, late + 120), "2026-11-04 23:59");
    }

    #[test]
    fn a_death_is_dated_by_its_day() {
        assert_eq!(date(AT), "2026-11-04");
        assert_eq!(date(AT - 2 * 86_400), "2026-11-02");
        assert_eq!(date(0), "1970-01-01");
    }

    #[test]
    fn the_clock_reads_day_and_time() {
        assert_eq!(clock(2, 7 * 60 + 40), "day 2 07:40");
        assert_eq!(clock(1, 0), "day 1 00:00");
        assert_eq!(clock(14, 23 * 60 + 59), "day 14 23:59");
    }

    #[test]
    fn the_first_part_of_an_objective_is_up_to_its_first_dot() {
        assert_eq!(
            first_part("Bring it to Coz · the Brass Lamp · Ironside"),
            "Bring it to Coz"
        );
        assert_eq!(first_part("Bring it to Coz"), "Bring it to Coz");
        assert_eq!(first_part(""), "");
    }

    #[test]
    fn the_story_is_the_first_mission_under_way() {
        use crate::missions::{Run, RunState};
        let mut st = AppState::new();
        assert_eq!(story(&st), (None, None), "no story before the city");
        let run = |state, objectives: &[&str]| Run {
            at: 0,
            since: 0,
            state,
            objectives: objectives.iter().map(|s| s.to_string()).collect(),
            ended: Default::default(),
            began: None,
        };
        st.missions.runs = st
            .book
            .missions
            .iter()
            .map(|_| run(RunState::Waiting, &[]))
            .collect();
        assert!(st.missions.runs.len() >= 3);
        st.missions.runs[0] = run(RunState::Done { at: 4 }, &["over"]);
        st.missions.runs[1] = run(RunState::Open, &["first", "now this"]);
        st.missions.runs[2] = run(RunState::Open, &["later"]);
        assert_eq!(
            story(&st),
            (
                Some(st.book.missions[1].title.clone()),
                Some("now this".to_string())
            )
        );
        st.missions.runs[1] = run(RunState::Open, &[]);
        assert_eq!(story(&st), (Some(st.book.missions[1].title.clone()), None));
    }

    #[test]
    fn the_place_line_says_where_you_stand() {
        let mut st = AppState::new();
        st.enter_city(
            citygen::Generator::run(citygen::GenParams {
                seed: 8,
                size: citygen::SizePreset::Medium,
                districts: 7,
                ..Default::default()
            })
            .unwrap(),
        );
        assert_eq!(place_line(&st), "", "nobody stands anywhere yet");
        let _ = crate::screens::explore::MapScreen::walking(&mut st);
        let home = st.home.unwrap().home;
        let name = crate::net::building_name(&st, home.building);
        let floor = FloorPlan::label(home.level);
        assert_eq!(place_line(&st), format!("{name} {floor} · at home"));

        let place = Place::Floor {
            building: home.building,
            level: home.level,
        };
        let outside = Pos::new(home.rect.x - 3, home.rect.y);
        st.player = Some(crate::walker::Player::at(place, outside));
        assert_eq!(place_line(&st), format!("{name} {floor}"));

        let city = st.city.as_ref().unwrap();
        let (pos, district) = city
            .stratum(StratumKind::Street)
            .unwrap()
            .tiles
            .iter()
            .find_map(|(p, t)| {
                let d = t.district?;
                (t.building.is_none()).then(|| (p, city.district(d).name.clone()))
            })
            .unwrap();
        st.player = Some(crate::walker::Player::at(
            Place::Stratum(StratumKind::Street),
            pos,
        ));
        assert_eq!(place_line(&st), district);

        let mut p = crate::walker::Player::at(Place::Stratum(StratumKind::Net), pos);
        p.body = Some((place, home.bed_side));
        st.player = Some(p);
        assert_eq!(place_line(&st), "on the Net");
    }
}
