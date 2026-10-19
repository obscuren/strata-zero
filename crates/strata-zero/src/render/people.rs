//! The living city on screen: walkers, cars, chats and lit doors inside the bubble.

use crate::foes::Foe;
use crate::render::map::MapRef;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::render::{Rgb, Visibility};
use crate::ui::chrome::{CYAN, DIM, MAGENTA, TEXT, head, plain};
use citygen::Grid;
use citygen::{Pos, Rect as TileRect};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use sim::{Card, MoverKind, PersonId, Tint, World};

/// Tiles beyond the viewport the bubble reaches.
pub const MARGIN: i32 = 20;
/// An open building's door at night.
pub const DOOR_LIT: Color = Color::Rgb(255, 200, 90);

pub(crate) fn tint_colour(t: Tint) -> Color {
    match t {
        Tint::Magenta => MAGENTA,
        Tint::Amber => Color::Rgb(230, 180, 60),
        Tint::Red => Color::Rgb(255, 80, 80),
        Tint::Green => Color::Rgb(120, 255, 120),
    }
}

pub struct PeopleOverlay<'a> {
    pub world: &'a World,
    pub night: bool,
    /// The player's sight when on foot; the free camera sees everyone.
    pub vis: Option<&'a Grid<Visibility>>,
    /// Dev: the walkers who can see you, drawn red.
    pub eyes: &'a [PersonId],
    /// Who stands against you; a down one is remembered where it fell.
    pub foes: &'a [Foe],
}

/// A standing foe.
pub const FOE: Color = Color::Rgb(255, 80, 80);
/// A foe down.
pub const FOE_DOWN: Color = Color::Rgb(120, 30, 30);

impl Overlay for PeopleOverlay<'_> {
    fn draw(
        &self,
        map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        style: &StylePack,
    ) {
        for f in self.foes.iter().filter(|f| f.place == map.place()) {
            let Some((sx, sy)) = tile_to_screen(area, origin, f.pos) else {
                continue;
            };
            let seen = match self.vis {
                None => Visibility::Visible,
                Some(v) => v.get(f.pos).copied().unwrap_or(Visibility::Unseen),
            };
            let cell = &mut buf[(sx, sy)];
            if f.down {
                if seen != Visibility::Unseen {
                    cell.set_char('%').set_fg(FOE_DOWN);
                }
                continue;
            }
            if seen != Visibility::Visible {
                continue;
            }
            let fg = if f.stunned > 0 {
                Rgb(255, 80, 80).scale(0.5).to_color()
            } else {
                FOE
            };
            cell.set_char('@').set_fg(fg);
            if f.hunting() {
                cell.modifier.insert(Modifier::BOLD);
            } else {
                cell.modifier.remove(Modifier::BOLD);
            }
        }
        if !map.place().is_street() {
            return;
        }
        if self.night {
            for b in self.world.open_buildings() {
                for &d in self.world.doors(b) {
                    if let Some((sx, sy)) = tile_to_screen(area, origin, d) {
                        buf[(sx, sy)].set_fg(DOOR_LIT);
                    }
                }
            }
        }
        let bubble = TileRect::new(
            origin.x - MARGIN,
            origin.y - MARGIN,
            area.width as i32 + 2 * MARGIN,
            area.height as i32 + 2 * MARGIN,
        );
        for m in self.world.visible(bubble) {
            let Some((sx, sy)) = tile_to_screen(area, origin, m.pos) else {
                continue;
            };
            // On foot, people and moving cars are seen only in sight; a parked car is
            // remembered like a pile.
            let seen = match self.vis {
                None => Visibility::Visible,
                Some(v) => v.get(m.pos).copied().unwrap_or(Visibility::Unseen),
            };
            let remembered_car = matches!(m.kind, MoverKind::Car { parked: true, .. })
                && seen == Visibility::Remembered;
            if seen != Visibility::Visible && !remembered_car {
                continue;
            }
            match m.kind {
                MoverKind::Walker { id, tint } => {
                    let fg = if self.eyes.contains(&id) {
                        Rgb(255, 70, 70).to_color()
                    } else {
                        tint.map(tint_colour).unwrap_or(DIM)
                    };
                    buf[(sx, sy)].set_char('@').set_fg(fg);
                }
                MoverKind::Car { hue, parked, .. } => {
                    let colour = style.hue(hue);
                    let fg = if parked || remembered_car {
                        colour.scale(if remembered_car { 0.35 } else { 0.55 })
                    } else {
                        colour
                    };
                    if !parked && self.night {
                        let c = &mut buf[(sx, sy)];
                        let bg = match c.bg {
                            Color::Rgb(r, g, b) => Rgb(r, g, b),
                            _ => Rgb(0, 0, 0),
                        };
                        c.set_bg(bg.mix(colour, 0.3).to_color());
                    }
                    buf[(sx, sy)].set_char('▬').set_fg(fg.to_color());
                }
                MoverKind::Chat => {
                    buf[(sx, sy)].set_char('…').set_fg(TEXT);
                }
            }
        }
    }
}

/// The building's occupants at their stations, drawn in sight.
pub struct InteriorPeople<'a> {
    pub placement: &'a crate::interiors::Placement,
    pub world: &'a World,
    pub vis: Option<&'a Grid<Visibility>>,
}

impl Overlay for InteriorPeople<'_> {
    fn draw(
        &self,
        _map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        for (pos, id) in self.placement.people() {
            if let Some(v) = self.vis
                && v.get(pos) != Some(&Visibility::Visible)
            {
                continue;
            }
            let Some((sx, sy)) = tile_to_screen(area, origin, pos) else {
                continue;
            };
            let tint = self.world.people()[id.0 as usize].tint;
            buf[(sx, sy)]
                .set_char('@')
                .set_fg(tint.map(tint_colour).unwrap_or(DIM));
        }
    }
}

/// A person's card as lines for the talk popup and the inspector.
pub fn card_lines(card: &Card) -> Vec<Line<'static>> {
    let mut out = vec![
        head(&card.name),
        Line::from(Span::styled(
            card.profession.clone(),
            Style::default().fg(DIM),
        )),
        plain(card.doing.clone()),
        plain(""),
        Line::from(Span::styled(
            format!("\u{201c}{}\u{201d}", card.line),
            Style::default().fg(CYAN).add_modifier(Modifier::ITALIC),
        )),
    ];
    if let Some(r) = &card.relation {
        out.push(Line::from(Span::styled(
            r.clone(),
            Style::default().fg(DIM),
        )));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::style::StylePack;
    use crate::ui::chrome::DIM;
    use citygen::{GenParams, Generator, SizePreset, StratumKind, Theme};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use sim::{People, SimParams, TICKS_PER_MINUTE, World};

    fn city() -> citygen::City {
        Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 8,
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn the_strip_draws_walkers_cars_and_lit_doors_at_night() {
        let c = city();
        let mut w = World::seed(
            &c,
            &People::load().unwrap(),
            &SimParams::default(),
            22 * 60,
            0,
        );
        let mut t = 0;
        while t < 60 * TICKS_PER_MINUTE {
            t += TICKS_PER_MINUTE;
            w.advance(&c, t);
            w.path_some(&c, usize::MAX);
        }
        let strip = c
            .districts
            .iter()
            .find(|d| d.theme == Theme::NeonStrip)
            .unwrap();
        let origin = citygen::Pos::new(strip.bounds.x, strip.bounds.y);
        let area = Rect::new(0, 0, strip.bounds.w as u16, strip.bounds.h as u16);
        let mut buf = Buffer::empty(area);
        let style = StylePack::load().unwrap();
        PeopleOverlay {
            world: &w,
            night: true,
            vis: None,
            eyes: &[],
            foes: &[],
        }
        .draw(
            &MapRef::stratum_of(&c, StratumKind::Street),
            origin,
            area,
            t,
            &mut buf,
            &style,
        );
        let ats = buf
            .content
            .iter()
            .filter(|cell| cell.symbol() == "@")
            .count();
        assert!(ats > 0, "walkers on the Strip at eleven");
        let tinted = buf
            .content
            .iter()
            .filter(|cell| cell.symbol() == "@" && cell.fg != DIM)
            .count();
        assert!(tinted > 0, "a sex worker or fixer wears a tint");
        // At nine in the morning the day shift is on the move: untinted walkers, dim.
        while t < (10 * 60 + 45) * TICKS_PER_MINUTE {
            t += TICKS_PER_MINUTE;
            w.advance(&c, t);
            w.path_some(&c, usize::MAX);
        }
        let whole = Rect::new(0, 0, c.size.w as u16, c.size.h as u16);
        let mut all = Buffer::empty(whole);
        PeopleOverlay {
            world: &w,
            night: false,
            vis: None,
            eyes: &[],
            foes: &[],
        }
        .draw(
            &MapRef::stratum_of(&c, StratumKind::Street),
            citygen::Pos::new(0, 0),
            whole,
            t,
            &mut all,
            &style,
        );
        let dim = all
            .content
            .iter()
            .filter(|cell| cell.symbol() == "@" && cell.fg == DIM)
            .count();
        assert!(dim > 0, "an untinted walker on the morning streets");
        let doors_lit = buf
            .content
            .iter()
            .filter(|cell| cell.fg == DOOR_LIT)
            .count();
        assert!(doors_lit > 0, "an open club's door glows at night");
        // Nothing of the Street draws on the Net.
        let mut net = Buffer::empty(area);
        PeopleOverlay {
            world: &w,
            night: true,
            vis: None,
            eyes: &[],
            foes: &[],
        }
        .draw(
            &MapRef::stratum_of(&c, StratumKind::Net),
            origin,
            area,
            t,
            &mut net,
            &style,
        );
        assert!(net.content.iter().all(|cell| cell.symbol() == " "));
    }

    #[test]
    fn on_foot_only_people_in_sight_are_drawn() {
        let c = city();
        let mut w = World::seed(
            &c,
            &People::load().unwrap(),
            &SimParams::default(),
            22 * 60,
            0,
        );
        let mut t = 0;
        while t < 60 * TICKS_PER_MINUTE {
            t += TICKS_PER_MINUTE;
            w.advance(&c, t);
            w.path_some(&c, usize::MAX);
        }
        let whole = Rect::new(0, 0, c.size.w as u16, c.size.h as u16);
        let origin = citygen::Pos::new(0, 0);
        let style = StylePack::load().unwrap();
        let mut all = Buffer::empty(whole);
        PeopleOverlay {
            world: &w,
            night: true,
            vis: None,
            eyes: &[],
            foes: &[],
        }
        .draw(
            &MapRef::stratum_of(&c, StratumKind::Street),
            origin,
            whole,
            t,
            &mut all,
            &style,
        );
        let (i, _) = all
            .content
            .iter()
            .enumerate()
            .find(|(_, cell)| cell.symbol() == "@")
            .expect("a walker");
        let seen = citygen::Pos::new(
            (i % whole.width as usize) as i32,
            (i / whole.width as usize) as i32,
        );
        let mut vis = Grid::filled(c.size, Visibility::Unseen);
        vis[seen] = Visibility::Visible;
        let mut buf = Buffer::empty(whole);
        PeopleOverlay {
            world: &w,
            night: true,
            vis: Some(&vis),
            eyes: &[],
            foes: &[],
        }
        .draw(
            &MapRef::stratum_of(&c, StratumKind::Street),
            origin,
            whole,
            t,
            &mut buf,
            &style,
        );
        let ats: Vec<usize> = buf
            .content
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.symbol() == "@")
            .map(|(i, _)| i)
            .collect();
        assert_eq!(ats, vec![i], "only the walker in sight is drawn");
        assert_eq!(
            buf.content
                .iter()
                .filter(|cell| cell.symbol() == "▬")
                .count(),
            0,
            "no car stands in sight"
        );
    }

    #[test]
    fn a_card_reads_as_lines() {
        let card = sim::Card {
            name: "Juno".into(),
            profession: "fixer".into(),
            doing: "holding the corner".into(),
            next: "home at 04:00".into(),
            lives: None,
            works: None,
            car: None,
            line: "I know a guy.".into(),
            relation: Some("you'll find me at the bar after".into()),
        };
        let lines = card_lines(&card);
        let text: String = lines.iter().map(|l| l.to_string() + "\n").collect();
        assert!(
            text.contains("JUNO") && text.contains("fixer") && text.contains("holding the corner"),
            "{text}"
        );
        assert!(text.contains("I know a guy.") && text.contains("find me at the bar"));
    }
}
