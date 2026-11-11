//! The title screen: block-letter logo, animated menu. In dev mode its DEVELOPER row opens
//! the test starts (`developer`).

mod developer;

use crate::anim::effects::typewriter;
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::Rgb;
use crate::runs::RunHeader;
use crate::runs::describe::{clock, first_part};
use crate::screens::create::chooser::Chooser;
use crate::screens::files::Files;
use crate::screens::runs::{FAINT, RunsScreen};
use crate::ui::chrome::{CYAN, DIM};
use crate::ui::logo::{self, Logo};
use crate::ui::menu::{Menu, MenuView};
use citygen::venue::MissionSize;
use crossterm::event::{KeyCode, KeyEvent};
use developer::{Developer, Pick};
#[cfg(test)]
use developer::{PICKED, UNPICKED};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::{Paragraph, Widget};
use std::path::PathBuf;

pub const TITLE: &str = "STRATA ZERO";
pub const SUBTITLE: &str = "h i g h   t e c h   ·   l o w   l i f e";

/// Which map the COMBAT start opens on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CombatMap {
    Net,
    Street,
}

/// A row of the title menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Entry {
    /// The newest run still going, opened where it was left.
    Continue,
    NewGame,
    /// Dev mode: D'SHAWN, maxed out, into a fresh city.
    Debug,
    /// Dev mode: the test starts, a menu of their own (the sites, SITE NET, COMBAT, LOAD
    /// CITY).
    Developer,
    /// The runs still going.
    Load,
    /// The runs that flatlined.
    Hall,
    /// The player's manual, on its cover.
    Manual,
    Quit,
}

impl Entry {
    /// The rows: CONTINUE when there is a run to go on with, the debug start and
    /// DEVELOPER only in dev mode.
    fn rows(dev: bool, can_continue: bool) -> Vec<Entry> {
        let mut rows = Vec::new();
        if can_continue {
            rows.push(Entry::Continue);
        }
        rows.push(Entry::NewGame);
        if dev {
            rows.extend([Entry::Debug, Entry::Developer]);
        }
        rows.extend([Entry::Load, Entry::Hall, Entry::Manual, Entry::Quit]);
        rows
    }

    fn label(self) -> &'static str {
        match self {
            Entry::Continue => "CONTINUE",
            Entry::NewGame => "NEW GAME",
            Entry::Debug => "NEW GAME (DEBUG)",
            Entry::Developer => "DEVELOPER",
            Entry::Load => "LOAD GAME",
            Entry::Hall => "HALL OF FAME",
            Entry::Manual => "MANUAL",
            Entry::Quit => "QUIT",
        }
    }
}

/// The logo's neon.
pub const LOGO_ACCENT: Rgb = Rgb(255, 0, 200);

/// Where the title puts its logo's top left on `area`: centred, a sixth of the way down.
pub fn logo_origin(area: Rect) -> (u16, u16) {
    (
        area.x + area.width.saturating_sub(logo::width(TITLE)) / 2,
        area.y + area.height / 6,
    )
}

pub struct Title {
    menu: Menu,
    entries: Vec<Entry>,
    opened: u64,
    /// DEVELOPER's menu while it's open, in the title's rows' place.
    developer: Option<Developer>,
    /// Keys before this tick are dropped, and each one dropped pushes it on (see
    /// `after_splash`).
    quiet_until: u64,
    /// The run CONTINUE opens: its file and what its header says.
    newest: Option<(PathBuf, RunHeader)>,
    /// Why CONTINUE couldn't open it, said where its line was.
    error: Option<String>,
}

/// How long the title the splash hands over to waits for the keys to stop: 0.8 s with
/// none, longer than the usual pause before a held key starts repeating.
const QUIET: u64 = 24;

#[cfg(test)]
thread_local! {
    /// On a test's thread, the seed `Title::fresh_params` gives in place of the clock's, so
    /// a start through the title builds the same city every run (`pin_seed`).
    static PINNED_SEED: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// Pins the seed of every city a new game through the title builds on this thread (none
/// gives the clock's back).
#[cfg(test)]
pub(crate) fn pin_seed(seed: Option<u64>) {
    PINNED_SEED.with(|s| s.set(seed));
}

impl Title {
    /// The current parameters with a seed nobody has played: the tick and the clock.
    pub fn fresh_params(st: &AppState, tick: u64) -> citygen::GenParams {
        #[cfg(test)]
        if let Some(seed) = PINNED_SEED.with(std::cell::Cell::get) {
            return st.params.clone().with_seed(seed);
        }
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        // Capped like Configure's RANDOM SEED so the status line stays readable.
        let mut seed = citygen::rng::hash64(tick, nanos) % 1_000_000_007;
        if seed == 0 {
            seed = 1;
        }
        st.params.clone().with_seed(seed)
    }

    /// NEW GAME (DEBUG): D'SHAWN, maxed out, straight into a fresh city, a run of his
    /// own. (The COMBAT starts are on the lot, which has none.)
    fn debug_game(&self, st: &mut AppState) -> ScreenCmd {
        st.character = game::Character::debug_runner(&st.content);
        let cmd = crate::screens::start_built(st, crate::screens::new_game_city(st));
        if matches!(cmd, ScreenCmd::Switch { .. }) {
            crate::runs::when::begin_run(st);
        }
        cmd
    }

    /// COMBAT on the Net: D'SHAWN into a fresh city, and as it opens, jacked into a
    /// construct with grey ICE on patrol.
    fn net_game(&self, st: &mut AppState) -> ScreenCmd {
        use citygen::{Place, Pos, StratumKind};
        st.character = game::Character::combat_runner(&st.content);
        let seed = Title::fresh_params(st, st.tick).seed;
        st.enter_city_at(citygen::City::lot(seed), 18 * 60 + 15);
        st.player = Some(crate::walker::Player::at(
            Place::Stratum(StratumKind::Street),
            Pos::new(6, 8),
        ));
        let result = crate::net::start_in_net_lot(st);
        crate::actions::report(st, result);
        ScreenCmd::Switch {
            to: crate::screens::explore_screen(st),
            fx: TransitionKind::Glitch,
        }
    }

    /// COMBAT on the street: D'SHAWN on the lot at dusk, a goon in the far corner
    /// and a dummy to shoot at.
    fn combat_game(&self, st: &mut AppState) -> ScreenCmd {
        use citygen::{Place, Pos, StratumKind};
        st.character = game::Character::combat_runner(&st.content);
        let seed = Title::fresh_params(st, st.tick).seed;
        st.enter_city_at(citygen::City::lot(seed), 18 * 60 + 15);
        let place = Place::Stratum(StratumKind::Street);
        let mut p = crate::walker::Player::at(place, Pos::new(6, 8));
        let minute = st.minute_of_day();
        let bonus = st.sight_bonus();
        if let Some(city) = st.city.as_ref() {
            p.update_fov(
                &crate::render::map::MapRef::stratum_at(city, StratumKind::Street, minute),
                bonus,
            );
        }
        st.player = Some(p);
        // The goon in the lot's far corner, bottom right: a walk across the
        // road before it sees you.
        st.foes
            .push(crate::foes::Foe::goon(place, Pos::new(38, 16)));
        // A training dummy six tiles north, for trying shots on something that
        // stands still and shrugs them off.
        st.foes.push(crate::foes::Foe::dummy(place, Pos::new(6, 2)));
        ScreenCmd::Switch {
            to: crate::screens::explore_screen(st),
            fx: TransitionKind::Glitch,
        }
    }

    /// A DEVELOPER site row: NEW GAME (DEBUG)'s start, then a building away from home laid
    /// out as type `type_id` at `size`, its mission's copy begun when `mission`, and you on
    /// the street at its main door.
    fn site_game(
        &self,
        st: &mut AppState,
        type_id: &str,
        size: MissionSize,
        mission: bool,
    ) -> ScreenCmd {
        self.at_a_site(st, |st| {
            crate::venues::dev_start(st, type_id, size, mission)
        })
    }

    /// DEVELOPER's SITE NET (Ruling 24): NEW GAME (DEBUG)'s start, then an office copy at
    /// `venues::SITE_NET_SECURITY` away from home, its job in a server room, and you on the
    /// street at its main door.
    fn site_net_game(&self, st: &mut AppState) -> ScreenCmd {
        self.at_a_site(st, |st| {
            crate::venues::dev_site_net(st, crate::venues::SITE_NET_SECURITY)
        })
    }

    /// NEW GAME (DEBUG)'s start, then `lay` putting a site down away from home and you on
    /// the street at its door (what it can't do warned of). A map opened now opens there;
    /// the start's own would stand where it woke you.
    fn at_a_site(
        &self,
        st: &mut AppState,
        lay: impl FnOnce(&mut AppState) -> Result<citygen::BuildingId, String>,
    ) -> ScreenCmd {
        let ScreenCmd::Switch { fx, .. } = self.debug_game(st) else {
            return ScreenCmd::Stay;
        };
        match lay(st) {
            // The start saved before the site was laid: saved again at the map's next
            // frame, CONTINUE opens with it.
            Ok(_) => st.save_due = true,
            Err(why) => st.warn(why),
        }
        ScreenCmd::Switch {
            to: Box::new(crate::screens::explore::MapScreen::walking(st)),
            fx,
        }
    }

    /// What DEVELOPER's row asks for: a start, LOAD CITY's list, or the title's own rows
    /// back, typing themselves in again.
    fn pick(&mut self, st: &mut AppState, pick: Pick) -> ScreenCmd {
        match pick {
            Pick::Site {
                type_id,
                size,
                mission,
            } => self.site_game(st, &type_id, size, mission),
            Pick::SiteNet => self.site_net_game(st),
            Pick::Combat(CombatMap::Street) => self.combat_game(st),
            Pick::Combat(CombatMap::Net) => self.net_game(st),
            Pick::LoadCity => ScreenCmd::Push {
                to: Box::new(Files::load(st)),
                fx: TransitionKind::SlideUp,
            },
            Pick::Back => {
                self.developer = None;
                self.menu.opened = st.tick;
                ScreenCmd::Stay
            }
        }
    }

    /// The title for `st`: CONTINUE on it, under the cursor, while a run in its saves
    /// folder is still going; the debug starts while dev mode is on.
    pub fn new(st: &AppState, tick: u64) -> Self {
        let newest = st.saves.as_deref().and_then(crate::runs::newest);
        let entries = Entry::rows(st.dev, newest.is_some());
        Title {
            menu: Menu::new(
                entries.iter().map(|e| e.label().to_string()).collect(),
                tick + 20,
            ),
            entries,
            opened: tick,
            developer: None,
            quiet_until: 0,
            newest,
            error: None,
        }
    }

    /// The title the launch splash hands over to: it drops keys until `QUIET` ticks pass
    /// with none, so a key mashed or held to skip the splash doesn't quit the game or
    /// start one. (A terminal never says when a key is let go; the quiet is how a hold
    /// is seen to end.)
    pub fn after_splash(st: &AppState, tick: u64) -> Self {
        Title {
            quiet_until: tick + QUIET,
            ..Title::new(st, tick)
        }
    }

    /// CONTINUE: the newest run opened, as LOAD GAME's space would.
    fn continue_run(&mut self, st: &mut AppState) -> ScreenCmd {
        let Some((path, _)) = self.newest.as_ref() else {
            return ScreenCmd::Stay;
        };
        match crate::screens::runs::resume(st, path) {
            Ok(cmd) => cmd,
            Err(why) => {
                self.error = Some(why);
                ScreenCmd::Stay
            }
        }
    }

    /// While CONTINUE is picked, two rows under the menu and centred, the run it opens:
    /// `VEX · NETRUNNER · Saintburg · day 2 07:40 · Bring it to Coz`, the runner cyan
    /// and bold, the objective dim, the rest faint; or why it wouldn't open.
    fn draw_run_line(&self, area: Rect, menu_y: u16, tick: u64, f: &mut Frame) {
        if self.selected() != Entry::Continue || self.menu.entrance(self.menu.selected, tick) < 1.0
        {
            return;
        }
        let y = menu_y + self.entries.len() as u16 + 2;
        if y >= area.bottom() {
            return;
        }
        let parts: Vec<(String, Style)> = match (&self.error, &self.newest) {
            (Some(why), _) => vec![(why.clone(), Style::default().fg(FAINT))],
            (None, Some((_, h))) => {
                let objective = h.objective.as_deref().map(first_part).unwrap_or_default();
                let mut rest = format!(
                    " · {} · {} · {}",
                    h.profession,
                    h.city,
                    clock(h.day, h.minute)
                );
                if !objective.is_empty() {
                    rest.push_str(" · ");
                }
                vec![
                    (
                        h.handle.to_uppercase(),
                        Style::default()
                            .fg(CYAN)
                            .add_modifier(ratatui::style::Modifier::BOLD),
                    ),
                    (rest, Style::default().fg(FAINT)),
                    (objective.to_string(), Style::default().fg(DIM)),
                ]
            }
            (None, None) => return,
        };
        let n: u16 = parts.iter().map(|(t, _)| t.chars().count() as u16).sum();
        let mut x = area.x + area.width.saturating_sub(n) / 2;
        let buf = f.buffer_mut();
        for (text, style) in &parts {
            for ch in text.chars() {
                if x >= area.right() {
                    return;
                }
                buf[(x, y)].set_char(ch).set_style(*style);
                x += 1;
            }
        }
    }

    /// The entry under the selection.
    fn selected(&self) -> Entry {
        self.entries[self.menu.selected]
    }
}

impl Screen for Title {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        if st.tick < self.quiet_until {
            if !keys.is_empty() {
                self.quiet_until = st.tick + QUIET;
            }
            return ScreenCmd::Stay;
        }
        for k in keys {
            // DEVELOPER, while open, takes the keys.
            if let Some(dev) = self.developer.as_mut() {
                if let Some(pick) = dev.key(k) {
                    let cmd = self.pick(st, pick);
                    if !matches!(cmd, ScreenCmd::Stay) {
                        return cmd;
                    }
                }
                continue;
            }
            match k.code {
                KeyCode::Up | KeyCode::Char('k') => self.menu.up(),
                KeyCode::Down | KeyCode::Char('j') => self.menu.down(),
                KeyCode::Char('q') | KeyCode::Esc => return ScreenCmd::Quit,
                KeyCode::Enter => {
                    let cmd = match self.selected() {
                        Entry::Continue => self.continue_run(st),
                        Entry::NewGame => ScreenCmd::Push {
                            to: Box::new(Chooser::new(st, st.tick)),
                            fx: TransitionKind::SlideUp,
                        },
                        Entry::Debug => self.debug_game(st),
                        Entry::Developer => {
                            self.developer = Some(Developer::new(st.tick));
                            ScreenCmd::Stay
                        }
                        Entry::Load => ScreenCmd::Push {
                            to: Box::new(RunsScreen::load(st)),
                            fx: TransitionKind::SlideUp,
                        },
                        Entry::Hall => ScreenCmd::Push {
                            to: Box::new(RunsScreen::hall(st)),
                            fx: TransitionKind::SlideUp,
                        },
                        Entry::Manual => ScreenCmd::Push {
                            to: Box::new(crate::screens::manual::Manual::new()),
                            fx: TransitionKind::SlideUp,
                        },
                        Entry::Quit => ScreenCmd::Quit,
                    };
                    if !matches!(cmd, ScreenCmd::Stay) {
                        return cmd;
                    }
                }
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let w = logo::width(TITLE);
        let (x, y) = logo_origin(area);
        Logo {
            text: TITLE,
            tick: st.tick,
            accent: LOGO_ACCENT,
            steady: false,
        }
        .render(
            Rect::new(x, y, w.min(area.width), logo::ROWS),
            f.buffer_mut(),
        );

        let p = ((st.tick.saturating_sub(self.opened)) as f32 / 30.0).clamp(0.0, 1.0);
        let sub = typewriter(SUBTITLE, p);
        let sx = area.x + area.width.saturating_sub(SUBTITLE.chars().count() as u16) / 2;
        f.render_widget(
            Paragraph::new(Span::styled(
                sub,
                Style::default().fg(Color::Rgb(0, 229, 255)),
            )),
            Rect::new(
                sx,
                y + logo::ROWS + 1,
                area.width.saturating_sub(sx - area.x),
                1,
            ),
        );

        let menu_y = y + logo::ROWS + 4;
        if let Some(dev) = self.developer.as_ref() {
            dev.draw(area, menu_y, st.tick, f);
        } else {
            let menu_x = area.x + area.width / 2 - 6;
            MenuView {
                menu: &self.menu,
                tick: st.tick,
                accent: Rgb(0, 255, 230),
            }
            .render(
                Rect::new(
                    menu_x,
                    menu_y,
                    area.width.saturating_sub(menu_x - area.x),
                    self.menu.items.len() as u16,
                ),
                f.buffer_mut(),
            );
            self.draw_run_line(area, menu_y, st.tick, f);
        }

        let hint =
            (self.developer.as_ref()).map_or("↑↓ select · enter confirm · q quit", Developer::hint);
        let hx = area.x + area.width.saturating_sub(hint.chars().count() as u16) / 2;
        f.render_widget(
            Paragraph::new(Span::styled(hint, Style::default().fg(FAINT))),
            Rect::new(
                hx,
                area.bottom() - 2,
                area.width.saturating_sub(hx - area.x),
                1,
            ),
        );
    }
}

/// The COMBAT start on `map` from a dev's title, as its keys reach it: DEVELOPER, then its
/// COMBAT row.
#[cfg(test)]
pub(crate) fn combat_start(st: &mut AppState, map: CombatMap) -> ScreenCmd {
    use crate::screens::testutil::key;
    let mut t = Title::new(st, 0);
    let down_to = |t: &mut Title, st: &mut AppState, label: &str| {
        let menu = |t: &Title| match t.developer.as_ref() {
            Some(d) => (d.menu.items.clone(), d.menu.selected),
            None => (t.menu.items.clone(), t.menu.selected),
        };
        let at = (menu(t).0.iter().position(|r| r == label)).expect("dev mode's rows");
        while menu(t).1 != at {
            t.update(st, &[key(KeyCode::Down)]);
        }
    };
    down_to(&mut t, st, "DEVELOPER");
    t.update(st, &[key(KeyCode::Enter)]);
    down_to(&mut t, st, "COMBAT");
    if map == CombatMap::Net {
        t.update(st, &[key(KeyCode::Left)]);
    }
    t.update(st, &[key(KeyCode::Enter)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, Screen, ScreenCmd};
    use crate::runs::testutil::{Dir, saving, small_city};
    use crate::screens::testutil::{buffer_text, key};
    use crossterm::event::KeyCode;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn title_logo_fits_an_80_column_terminal() {
        assert!(logo::width(TITLE) <= 80);
    }

    #[test]
    fn title_renders_logo_and_menu_and_quits() {
        let mut st = AppState::new();
        st.unlock_dev();
        let mut title = Title::new(&st, 0);
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        // The menu types itself in: by tick 120 all five items stand.
        st.tick = 120;
        t.draw(|f| title.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(
            text.contains("NEW GAME")
                && text.contains("NEW GAME (DEBUG)")
                && text.contains("LOAD GAME")
                && text.contains("QUIT"),
            "{text}"
        );
        assert!(text.contains('█'));
        st.tick = 600;
        t.draw(|f| title.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("h i g h   t e c h"), "{text}");
        assert!(!text.contains("g e n e r a t o r"), "{text}");
        assert!(matches!(
            title.update(&mut st, &[key(KeyCode::Char('q'))]),
            ScreenCmd::Quit
        ));
        // NEW GAME, NEW GAME (DEBUG), DEVELOPER, LOAD GAME, HALL OF FAME, MANUAL, QUIT.
        title.update(&mut st, &[key(KeyCode::Down); 6]);
        assert!(matches!(
            title.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Quit
        ));
        // MANUAL, one up, opens the booklet over the title
        title.update(&mut st, &[key(KeyCode::Up)]);
        assert!(matches!(
            title.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Push { .. }
        ));
        title.update(&mut st, &[key(KeyCode::Up); 5]);
        assert!(matches!(
            title.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Push { .. }
        ));
        title.update(
            &mut st,
            &[
                key(KeyCode::Char('j')),
                key(KeyCode::Char('j')),
                key(KeyCode::Char('j')),
            ],
        );
        assert!(
            matches!(
                title.update(&mut st, &[key(KeyCode::Enter)]),
                ScreenCmd::Push { .. }
            ),
            "load is pushed over the title"
        );
    }

    #[test]
    fn without_dev_mode_the_title_has_no_debug_starts() {
        let mut st = AppState::new();
        let mut title = Title::new(&st, 0);
        assert_eq!(
            title.entries,
            [
                Entry::NewGame,
                Entry::Load,
                Entry::Hall,
                Entry::Manual,
                Entry::Quit
            ],
            "a player's title"
        );
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        st.tick = 120;
        t.draw(|f| title.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(
            text.contains("NEW GAME") && text.contains("LOAD GAME") && text.contains("QUIT"),
            "{text}"
        );
        assert!(
            !text.contains("DEBUG")
                && !text.contains("DEVELOPER")
                && !text.contains("COMBAT")
                && !text.contains("STREET"),
            "{text}"
        );
        // The second row loads, the third is the hall, the fourth the manual, the fifth
        // quits; the arrows pick nothing.
        title.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Left)]);
        assert!(title.developer.is_none());
        assert_eq!(title.selected(), Entry::Load);
        assert!(matches!(
            title.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Push { .. }
        ));
        for _ in 0..2 {
            title.update(&mut st, &[key(KeyCode::Down)]);
            assert!(matches!(
                title.update(&mut st, &[key(KeyCode::Enter)]),
                ScreenCmd::Push { .. }
            ));
        }
        title.update(&mut st, &[key(KeyCode::Down)]);
        assert!(matches!(
            title.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Quit
        ));
        // Dev mode turned off in a game leaves the debug starts off the next title.
        st.unlock_dev();
        st.set_dev(false);
        assert_eq!(Title::new(&st, 0).entries.len(), 5);
        st.set_dev(true);
        assert_eq!(Title::new(&st, 0).entries.len(), 7);
    }

    #[test]
    fn new_game_opens_the_chooser() {
        let mut st = AppState::new();
        let mut t = Title::new(&st, 0);
        assert!(
            matches!(
                t.update(&mut st, &[key(KeyCode::Enter)]),
                ScreenCmd::Push {
                    fx: TransitionKind::SlideUp,
                    ..
                }
            ),
            "new game opens the chooser"
        );
        let mut t = Title::new(&st, 0);
        t.update(&mut st, &[key(KeyCode::Down)]);
        assert!(
            matches!(
                t.update(&mut st, &[key(KeyCode::Enter)]),
                ScreenCmd::Push { .. }
            ),
            "load is pushed over the title"
        );
    }

    #[test]
    fn the_combat_entry_opens_the_lot_at_dusk_with_a_goon_twelve_tiles_off() {
        use citygen::{Place, Pos, StratumKind};
        let mut st = AppState::new();
        st.unlock_dev();
        let mut t = Title::new(&st, 0);
        open_developer(&mut t, &mut st);
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        st.tick = 120;
        term.draw(|f| t.draw(&st, f)).unwrap();
        let text = buffer_text(term.backend().buffer());
        assert!(text.contains("COMBAT"), "{text}");
        // Five hours of ticks already spent this session: the lot still opens at 18:15.
        st.tick = 90 * 300;
        to_row(&mut t, &mut st, "COMBAT");
        let ScreenCmd::Switch {
            to: map,
            fx: TransitionKind::Glitch,
        } = t.update(&mut st, &[key(KeyCode::Enter)])
        else {
            panic!("straight to the lot")
        };
        assert_eq!(st.character.handle, "D'SHAWN");
        assert!(st.character.has_skill(&game::SkillId::new("handguns")));
        assert!(st.city.as_ref().is_some_and(|c| c.is_lot()));
        assert_eq!(st.minute_of_day(), 18 * 60 + 15);
        let p = st.player.as_ref().expect("on the lot");
        assert_eq!(
            (p.place, p.pos),
            (Place::Stratum(StratumKind::Street), Pos::new(6, 8))
        );
        assert_eq!(st.foes.len(), 2);
        assert_eq!(st.foes[0].pos, Pos::new(38, 16), "the far corner");
        assert!(!st.foes[0].hunting());
        assert!(st.foes[1].inert && st.foes[1].pos == Pos::new(6, 2));
        assert!(
            st.world.as_ref().is_some_and(|w| w.people().is_empty()),
            "nobody lives on the lot"
        );
        assert!(st.net.engaged.is_none());
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| map.draw(&st, f)).unwrap();
        let text = buffer_text(term.backend().buffer());
        assert!(text.contains("on foot · the lot"), "{text}");
    }

    #[test]
    fn the_combat_row_picks_its_map_with_the_arrows_and_net_jacks_you_into_a_construct() {
        use citygen::Place;
        let mut st = AppState::new();
        st.unlock_dev();
        let mut t = Title::new(&st, 0);
        open_developer(&mut t, &mut st);
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        st.tick = 120;
        term.draw(|f| t.draw(&st, f)).unwrap();
        let text = buffer_text(term.backend().buffer());
        let row = text.lines().position(|l| l.contains("COMBAT")).unwrap();
        assert!(
            text.lines().nth(row).unwrap().contains("NET · STREET"),
            "the pick on COMBAT's row: {text}"
        );
        // STREET picked by default, in pink; NET in grey. Left flips them.
        let cell_of = |term: &Terminal<TestBackend>, word: &str| {
            let line = text.lines().nth(row).unwrap();
            let col = line[..line.find(word).unwrap()].chars().count() as u16;
            term.backend().buffer()[(col, row as u16)].fg
        };
        assert_eq!(cell_of(&term, "STREET"), PICKED);
        assert_eq!(cell_of(&term, "NET "), UNPICKED);
        let combat = |t: &Title| t.developer.as_ref().unwrap().combat;
        to_row(&mut t, &mut st, "COMBAT");
        t.update(&mut st, &[key(KeyCode::Left)]);
        assert_eq!(combat(&t), CombatMap::Net);
        term.draw(|f| t.draw(&st, f)).unwrap();
        assert_eq!(cell_of(&term, "NET "), PICKED);
        assert_eq!(cell_of(&term, "STREET"), UNPICKED);
        // The arrows pick nothing of COMBAT's on another row.
        t.update(&mut st, &[key(KeyCode::Down)]);
        t.update(&mut st, &[key(KeyCode::Right)]);
        assert_eq!(combat(&t), CombatMap::Net);
        t.update(&mut st, &[key(KeyCode::Up)]);
        // NET: straight onto the lot's construct, no broadcast.
        let ScreenCmd::Switch { to: map, .. } = t.update(&mut st, &[key(KeyCode::Enter)]) else {
            panic!("straight to the map")
        };
        assert_eq!(st.character.handle, "D'SHAWN");
        assert!(st.city.as_ref().is_some_and(|c| c.is_lot()));
        let p = st.player.as_ref().expect("jacked in");
        assert!(p.jacked_in());
        let Place::Construct(b) = p.place else {
            panic!("inside a construct: {:?}", p.place)
        };
        let k = st.constructs.get(b).expect("the construct stands");
        assert_eq!(p.pos, k.entry);
        assert_eq!(k.guards.len(), 2, "a grey ICE and a dummy");
        assert!(!k.guards[0].inert && k.guards[1].inert);
        assert!(
            st.character.loaded.contains(&game::ItemId::new("attack")),
            "attack loaded"
        );
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        term.draw(|f| map.draw(&st, f)).unwrap();
        let text = buffer_text(term.backend().buffer());
        assert!(text.contains("the lot"), "{text}");
    }

    #[test]
    fn new_game_debug_starts_dshawn_maxed_out_in_a_fresh_city() {
        let mut st = AppState::new();
        st.unlock_dev();
        let mut t = Title::new(&st, 0);
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        st.tick = 60;
        term.draw(|f| t.draw(&st, f)).unwrap();
        assert!(buffer_text(term.backend().buffer()).contains("NEW GAME (DEBUG)"));
        t.update(&mut st, &[key(KeyCode::Down)]);
        let ScreenCmd::Switch {
            fx: TransitionKind::Glitch,
            ..
        } = t.update(&mut st, &[key(KeyCode::Enter)])
        else {
            panic!("straight into the city")
        };
        assert_eq!(st.character.handle, "D'SHAWN");
        assert_eq!(st.character.level, 50);
        assert!(st.character.flags(&st.content).contains("deep_end"));
        let p = st.player.as_ref().expect("walking in the new city");
        if let Some(h) = st.home {
            assert_eq!(
                p.place,
                citygen::Place::Floor {
                    building: h.home.building,
                    level: h.home.level
                },
                "a new game wakes up at home"
            );
        }
    }

    /// DEVELOPER opened from the title.
    fn open_developer(t: &mut Title, st: &mut AppState) {
        let at = t
            .entries
            .iter()
            .position(|e| *e == Entry::Developer)
            .unwrap();
        while t.menu.selected != at {
            t.update(st, &[key(KeyCode::Down)]);
        }
        assert!(matches!(
            t.update(st, &[key(KeyCode::Enter)]),
            ScreenCmd::Stay
        ));
    }

    /// DEVELOPER's row labelled `label` under the cursor.
    fn to_row(t: &mut Title, st: &mut AppState, label: &str) {
        let items = &t.developer.as_ref().unwrap().menu.items;
        let at = items.iter().position(|r| r == label).unwrap();
        while t.developer.as_ref().unwrap().menu.selected != at {
            t.update(st, &[key(KeyCode::Down)]);
        }
    }

    #[test]
    fn developer_holds_the_test_starts() {
        use citygen::venue::VenuePack;
        let mut st = AppState::new();
        st.unlock_dev();
        let mut t = Title::new(&st, 0);
        assert_eq!(
            t.menu.items,
            [
                "NEW GAME",
                "NEW GAME (DEBUG)",
                "DEVELOPER",
                "LOAD GAME",
                "HALL OF FAME",
                "MANUAL",
                "QUIT"
            ]
        );
        let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
        st.tick = 120;
        term.draw(|f| t.draw(&st, f)).unwrap();
        let text = buffer_text(term.backend().buffer());
        assert!(text.contains("DEVELOPER"), "{text}");
        assert!(
            !text.contains("NEW GAME · COMBAT") && !text.contains("LOAD CITY"),
            "{text}"
        );
        // DEVELOPER: a row per type, named from the pack, then SITE NET, COMBAT and LOAD
        // CITY, under the logo in place of the title's own rows.
        open_developer(&mut t, &mut st);
        let mut rows: Vec<String> = (VenuePack::builtin().types.iter())
            .map(|x| x.name.to_uppercase())
            .collect();
        let types = rows.len();
        rows.extend(["SITE NET", "COMBAT", "LOAD CITY"].map(String::from));
        assert_eq!(t.developer.as_ref().unwrap().menu.items, rows);
        st.tick = 300;
        term.draw(|f| t.draw(&st, f)).unwrap();
        let text = buffer_text(term.backend().buffer());
        assert!(text.contains('█'), "the logo stays: {text}");
        let lines: Vec<&str> = text.lines().collect();
        let line_of = |label: &str| {
            *lines
                .iter()
                .find(|l| l.contains(&format!(" {label} ")))
                .unwrap_or_else(|| panic!("no row {label}: {text}"))
        };
        for name in &rows[..types] {
            let line = line_of(name);
            assert!(
                line.contains("SMALL · MEDIUM · LARGE") && line.contains("PLAIN · MISSION"),
                "{line}"
            );
        }
        assert!(
            !line_of("SITE NET").contains("SMALL"),
            "a start with no picks"
        );
        assert!(line_of("COMBAT").contains("NET · STREET"));
        assert!(!text.contains("HALL OF FAME"), "{text}");
        assert!(text.contains("esc back"), "its keys under it: {text}");
        assert!(
            text.contains("←→ size") && text.contains("tab plain/mission"),
            "{text}"
        );
        // The arrows and tab pick on a type's row.
        let picks = |t: &Title| t.developer.as_ref().unwrap().sites[0];
        assert_eq!(
            picks(&t),
            (citygen::venue::MissionSize::Small, true),
            "SMALL and MISSION to start"
        );
        t.update(&mut st, &[key(KeyCode::Right), key(KeyCode::Right)]);
        t.update(&mut st, &[key(KeyCode::Tab)]);
        assert_eq!(picks(&t), (citygen::venue::MissionSize::Large, false));
        t.update(&mut st, &[key(KeyCode::Left)]);
        assert_eq!(picks(&t).0, citygen::venue::MissionSize::Medium);
        // Esc goes back to the title, on DEVELOPER.
        assert!(matches!(
            t.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Stay
        ));
        assert!(t.developer.is_none());
        assert_eq!(t.selected(), Entry::Developer);
        st.tick = 600;
        term.draw(|f| t.draw(&st, f)).unwrap();
        assert!(buffer_text(term.backend().buffer()).contains("HALL OF FAME"));
        // A player's title has none of it.
        assert!(!labels(&AppState::new()).contains(&"DEVELOPER".to_string()));
    }

    #[test]
    fn a_developer_site_row_starts_at_its_door() {
        use crate::venues::{DoorMark, door_mark_at};
        use citygen::venue::{MissionSize, VenuePack};
        use citygen::{Place, StratumKind};
        let t0 = &VenuePack::builtin().types[0];
        // A fixed city, the same every run: the clock's seed once gave one it failed on.
        pin_seed(Some(DEVELOPER_SEED));
        for mission in [true, false] {
            let dir = Dir::new("developer-site");
            let mut st = AppState::new();
            st.unlock_dev();
            st.saves = Some(dir.0.clone());
            let mut t = Title::new(&st, 0);
            open_developer(&mut t, &mut st);
            to_row(&mut t, &mut st, &t0.name.to_uppercase());
            t.update(&mut st, &[key(KeyCode::Right), key(KeyCode::Right)]);
            if !mission {
                t.update(&mut st, &[key(KeyCode::Tab)]);
            }
            let ScreenCmd::Switch {
                to: map,
                fx: TransitionKind::Glitch,
            } = t.update(&mut st, &[key(KeyCode::Enter)])
            else {
                panic!("straight into the city")
            };
            assert_eq!(st.character.handle, "D'SHAWN");
            assert_eq!(st.character.level, 50);
            assert!(st.run.is_some(), "a run of his own");
            let p = st.player.as_ref().expect("on the street");
            assert_eq!(p.place, Place::Stratum(StratumKind::Street));
            // Beside a street door of a building laid out as the type, away from home.
            let city = st.city.as_ref().unwrap();
            let home = st.home.map(|h| h.home.building);
            let (b, door) = (city.buildings.iter())
                .filter(|x| Some(x.id) != home)
                .find_map(|x| {
                    let v = st.interiors.plan_of(x.id)?.venue.as_deref()?;
                    let d = (v.frame.doors.iter()).find(|d| {
                        d.stratum == StratumKind::Street && d.door.manhattan(p.pos) == 1
                    })?;
                    (v.type_id == t0.id).then_some((x.id, d.door))
                })
                .expect("beside the site's street door");
            let mark = door_mark_at(&st, StratumKind::Street, door).map(|(_, m)| m);
            let copy = st.interiors.copy(b).map(|c| (c.live, c.inputs.size));
            if mission {
                assert_eq!(mark, Some(DoorMark::Flagged));
                assert_eq!(copy, Some((true, MissionSize::Large)));
            } else {
                assert_eq!(mark, None, "a plain site's door");
                assert_eq!(copy, None);
            }
            // The map opens where you stand, not at home.
            let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
            term.draw(|f| map.draw(&st, f)).unwrap();
            let text = buffer_text(term.backend().buffer());
            let footer = text.lines().find(|l| l.contains(" on foot · ")).unwrap();
            assert!(footer.contains(" · Street"), "{footer}");
            // The start was saved before the site was laid: it's saved again at the map's
            // next frame, so CONTINUE opens there with the site.
            assert!(st.last_save.is_some() && st.save_due, "a save due");
        }
    }

    /// The city seed the DEVELOPER starts' tests pin NEW GAME (DEBUG)'s city to.
    const DEVELOPER_SEED: u64 = 11;

    #[test]
    fn site_net_starts_outside_an_office_copy_at_190() {
        use citygen::venue::{MissionSize, VenuePack};
        use citygen::{BuildingKind, Place, RoomKind, StratumKind};
        pin_seed(Some(DEVELOPER_SEED));
        let dir = Dir::new("developer-site-net");
        let mut st = AppState::new();
        st.unlock_dev();
        st.saves = Some(dir.0.clone());
        let mut t = Title::new(&st, 0);
        open_developer(&mut t, &mut st);
        to_row(&mut t, &mut st, "SITE NET");
        let ScreenCmd::Switch {
            to: map,
            fx: TransitionKind::Glitch,
        } = t.update(&mut st, &[key(KeyCode::Enter)])
        else {
            panic!("straight into the city")
        };
        assert_eq!(st.character.handle, "D'SHAWN");
        assert!(st.run.is_some(), "a run of his own");
        // On the street beside the main door of a building away from home…
        let p = st.player.as_ref().expect("on the street");
        assert_eq!(p.place, Place::Stratum(StratumKind::Street));
        let at = game::At {
            place: p.place,
            pos: p.pos,
        };
        let b = crate::venues::building_at_door(&st, at).expect("at a building's door");
        assert_ne!(st.home.map(|h| h.home.building), Some(b), "away from home");
        // …laid out as the type that builds offices, a live MEDIUM copy at 190 on it…
        let t = (VenuePack::builtin().for_kind(BuildingKind::Office)).unwrap();
        let v = st.interiors.plan_of(b).unwrap().venue.as_deref().unwrap();
        assert_eq!(v.type_id, t.id);
        let copy = st.interiors.copy(b).expect("the mission's copy");
        assert!(copy.live);
        assert_eq!(copy.inputs.size, MissionSize::Medium);
        assert_eq!(copy.inputs.security, 190);
        // …whose objective is in a server room.
        let room = &t.objectives[copy.inputs.objective].room;
        assert_eq!(t.room(room).map(|r| r.kind), Some(RoomKind::Server));
        // The map opens there, and the start is saved again at its next frame.
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| map.draw(&st, f)).unwrap();
        let text = buffer_text(term.backend().buffer());
        let footer = text.lines().find(|l| l.contains(" on foot · ")).unwrap();
        assert!(footer.contains(" · Street"), "{footer}");
        assert!(st.last_save.is_some() && st.save_due, "a save due");
    }

    /// A run saved in `dir` by `handle` at frame `tick`, standing at `at` when given, its
    /// header stamped as saved at Unix second `saved`.
    fn save_run(
        dir: &Dir,
        handle: &str,
        tick: u64,
        at: Option<(citygen::Place, citygen::Pos)>,
        saved: u64,
    ) -> (AppState, std::path::PathBuf) {
        save_run_in(dir, small_city(), handle, tick, at, saved)
    }

    /// `save_run` in `city`.
    fn save_run_in(
        dir: &Dir,
        city: citygen::City,
        handle: &str,
        tick: u64,
        at: Option<(citygen::Place, citygen::Pos)>,
        saved: u64,
    ) -> (AppState, std::path::PathBuf) {
        let (mut st, _) = saving(dir, city);
        st.character.handle = handle.into();
        st.tick = tick;
        if let Some((place, pos)) = at {
            st.player = Some(crate::walker::Player::at(place, pos));
        }
        assert!(crate::runs::when::save_now(&mut st));
        let path = dir.0.join(format!("{}.run", st.run.unwrap().id));
        let mut h = crate::runs::file::header(&path).unwrap();
        h.saved = saved;
        crate::runs::file::rewrite_header(&path, &h).unwrap();
        (st, path)
    }

    /// A street tile outside any building.
    fn on_the_street(city: &citygen::City) -> (citygen::Place, citygen::Pos) {
        use citygen::{Place, StratumKind};
        let pos = city
            .stratum(StratumKind::Street)
            .unwrap()
            .tiles
            .iter()
            .find_map(|(p, t)| (t.building.is_none() && t.district.is_some()).then_some(p))
            .unwrap();
        (Place::Stratum(StratumKind::Street), pos)
    }

    fn labels(st: &AppState) -> Vec<String> {
        Title::new(st, 0).menu.items
    }

    #[test]
    fn no_continue_without_a_run() {
        let rows = ["NEW GAME", "LOAD GAME", "HALL OF FAME", "MANUAL", "QUIT"];
        let mut st = AppState::new();
        assert_eq!(labels(&st), rows, "no saves folder");
        let dir = Dir::new("no-continue");
        st.saves = Some(dir.0.clone());
        assert_eq!(labels(&st), rows, "an empty one");
        // A run that flatlined, and a file that isn't a run: still nothing to continue.
        let (_, path) = save_run(&dir, "Gone", 900, None, 100);
        let mut h = crate::runs::file::header(&path).unwrap();
        h.death = Some(crate::runs::Death {
            cause: "shot".into(),
            day: 1,
            minute: 0,
            at: 100,
            lasted: 40,
        });
        crate::runs::file::rewrite_header(&path, &h).unwrap();
        std::fs::write(dir.0.join("broken.run"), b"not a run").unwrap();
        assert_eq!(labels(&st), rows);
        // In dev mode the debug start and DEVELOPER join them.
        st.unlock_dev();
        assert_eq!(
            labels(&st),
            [
                "NEW GAME",
                "NEW GAME (DEBUG)",
                "DEVELOPER",
                "LOAD GAME",
                "HALL OF FAME",
                "MANUAL",
                "QUIT"
            ]
        );
    }

    #[test]
    fn continue_opens_the_newest_run() {
        let dir = Dir::new("continue");
        save_run(&dir, "Older", 5_000, None, 1_000);
        // The newer in a city with a home, its story not begun: a resume that started
        // the story would show.
        let city = crate::runs::testutil::story_city();
        let street = on_the_street(&city);
        let (newer, _) = save_run_in(&dir, city, "Newer", 7_000, Some(street), 2_000);
        assert!(newer.home.is_some() && !newer.missions.begun);
        let mut st = AppState::new();
        st.saves = Some(dir.0.clone());
        let mut t = Title::new(&st, 0);
        assert_eq!(
            t.entries,
            [
                Entry::Continue,
                Entry::NewGame,
                Entry::Load,
                Entry::Hall,
                Entry::Manual,
                Entry::Quit
            ]
        );
        assert_eq!(t.selected(), Entry::Continue, "the cursor starts on it");
        let ScreenCmd::Switch { to: mut on_air, .. } = t.update(&mut st, &[key(KeyCode::Enter)])
        else {
            panic!("CONTINUE plays the broadcast")
        };
        assert_eq!(st.character.handle, "Newer");
        assert_eq!(st.run, newer.run, "the newer run");
        assert_eq!(st.tick, 7_000, "its clock");
        assert!(
            !crate::runs::when::marked(&st),
            "opening a run isn't saving it"
        );
        let wall = st
            .last_save
            .map(|(_, at)| at.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs());
        assert_eq!(wall, Some(2_000), "last saved when its file says");
        // The broadcast shown whole, then JACK IN: back where it was saved.
        let space = key(KeyCode::Char(' '));
        on_air.update(&mut st, &[space]);
        let ScreenCmd::Switch { to: mut map, .. } = on_air.update(&mut st, &[space]) else {
            panic!("JACK IN")
        };
        let world = st.clock_tick();
        st.tick += 1;
        map.update(&mut st, &[]);
        let p = st.player.as_ref().unwrap();
        assert_eq!((p.place, p.pos), street, "where it stood");
        assert!(
            st.clock_tick() - world <= 1,
            "the first frame runs a frame, not a minute"
        );
        assert!(!st.missions.begun, "the story isn't started over");
        let text = buffer_text(&crate::screens::shots::draw_stack(
            &st,
            &[map.as_ref()],
            (120, 40),
        ));
        assert!(!text.contains("SAVED"), "no mark on the way in");
    }

    #[test]
    fn continue_shows_its_run_under_the_menu() {
        let dir = Dir::new("continue-line");
        let (run, path) = save_run(&dir, "Vex", 5_000, None, 1_000);
        let mut h = crate::runs::file::header(&path).unwrap();
        h.day = 2;
        h.minute = 7 * 60 + 40;
        h.objective = Some("Bring it to Coz · the Brass Lamp · Ironside".into());
        crate::runs::file::rewrite_header(&path, &h).unwrap();
        let mut st = AppState::new();
        st.saves = Some(dir.0.clone());
        let mut t = Title::new(&st, 0);
        st.tick = 120;
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| t.draw(&st, f)).unwrap();
        let buf = term.backend().buffer().clone();
        let text = buffer_text(&buf);
        let city = &run.city.as_ref().unwrap().name;
        let line = format!("VEX · NETRUNNER · {city} · day 2 07:40 · Bring it to Coz");
        let x = (120 - line.chars().count() as u16) / 2;
        let row = text.lines().nth(23).unwrap();
        assert_eq!(&row.trim(), &line, "two rows under the menu, centred");
        assert!(row.starts_with(&" ".repeat(x as usize)), "{row:?}");
        assert!(text.lines().nth(15).unwrap().contains("▸ CONTINUE"));
        let v = &buf[(x, 23)];
        assert_eq!(v.fg, Color::Rgb(0, 229, 255));
        assert!(v.modifier.contains(ratatui::style::Modifier::BOLD));
        assert_eq!(
            buf[(x + 4, 23)].fg,
            Color::Rgb(100, 110, 125),
            "the rest faint"
        );
        let bring = x + line[..line.find("Bring").unwrap()].chars().count() as u16;
        assert_eq!(
            buf[(bring, 23)].fg,
            Color::Rgb(160, 170, 185),
            "the objective dim"
        );
        // Off CONTINUE, the line goes.
        t.update(&mut st, &[key(KeyCode::Down)]);
        term.draw(|f| t.draw(&st, f)).unwrap();
        let text = buffer_text(term.backend().buffer());
        assert!(!text.contains("NETRUNNER"), "{text}");
    }

    #[test]
    fn continue_says_why_a_run_wont_open() {
        let dir = Dir::new("continue-broken");
        let (_, path) = save_run(&dir, "Vex", 5_000, None, 1_000);
        // The header stands, the body is noise.
        let mut bytes = std::fs::read(&path).unwrap();
        let end = 10 + u32::from_le_bytes(bytes[6..10].try_into().unwrap()) as usize;
        for (i, b) in bytes[end..].iter_mut().enumerate() {
            *b = citygen::rng::hash64(i as u64, 7) as u8;
        }
        std::fs::write(&path, &bytes).unwrap();
        let mut st = AppState::new();
        st.saves = Some(dir.0.clone());
        let mut t = Title::new(&st, 0);
        assert_eq!(t.selected(), Entry::Continue, "its header reads");
        assert!(matches!(
            t.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Stay
        ));
        assert!(st.run.is_none() && st.city.is_none(), "nothing half opened");
        st.tick = 120;
        let mut term = Terminal::new(TestBackend::new(120, 40)).unwrap();
        term.draw(|f| t.draw(&st, f)).unwrap();
        let text = buffer_text(term.backend().buffer());
        assert_eq!(
            text.lines().nth(23).unwrap().trim(),
            "the run is broken",
            "where its line was"
        );
    }

    #[test]
    fn the_title_opens_the_lists() {
        let dir = Dir::new("title-lists");
        save_run(&dir, "Vex", 5_000, None, 1_000);
        let mut st = AppState::new();
        st.unlock_dev();
        st.saves = Some(dir.0.clone());
        let mut t = Title::new(&st, 0);
        let opens = |t: &mut Title, st: &mut AppState, label: &str| {
            let at = t.menu.items.iter().position(|r| r == label).unwrap();
            while t.menu.selected != at {
                t.update(st, &[key(KeyCode::Down)]);
            }
            let ScreenCmd::Push { to, .. } = t.update(st, &[key(KeyCode::Enter)]) else {
                panic!("{label} pushes its list")
            };
            buffer_text(&crate::screens::shots::draw_stack(
                st,
                &[&*t, to.as_ref()],
                (120, 40),
            ))
        };
        let text = opens(&mut t, &mut st, "LOAD GAME");
        assert!(text.contains("LOAD GAME") && text.contains("VEX"), "{text}");
        let text = opens(&mut t, &mut st, "HALL OF FAME");
        assert!(
            text.contains("HALL OF FAME") && text.contains("Nobody you ran has flatlined. Yet."),
            "{text}"
        );
        // LOAD CITY, inside DEVELOPER.
        open_developer(&mut t, &mut st);
        to_row(&mut t, &mut st, "LOAD CITY");
        let ScreenCmd::Push { to, .. } = t.update(&mut st, &[key(KeyCode::Enter)]) else {
            panic!("LOAD CITY pushes its list")
        };
        let text = buffer_text(&crate::screens::shots::draw_stack(
            &st,
            &[&t, to.as_ref()],
            (120, 40),
        ));
        assert!(
            text.contains("enter load"),
            "the old list of cities: {text}"
        );
        assert_eq!(st.run, None, "nothing opened");
    }

    #[test]
    fn new_game_rolls_a_fresh_seed() {
        let st = AppState::new();
        let a = Title::fresh_params(&st, 10);
        let b = Title::fresh_params(&st, 11);
        assert_ne!(a.seed, 0);
        assert_ne!(a.seed, b.seed, "two new games are two cities");
        assert_eq!(a.size, st.params.size, "everything but the seed is kept");
        assert!(
            a.seed < 1_000_000_007 && b.seed < 1_000_000_007,
            "capped like Configure's RANDOM SEED so the HUD stays readable"
        );
    }
}
