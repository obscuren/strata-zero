//! The way into a game: STRATA ZERO, what the city broadcasts as you drop in, the city
//! cut open beside it, and the tutorial and perma death a last time. NEW GAME and a city
//! load come through here with the city already built, and a run opened from its file
//! (CONTINUE, LOAD GAME) with it already in place; `space` enters it.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::broadcasts::{Aired, When, air};
use crate::missions::call::{TICKS_PER_SECOND, TYPE_PER_SECOND};
use crate::render::Rgb;
use crate::screens::create::{keys_line, wrap};
use crate::screens::title::{SUBTITLE, TITLE};
use crate::ui::chrome::{
    CYAN, DIM, FAINT, FAINTER, MAGENTA, RED, RED_TEXT, RULE, TEXT, put, reveal, solid,
};
use crate::ui::cutaway;
use crate::ui::logo::{self, Logo};
use citygen::City;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use unicode_width::UnicodeWidthStr;

/// The city cut open beside the broadcast. Off, the broadcast takes the middle and
/// nothing else changes; the art is `ui::cutaway` and the one call in `draw_art`.
pub const CUTAWAY: bool = true;

/// The smallest terminal that has room for the art beside the broadcast.
const ART_W: u16 = 120;
const ART_H: u16 = 36;
/// The broadcast's column beside the art, the gutter the strata's names sit in, and the
/// art's widest.
const COLUMN_W: u16 = 50;
const GUTTER_W: u16 = 13;
const ART_MAX_W: u16 = 56;
/// The broadcast's widest without the art.
const ALONE_W: u16 = 76;
/// The banner's width on a terminal that has room for it.
const BANNER_W: u16 = 112;
const BANNER_BG: Color = Color::Rgb(28, 5, 8);
const WHITE: Color = Color::Rgb(255, 255, 255);

/// How the screen was reached, and so what `space` and `esc` do.
pub enum Entry {
    NewGame,
    /// A city from its `.city` file (dev): the run starts over in it.
    Load {
        path: String,
    },
    /// A run opened from its file (CONTINUE, LOAD GAME), already in place: its flags are
    /// its own, and JACK IN puts you where you stood.
    Resume,
}

pub struct Broadcast {
    /// The city to enter, until it is entered.
    city: Option<City>,
    entry: Entry,
    aired: Aired,
    handle: String,
    city_name: String,
    seed: u64,
    /// The clock as the city is entered, for the tag line.
    clock: String,
    opened: u64,
    /// The broadcast's characters; it has typed out once this many have.
    chars: usize,
    /// `space` showed it all before it finished typing.
    full: bool,
    pub perma_death: bool,
    pub tutorial_off: bool,
}

/// Where everything goes on a terminal of one size.
struct Places {
    logo_y: u16,
    subtitle_y: Option<u16>,
    /// The broadcast's column, from its first row down to the row above the toggles.
    column: Rect,
    /// The art and the gutter's rule, when there is room for them.
    art: Option<(Rect, u16)>,
    toggles_y: u16,
    banner: Rect,
    footer_y: u16,
    /// The broadcast as it fits the column.
    lines: Vec<Line<'static>>,
}

/// The tag line's clock for a minute of the day: `07:40`.
fn clock(minute: u16) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

/// Whether the story's mission `id` is under way in `st`: begun and not yet done.
fn under_way(st: &AppState, id: &str) -> bool {
    st.book
        .missions
        .iter()
        .zip(&st.missions.runs)
        .any(|(m, run)| m.id == id && run.state == crate::missions::RunState::Open)
}

impl Broadcast {
    /// After JACK IN on the sheet: the build's runner, its perma death.
    pub fn new_game(st: &AppState, city: City) -> Self {
        let (handle, perma_death) = st.build.as_ref().map_or((String::new(), true), |b| {
            (b.handle.trim().to_string(), b.perma_death)
        });
        let b = st.broadcasts.pick_for(When::NewGame, city.seed, |_| false);
        let aired = air(b, &city, &handle);
        Broadcast::open(st, &city, Entry::NewGame, aired, handle, perma_death).holding(city)
    }

    /// After picking a city file (dev): the runner the session has, its flags. The story
    /// starts over in it, so no mission is under way.
    pub fn load(st: &AppState, city: City, path: String) -> Self {
        let handle = st.character.handle.clone();
        let b = st.broadcasts.pick_for(When::LoadGame, city.seed, |_| false);
        let aired = air(b, &city, &handle);
        Broadcast::open(
            st,
            &city,
            Entry::Load { path },
            aired,
            handle,
            st.perma_death,
        )
        .holding(city)
    }

    /// Over a run just opened (`runs::open`): its city, its runner, its own flags with
    /// no switch for them, its clock, and what airs at its point in the story.
    pub fn resume(st: &AppState) -> Self {
        let lot;
        let city = match st.city.as_ref() {
            Some(city) => city,
            None => {
                lot = City::lot(0);
                &lot
            }
        };
        let handle = st.character.handle.clone();
        let b = st
            .broadcasts
            .pick_for(When::LoadGame, city.seed, |id| under_way(st, id));
        let aired = air(b, city, &handle);
        let mut on_air = Broadcast::open(st, city, Entry::Resume, aired, handle, st.perma_death);
        on_air.clock = clock(st.minute_of_day());
        on_air
    }

    /// The city to enter on JACK IN.
    fn holding(mut self, city: City) -> Self {
        self.city = Some(city);
        self
    }

    /// A run opened from its file, whose flags are its own.
    fn resumed(&self) -> bool {
        matches!(self.entry, Entry::Resume)
    }

    fn open(
        st: &AppState,
        city: &City,
        entry: Entry,
        aired: Aired,
        handle: String,
        perma_death: bool,
    ) -> Self {
        let chars = aired
            .tag
            .iter()
            .map(|(t, _)| t.chars().count())
            .sum::<usize>()
            + aired.shout.chars().count()
            + aired.paras.iter().map(|p| p.chars().count()).sum::<usize>()
            + aired.sign.chars().count();
        Broadcast {
            city_name: city.name.clone(),
            seed: city.seed,
            city: None,
            entry,
            aired,
            handle,
            // The clock a city entered now starts on.
            clock: clock(crate::ui::hud::minute_at(st.tick)),
            opened: st.tick,
            chars,
            full: false,
            perma_death,
            tutorial_off: st.tutorial_off,
        }
    }

    /// A broadcast of the caller's choosing, for tests and shots.
    #[cfg(test)]
    pub fn with(st: &AppState, city: City, b: &crate::broadcasts::Broadcast) -> Self {
        let handle = st.character.handle.clone();
        let aired = air(b, &city, &handle);
        Broadcast::open(st, &city, Entry::NewGame, aired, handle, st.perma_death).holding(city)
    }

    /// How much of the broadcast has typed, 0 to 1.
    fn typed(&self, tick: u64) -> f32 {
        if self.full || self.chars == 0 {
            return 1.0;
        }
        let n = tick.saturating_sub(self.opened) * TYPE_PER_SECOND / TICKS_PER_SECOND;
        (n as f32 / self.chars as f32).min(1.0)
    }

    /// Makes the runner on a new game, sets the run's flags and drops into the city; a
    /// new game's run starts there, and is saved, once you stand in it. An opened run is
    /// in place already: back onto its map where you stood.
    fn jack_in(&mut self, st: &mut AppState) -> ScreenCmd {
        if self.resumed() {
            return ScreenCmd::Switch {
                to: crate::screens::resume_screen(st),
                fx: TransitionKind::Glitch,
            };
        }
        let Some(city) = self.city.take() else {
            return ScreenCmd::Stay;
        };
        match &self.entry {
            Entry::NewGame => {
                let Some(b) = st.build.take() else {
                    self.city = Some(city);
                    return ScreenCmd::Stay;
                };
                st.character = game::Character::from_build(&st.content, &b);
            }
            Entry::Load { path } => st.say(format!("loaded {path}")),
            Entry::Resume => {}
        }
        st.perma_death = self.perma_death;
        st.tutorial_off = self.tutorial_off;
        let cmd = crate::screens::enter_built_city(st, city);
        if matches!(self.entry, Entry::NewGame) {
            crate::runs::when::begin_run(st);
        }
        cmd
    }

    /// The broadcast's lines for a column `width` wide: the tag line and the clock, a
    /// rule, the shout, `paras` of the paragraphs and the sign-off. `tight` takes blank
    /// rows out, a few at a time: 1 the one under the rule, 2 those between
    /// paragraphs, 3 the one over the sign-off.
    fn compose(&self, width: u16, tight: u8, paras: usize) -> Vec<Line<'static>> {
        let gaps = tight < 2;
        let w = width as usize;
        let mut out = Vec::new();
        let mut tag: Vec<Span<'static>> = self
            .aired
            .tag
            .iter()
            .map(|(t, c)| match c {
                Some(c) => Span::styled(
                    t.clone(),
                    Style::default().fg(*c).add_modifier(Modifier::BOLD),
                ),
                None => Span::styled(t.clone(), Style::default().fg(TEXT)),
            })
            .collect();
        let used: usize = self.aired.tag.iter().map(|(t, _)| t.width()).sum();
        let pad = w.saturating_sub(used + self.clock.len());
        tag.push(Span::raw(" ".repeat(pad)));
        tag.push(Span::styled(self.clock.clone(), Style::default().fg(FAINT)));
        out.push(Line::from(tag));
        out.push(Line::styled("─".repeat(w), Style::default().fg(RULE)));
        if tight < 1 {
            out.push(Line::default());
        }
        let shout = Style::default().fg(WHITE).add_modifier(Modifier::BOLD);
        out.extend(
            wrap(&self.aired.shout, w)
                .into_iter()
                .map(|l| Line::styled(l, shout)),
        );
        out.push(Line::default());
        for p in self.aired.paras.iter().take(paras) {
            out.extend(
                wrap(p, w)
                    .into_iter()
                    .map(|l| Line::styled(l, Style::default().fg(TEXT))),
            );
            if gaps {
                out.push(Line::default());
            }
        }
        if tight == 2 {
            out.push(Line::default());
        }
        let sign = Style::default()
            .fg(self.aired.sign_colour)
            .add_modifier(Modifier::BOLD);
        out.extend(
            wrap(&self.aired.sign, w)
                .into_iter()
                .map(|l| Line::styled(l, sign)),
        );
        out
    }

    fn places(&self, area: Rect) -> Places {
        let roomy = area.height >= ART_H;
        let logo_y = area.y + if roomy { 2 } else { 1 };
        let subtitle_y = roomy.then_some(logo_y + logo::ROWS + 1);
        let top = match subtitle_y {
            Some(y) => y + 3,
            None => logo_y + logo::ROWS + 1,
        };
        let bottom = area.bottom();
        let toggles_y = bottom.saturating_sub(9);
        let footer_y = bottom.saturating_sub(2);
        // Rows for the broadcast, a blank one left above the toggles.
        let room = toggles_y.saturating_sub(top + 1);
        let with_art = CUTAWAY && area.width >= ART_W && roomy;
        let (column_x, column_w, art, span) = if with_art {
            let art_w = ART_MAX_W.min(area.width.saturating_sub(8 + COLUMN_W + GUTTER_W + 1));
            let span = COLUMN_W + GUTTER_W + 1 + art_w;
            let x = area.x + area.width.saturating_sub(span) / 2;
            let rule = x + COLUMN_W + GUTTER_W;
            let art = Rect::new(rule + 1, top, art_w, toggles_y + 1 - top);
            (x, COLUMN_W, Some((art, rule)), span)
        } else {
            let w = ALONE_W.min(area.width.saturating_sub(12));
            (area.x + area.width.saturating_sub(w) / 2, w, None, w)
        };
        // Blank rows go first, then paragraphs from the end.
        let all = self.aired.paras.len();
        let lines = (0..=3)
            .map(|tight| self.compose(column_w, tight, all))
            .chain((1..all).rev().map(|n| self.compose(column_w, 3, n)))
            .find(|l| l.len() <= room as usize)
            .unwrap_or_else(|| self.compose(column_w, 3, all.min(1)));
        // Alone, the broadcast sits in the middle of the rows it has.
        let y = if art.is_none() {
            top + room.saturating_sub(lines.len() as u16) / 2
        } else {
            top
        };
        let banner_w = BANNER_W.max(span).min(area.width.saturating_sub(8));
        Places {
            logo_y,
            subtitle_y,
            column: Rect::new(column_x, y, column_w, room.min(toggles_y.saturating_sub(y))),
            art,
            toggles_y,
            banner: Rect::new(
                area.x + area.width.saturating_sub(banner_w) / 2,
                bottom.saturating_sub(7),
                banner_w,
                4,
            ),
            footer_y,
            lines,
        }
    }

    fn draw_art(&self, buf: &mut Buffer, art: Rect, rule: u16) {
        cutaway::draw(buf, art, self.seed);
        for y in art.y..art.bottom() {
            put(buf, rule, y, "│", Style::default().fg(RULE), rule + 1);
        }
        for (name, row) in cutaway::BANDS.iter().zip(cutaway::band_rows(art.height)) {
            let label = format!("{name} ─");
            let x = rule.saturating_sub(label.chars().count() as u16);
            put(
                buf,
                x,
                art.y + row,
                &label,
                Style::default().fg(FAINTER),
                rule,
            );
        }
    }

    fn draw_toggles(&self, buf: &mut Buffer, column: Rect, y: u16) {
        let right = column.right();
        let bold = |c: Color| Style::default().fg(c).add_modifier(Modifier::BOLD);
        put(buf, column.x, y, "[", Style::default().fg(FAINT), right);
        let tick = if self.tutorial_off { "✓" } else { " " };
        put(buf, column.x + 1, y, tick, bold(CYAN), right);
        put(buf, column.x + 2, y, "]", Style::default().fg(FAINT), right);
        let label = if self.tutorial_off {
            bold(TEXT)
        } else {
            Style::default().fg(DIM)
        };
        put(buf, column.x + 4, y, "DISABLE TUTORIAL", label, right);
        let x = right.saturating_sub("[space] JACK IN".len() as u16);
        put(buf, x, y, "[", Style::default().fg(FAINT), right);
        put(buf, x + 1, y, "space", bold(CYAN), right);
        put(buf, x + 6, y, "]", Style::default().fg(FAINT), right);
        put(buf, x + 8, y, "JACK IN", bold(MAGENTA), right);
    }

    /// Perma death's banner: red while it is on, grey once it is off.
    fn draw_banner(&self, buf: &mut Buffer, r: Rect) {
        if r.width < 20 || r.bottom() > buf.area.bottom() {
            return;
        }
        let right = r.right();
        let inner_right = right - 1;
        let (frame, bg, title, title_style) = if self.perma_death {
            (
                RED,
                Some(BANNER_BG),
                " ▲ PERMA DEATH IS ON ",
                Style::default()
                    .fg(RED)
                    .bg(BANNER_BG)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            (
                FAINTER,
                None,
                " PERMA DEATH IS OFF ",
                Style::default().fg(DIM),
            )
        };
        let on_bg = |s: Style| match bg {
            Some(bg) => s.bg(bg),
            None => s,
        };
        let (tl, tr, bl, br, h, v) = if self.perma_death {
            ('╔', '╗', '╚', '╝', '═', '║')
        } else {
            ('╭', '╮', '╰', '╯', '─', '│')
        };
        let edge = on_bg(Style::default().fg(frame));
        let inner = (r.width - 2) as usize;
        put(
            buf,
            r.x,
            r.y,
            &format!("{tl}{}{tr}", h.to_string().repeat(inner)),
            edge,
            right,
        );
        put(
            buf,
            r.x,
            r.y + 3,
            &format!("{bl}{}{br}", h.to_string().repeat(inner)),
            edge,
            right,
        );
        for y in [r.y + 1, r.y + 2] {
            put(buf, r.x, y, &v.to_string(), edge, right);
            put(
                buf,
                r.x + 1,
                y,
                &" ".repeat(inner),
                on_bg(Style::default()),
                inner_right,
            );
            put(buf, inner_right, y, &v.to_string(), edge, right);
        }
        put(buf, r.x + 2, r.y, title, title_style, inner_right);
        let who = if self.handle.is_empty() {
            "your runner".to_string()
        } else {
            self.handle.to_uppercase()
        };
        let (first, second, second_style, verb) = if self.perma_death {
            (
                format!("One life. If {who} flatlines, the save is wiped and the run is over."),
                "This is your last chance to turn it off.",
                Style::default().fg(RED_TEXT).add_modifier(Modifier::BOLD),
                "turn it off",
            )
        } else {
            (
                "A flatline sends you back to your last save.".to_string(),
                "Turn it on now, or play this run without it.",
                Style::default().fg(FAINT),
                "turn it on",
            )
        };
        let first_style = Style::default().fg(if self.perma_death { TEXT } else { DIM });
        put(
            buf,
            r.x + 3,
            r.y + 1,
            &first,
            on_bg(first_style),
            inner_right - 1,
        );
        // An opened run's perma death is its own: no line about switching it.
        if self.resumed() {
            return;
        }
        // `[p]turn it off` at the right, in the footer's style.
        let keys_x = inner_right.saturating_sub(2 + 3 + verb.len() as u16);
        put(
            buf,
            r.x + 3,
            r.y + 2,
            second,
            on_bg(second_style),
            keys_x.saturating_sub(1),
        );
        let mut x = keys_x;
        for span in keys_line(&[("p", verb)]).spans {
            let text = span.content.trim_end();
            put(buf, x, r.y + 2, text, on_bg(span.style), inner_right - 1);
            x += text.width() as u16;
        }
    }
}

impl Screen for Broadcast {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        // A space that shows it all does only that, however many come with it.
        let mut shown = false;
        for k in keys {
            match k.code {
                KeyCode::Char(' ') if self.typed(st.tick) < 1.0 => {
                    self.full = true;
                    shown = true;
                }
                KeyCode::Char(' ') if !shown => return self.jack_in(st),
                // An opened run keeps its own: quitting and reopening it can't turn perma
                // death off.
                KeyCode::Char('t') if !self.resumed() => self.tutorial_off = !self.tutorial_off,
                KeyCode::Char('p') if !self.resumed() => self.perma_death = !self.perma_death,
                // The run opened is in place of whatever was under the list, so there is
                // nothing to go back to: it is left, for the title.
                KeyCode::Esc if self.resumed() => {
                    st.run = None;
                    return ScreenCmd::Switch {
                        to: Box::new(crate::screens::title::Title::new(st, st.tick)),
                        fx: TransitionKind::SlideUp,
                    };
                }
                KeyCode::Esc => {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::SlideUp,
                    };
                }
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let buf = f.buffer_mut();
        solid(buf, area);
        let at = self.places(area);
        let w = logo::width(TITLE);
        Logo {
            text: TITLE,
            tick: st.tick,
            accent: Rgb(255, 0, 200),
            steady: false,
        }
        .render(
            Rect::new(
                area.x + area.width.saturating_sub(w) / 2,
                at.logo_y,
                w.min(area.width),
                logo::ROWS,
            ),
            buf,
        );
        if let Some(y) = at.subtitle_y {
            let x = area.x + area.width.saturating_sub(SUBTITLE.chars().count() as u16) / 2;
            put(buf, x, y, SUBTITLE, Style::default().fg(CYAN), area.right());
        }
        if let Some((art, rule)) = at.art {
            self.draw_art(buf, art, rule);
        }
        let shown = reveal(
            &at.lines,
            self.typed(st.tick),
            (st.tick / 4).is_multiple_of(2),
        );
        for (i, line) in shown.iter().enumerate() {
            let y = at.column.y + i as u16;
            if y >= at.toggles_y {
                break;
            }
            buf.set_line(at.column.x, y, line, at.column.width);
        }
        self.draw_toggles(buf, at.column, at.toggles_y);
        self.draw_banner(buf, at.banner);
        let keys: &[(&str, &str)] = if self.resumed() {
            &[("space", "JACK IN"), ("esc", "back")]
        } else {
            &[
                ("space", "JACK IN"),
                ("t", "tutorial"),
                ("p", "perma death"),
                ("esc", "back"),
            ]
        };
        buf.set_line(
            area.x + 2,
            at.footer_y,
            &keys_line(keys),
            area.width.saturating_sub(4),
        );
        let who = format!("{} · {}", self.city_name, self.handle);
        let x = area.right().saturating_sub(2 + who.width() as u16);
        put(
            buf,
            x,
            at.footer_y,
            &who,
            Style::default().fg(FAINTER),
            area.right(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::testutil::{buffer_text, key};
    use citygen::{GenParams, Generator, SizePreset};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn small(seed: u64) -> City {
        Generator::run(GenParams {
            seed,
            size: SizePreset::Custom { w: 80, h: 40 },
            districts: 4,
            ..Default::default()
        })
        .unwrap()
    }

    /// A new game's state, VEX's build on the sheet.
    fn new_game() -> (AppState, Broadcast) {
        let mut st = AppState::new();
        let t = st
            .content
            .template(&game::TemplateId::new("vex"))
            .unwrap()
            .clone();
        st.build = Some(game::Build::from_template(&st.content, &t));
        let b = Broadcast::new_game(&st, small(2));
        (st, b)
    }

    fn draw(b: &Broadcast, st: &AppState, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| b.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    const SPACE: KeyCode = KeyCode::Char(' ');

    #[test]
    fn the_first_space_only_shows_it_all() {
        let (mut st, mut b) = new_game();
        assert!(b.typed(st.tick) < 1.0);
        let cmd = b.update(&mut st, &[key(SPACE), key(SPACE), key(SPACE)]);
        assert!(matches!(cmd, ScreenCmd::Stay));
        assert_eq!(b.typed(st.tick), 1.0);
        assert!(st.city.is_none());
        assert!(matches!(
            b.update(&mut st, &[key(SPACE)]),
            ScreenCmd::Switch { .. }
        ));
    }

    #[test]
    fn space_enters_the_city_with_the_build_and_the_flags() {
        let (mut st, mut b) = new_game();
        st.build.as_mut().unwrap().handle = "Nyx".into();
        b.update(&mut st, &[key(KeyCode::Char('p')), key(KeyCode::Char('t'))]);
        b.update(&mut st, &[key(SPACE)]);
        let cmd = b.update(&mut st, &[key(SPACE)]);
        assert!(matches!(cmd, ScreenCmd::Switch { .. }));
        assert!(st.city.is_some());
        assert_eq!(st.character.handle, "Nyx");
        assert!(st.build.is_none());
        assert!(!st.perma_death && st.tutorial_off);
    }

    #[test]
    fn a_load_starts_from_the_runs_flags_and_says_so() {
        let mut st = AppState::new();
        st.perma_death = false;
        let mut b = Broadcast::load(&st, small(3), "cities/x.city".into());
        b.full = true;
        let text = draw(&b, &st, 120, 40);
        assert!(text.contains("PERMA DEATH IS OFF"), "{text}");
        assert!(matches!(
            b.update(&mut st, &[key(SPACE)]),
            ScreenCmd::Switch { .. }
        ));
        assert!(!st.perma_death && !st.tutorial_off);
        assert!(
            st.log
                .iter()
                .any(|e| e.text.contains("loaded cities/x.city"))
        );
    }

    /// A run saved in `dir` with these flags, and the path of its file.
    fn saved_run(dir: &crate::runs::testutil::Dir, perma: bool) -> std::path::PathBuf {
        use crate::runs::testutil::{saving, small_city};
        let (mut a, _) = saving(dir, small_city());
        a.tick = 4_000;
        a.perma_death = perma;
        a.tutorial_off = true;
        assert!(crate::runs::when::save_now(&mut a));
        dir.0.join(format!("{}.run", a.run.unwrap().id))
    }

    #[test]
    fn a_reopened_run_keeps_its_flags() {
        let dir = crate::runs::testutil::Dir::new("flags");
        for perma in [true, false] {
            let path = saved_run(&dir, perma);
            let mut st = AppState::new();
            st.saves = Some(dir.0.clone());
            // The session's flags are the other way round.
            st.perma_death = !perma;
            st.tutorial_off = false;
            crate::runs::open(&mut st, &path).unwrap();
            assert_eq!(
                (st.perma_death, st.tutorial_off),
                (perma, true),
                "the run's own"
            );
            let mut b = Broadcast::resume(&st);
            b.full = true;
            let text = draw(&b, &st, 120, 40);
            let banner = if perma {
                "PERMA DEATH IS ON"
            } else {
                "PERMA DEATH IS OFF"
            };
            for want in [banner, "[✓] DISABLE TUTORIAL", "[space] JACK IN", "VEX"] {
                assert!(text.contains(want), "wants {want:?}: {text}");
            }
            for hint in [
                "[p]",
                "[t]",
                "last chance",
                "Turn it on now",
                "perma death",
                "tutorial",
            ] {
                assert!(!text.contains(hint), "no switch: {hint:?} in {text}");
            }
            b.update(&mut st, &[key(KeyCode::Char('p')), key(KeyCode::Char('t'))]);
            assert_eq!((b.perma_death, b.tutorial_off), (perma, true));
            assert_eq!(draw(&b, &st, 120, 40), text, "p and t change nothing");
            assert!(matches!(
                b.update(&mut st, &[key(SPACE)]),
                ScreenCmd::Switch { .. }
            ));
            assert_eq!((st.perma_death, st.tutorial_off), (perma, true));
            assert!(st.run.is_some(), "the run goes on");
        }
    }

    #[test]
    fn esc_on_a_reopened_run_leaves_it_for_the_title() {
        let dir = crate::runs::testutil::Dir::new("resume-esc");
        let path = saved_run(&dir, true);
        let mut st = AppState::new();
        st.saves = Some(dir.0.clone());
        crate::runs::open(&mut st, &path).unwrap();
        let mut b = Broadcast::resume(&st);
        let ScreenCmd::Switch { to: title, .. } = b.update(&mut st, &[key(KeyCode::Esc)]) else {
            panic!("esc goes to the title: what was under the list was another run")
        };
        assert_eq!(st.run, None, "left: nothing on the title saves it");
        let text = buffer_text(&crate::screens::shots::draw_stack(
            &st,
            &[title.as_ref()],
            (120, 40),
        ));
        assert!(text.contains('█'), "the title's logo: {text}");
    }

    #[test]
    fn the_courier_plays_while_the_morning_after_is_under_way() {
        use crate::missions::{Run, RunState};
        let mut st = AppState::new();
        st.enter_city(small(5));
        let run = |state| Run {
            at: 0,
            since: 0,
            state,
            objectives: Vec::new(),
            ended: Default::default(),
            began: None,
        };
        let morning = st
            .book
            .missions
            .iter()
            .position(|m| m.id == "morning_after")
            .unwrap();
        let mut shouts = Vec::new();
        for state in [RunState::Waiting, RunState::Open, RunState::Done { at: 9 }] {
            st.missions.runs = st
                .book
                .missions
                .iter()
                .map(|_| run(RunState::Open))
                .collect();
            st.missions.runs[morning] = run(state);
            shouts.push(Broadcast::resume(&st).aired.shout);
        }
        assert!(shouts[1].contains("WHO'S THE COURIER"), "{shouts:?}");
        assert!(
            !shouts[0].contains("COURIER") && !shouts[2].contains("COURIER"),
            "not before it begins, nor once it is done: {shouts:?}"
        );
    }

    #[test]
    fn esc_goes_back_and_enter_does_nothing() {
        let (mut st, mut b) = new_game();
        b.full = true;
        assert!(matches!(
            b.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Stay
        ));
        assert!(matches!(
            b.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
        assert!(st.build.is_some() && st.city.is_none());
    }

    #[test]
    fn the_screen_draws_the_broadcast_the_toggles_and_the_banner() {
        let (mut st, mut b) = new_game();
        b.full = true;
        let text = draw(&b, &st, 120, 40);
        for want in [
            "ON AIR",
            "WAKE UP!",
            "[ ] DISABLE TUTORIAL",
            "[space] JACK IN",
            "PERMA DEATH IS ON",
            "last chance",
            "[p]turn it off",
            "HEIGHTS ─",
            "NET ─",
            "[space]JACK IN",
        ] {
            assert!(text.contains(want), "wants {want:?}: {text}");
        }
        b.update(&mut st, &[key(KeyCode::Char('t')), key(KeyCode::Char('p'))]);
        let text = draw(&b, &st, 120, 40);
        for want in [
            "[✓] DISABLE TUTORIAL",
            "PERMA DEATH IS OFF",
            "[p]turn it on",
        ] {
            assert!(text.contains(want), "wants {want:?}: {text}");
        }
    }

    #[test]
    fn every_broadcast_fits_both_sizes() {
        let st = AppState::new();
        for b in &st.broadcasts.0 {
            for seed in [2, 77, 1234] {
                let screen = Broadcast::with(&st, small(seed), b);
                for (w, h) in [(100, 30), (120, 40)] {
                    let area = Rect::new(0, 0, w, h);
                    let at = screen.places(area);
                    let end = at.column.y + at.lines.len() as u16;
                    assert!(end < at.toggles_y, "{} at {w}x{h}, seed {seed}", b.id);
                    let words = |t: &str| t.split_whitespace().collect::<Vec<_>>().join(" ");
                    let drawn: Vec<String> = at.lines.iter().map(|l| l.to_string()).collect();
                    let drawn = words(&drawn.join(" "));
                    for p in &screen.aired.paras {
                        assert!(drawn.contains(&words(p)), "{} keeps {p:?}", b.id);
                    }
                    assert_eq!(at.art.is_some(), w >= 120, "{} at {w}x{h}", b.id);
                }
            }
        }
    }

    #[test]
    fn the_art_stays_narrow_on_a_wide_terminal() {
        let (_, b) = new_game();
        let at = b.places(Rect::new(0, 0, 220, 60));
        let (art, _) = at.art.expect("room for the art");
        assert!(art.width <= ART_MAX_W);
        let left = at.column.x;
        let right = 220 - art.right();
        assert!(left.abs_diff(right) <= 1, "{left} and {right}");
    }

    #[test]
    fn a_long_handle_stays_inside_the_banner() {
        let (mut st, _) = new_game();
        st.build.as_mut().unwrap().handle = "ABCDEFGHIJKLMNOP".into();
        let mut b = Broadcast::new_game(&st, small(2));
        b.full = true;
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| b.draw(&st, f)).unwrap();
        let buf = t.backend().buffer();
        let banner = b.places(Rect::new(0, 0, 100, 30)).banner;
        for y in banner.y + 1..banner.bottom() - 1 {
            assert_eq!(buf[(banner.right() - 1, y)].symbol(), "║", "row {y}");
        }
        assert!(buffer_text(buf).contains("If ABCDEFGHIJKLMNOP flatlines"));
    }
}
