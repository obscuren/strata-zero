//! Bots out with you on the map: each in its own light, its frame turning with the
//! world's clock, and the wake it leaves as it flies.

use crate::bots::{BotOut, WAKE_TICKS};
use crate::foes::Foe;
use crate::render::map::MapRef;
use crate::render::style::StylePack;
use crate::render::viewport::{Overlay, tile_to_screen};
use crate::render::{Rgb, Visibility};
use citygen::{Grid, Pos};
use game::Content;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

/// Ticks a frame shows while a bot drifts or stands, and while it flies to you.
pub const PACE_IDLE: u64 = 6;
pub const PACE_MOVING: u64 = 3;
/// Ticks a wake's dot is the bright one before it is the faint one.
const WAKE_BRIGHT: u64 = 3;

/// The frame a bot shows at `now`: its frames in turn, twice as fast on the move.
pub fn frame(bot: &game::Bot, now: u64, moving: bool) -> char {
    let pace = if moving { PACE_MOVING } else { PACE_IDLE };
    let n = bot.frames.len().max(1) as u64;
    bot.frames
        .get(((now / pace) % n) as usize)
        .copied()
        .unwrap_or('?')
}

pub struct BotOverlay<'a> {
    pub bots: &'a [BotOut],
    pub content: &'a Content,
    /// Who stands against you: a foe draws over a bot on its tile.
    pub foes: &'a [Foe],
    /// Your sight on foot; the free camera sees everything.
    pub vis: Option<&'a Grid<Visibility>>,
    /// The world's clock: the rotor and the wake stand when it does.
    pub now: u64,
}

impl BotOverlay<'_> {
    fn seen(&self, p: Pos) -> bool {
        self.vis
            .is_none_or(|v| v.get(p).copied() == Some(Visibility::Visible))
    }

    fn held(&self, map: &MapRef, p: Pos) -> bool {
        let place = map.place();
        self.foes
            .iter()
            .any(|f| f.standing() && f.place == place && f.pos == p)
            || self
                .bots
                .iter()
                .any(|b| b.place == place && b.pos == Some(p))
    }
}

fn rgb(c: Color) -> Rgb {
    match c {
        Color::Rgb(r, g, b) => Rgb(r, g, b),
        _ => Rgb(0, 0, 0),
    }
}

impl Overlay for BotOverlay<'_> {
    fn draw(
        &self,
        map: &MapRef,
        origin: Pos,
        area: Rect,
        _tick: u64,
        buf: &mut Buffer,
        _style: &StylePack,
    ) {
        for b in self.bots.iter().filter(|b| b.place == map.place()) {
            let Some(bot) = self.content.item(&b.item).and_then(|i| i.bot.as_ref()) else {
                continue;
            };
            let (r, g, bl) = bot.light;
            let light = Rgb(r, g, bl);
            // The wake: a dot over the tile's own ground, the light going out of it.
            if bot.wake {
                for &(p, left) in &b.wake {
                    let age = self.now.saturating_sub(left);
                    if age >= WAKE_TICKS || !self.seen(p) || self.held(map, p) {
                        continue;
                    }
                    let Some((sx, sy)) = tile_to_screen(area, origin, p) else {
                        continue;
                    };
                    let cell = &mut buf[(sx, sy)];
                    let ground = rgb(cell.bg);
                    let t = (age + 1) as f32 / (WAKE_TICKS + 1) as f32;
                    let fg = light.mix(ground, 0.35 + 0.6 * t);
                    let dot = if age < WAKE_BRIGHT { '∙' } else { '·' };
                    cell.set_char(dot).set_fg(fg.to_color());
                    cell.modifier.remove(Modifier::BOLD);
                }
            }
            let Some(p) = b.pos else {
                continue;
            };
            let foe_here = self
                .foes
                .iter()
                .any(|f| f.standing() && f.place == b.place && f.pos == p);
            if foe_here || !self.seen(p) {
                continue;
            }
            let Some((sx, sy)) = tile_to_screen(area, origin, p) else {
                continue;
            };
            let cell = &mut buf[(sx, sy)];
            cell.set_char(frame(bot, self.now, b.catching_up))
                .set_fg(light.to_color());
            cell.modifier.insert(Modifier::BOLD);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bots::BotOut;
    use crate::render::map::MapRef;
    use crate::render::style::StylePack;
    use citygen::{City, Grid, Place, Pos, StratumKind};
    use game::{Content, ItemId, Site};
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier};

    const STREET: Place = Place::Stratum(StratumKind::Street);
    const SCOUT: Color = Color::Rgb(120, 255, 120);

    fn scout(c: &Content) -> game::Bot {
        c.item(&ItemId::new("scout_drone"))
            .unwrap()
            .bot
            .clone()
            .unwrap()
    }

    fn out(at: Pos, wake: Vec<(Pos, u64)>, catching_up: bool) -> BotOut {
        BotOut {
            item: ItemId::new("scout_drone"),
            dock: (Site::Back, 0),
            place: STREET,
            pos: Some(at),
            moved_at: 0,
            next_drift: 0,
            catching_up,
            wake,
        }
    }

    /// The lot drawn whole with `bots` over it at `now`; `vis` as given, the foes too.
    fn drawn(
        bots: &[BotOut],
        foes: &[crate::foes::Foe],
        vis: Option<&Grid<Visibility>>,
        now: u64,
    ) -> Buffer {
        let c = Content::load().unwrap();
        let city = City::lot(1);
        let area = Rect::new(0, 0, city.size.w as u16, city.size.h as u16);
        let mut buf = Buffer::empty(area);
        BotOverlay {
            bots,
            content: &c,
            foes,
            vis,
            now,
        }
        .draw(
            &MapRef::stratum_of(&city, StratumKind::Street),
            Pos::new(0, 0),
            area,
            now,
            &mut buf,
            &StylePack::load().unwrap(),
        );
        buf
    }

    #[test]
    fn the_rotor_turns() {
        let c = Content::load().unwrap();
        let s = scout(&c);
        assert_eq!(frame(&s, 0, false), '┼');
        assert_eq!(frame(&s, 5, false), '┼');
        assert_eq!(frame(&s, 6, false), '╳');
        assert_eq!(frame(&s, 3, true), '╳', "twice as fast on the move");
        assert_eq!(frame(&s, 6, true), '┼');
    }

    #[test]
    fn it_draws_in_its_light_where_you_see() {
        let at = Pos::new(5, 5);
        let buf = drawn(&[out(at, vec![], false)], &[], None, 0);
        let cell = &buf[(5, 5)];
        assert_eq!(cell.symbol(), "┼");
        assert_eq!(cell.fg, SCOUT);
        assert!(cell.modifier.contains(Modifier::BOLD));
        let mut vis = Grid::filled(citygen::Size { w: 41, h: 19 }, Visibility::Unseen);
        vis.set(at, Visibility::Remembered);
        let buf = drawn(&[out(at, vec![], false)], &[], Some(&vis), 0);
        assert_eq!(buf[(5, 5)].symbol(), " ", "not remembered where it was");
    }

    #[test]
    fn a_foe_draws_over_it() {
        let at = Pos::new(5, 5);
        let goon = crate::foes::Foe::goon(STREET, at);
        let buf = drawn(&[out(at, vec![], false)], &[goon], None, 0);
        assert_eq!(
            buf[(5, 5)].symbol(),
            " ",
            "the foe's overlay draws it, not this one"
        );
    }

    #[test]
    fn the_wake_fades() {
        let at = Pos::new(8, 5);
        let wake = vec![
            (Pos::new(5, 5), 0),
            (Pos::new(6, 5), 4),
            (Pos::new(7, 5), 9),
        ];
        let buf = drawn(&[out(at, wake, true)], &[], None, 10);
        assert_eq!(buf[(5, 5)].symbol(), " ", "ten ticks old: gone");
        assert_eq!(buf[(6, 5)].symbol(), "·", "six ticks old: the faint dot");
        assert_eq!(buf[(7, 5)].symbol(), "∙", "a tick old: the bright one");
        let lum = |c: Color| match c {
            Color::Rgb(r, g, b) => r as u32 + g as u32 + b as u32,
            _ => 0,
        };
        assert!(
            lum(buf[(7, 5)].fg) > lum(buf[(6, 5)].fg),
            "it fades with age"
        );
        assert!(lum(buf[(7, 5)].fg) < lum(SCOUT), "dimmer than the drone");
    }
}
