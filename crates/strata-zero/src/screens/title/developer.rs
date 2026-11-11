//! The title's DEVELOPER menu (dev mode): a row per venue type in the pack, each starting
//! D'SHAWN at a building laid out as that type, then the COMBAT starts and LOAD CITY. It
//! takes the title's rows' place under the logo, each row's picks drawn after its label.

use super::CombatMap;
use crate::render::Rgb;
use crate::ui::menu::{Menu, MenuView};
use citygen::venue::{MissionSize, VenuePack};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

/// The pick in pink, the other in a grey that still reads.
pub(super) const PICKED: Color = Color::Rgb(255, 0, 200);
pub(super) const UNPICKED: Color = Color::Rgb(120, 125, 135);

/// A type's row's picks, `SMALL · MEDIUM · LARGE  PLAIN · MISSION`, in cells.
const PICKS_WIDTH: u16 = 39;

/// A row of the DEVELOPER menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Row {
    /// A start at a site of the pack's type, by index.
    Site(usize),
    /// D'SHAWN on the lot, on the street or the Net.
    Combat,
    /// A city from its `.city` file, a run started over in it.
    LoadCity,
}

impl Row {
    fn label(self) -> String {
        match self {
            Row::Site(t) => VenuePack::builtin().types[t].name.to_uppercase(),
            Row::Combat => "COMBAT".to_string(),
            Row::LoadCity => "LOAD CITY".to_string(),
        }
    }
}

/// What a key on the DEVELOPER menu asks of the title.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Pick {
    /// A start at a site: its type, its size, and whether the mission's copy begins.
    Site {
        type_id: String,
        size: MissionSize,
        mission: bool,
    },
    Combat(CombatMap),
    LoadCity,
    /// Back to the title's own rows.
    Back,
}

pub(super) struct Developer {
    pub(super) menu: Menu,
    rows: Vec<Row>,
    /// Each type's row's size, and whether it begins the mission's copy (MISSION) or
    /// leaves the plain site (PLAIN), by type.
    pub(super) sites: Vec<(MissionSize, bool)>,
    /// The COMBAT start's map.
    pub(super) combat: CombatMap,
}

impl Developer {
    /// The menu typing itself in from `tick`, every type on SMALL and MISSION, COMBAT on
    /// the street.
    pub(super) fn new(tick: u64) -> Developer {
        let types = VenuePack::builtin().types.len();
        let mut rows: Vec<Row> = (0..types).map(Row::Site).collect();
        rows.extend([Row::Combat, Row::LoadCity]);
        Developer {
            menu: Menu::new(rows.iter().map(|r| r.label()).collect(), tick),
            rows,
            sites: vec![(MissionSize::Small, true); types],
            combat: CombatMap::Street,
        }
    }

    fn selected(&self) -> Row {
        self.rows[self.menu.selected]
    }

    /// One key: up and down move; on a type's row left and right pick the size and tab
    /// PLAIN or MISSION, on COMBAT left and right the map; enter asks for the row's start,
    /// esc to go back.
    pub(super) fn key(&mut self, k: &KeyEvent) -> Option<Pick> {
        let row = self.selected();
        match (k.code, row) {
            (KeyCode::Up | KeyCode::Char('k'), _) => self.menu.up(),
            (KeyCode::Down | KeyCode::Char('j'), _) => self.menu.down(),
            (KeyCode::Left | KeyCode::Char('h'), Row::Site(t)) => {
                self.sites[t].0 = step(self.sites[t].0, -1);
            }
            (KeyCode::Right | KeyCode::Char('l'), Row::Site(t)) => {
                self.sites[t].0 = step(self.sites[t].0, 1);
            }
            (KeyCode::Tab, Row::Site(t)) => self.sites[t].1 = !self.sites[t].1,
            (
                KeyCode::Left | KeyCode::Right | KeyCode::Char('h') | KeyCode::Char('l'),
                Row::Combat,
            ) => {
                self.combat = match self.combat {
                    CombatMap::Net => CombatMap::Street,
                    CombatMap::Street => CombatMap::Net,
                };
            }
            (KeyCode::Esc | KeyCode::Char('q'), _) => return Some(Pick::Back),
            (KeyCode::Enter, Row::Site(t)) => {
                let (size, mission) = self.sites[t];
                return Some(Pick::Site {
                    type_id: VenuePack::builtin().types[t].id.clone(),
                    size,
                    mission,
                });
            }
            (KeyCode::Enter, Row::Combat) => return Some(Pick::Combat(self.combat)),
            (KeyCode::Enter, Row::LoadCity) => return Some(Pick::LoadCity),
            _ => {}
        }
        None
    }

    /// What the keys do on the row under the cursor.
    pub(super) fn hint(&self) -> &'static str {
        match self.selected() {
            Row::Site(_) => "↑↓ select · ←→ size · tab plain/mission · enter start · esc back",
            Row::Combat => "↑↓ select · ←→ map · enter start · esc back",
            Row::LoadCity => "↑↓ select · enter open · esc back",
        }
    }

    /// The menu from row `y` of `area`, centred with its picks: a column after the labels
    /// holds each type's `SMALL · MEDIUM · LARGE  PLAIN · MISSION` and COMBAT's
    /// `NET · STREET`, each row's once it has typed itself in.
    pub(super) fn draw(&self, area: Rect, y: u16, tick: u64, f: &mut Frame) {
        let labels = (self.menu.items.iter())
            .map(|l| l.chars().count() as u16)
            .max()
            .unwrap_or(0);
        let x = area.x + area.width.saturating_sub(2 + labels + 2 + PICKS_WIDTH) / 2;
        MenuView {
            menu: &self.menu,
            tick,
            accent: Rgb(0, 255, 230),
        }
        .render(
            Rect::new(
                x,
                y,
                area.width.saturating_sub(x - area.x),
                self.menu.items.len() as u16,
            ),
            f.buffer_mut(),
        );
        let picks = x + 2 + labels + 2;
        let buf = f.buffer_mut();
        for (i, row) in self.rows.iter().enumerate() {
            let ry = y + i as u16;
            if ry >= area.bottom() || self.menu.entrance(i, tick) < 1.0 {
                continue;
            }
            match *row {
                Row::Site(t) => {
                    let (size, mission) = self.sites[t];
                    let sizes = MissionSize::ALL.map(|s| s.label().to_uppercase());
                    let sizes: Vec<&str> = sizes.iter().map(String::as_str).collect();
                    let picked = MissionSize::ALL.iter().position(|s| *s == size);
                    let end = draw_pick(buf, area, picks, ry, &sizes, picked.unwrap_or(0));
                    draw_pick(
                        buf,
                        area,
                        end + 2,
                        ry,
                        &["PLAIN", "MISSION"],
                        mission.into(),
                    );
                }
                Row::Combat => {
                    let street = self.combat == CombatMap::Street;
                    draw_pick(buf, area, picks, ry, &["NET", "STREET"], street.into());
                }
                Row::LoadCity => {}
            }
        }
    }
}

/// The size `by` steps on from `size`, round from LARGE to SMALL and back.
fn step(size: MissionSize, by: isize) -> MissionSize {
    let n = MissionSize::ALL.len() as isize;
    let at = MissionSize::ALL
        .iter()
        .position(|s| *s == size)
        .unwrap_or(0) as isize;
    MissionSize::ALL[(at + by).rem_euclid(n) as usize]
}

/// `options` from column `x` of row `y`, ` · ` between them, the `picked`th in pink and
/// bold and the rest in grey; the column after the last comes back.
fn draw_pick(buf: &mut Buffer, area: Rect, x: u16, y: u16, options: &[&str], picked: usize) -> u16 {
    let mut at = x;
    let mut put = |s: &str, style: Style| {
        for ch in s.chars() {
            if at < area.right() {
                buf[(at, y)].set_char(ch).set_style(style);
            }
            at += 1;
        }
    };
    for (i, o) in options.iter().enumerate() {
        if i > 0 {
            put(" · ", Style::default().fg(UNPICKED));
        }
        let style = if i == picked {
            Style::default().fg(PICKED).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(UNPICKED)
        };
        put(o, style);
    }
    at
}
