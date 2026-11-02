//! What Space offers: the things in reach, one window each, with fixed verbs on fixed
//! letters (`x` look, `t` talk, `i` interact, `j` jack, `g` take, `>` `<` go).

use crate::app::AppState;
use crate::render::map::MapRef;
use crate::render::map::TileMap;
use crate::render::mark::{mark_items, mark_person, mark_tile};
use crate::render::people::card_lines;
use crate::screens::popup::{Choice, Header, Pick, Popup, WorldAct};
use crate::ui::chrome::{CYAN, MAGENTA, TEXT};
use crate::ui::hud::{AMBER, capitalised};
use citygen::{Dir, Feature, FloorPlan, Furniture, Place, Pos};
use ratatui::style::Color;

/// Letters for take, ride and program rows: no verb letter, none of the window's
/// own keys (`h j k l q`).
pub const FREE_LETTERS: &str = "abcdefmnopsuvwyz";

/// One thing in reach and what can be done to it.
#[derive(Clone, Debug)]
pub struct Subject {
    pub pos: Pos,
    /// Where it lies from the player; `None` for the player's own tile.
    pub dir: Option<Dir>,
    pub header: Option<Header>,
    pub subtitle: Option<String>,
    /// A paragraph under the subtitle, word-wrapped: what someone says.
    pub body: Vec<String>,
    /// Its verbs, Look not included: `window` puts Look first.
    pub choices: Vec<Choice>,
}

impl Subject {
    /// What the which-way prompt calls it.
    pub fn name(&self) -> String {
        self.header
            .as_ref()
            .map_or_else(|| "here".to_string(), |h| h.name.clone())
    }

    /// Its window: Look first, then its verbs.
    pub fn window(&self, tick: u64) -> Popup {
        let mut rows = vec![Choice::new('x', "Look", Pick::Look(self.pos))];
        rows.extend(self.choices.iter().cloned());
        Popup::about(self.header.clone(), self.subtitle.clone(), rows, tick)
            .with_body(self.body.clone())
    }
}

/// A header named for a thing, in the colour its glyph is drawn in.
fn header(name: impl Into<String>, glyph: Option<(char, Color)>) -> Header {
    Header {
        glyph,
        name: name.into(),
        fg: glyph.map_or(TEXT, |(_, c)| c),
    }
}

/// What a Take row calls a stack. A copy carries the slot picker's marks, so two
/// copies of one item read apart.
fn ground_name(st: &AppState, item: &game::Item, s: &game::Stack) -> String {
    let name = stack_name(&item.name, s.qty);
    let Some(copy) = s.copy.as_ref() else {
        return name;
    };
    let marks: String = crate::screens::deck::marks(st, item, Some(copy), false, false)
        .iter()
        .map(|m| m.content.as_ref())
        .collect();
    if marks.is_empty() {
        name
    } else {
        format!("{name} {marks}")
    }
}

/// Takes a stack off the ground: a copy by its id, so it comes back with what is
/// fitted in it; anything else by item and count.
fn take(s: &game::Stack) -> Pick {
    Pick::Act(match s.copy_id() {
        Some(id) => game::Action::TakeCopy(id),
        None => game::Action::Take {
            item: s.item.clone(),
            qty: s.qty,
        },
    })
}

/// A stack as rows and headers name it: "Copper coil ×2", the count only above one.
fn stack_name(name: &str, qty: u32) -> String {
    if qty > 1 {
        format!("{name} ×{qty}")
    } else {
        name.to_string()
    }
}

/// Adds rows for a thing at `at`: onto the player's own tile, onto a subject already
/// on that tile, or as a new subject.
fn add(
    subs: &mut Vec<Subject>,
    here: &mut Subject,
    at: Pos,
    head: Option<Header>,
    subtitle: Option<String>,
    rows: Vec<Choice>,
) {
    if at == here.pos {
        here.choices.extend(rows);
        return;
    }
    if let Some(s) = subs.iter_mut().find(|s| s.pos == at) {
        s.choices.extend(rows);
        return;
    }
    subs.push(Subject {
        pos: at,
        dir: Dir::ALL.into_iter().find(|d| here.pos.step(*d) == at),
        header: head,
        subtitle,
        body: Vec::new(),
        choices: rows,
    });
}

/// The things in reach of the player, their own tile first, then by direction.
/// Constructs have none: their window is the program list.
pub fn gather(
    st: &AppState,
    inside_person: &dyn Fn(Place, Pos) -> Option<sim::PersonId>,
) -> Vec<Subject> {
    let (Some(city), Some(player)) = (st.city.as_ref(), st.player.as_ref()) else {
        return Vec::new();
    };
    let (place, pos) = (player.place, player.pos);
    if matches!(place, Place::Construct(_)) {
        return Vec::new();
    }
    let Some(map) = MapRef::for_place(
        city,
        &st.interiors,
        &st.constructs,
        place,
        st.minute_of_day(),
    ) else {
        return Vec::new();
    };
    let mut subs: Vec<Subject> = Vec::new();
    let mut here = Subject {
        pos,
        dir: None,
        header: None,
        subtitle: None,
        body: Vec::new(),
        choices: Vec::new(),
    };
    let mut letters = FREE_LETTERS.chars();

    // People beside you: on the street the city's walkers, indoors the occupants.
    for n in pos.neighbors4() {
        let id = if place.is_street() {
            st.world.as_ref().and_then(|w| w.person_at(n))
        } else {
            inside_person(place, n)
        };
        let (Some(id), Some(world)) = (id, st.world.as_ref()) else {
            continue;
        };
        let card = world.card(id, city);
        let said = format!("{}: \u{201c}{}\u{201d}", card.name, card.line);
        let lines = card_lines(&card).into_iter().skip(1).collect();
        // A fixer talks work; anyone may be the hands a package is bound for.
        let mut rows = if world.people()[id.0 as usize].profession == "fixer" {
            crate::jobs::ui::rows(st, id)
        } else {
            vec![Choice::new(
                't',
                "Talk",
                Pick::Talk {
                    lines,
                    said,
                    who: Some(id),
                },
            )]
        };
        rows.extend(crate::jobs::ui::receiver_rows(st, id));
        add(
            &mut subs,
            &mut here,
            n,
            Some(header(card.name.clone(), Some(mark_person(st, id)))),
            Some(format!("{} · {}", card.profession, card.doing)),
            rows,
        );
    }

    // A shut clinic's door beside you: when it opens.
    if let Some(k) = place.stratum() {
        for d in pos.neighbors4() {
            let Some(b) = city
                .tile(k, d)
                .filter(|t| t.terrain == citygen::Terrain::Door)
                .and_then(|t| t.building)
                .filter(|b| city.portals(*b).contains(&(k, d)))
                .filter(|b| !crate::clinic::is_open(st, *b))
            else {
                continue;
            };
            // Someone standing in the doorway keeps their window; the door's hours go
            // on it as a greyed row.
            if let Some(s) = subs.iter_mut().find(|s| s.pos == d) {
                s.choices.push(
                    Choice {
                        thing: Some((crate::clinic::name(st, b), AMBER)),
                        ..Choice::disabled('i', "Door")
                    }
                    .tail(format!(
                        " · CLOSED · opens {}",
                        crate::clinic::hhmm(crate::clinic::hours(st).0)
                    )),
                );
                continue;
            }
            add(
                &mut subs,
                &mut here,
                d,
                Some(header(crate::clinic::name(st, b), Some(('▯', AMBER)))),
                Some(crate::clinic::hours_line(st)),
                Vec::new(),
            );
        }
    }

    // A ripperdoc's chair beside you: sit in it, while the doc is in.
    if let Place::Floor { building, level } = place
        && let Some(chair) = st
            .interiors
            .floor(building, level)
            .and_then(|(_, f)| f.chair())
            .filter(|c| pos.neighbors4().any(|n| n == *c))
    {
        let doc = crate::clinic::doc_at_work(st, building);
        let (subtitle, said, row) = match doc {
            Some(id) => (
                Some(crate::clinic::doc_name(st, id)),
                vec![format!("“{}”", crate::clinic::doc_line(st, id))],
                Choice::on(
                    'i',
                    "Interact",
                    "Sit in the chair",
                    AMBER,
                    Pick::Chair { building, level },
                ),
            ),
            None => (
                None,
                Vec::new(),
                Choice {
                    thing: Some(("Sit in the chair".to_string(), AMBER)),
                    ..Choice::disabled('i', "Interact")
                }
                .tail(" · the doc is out"),
            ),
        };
        add(
            &mut subs,
            &mut here,
            chair,
            Some(header("Operating chair", mark_tile(st, &map, chair))),
            subtitle,
            vec![row],
        );
        // What the doc says, wrapped under their name.
        if let Some(s) = subs.iter_mut().find(|s| s.pos == chair) {
            s.body = said;
        }
    }

    // A locked flat's door beside you: its keypad.
    if let Place::Floor { building, level } = place
        && let Some((_, floor)) = st.interiors.floor(building, level)
    {
        let now = st.net.now(st);
        for d in pos.neighbors4() {
            let Some(r) = floor.door_of(d).filter(|r| {
                r.private && r.kind == citygen::RoomKind::Flat && !r.rect.contains(pos)
            }) else {
                continue;
            };
            let mut row = Choice::on(
                'i',
                "Interact",
                "Keypad",
                AMBER,
                Pick::Keypad {
                    building,
                    level,
                    number: r.number,
                },
            );
            if let Some(until) = st
                .journal
                .keypad_locked_until(building, level, r.number, now)
            {
                row = row.tail(format!(" · locked {}m", until.saturating_sub(now).max(1)));
            }
            let name = capitalised(&crate::credentials::flat_label(
                st, building, level, r.number,
            ));
            add(
                &mut subs,
                &mut here,
                d,
                Some(header(name, mark_tile(st, &map, d))),
                None,
                vec![row],
            );
        }
    }

    // The machine within reach: the rig at home, a district terminal on the street.
    if !player.jacked_in()
        && let Some(port) = player.terminal_in_reach(city, st.home.as_ref())
    {
        use crate::helix::shell::HostId;
        let (origin, tile, name) = match (place, st.home) {
            (Place::Floor { .. }, Some(rig)) => (Some(HostId::Rig), rig.home.rig, "Access rig"),
            (Place::Stratum(k), _) => (
                crate::access::district_of_terminal(city, k, port).map(HostId::District),
                port,
                "Access terminal",
            ),
            _ => (None, port, "Access terminal"),
        };
        let mut rows = Vec::new();
        if let Some(origin) = origin {
            rows.push(Choice::on(
                'i',
                "Interact",
                "Terminal",
                CYAN,
                Pick::Terminal {
                    origin,
                    key: (place, tile),
                },
            ));
        }
        rows.push(Choice::new('j', "Jack in", Pick::World(WorldAct::JackIn)));
        add(
            &mut subs,
            &mut here,
            tile,
            Some(header(name, mark_tile(st, &map, tile))),
            None,
            rows,
        );
    }

    // Your own bed beside you, in your flat: sleep in it.
    if let Some(rig) = st.home.as_ref()
        && place == rig.place()
        && let Some(bed) = pos.neighbors4().find(|n| {
            rig.home.rect.contains(*n)
                && map
                    .tile(*n)
                    .is_some_and(|t| t.feature == Some(Feature::Furniture(Furniture::Bed)))
        })
    {
        add(
            &mut subs,
            &mut here,
            bed,
            Some(header("Bed", mark_tile(st, &map, bed))),
            None,
            vec![Choice::on('i', "Interact", "Sleep", CYAN, Pick::Sleep)],
        );
    }

    // Your own holo set beside you, in your flat, while there is news: watch it.
    if let Some(rig) = st.home.as_ref()
        && place == rig.place()
        && crate::missions::news::fresh(st)
        && let Some(set) = pos.neighbors4().find(|n| {
            rig.home.rect.contains(*n)
                && map
                    .tile(*n)
                    .is_some_and(|t| t.feature == Some(Feature::Furniture(Furniture::Holo)))
        })
    {
        add(
            &mut subs,
            &mut here,
            set,
            Some(header("Holo set", mark_tile(st, &map, set))),
            None,
            vec![Choice::on('i', "Interact", "Watch", CYAN, Pick::Watch)],
        );
    }

    // Your own tile: jack out, what lies there, the ways up and down.
    let mut here_rows = Vec::new();
    let mut fixture: Option<&str> = None;
    if player.jacked_in() {
        here_rows.push(Choice::new('j', "Jack out", Pick::JackOutAsk));
    }
    let stacks = st.ground.at(place, pos);
    if let [s] = stacks
        && let Some(item) = st.content.item(&s.item)
    {
        here_rows.push(Choice::on(
            'g',
            "Take",
            ground_name(st, item, s),
            AMBER,
            take(s),
        ));
    } else if stacks.len() > 1 {
        here_rows.push(Choice::new(
            'g',
            "Take all",
            Pick::Act(game::Action::TakeAll),
        ));
        for s in stacks {
            let Some(item) = st.content.item(&s.item) else {
                continue;
            };
            let Some(key) = letters.next() else { break };
            here_rows.push(Choice::on(
                key,
                "Take",
                ground_name(st, item, s),
                AMBER,
                take(s),
            ));
        }
    }
    if let Some(k) = place.stratum() {
        let (mut up, mut down) = (None, None);
        for c in city
            .connectors_at(pos)
            .filter(|c| c.kind != citygen::ConnectorKind::Jack)
        {
            // Only directions that lead somewhere walkable: a row never refuses after being offered.
            if let Some((below, above)) = c.serves_from(k) {
                let (u, d) = (
                    above.filter(|d| city.walkable(*d, pos)),
                    below.filter(|d| city.walkable(*d, pos)),
                );
                if (u.is_some() || d.is_some()) && fixture.is_none() {
                    fixture = Some(match c.kind {
                        citygen::ConnectorKind::Stair => "Stairs",
                        citygen::ConnectorKind::Lift => "Lift",
                        citygen::ConnectorKind::Ladder => "Ladder",
                        _ => "Manhole",
                    });
                }
                up = up.or(u);
                down = down.or(d);
            }
        }
        if let Some(dest) = up {
            here_rows.push(Choice::on(
                '>',
                "Go up",
                dest.label(),
                TEXT,
                Pick::World(WorldAct::Up),
            ));
        }
        if let Some(dest) = down {
            here_rows.push(Choice::on(
                '<',
                "Go down",
                dest.label(),
                TEXT,
                Pick::World(WorldAct::Down),
            ));
        }
    }
    // Inside: the lift panel on a lift bank, the next floors on the stairs.
    if let Place::Floor { building, level } = place
        && let Some((plan, floor)) = st.interiors.floor(building, level)
    {
        match floor.tiles.get(pos).map(|t| t.terrain) {
            Some(citygen::Terrain::Lift) => {
                fixture = Some("Lift");
                here_rows.extend(ride_rows(st, &mut letters).unwrap_or_default());
            }
            Some(citygen::Terrain::Stair) => {
                fixture = Some("Stairwell");
                if level < plan.top {
                    here_rows.push(Choice::on(
                        '>',
                        "Go up",
                        FloorPlan::label(level + 1),
                        TEXT,
                        Pick::World(WorldAct::Climb(true)),
                    ));
                }
                if level > plan.lowest {
                    here_rows.push(Choice::on(
                        '<',
                        "Go down",
                        FloorPlan::label(level - 1),
                        TEXT,
                        Pick::World(WorldAct::Climb(false)),
                    ));
                }
            }
            _ => {}
        }
    }
    here.choices.splice(0..0, here_rows);
    if !here.choices.is_empty() {
        here.header = match fixture {
            Some(name) => Some(header(name, mark_tile(st, &map, pos))),
            None => stacks.first().and_then(|s| {
                let item = st.content.item(&s.item)?;
                Some(header(
                    stack_name(&item.name, s.qty),
                    mark_items(st, place, pos),
                ))
            }),
        };
    }
    subs.sort_by_key(|s| s.dir.and_then(|d| Dir::ALL.iter().position(|x| *x == d)));
    if !here.choices.is_empty() {
        subs.insert(0, here);
    }
    subs
}

/// The lift panel's rows: every floor of the building, the current one greyed, the one
/// the track wants you on marked `⌖` in cyan.
pub fn ride_rows(st: &AppState, letters: &mut std::str::Chars) -> Option<Vec<Choice>> {
    let player = st.player.as_ref()?;
    let Place::Floor { building, level } = player.place else {
        return None;
    };
    let plan = st.interiors.plan_of(building)?;
    let wanted = crate::track::floor_wanted(st, building).filter(|w| *w != level);
    Some(
        crate::interiors::floor_rows(plan, level)
            .into_iter()
            .rev()
            .map(|(l, label)| {
                let key = letters.next().unwrap_or(' ');
                let (floor, marks) = match label.split_once(" · ") {
                    Some((f, m)) => (f.to_string(), format!(" · {m}")),
                    None => (label, String::new()),
                };
                let (floor, colour) = if wanted == Some(l) {
                    (format!("⌖ {floor}"), CYAN)
                } else {
                    (floor, TEXT)
                };
                let mut c = Choice::on(key, "Ride", floor, colour, Pick::World(WorldAct::Lift(l)))
                    .tail(marks);
                c.enabled = l != level;
                c
            })
            .collect(),
    )
}

/// One row per program that applies to a target in reach, with the odds; `only`
/// keeps it to one program. The program is the verb, the target the thing.
pub fn run_rows(
    st: &AppState,
    b: citygen::BuildingId,
    pos: Pos,
    only: Option<&game::ItemId>,
    letters: &mut std::str::Chars,
) -> Vec<Choice> {
    let mut choices = Vec::new();
    let now = st.net.now(st);
    let security = st.constructs.get(b).map(|k| k.security).unwrap_or(0);
    for (target, what) in crate::net::targets_near(st, b, pos) {
        let Some(kind) = crate::net::target_kind(st, b, target) else {
            continue;
        };
        let colour = match target {
            game::HackTarget::Node(_) => CYAN,
            game::HackTarget::Guard { .. } => MAGENTA,
            _ => TEXT,
        };
        for item in st.character.loaded_programs(&st.content) {
            if only.is_some_and(|id| *id != item.id) {
                continue;
            }
            let Some(spec) = item.program else { continue };
            if !game::applies(spec.op, kind) {
                continue;
            }
            let Some(key) = letters.next() else { break };
            let verb = capitalised(spec.op.label());
            let copied = spec.op == game::Op::Copy
                && matches!(target, game::HackTarget::Node(id) if st.net.copied(id, now));
            if copied {
                let mut c = Choice::on(key, verb, what.clone(), colour, Pick::Close)
                    .tail(" · copied already");
                c.enabled = false;
                choices.push(c);
                continue;
            }
            if spec.op == game::Op::Backdoor
                && let Some(why) = crate::net::plant_net_refusal(st, b)
            {
                let mut c = Choice::on(key, verb, what.clone(), colour, Pick::Close)
                    .tail(format!(" · {why}"));
                c.enabled = false;
                choices.push(c);
                continue;
            }
            let pw = game::power(&st.character, &st.content, &spec, kind);
            // A program on yourself never fails.
            let odds = if kind == game::Target::Deck {
                100
            } else {
                game::chance(pw, game::difficulty(security))
            };
            choices.push(
                Choice::on(
                    key,
                    verb,
                    what.clone(),
                    colour,
                    Pick::Run(item.id.clone(), target),
                )
                .tail(format!(
                    " · {odds}% · {} turn{} · noise {}",
                    spec.work,
                    if spec.work == 1 { "" } else { "s" },
                    spec.noise
                )),
            );
        }
    }
    choices.extend(hop_rows(st, b, pos, letters));
    choices
}

/// On the server of a construct you hold a backdoor on: `Hop: <building>` to every
/// other backdoor's gate. A street run has no way home, so its rows are greyed.
fn hop_rows(
    st: &AppState,
    b: citygen::BuildingId,
    pos: Pos,
    letters: &mut std::str::Chars,
) -> Vec<Choice> {
    let on_server = st.constructs.get(b).is_some_and(|k| {
        k.nodes.iter().any(|(id, p)| {
            st.city
                .as_ref()
                .is_some_and(|c| c.net.node(*id).kind == citygen::NodeKind::Server)
                && p.chebyshev(pos) <= 1
        })
    });
    if !on_server || st.backdoors.held(b).is_none() {
        return Vec::new();
    }
    let now = st.net.now(st);
    let home_run = st.backdoors.route.is_some();
    let mut rows = Vec::new();
    for d in st.backdoors.standing().filter(|d| d.building != b) {
        let Some(key) = letters.next() else { break };
        let name = crate::net::building_name(st, d.building);
        let state = format!(" · {} · {}", d.user_label(), d.health(now).label());
        let mut row = Choice::on(key, "Hop", name, CYAN, Pick::Hop(d.building));
        if home_run {
            row = row.tail(state);
        } else {
            row = row.tail(format!("{state} · no route home"));
            row.pick = Pick::Close;
            row.enabled = false;
        }
        rows.push(row);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Screen;
    use crate::jobs::testutil::{a_fixer, state_home};
    use crate::screens::testutil::buffer_text;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// The floors the lift panel marks `⌖` in cyan.
    fn marked_floors(st: &AppState) -> Vec<i32> {
        ride_rows(st, &mut "abcdefghijklmnopqrstuvwxyz0123456789".chars())
            .unwrap()
            .into_iter()
            .filter(|c| {
                c.thing
                    .as_ref()
                    .is_some_and(|(t, colour)| t.starts_with('⌖') && *colour == CYAN)
            })
            .filter_map(|c| match c.pick {
                Pick::World(WorldAct::Lift(l)) => Some(l),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_lift_marks_the_floor_the_track_wants() {
        let mut st = state_home();
        crate::screens::explore::MapScreen::walking(&mut st);
        let b = st.home.unwrap().home.building;
        let (flat, pos) = crate::track::flat_tile(&st).unwrap();
        let Place::Floor { level, .. } = flat else {
            unreachable!()
        };
        let lift = {
            let city = st.city.as_ref().unwrap();
            st.interiors.ensure(city, b, 0).unwrap();
            st.interiors.ensure(city, b, level).unwrap();
            st.interiors.plan_of(b).unwrap().lift.unwrap()
        };
        let stand = |st: &mut AppState, level: i32| {
            let p = st.player.as_mut().unwrap();
            p.place = Place::Floor { building: b, level };
            p.pos = lift;
        };
        // In the lobby's lift, untracked: no floor marked.
        stand(&mut st, 0);
        assert!(marked_floors(&st).is_empty());
        // Tracking the flat: its floor, and only it.
        let home = crate::track::Source::Spot {
            place: flat,
            pos,
            radius: 1,
        };
        crate::track::set(&mut st, home).unwrap();
        assert_eq!(marked_floors(&st), vec![level]);
        // From the flat's floor, a walk out to the street marks the ground floor.
        stand(&mut st, level);
        let outside = {
            let city = st.city.as_ref().unwrap();
            let door = city
                .portals(b)
                .into_iter()
                .find(|(k, _)| *k == citygen::StratumKind::Street)
                .map(|(_, p)| p)
                .unwrap();
            door.neighbors4()
                .find(|&n| city.walkable(citygen::StratumKind::Street, n))
                .unwrap()
        };
        let street = crate::track::Source::Spot {
            place: Place::Stratum(citygen::StratumKind::Street),
            pos: outside,
            radius: 1,
        };
        crate::track::set(&mut st, street).unwrap();
        assert_eq!(marked_floors(&st), vec![0]);
    }

    /// The window of the one subject beside the player, as drawn.
    fn window_text(st: &AppState, subjects: &[Subject]) -> String {
        // Opened a while ago, so the rows' typewriter has finished.
        let popup = subjects[0].window(st.tick.saturating_sub(200));
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| popup.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    #[test]
    fn the_chair_has_a_window() {
        let (mut st, b, level, chair) = crate::clinic::testutil::inside();
        st.tick += 200;
        let subjects = gather(&st, &|_, _| None);
        let s = subjects
            .iter()
            .find(|s| s.pos == chair)
            .expect("the chair's window");
        let h = s.header.as_ref().unwrap();
        assert_eq!(h.name, "Operating chair");
        assert_eq!(h.glyph.map(|g| g.0), Some('╦'));
        let sub = s.subtitle.as_deref().unwrap();
        assert!(sub.starts_with("Doc ") && !sub.contains('-'), "{sub}");
        assert!(
            s.body[0].starts_with('“') && s.body[0].ends_with('”'),
            "{:?}",
            s.body
        );
        let sit = s.choices.iter().find(|c| c.key == 'i').unwrap();
        assert!(sit.enabled);
        assert!(
            matches!(sit.pick, Pick::Chair { building, level: l } if building == b && l == level)
        );
        let text = window_text(&st, std::slice::from_ref(s));
        assert!(text.contains("Interact: Sit in the chair"), "{text}");
        // The doc gone, the chair is just a chair.
        let doc = crate::clinic::doc_at_work(&st, b).unwrap();
        st.world.as_mut().unwrap().park(doc, Pos::new(0, 0));
        let subjects = gather(&st, &|_, _| None);
        let s = subjects.iter().find(|s| s.pos == chair).unwrap();
        let sit = s.choices.iter().find(|c| c.key == 'i').unwrap();
        assert!(!sit.enabled);
        assert_eq!(sit.tail, " · the doc is out");
        assert_eq!(s.subtitle, None, "the row says it once");
    }

    #[test]
    fn a_shut_door_says_when() {
        let (mut st, b, door, _) = crate::clinic::testutil::outside(crate::clinic::testutil::DAY);
        // Somebody standing in the doorway keeps their window, with the door's hours on it.
        let doc = st
            .world
            .as_ref()
            .unwrap()
            .people()
            .iter()
            .find(|p| p.profession == "ripperdoc" && p.work == sim::Work::Building(b))
            .unwrap()
            .id;
        st.world.as_mut().unwrap().park(doc, door);
        let subjects = gather(&st, &|_, _| None);
        let s = subjects.iter().find(|s| s.pos == door).unwrap();
        let row = s.choices.iter().find(|c| c.key == 'i').unwrap();
        assert!(!row.enabled);
        assert_eq!(row.tail, " · CLOSED · opens 18:00");
        // Nobody standing in the doorway: the door's window is its own.
        let world = st.world.as_mut().unwrap();
        world.park(doc, Pos::new(0, 0));
        if let Some(id) = world.person_at(door) {
            world.park(id, Pos::new(0, 0));
        }
        st.tick += 200;
        let subjects = gather(&st, &|_, _| None);
        let shut: Vec<&Subject> = subjects.iter().filter(|s| s.pos == door).collect();
        assert_eq!(shut.len(), 1, "{subjects:?}");
        let h = shut[0].header.as_ref().unwrap();
        assert_eq!(h.name, crate::clinic::name(&st, b));
        assert_eq!(h.glyph, Some(('▯', AMBER)));
        assert_eq!(
            shut[0].subtitle.as_deref(),
            Some("CLOSED · opens 18:00 · closes 04:00")
        );
        assert!(shut[0].choices.is_empty());
        let text = window_text(&st, &[shut[0].clone()]);
        assert!(
            text.contains("x  Look") && text.contains("CLOSED"),
            "{text}"
        );
        // Open, the door has no window: you walk in.
        let (st, _, door, _) = crate::clinic::testutil::outside(crate::clinic::testutil::NIGHT);
        assert!(gather(&st, &|_, _| None).iter().all(|s| {
            s.pos != door
                || s.header
                    .as_ref()
                    .is_some_and(|h| h.name != crate::clinic::name(&st, b))
        }));
    }

    /// Stands the player on a street tile beside `who`, who is parked where they are.
    fn stand_beside(st: &mut AppState, who: sim::PersonId) -> Pos {
        let spot = st.world.as_ref().unwrap().spot_of(who).unwrap();
        let city = st.city.clone().unwrap();
        let beside = spot
            .neighbors4()
            .find(|p| city.walkable(citygen::StratumKind::Street, *p))
            .unwrap();
        let mut p = crate::walker::Player::spawn(&city, 1);
        p.place = Place::Stratum(citygen::StratumKind::Street);
        p.pos = beside;
        st.player = Some(p);
        st.world.as_mut().unwrap().park(who, spot);
        spot
    }

    /// The Space window on `who` beside the player, its Talk row fired.
    fn talk_to(st: &mut AppState, who: sim::PersonId) -> crate::app::ScreenCmd {
        let spot = stand_beside(st, who);
        let subjects = gather(st, &|_, _| None);
        let mut window = subjects
            .iter()
            .find(|s| s.pos == spot)
            .expect("the person is beside")
            .window(st.tick.saturating_sub(200));
        let key = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('t'),
            crossterm::event::KeyModifiers::NONE,
        );
        window.update(st, &[key])
    }

    fn talk_book(st: &mut AppState) {
        st.book = crate::missions::Book::parse(
            r#"Story(
                cast: { "mezz": Role(name: "Mezz", title: "fixer", pick: NearestFixer, face: None) },
                faces: { "silhouette": Face(rows: ["......dd........", "....dmmmmd......"], eye: (255, 255, 255)) },
                missions: [Mission(id: "chat", title: "Chat", from: "mezz", steps: [
                    Step(id: "hello", when: Talk("mezz"), then: [
                        Feed(from: "mezz", close: "ok", text: ["Hello."]),
                    ]),
                ])],
            )"#,
        )
        .unwrap();
    }

    #[test]
    fn talking_to_a_role_fires_its_step() {
        let mut st = state_home();
        talk_book(&mut st);
        crate::missions::begin(&mut st);
        let mezz = crate::missions::person_of(&st, "mezz").unwrap();
        assert!(st.missions.feeds.is_empty());
        let cmd = talk_to(&mut st, mezz);
        assert!(matches!(cmd, crate::app::ScreenCmd::Pop { .. }));
        assert_eq!(st.missions.feeds.len(), 1);
    }

    #[test]
    fn talk_is_unchanged_for_anyone_else() {
        let mut st = state_home();
        talk_book(&mut st);
        crate::missions::begin(&mut st);
        let mezz = crate::missions::person_of(&st, "mezz").unwrap();
        let other = st
            .world
            .as_ref()
            .unwrap()
            .people()
            .iter()
            .enumerate()
            .map(|(i, _)| sim::PersonId(i as u32))
            .find(|p| {
                *p != mezz
                    && !st.world.as_ref().unwrap().is_dead(*p)
                    && st.world.as_ref().unwrap().spot_of(*p).is_some()
            })
            .unwrap();
        let cmd = talk_to(&mut st, other);
        assert!(matches!(cmd, crate::app::ScreenCmd::Stay));
        assert!(st.missions.feeds.is_empty());
    }

    #[test]
    fn a_fixer_beside_you_has_work_rows() {
        let mut st = state_home();
        let fx = a_fixer(&st);
        // The welcome may be theirs; clear it so the window shows an offer.
        let welcome = st.jobs.active().unwrap().id;
        st.jobs.drop_job(welcome);
        st.jobs.fixer_mut(fx).standing = 0;
        let spot = st.world.as_ref().unwrap().spot_of(fx).unwrap();
        let city = st.city.clone().unwrap();
        let beside = spot
            .neighbors4()
            .find(|p| city.walkable(citygen::StratumKind::Street, *p))
            .unwrap();
        let mut p = crate::walker::Player::spawn(&city, 1);
        p.place = Place::Stratum(citygen::StratumKind::Street);
        p.pos = beside;
        st.player = Some(p);
        st.world.as_mut().unwrap().park(fx, spot);
        st.tick += 200;
        crate::jobs::ui::prepare_offers(&mut st, &|_, _| None);
        let subjects = gather(&st, &|_, _| None);
        let beside_fixer: Vec<&Subject> = subjects.iter().filter(|s| s.pos == spot).collect();
        assert_eq!(beside_fixer.len(), 1, "{subjects:?}");
        let text = window_text(&st, &[beside_fixer[0].clone()]);
        assert!(
            text.contains("t  Talk") && text.contains("w  Work: "),
            "{text}"
        );
        assert!(text.contains("¤"), "{text}");
    }

    #[test]
    fn a_copy_on_the_ground_is_taken_by_its_id() {
        let mut st = AppState::new();
        st.city = Some(
            citygen::Generator::run(citygen::GenParams {
                seed: 6,
                size: citygen::SizePreset::Custom { w: 120, h: 60 },
                districts: 5,
                ..Default::default()
            })
            .unwrap(),
        );
        st.player = Some(crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1));
        let (place, pos) = st.player.as_ref().map(|p| (p.place, p.pos)).unwrap();
        let deck = game::ItemId::new("rebuilt_deck");
        st.character.give_item(&st.content, deck, 1);
        let id = st.character.inventory.last().unwrap().copy_id().unwrap();
        crate::actions::perform(&mut st, game::Action::DropCopy(id)).unwrap();
        let takes = |st: &AppState| -> Vec<Pick> {
            let subjects = gather(st, &|_, _| None);
            let here = subjects
                .iter()
                .find(|s| s.dir.is_none())
                .expect("your tile");
            here.choices
                .iter()
                .filter(|c| c.label == "Take")
                .map(|c| c.pick.clone())
                .collect()
        };
        let by_id = |p: &Pick| matches!(p, Pick::Act(game::Action::TakeCopy(k)) if *k == id);
        let alone = takes(&st);
        assert!(alone.len() == 1 && by_id(&alone[0]), "{alone:?}");
        // Beside a plain stack, its own row still takes it by its id.
        st.ground
            .add(place, pos, game::ItemId::new("copper_coil"), 2);
        let both = takes(&st);
        assert_eq!(both.len(), 2, "{both:?}");
        assert!(both.iter().any(by_id), "{both:?}");
        assert!(
            both.iter()
                .any(|p| matches!(p, Pick::Act(game::Action::Take { qty: 2, .. }))),
            "{both:?}"
        );
    }

    #[test]
    fn ground_copies_read_apart() {
        let mut st = AppState::new();
        st.city = Some(
            citygen::Generator::run(citygen::GenParams {
                seed: 6,
                size: citygen::SizePreset::Custom { w: 120, h: 60 },
                districts: 5,
                ..Default::default()
            })
            .unwrap(),
        );
        st.player = Some(crate::walker::Player::spawn(st.city.as_ref().unwrap(), 1));
        let (deck, sidecar) = (
            game::ItemId::new("rebuilt_deck"),
            game::ItemId::new("sidecar"),
        );
        st.character.give_item(&st.content, deck.clone(), 2);
        st.character.give_item(&st.content, sidecar.clone(), 1);
        let copies = |st: &AppState, item: &game::ItemId| -> Vec<game::CopyId> {
            let mut ids: Vec<_> = st
                .character
                .inventory
                .iter()
                .filter(|s| &s.item == item)
                .filter_map(|s| s.copy_id())
                .collect();
            ids.sort_by_key(|k| k.0);
            ids
        };
        let (plain, modded) = (copies(&st, &deck)[0], copies(&st, &deck)[1]);
        let piece = copies(&st, &sidecar)[0];
        let fit = game::Action::Fit {
            host: game::Host::Pack(modded),
            socket: 0,
            piece,
        };
        crate::actions::perform(&mut st, fit).unwrap();
        for id in [plain, modded] {
            crate::actions::perform(&mut st, game::Action::DropCopy(id)).unwrap();
        }
        let subjects = gather(&st, &|_, _| None);
        let here = subjects.iter().find(|s| s.dir.is_none()).unwrap();
        let rows: Vec<(String, Pick)> = here
            .choices
            .iter()
            .filter(|c| c.label == "Take")
            .filter_map(|c| Some((c.thing.clone()?.0, c.pick.clone())))
            .collect();
        assert_eq!(rows.len(), 2, "{rows:?}");
        for (id, marks) in [(plain, "□□□□□ · □□"), (modded, "□□□□□ · ■□")]
        {
            let row = rows
                .iter()
                .find(|(_, p)| matches!(p, Pick::Act(game::Action::TakeCopy(k)) if *k == id))
                .unwrap_or_else(|| panic!("{rows:?}"));
            assert!(row.0.ends_with(&format!(" {marks}")), "{rows:?}");
        }
    }

    #[test]
    fn the_bed_offers_sleep() {
        let mut st = crate::jobs::testutil::state_home();
        crate::screens::explore::MapScreen::walking(&mut st);
        let home = st.home.unwrap();
        let named = |st: &AppState, name: &str| {
            gather(st, &|_, _| None)
                .into_iter()
                .find(|s| s.header.as_ref().is_some_and(|h| h.name == name))
        };
        // Beside the bed.
        st.player.as_mut().unwrap().pos = home.home.bed_side;
        let bed = named(&st, "Bed").expect("the bed");
        assert!(bed.choices.iter().any(|c| c.label == "Interact"
            && c.thing.as_ref().is_some_and(|(t, _)| t == "Sleep")
            && matches!(c.pick, Pick::Sleep)));
        // Beside the rig, away from the bed.
        let floor = st
            .interiors
            .floor(home.home.building, home.home.level)
            .map(|(_, f)| f.clone())
            .unwrap();
        let has_bed = |p: Pos| {
            p.neighbors4().any(|n| {
                floor
                    .tiles
                    .get(n)
                    .is_some_and(|t| t.feature == Some(Feature::Furniture(Furniture::Bed)))
            })
        };
        let rig_side = home
            .home
            .rig
            .neighbors4()
            .find(|p| {
                home.home.rect.contains(*p)
                    && floor.tiles.get(*p).is_some_and(|t| t.is_walkable())
                    && !has_bed(*p)
            })
            .expect("a floor tile by the rig, away from the bed");
        st.player.as_mut().unwrap().pos = rig_side;
        assert!(named(&st, "Bed").is_none());
        assert!(named(&st, "Access rig").is_some());
    }

    #[test]
    fn the_set_offers_watch_while_the_news_is_fresh() {
        use crate::missions::news;
        let mut st = state_home();
        crate::screens::explore::MapScreen::walking(&mut st);
        crate::missions::begin(&mut st);
        let set_subject = |st: &AppState| {
            gather(st, &|_, _| None)
                .into_iter()
                .find(|s| s.header.as_ref().is_some_and(|h| h.name == "Holo set"))
        };
        // Between the set and the couch.
        let (place, set) = news::set_tile(&st).unwrap();
        st.player = Some(crate::walker::Player::at(place, set.offset(0, 1)));
        assert!(set_subject(&st).is_none(), "no news, no Watch");
        news::put_out(&mut st, "marlo", usize::MAX, usize::MAX);
        let watch = |st: &AppState| {
            let s = set_subject(st).expect("the set");
            assert_eq!(s.pos, set);
            s.choices.iter().any(|c| {
                c.key == 'i'
                    && c.label == "Interact"
                    && c.thing.as_ref().is_some_and(|(t, _)| t == "Watch")
                    && matches!(c.pick, Pick::Watch)
            })
        };
        assert!(watch(&st));
        // Switched off, it is still offered while the news is fresh.
        news::switch_off(&mut st);
        assert!(watch(&st));
        // Away from the set: not in reach.
        st.player = Some(crate::walker::Player::at(place, set.offset(0, 2)));
        assert!(set_subject(&st).is_none());
        // Stale: the set is furniture.
        st.player = Some(crate::walker::Player::at(place, set.offset(0, 1)));
        st.missions.news.as_mut().unwrap().fresh = 0;
        assert!(set_subject(&st).is_none());
    }
}
