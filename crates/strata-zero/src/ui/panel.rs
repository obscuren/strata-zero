//! Side panels: tile inspector, legend, strata selector, status bar.

use crate::render::tileset::legend_entries;
use citygen::{City, Node, Pos, StratumKind, Terrain};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders};

const DIM: Color = Color::Rgb(160, 170, 185);
const TEXT: Color = Color::Rgb(205, 212, 222);
const ACCENT: Color = Color::Rgb(0, 229, 255);

fn kv(key: &str, value: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{key:<10}"), Style::default().fg(DIM)),
        Span::styled(value.into(), Style::default().fg(TEXT)),
    ])
}

/// A bordered panel with a titled top edge.
pub fn boxed(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(70, 90, 110)))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(ACCENT),
        ))
}

/// Everything known about one tile, for the side panel.
pub fn inspector_lines(
    city: &City,
    stratum: StratumKind,
    p: Pos,
    node: Option<&Node>,
    items: &[game::Stack],
    content: &game::Content,
    person: Option<&sim::Card>,
) -> Vec<Line<'static>> {
    let mut out = vec![kv("pos", format!("{},{}  {}", p.x, p.y, stratum.label()))];
    if let Some(c) = person {
        out.push(kv("name", c.name.clone()));
        out.push(kv("profession", c.profession.clone()));
        out.push(kv("doing", c.doing.clone()));
        out.push(kv("next", c.next.clone()));
        if let Some(h) = &c.lives {
            out.push(kv("lives", h.clone()));
        }
        if let Some(w) = &c.works {
            out.push(kv("works", w.clone()));
        }
        if let Some(car) = &c.car {
            out.push(kv("car", car.clone()));
        }
    }
    let Some(t) = city.tile(stratum, p) else {
        out.push(kv("terrain", "outside the map"));
        return out;
    };
    out.push(kv("terrain", t.terrain.label()));
    match t.feature {
        Some(citygen::Feature::Node(k)) => {
            out.push(kv("feature", format!("node · {}", k.label())));
        }
        Some(f) => out.push(kv("feature", format!("{f:?}").to_lowercase())),
        None => {}
    }
    if let Some(d) = t.district.and_then(|d| city.district_opt(d)) {
        out.push(kv("district", d.name.clone()));
        out.push(kv("theme", d.theme.label()));
        out.push(kv(
            "wealth",
            format!("{:.0}%  decay {:.0}%", d.wealth * 100.0, d.decay * 100.0),
        ));
    }
    if let Some(b) = t.building.and_then(|b| city.building_opt(b)) {
        out.push(kv(
            "building",
            b.name.clone().unwrap_or_else(|| b.kind.label().to_string()),
        ));
        out.push(kv(
            "kind",
            format!("{}  {}x{}", b.kind.label(), b.footprint.w, b.footprint.h),
        ));
        out.push(kv(
            "floors",
            format!("{} → {}", b.span.lo.label(), b.span.hi.label()),
        ));
    }
    for c in city.connectors_at(p) {
        if !c.serves.contains(&stratum) {
            continue;
        }
        let served: Vec<&str> = c.serves.iter().map(|k| k.label()).collect();
        out.push(kv(c.kind.label(), served.join(" · ")));
        if c.kind == citygen::ConnectorKind::Lift {
            for k in &c.serves {
                let marker = if *k == stratum { "▸" } else { " " };
                out.push(Line::from(Span::styled(
                    format!("   {marker} {}", k.label()),
                    Style::default().fg(if *k == stratum { ACCENT } else { DIM }),
                )));
            }
        }
    }
    if let Some(n) = node {
        let sn = city.net.subnet(n.subnet);
        out.push(Line::from(Span::styled(
            "── net ──",
            Style::default().fg(Color::Rgb(0, 200, 160)),
        )));
        out.push(kv("node", n.kind.label()));
        let owner = match sn.owner {
            citygen::SubnetOwner::Building(b) => city
                .building(b)
                .name
                .clone()
                .unwrap_or_else(|| city.building(b).kind.label().to_string()),
            citygen::SubnetOwner::District(d) => city.district(d).name.clone(),
            citygen::SubnetOwner::City => city.name.clone(),
        };
        out.push(kv("subnet", owner));
        out.push(kv("security", format!("{}", sn.security)));
    }
    if t.terrain == Terrain::Water {
        out.push(kv("note", "deep water"));
    }
    if !items.is_empty() {
        let list: Vec<String> = items
            .iter()
            .map(|st| {
                let name = content
                    .item(&st.item)
                    .map(|i| i.name.clone())
                    .unwrap_or_else(|| st.item.to_string());
                if st.qty > 1 {
                    format!("{}× {name}", st.qty)
                } else {
                    name
                }
            })
            .collect();
        out.push(kv("items", list.join(", ")));
    }
    out
}

pub fn legend(stratum: StratumKind) -> Vec<Line<'static>> {
    legend_entries(stratum)
        .into_iter()
        .map(|(g, label)| {
            Line::from(vec![
                Span::styled(format!(" {g} "), Style::default().fg(ACCENT)),
                Span::styled(label.to_string(), Style::default().fg(TEXT)),
            ])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, Pos, SizePreset, StratumKind};

    #[test]
    fn inspector_and_legend_have_content() {
        let c = Generator::run(GenParams {
            seed: 3,
            size: SizePreset::Custom { w: 80, h: 40 },
            districts: 4,
            ..Default::default()
        })
        .unwrap();
        let b = &c.buildings[0];
        let p = b.footprint.center();
        let content = game::Content::load().unwrap();
        let lines = inspector_lines(&c, StratumKind::Street, p, None, &[], &content, None);
        let text: String = lines.iter().map(|l| l.to_string() + "\n").collect();
        assert!(text.contains(&format!("{},{}", p.x, p.y)));
        assert!(text.contains(b.kind.label()));
        assert!(!text.contains("items"));
        let stacks = [
            game::Stack {
                item: game::ItemId::new("copper_coil"),
                qty: 3,
            },
            game::Stack {
                item: game::ItemId::new("dead_phone"),
                qty: 1,
            },
        ];
        let lines = inspector_lines(&c, StratumKind::Street, p, None, &stacks, &content, None);
        let text: String = lines.iter().map(|l| l.to_string() + "\n").collect();
        assert!(
            text.contains("items") && text.contains("3× Copper coil, Dead phone"),
            "{text}"
        );
        let off = inspector_lines(
            &c,
            StratumKind::Street,
            Pos::new(-5, -5),
            None,
            &[],
            &content,
            None,
        );
        assert!(!off.is_empty());
        assert!(!legend(StratumKind::Undercity).is_empty());
    }

    #[test]
    fn the_inspector_reads_a_person() {
        let content = game::Content::load().unwrap();
        let city = citygen::Generator::run(citygen::GenParams {
            seed: 6,
            size: citygen::SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        let card = sim::Card {
            name: "Juno".into(),
            profession: "fixer".into(),
            doing: "holding the corner".into(),
            next: "the bar at 02:00".into(),
            lives: Some("Sector 88".into()),
            works: Some("the corner".into()),
            car: Some("parked at 4,4".into()),
            line: "I know a guy.".into(),
            relation: None,
        };
        let lines = inspector_lines(
            &city,
            StratumKind::Street,
            Pos::new(5, 5),
            None,
            &[],
            &content,
            Some(&card),
        );
        let text: String = lines.iter().map(|l| l.to_string() + "\n").collect();
        assert!(
            text.contains("Juno") && text.contains("fixer") && text.contains("holding the corner"),
            "{text}"
        );
        assert!(
            text.contains("the bar at 02:00")
                && text.contains("Sector 88")
                && text.contains("parked at 4,4"),
            "{text}"
        );
    }
}
