//! The living city on screen: walkers, cars, chats and lit doors inside the bubble.

use crate::foes::Foe;
use crate::missions::Body;
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
    /// The cells the city's people and lit doors draw on: all with none; where other
    /// areas share the view, the cells the city owns.
    pub keep: Option<&'a dyn Fn(Pos) -> bool>,
    /// Dev: the walkers who can see you, drawn red.
    pub eyes: &'a [PersonId],
    /// Who stands against you; a down one is remembered where it fell.
    pub foes: &'a [Foe],
    /// People the story killed, lying where they fell.
    pub bodies: &'a [Body],
}

/// A standing foe.
pub const FOE: Color = Color::Rgb(255, 80, 80);
/// A foe down.
pub const FOE_DOWN: Color = Color::Rgb(120, 30, 30);
/// Under a marked foe, the bright half of its pulse and the dim half.
pub const MARK_BRIGHT: Color = Color::Rgb(140, 0, 105);
pub const MARK_DIM: Color = Color::Rgb(70, 0, 52);
/// Ticks in each half of the pulse.
pub const MARK_PULSE: u64 = 8;
/// The reticle over a marked foe.
const MARK_RETICLE: Color = Color::Rgb(255, 0, 200);

impl Overlay for PeopleOverlay<'_> {
    fn draw(
        &self,
        map: &MapRef,
        origin: Pos,
        area: Rect,
        tick: u64,
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
            // ICE on the grid in its grade's colour, as in a construct.
            let fg = match (&f.ice, f.stunned > 0) {
                (Some(ice), stunned) => {
                    let c = crate::render::netlook::ice_colour(ice.grade, tick);
                    if stunned { c.scale(0.5) } else { c }.to_color()
                }
                (None, true) => Rgb(255, 80, 80).scale(0.5).to_color(),
                (None, false) => FOE,
            };
            cell.set_char(f.glyph).set_fg(fg);
            // A grid piece glows in its grade's colour, apart from the nodes around it.
            if let Some(ice) = &f.ice {
                let glow = crate::render::netlook::ice_colour(ice.grade, tick).scale(0.32);
                cell.set_bg(glow.to_color());
            }
            if f.hunting() {
                cell.modifier.insert(Modifier::BOLD);
            } else {
                cell.modifier.remove(Modifier::BOLD);
            }
            if f.marked {
                let bright = (tick / MARK_PULSE).is_multiple_of(2);
                cell.set_bg(if bright { MARK_BRIGHT } else { MARK_DIM });
                if sy > area.y {
                    let above = &mut buf[(sx, sy - 1)];
                    above.set_char('⌖').set_fg(MARK_RETICLE);
                    if bright {
                        above.modifier.insert(Modifier::BOLD);
                    } else {
                        above.modifier.remove(Modifier::BOLD);
                    }
                }
            }
        }
        for b in self.bodies.iter().filter(|b| b.place == map.place()) {
            let Some((sx, sy)) = tile_to_screen(area, origin, b.pos) else {
                continue;
            };
            let seen = match self.vis {
                None => Visibility::Visible,
                Some(v) => v.get(b.pos).copied().unwrap_or(Visibility::Unseen),
            };
            if seen != Visibility::Unseen {
                buf[(sx, sy)].set_char('%').set_fg(FOE_DOWN);
            }
        }
        if !map.place().is_street() {
            return;
        }
        let seen = |p: Pos| match self.vis {
            None => Visibility::Visible,
            Some(v) => v.get(p).copied().unwrap_or(Visibility::Unseen),
        };
        StreetPeople {
            world: self.world,
            night: self.night,
            seen: &seen,
            keep: self.keep,
            eyes: self.eyes,
        }
        .draw(origin, area, buf, style);
    }
}

/// A city's people out on its street, with the doors its open buildings light by night:
/// the live city's under the people overlay, and every other city's and town's held in
/// view, each in its own frame on the cells it owns.
pub struct StreetPeople<'a> {
    pub world: &'a World,
    pub night: bool,
    /// How well each cell of the city's frame is seen.
    pub seen: &'a dyn Fn(Pos) -> Visibility,
    /// The cells they draw on: all with none.
    pub keep: Option<&'a dyn Fn(Pos) -> bool>,
    /// Dev: the walkers who can see you, drawn red.
    pub eyes: &'a [PersonId],
}

impl StreetPeople<'_> {
    /// Drawn into `area` with the city's cell `origin` at its top left.
    pub fn draw(&self, origin: Pos, area: Rect, buf: &mut Buffer, style: &StylePack) {
        let kept = |p: Pos| self.keep.is_none_or(|keep| keep(p));
        if self.night {
            for b in self.world.open_buildings() {
                for &d in self.world.doors(b) {
                    if kept(d)
                        && let Some((sx, sy)) = tile_to_screen(area, origin, d)
                    {
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
            if !kept(m.pos) {
                continue;
            }
            // On foot, people and moving cars are seen only in sight; a parked car is
            // remembered like a pile.
            let seen = (self.seen)(m.pos);
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
            bodies: &[],
            keep: None,
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
            bodies: &[],
            keep: None,
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
            bodies: &[],
            keep: None,
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
            bodies: &[],
            keep: None,
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
            bodies: &[],
            keep: None,
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
    fn a_citys_people_draw_only_on_the_cells_it_keeps() {
        // the left half of the map another area's: the city's people and lit doors
        // draw on the right half only, in sight as `seen` says
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
        let style = StylePack::load().unwrap();
        let half = c.size.w as i32 / 2;
        let draw = |keep: Option<&dyn Fn(citygen::Pos) -> bool>,
                    seen: &dyn Fn(citygen::Pos) -> Visibility| {
            let mut buf = Buffer::empty(whole);
            StreetPeople {
                world: &w,
                night: true,
                seen,
                keep,
                eyes: &[],
            }
            .draw(citygen::Pos::new(0, 0), whole, &mut buf, &style);
            buf
        };
        let marked = |buf: &Buffer, x0: i32, x1: i32| {
            buf.content
                .iter()
                .enumerate()
                .filter(|(i, cell)| {
                    let x = (*i % whole.width as usize) as i32;
                    (x0..x1).contains(&x) && (cell.symbol() != " " || cell.fg == DOOR_LIT)
                })
                .count()
        };
        let all = draw(None, &|_| Visibility::Visible);
        assert!(marked(&all, 0, half) > 0 && marked(&all, half, c.size.w as i32) > 0);
        let right = |p: citygen::Pos| p.x >= half;
        let kept = draw(Some(&right), &|_| Visibility::Visible);
        assert_eq!(marked(&kept, 0, half), 0, "nothing on the cells not kept");
        assert_eq!(
            marked(&kept, half, c.size.w as i32),
            marked(&all, half, c.size.w as i32),
            "all of it on those kept"
        );
        let unseen = draw(Some(&right), &|_| Visibility::Unseen);
        assert!(
            unseen.content.iter().all(|cell| cell.symbol() != "@"),
            "no walker out of sight"
        );
    }

    #[test]
    fn a_body_is_drawn_once_seen() {
        let c = city();
        let w = World::seed(
            &c,
            &People::load().unwrap(),
            &SimParams::default(),
            22 * 60,
            0,
        );
        let at = citygen::Pos::new(3, 2);
        let bodies = [Body {
            person: PersonId(0),
            name: "Marlo".into(),
            place: citygen::Place::Stratum(StratumKind::Street),
            pos: at,
            look: "Face down.".into(),
            near: false,
            left: None,
        }];
        let area = Rect::new(0, 0, 8, 6);
        let style = StylePack::load().unwrap();
        let cell_at = |seen: Visibility| {
            let mut vis = Grid::filled(c.size, Visibility::Unseen);
            vis[at] = seen;
            let mut buf = Buffer::empty(area);
            PeopleOverlay {
                world: &w,
                night: false,
                vis: Some(&vis),
                eyes: &[],
                foes: &[],
                bodies: &bodies,
                keep: None,
            }
            .draw(
                &MapRef::stratum_of(&c, StratumKind::Street),
                citygen::Pos::new(0, 0),
                area,
                0,
                &mut buf,
                &style,
            );
            buf[(3, 2)].clone()
        };
        let seen = cell_at(Visibility::Remembered);
        assert_eq!(seen.symbol(), "%");
        assert_eq!(seen.fg, FOE_DOWN);
        assert_ne!(cell_at(Visibility::Unseen).symbol(), "%");
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

    #[test]
    fn a_marked_foe_pulses_with_a_reticle() {
        let c = city();
        let w = World::seed(&c, &People::load().unwrap(), &SimParams::default(), 0, 0);
        let map = MapRef::stratum_of(&c, StratumKind::Street);
        let place = citygen::Place::Stratum(StratumKind::Street);
        let mut foe = Foe::goon(place, citygen::Pos::new(5, 4));
        foe.marked = true;
        let foes = [foe];
        let area = Rect::new(0, 0, 12, 8);
        let style = StylePack::load().unwrap();
        let draw = |tick: u64| {
            let mut buf = Buffer::empty(area);
            PeopleOverlay {
                world: &w,
                night: false,
                vis: None,
                eyes: &[],
                foes: &foes,
                bodies: &[],
                keep: None,
            }
            .draw(&map, citygen::Pos::new(0, 0), area, tick, &mut buf, &style);
            buf
        };
        let bright = draw(0);
        assert_eq!(bright[(5, 4)].bg, MARK_BRIGHT);
        let r = &bright[(5, 3)];
        assert_eq!(r.symbol(), "⌖");
        assert_eq!(r.fg, Color::Rgb(255, 0, 200));
        assert!(r.modifier.contains(Modifier::BOLD));
        let dim = draw(8);
        assert_eq!(dim[(5, 4)].bg, MARK_DIM);
        assert_eq!(dim[(5, 3)].symbol(), "⌖");
        assert!(!dim[(5, 3)].modifier.contains(Modifier::BOLD));
        assert_eq!(draw(16)[(5, 4)].bg, MARK_BRIGHT);
    }
}
