//! Side panels: tile inspector, legend, strata selector, status bar.

use crate::anim::effects::pulse;
use crate::render::tileset::legend_entries;
use citygen::{City, Node, Pos, StratumKind, Terrain};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders};

const DIM: Color = Color::Rgb(120, 130, 145);
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
) -> Vec<Line<'static>> {
    let mut out = vec![kv("pos", format!("{},{}  {}", p.x, p.y, stratum.label()))];
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

/// Every stratum, top first (Net above Heights), with the current one marked.
pub fn strata_selector(current: StratumKind, tick: u64) -> Vec<Line<'static>> {
    StratumKind::ALL
        .iter()
        .rev()
        .map(|k| {
            let here = *k == current;
            let marker = if here { "▌" } else { " " };
            let style = if here {
                let g = (160.0 + 95.0 * pulse(tick, 40)) as u8;
                Style::default()
                    .fg(Color::Rgb(0, g, 255))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(DIM)
            };
            Line::from(Span::styled(format!("{marker} {}", k.label()), style))
        })
        .collect()
}

pub fn status_bar(
    city: Option<&City>,
    stratum: StratumKind,
    fps: Option<f32>,
    hints: &str,
    status: Option<&str>,
    dev: bool,
) -> Line<'static> {
    let mut spans = Vec::new();
    if dev {
        spans.push(Span::styled(
            " DEV ",
            Style::default()
                .fg(Color::Rgb(10, 11, 16))
                .bg(Color::Rgb(255, 200, 80))
                .add_modifier(Modifier::BOLD),
        ));
    }
    if let Some(c) = city {
        spans.push(Span::styled(
            format!(" {} ", c.name),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("seed {} ", c.seed),
            Style::default().fg(DIM),
        ));
    }
    spans.push(Span::styled(
        format!("· {} ", stratum.label()),
        Style::default().fg(TEXT),
    ));
    if let Some(fps) = fps {
        spans.push(Span::styled(
            format!("· {fps:.0} fps "),
            Style::default().fg(DIM),
        ));
    }
    match status {
        Some(msg) => spans.push(Span::styled(
            format!("· {msg}"),
            Style::default()
                .fg(Color::Rgb(255, 200, 80))
                .add_modifier(Modifier::BOLD),
        )),
        None => spans.push(Span::styled(format!("· {hints}"), Style::default().fg(DIM))),
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, Pos, SizePreset, StratumKind};

    #[test]
    fn inspector_and_selector_have_content() {
        let c = Generator::run(GenParams {
            seed: 3,
            size: SizePreset::Custom { w: 80, h: 40 },
            districts: 4,
            ..Default::default()
        })
        .unwrap();
        let b = &c.buildings[0];
        let p = b.footprint.center();
        let lines = inspector_lines(&c, StratumKind::Street, p, None);
        let text: String = lines.iter().map(|l| l.to_string() + "\n").collect();
        assert!(text.contains(&format!("{},{}", p.x, p.y)));
        assert!(text.contains(b.kind.label()));
        let off = inspector_lines(&c, StratumKind::Street, Pos::new(-5, -5), None);
        assert!(!off.is_empty());
        let sel = strata_selector(StratumKind::Skyways, 0);
        let text: String = sel.iter().map(|l| l.to_string() + "\n").collect();
        assert!(text.contains("Skyways") && text.contains("▌"));
        let bar = status_bar(
            Some(&c),
            StratumKind::Street,
            Some(30.0),
            "q quit",
            None,
            true,
        );
        assert!(bar.to_string().contains("fps"));
        let bar = status_bar(Some(&c), StratumKind::Street, None, "q quit", None, true);
        assert!(!bar.to_string().contains("fps"));
        assert!(bar.to_string().contains("DEV"));
        assert!(bar.to_string().contains(&c.name) && bar.to_string().contains("q quit"));
        let bar = status_bar(
            Some(&c),
            StratumKind::Street,
            Some(30.0),
            "q quit",
            Some("saved x"),
            false,
        );
        assert!(bar.to_string().contains("saved x") && !bar.to_string().contains("q quit"));
        assert!(!legend(StratumKind::Undercity).is_empty());
    }
}
