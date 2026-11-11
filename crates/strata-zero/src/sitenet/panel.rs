//! The site's Net panel beside the tower (look D): its BODY lines, where your body stands
//! and who is near it (Ruling 19). Spec `docs/superpowers/specs/2026-11-11-site-net-design.md`,
//! "The panel".

use crate::app::AppState;
use citygen::{FloorPlan, Place};

/// BODY: where your body stands and who is near it (Ruling 19). On a site's floor,
/// `L · at the lobby kiosk`, then `3 guards on L · nearest 4 tiles` (the guards posted to
/// the building on that floor, not down) or `nobody on L`; at home `home · at the rig`; at
/// a street port `{district} · at the port`. None while you're not jacked in.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "the site's Net panel draws it from Task 15 on")
)]
pub fn body_lines(st: &AppState) -> Vec<String> {
    let Some((place, at)) = st.player.as_ref().and_then(|p| p.body) else {
        return Vec::new();
    };
    if st.home.is_some_and(|h| h.place() == place) {
        return vec!["home · at the rig".to_string()];
    }
    match place {
        Place::Stratum(_) => {
            let district = (st.city.as_ref())
                .map(|c| crate::net::district_name(c, at))
                .unwrap_or_default();
            vec![format!("{district} · at the port")]
        }
        Place::Floor { building: b, level } => {
            let label = FloorPlan::label(level);
            let port = (st.constructs.site(b).into_iter())
                .flat_map(|k| k.devices.iter())
                .find(|d| {
                    d.node.level == level && super::is_port(d.kind) && d.node.at.chebyshev(at) <= 1
                });
            let here = match port {
                Some(d) => format!("{label} · at the {}", d.name),
                None => label.clone(),
            };
            let guards: Vec<i32> = (st.foes.iter())
                .filter(|f| !f.down && f.place == place)
                .filter(|f| f.post.as_ref().is_some_and(|p| p.building == b))
                .map(|f| f.pos.chebyshev(at))
                .collect();
            let near = match guards.iter().min() {
                None => format!("nobody on {label}"),
                Some(d) => format!(
                    "{} {} on {label} · nearest {d} {}",
                    guards.len(),
                    if guards.len() == 1 { "guard" } else { "guards" },
                    if *d == 1 { "tile" } else { "tiles" },
                ),
            };
            vec![here, near]
        }
        Place::Construct(_) | Place::SiteNet { .. } => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::testutil::*;
    use super::*;
    use crate::foes::{Foe, Post};
    use citygen::{Place, Pos, StratumKind};

    #[test]
    fn the_body_line_says_where_it_stands_and_who_is_near() {
        let (mut st, b) = office_copy(190);
        stand_beside_the_kiosk(&mut st, b);
        crate::net::jack_in_here(&mut st).unwrap();
        let lines = body_lines(&st);
        assert!(
            lines[0].starts_with("L · at the ") && lines[0].contains("kiosk"),
            "{lines:?}"
        );
        assert!(
            lines[1].contains(" guards on L · nearest ") || lines[1] == "nobody on L",
            "{lines:?}"
        );
    }

    #[test]
    fn who_is_near_counts_the_guards_posted_on_the_bodys_floor() {
        let (mut st, b) = office_copy(190);
        stand_beside_the_kiosk(&mut st, b);
        let body = st.player.as_ref().unwrap().pos;
        crate::net::jack_in_here(&mut st).unwrap();
        let on = |level: i32| Place::Floor { building: b, level };
        let guard = |building, level: i32, dx: i32| Foe {
            post: Some(Post {
                building,
                zone: 0,
                patrol: Vec::new(),
            }),
            ..Foe::goon(
                Place::Floor { building, level },
                Pos::new(body.x + dx, body.y),
            )
        };
        st.foes.clear();
        assert_eq!(body_lines(&st)[1], "nobody on L");
        st.foes.push(guard(b, 0, 3));
        assert_eq!(body_lines(&st)[1], "1 guard on L · nearest 3 tiles");
        st.foes.push(guard(b, 0, -5));
        // Not counted: one down, one on another floor, a goon with no post.
        let mut down = guard(b, 0, 1);
        down.down = true;
        st.foes.push(down);
        st.foes.push(guard(b, 1, 1));
        st.foes.push(Foe::goon(on(0), Pos::new(body.x + 1, body.y)));
        assert_eq!(body_lines(&st)[1], "2 guards on L · nearest 3 tiles");
    }

    #[test]
    fn a_body_at_the_rig_or_a_street_port_says_so() {
        let (mut st, _) = office_copy(190);
        assert!(body_lines(&st).is_empty(), "not jacked in");
        stand_at_the_rig(&mut st);
        crate::net::jack_in_here(&mut st).unwrap();
        assert_eq!(body_lines(&st), vec!["home · at the rig".to_string()]);
        crate::net::jack_out(&mut st).unwrap();
        // Beside a street Jack.
        let city = st.city.clone().unwrap();
        let street = StratumKind::Street;
        let at = (city.connectors.iter())
            .filter(|k| k.kind == citygen::ConnectorKind::Jack && k.serves.contains(&street))
            .filter(|k| city.walkable(StratumKind::Net, k.pos))
            .find_map(|k| (k.pos.neighbors8()).find(|q| city.walkable(street, *q)))
            .expect("a street Jack");
        crate::constructs::land(&mut st, Place::Stratum(street), at);
        crate::net::jack_in_here(&mut st).unwrap();
        let district = crate::net::district_name(&city, at);
        assert_eq!(body_lines(&st), vec![format!("{district} · at the port")]);
    }
}
