//! Character creation: the chooser and the six-step wizard over the build in `AppState`.
//!
//! One frame for every step: a step strip, the caption in the logo's block font with a
//! tagline, a left list, a right dossier, heavy rules, and the HUD's `[key]verb` footer.
//! The `Build` lives in `AppState`, so stepping back and forth never loses a choice.

mod attributes;
pub mod chooser;
mod name;
mod pick;
mod skills;
mod summary;

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::Rgb;
use crate::screens::generating::Generating;
use crate::screens::title::Title;
use crate::ui::chrome::{CYAN, CYAN_RGB, DIM, FAINT, MAGENTA, MAGENTA_RGB, RULE, solid};
use crate::ui::logo::{self, Logo};
use crossterm::event::{KeyCode, KeyEvent};
use game::{Attribute, Build, CategoryId, Character, Step};
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use std::collections::BTreeSet;
use unicode_width::UnicodeWidthStr;

/// Ticks a bought segment or row stays lit.
pub const FLASH_TICKS: u64 = 6;

/// How long a refusal stays in the footer.
pub const NOTICE_TICKS: u64 = 90;
/// Rows from which the caption is set in the block font.
pub const TALL: u16 = 36;

/// Each attribute's colour, the HUD's where they overlap.
pub(crate) fn attribute_rgb(s: Attribute) -> Rgb {
    match s {
        Attribute::Body => Rgb(120, 255, 120),
        Attribute::Reflex => CYAN_RGB,
        Attribute::Tech => Rgb(230, 180, 60),
        Attribute::Cool => MAGENTA_RGB,
        Attribute::Net => Rgb(110, 150, 255),
    }
}

pub(crate) fn attribute_colour(s: Attribute) -> Color {
    attribute_rgb(s).to_color()
}

/// The wizard: which step is up, where each list's cursor sits, and the footer notice.
pub struct Create {
    pub(crate) step: Step,
    /// Tick the current step opened: drives the slide-in and the typewriter.
    opened: u64,
    /// Tick the dossier last changed, so it types itself out again.
    changed: u64,
    cursor: [usize; 6],
    notice: Option<(String, u64)>,
    /// Unfolded categories on the SKILLS board.
    open: BTreeSet<CategoryId>,
    /// A board row lit after a buy, until the tick.
    flash: Option<(usize, u64)>,
    /// An attribute whose newest segment is lit after a buy, until the tick.
    attribute_flash: Option<(Attribute, u64)>,
}

/// The two panes a step draws into.
pub(crate) struct Panes {
    pub left: Rect,
    pub right: Rect,
}

impl Create {
    pub fn new(st: &AppState, step: Step, tick: u64) -> Self {
        let mut c = Create {
            step,
            opened: tick,
            changed: tick,
            cursor: [0; 6],
            notice: None,
            open: BTreeSet::new(),
            flash: None,
            attribute_flash: None,
        };
        if let Some(b) = &st.build {
            c.sync_cursor(st, b);
            c.open = b.categories.clone();
        }
        c
    }

    /// Points the ORIGIN and PROFESSION cursors at what the build already holds.
    fn sync_cursor(&mut self, st: &AppState, b: &Build) {
        if let Some(o) = &b.origin
            && let Some(i) = st.content.origins.iter().position(|x| &x.id == o)
        {
            self.cursor[Step::Origin.index()] = i;
        }
        if let Some(p) = &b.profession
            && let Some(i) = st.content.professions.iter().position(|x| &x.id == p)
        {
            self.cursor[Step::Profession.index()] = i;
        }
    }

    fn say(&mut self, msg: String, tick: u64) {
        self.notice = Some((msg, tick + NOTICE_TICKS));
    }

    /// Moves to a step with a glitch; the list steps pick what the cursor is on.
    fn go(&mut self, st: &mut AppState, step: Step) -> ScreenCmd {
        self.step = step;
        self.opened = st.tick;
        self.changed = st.tick;
        self.notice = None;
        self.pick_under_cursor(st);
        ScreenCmd::Flash(TransitionKind::Glitch)
    }

    /// On ORIGIN and PROFESSION the cursor is the pick.
    fn pick_under_cursor(&mut self, st: &mut AppState) {
        let i = self.cursor[self.step.index()];
        match self.step {
            Step::Origin => {
                if let (Some(o), Some(b)) = (st.content.origins.get(i), st.build.as_mut()) {
                    b.origin = Some(o.id.clone());
                }
            }
            Step::Profession => {
                if let (Some(p), Some(b)) = (st.content.professions.get(i), st.build.as_mut()) {
                    b.profession = Some(p.id.clone());
                }
            }
            _ => {}
        }
    }

    fn next(&mut self, st: &mut AppState) -> ScreenCmd {
        let ready = match st.build.as_ref() {
            Some(b) => b.ready(&st.content, self.step),
            None => {
                return ScreenCmd::Pop {
                    fx: TransitionKind::None,
                };
            }
        };
        if let Err(e) = ready {
            self.say(e.to_string(), st.tick);
            return ScreenCmd::Stay;
        }
        match self.step.next() {
            Some(s) => self.go(st, s),
            None => self.jack_in(st),
        }
    }

    fn back(&mut self, st: &mut AppState) -> ScreenCmd {
        match self.step.prev() {
            Some(s) => self.go(st, s),
            None => ScreenCmd::Pop {
                fx: TransitionKind::SlideUp,
            },
        }
    }

    /// Builds the character and heads for a fresh city, as NEW GAME used to.
    fn jack_in(&mut self, st: &mut AppState) -> ScreenCmd {
        let Some(b) = st.build.take() else {
            return ScreenCmd::Stay;
        };
        st.character = Character::from_build(&st.content, &b);
        match Generating::new(Title::fresh_params(st, st.tick), st.tick) {
            Ok(g) => ScreenCmd::Switch {
                to: Box::new(g),
                fx: TransitionKind::Glitch,
            },
            Err(e) => {
                st.warn(e.to_string());
                st.build = Some(b);
                ScreenCmd::Stay
            }
        }
    }

    /// Moves a list cursor with wrap; returns whether it moved.
    fn move_cursor(&mut self, n: usize, code: KeyCode) -> bool {
        if n == 0 {
            return false;
        }
        let i = self.step.index();
        let cur = self.cursor[i];
        let next = match code {
            KeyCode::Up | KeyCode::Char('k') => (cur + n - 1) % n,
            KeyCode::Down | KeyCode::Char('j') => (cur + 1) % n,
            _ => return false,
        };
        self.cursor[i] = next;
        true
    }

    fn step_key(&mut self, st: &mut AppState, code: KeyCode) {
        match self.step {
            Step::Name => name::key(st, code),
            Step::Origin => {
                if self.move_cursor(st.content.origins.len(), code) {
                    self.pick_under_cursor(st);
                    self.changed = st.tick;
                }
            }
            Step::Profession => {
                if self.move_cursor(st.content.professions.len(), code) {
                    self.pick_under_cursor(st);
                    self.changed = st.tick;
                }
            }
            Step::Attributes => attributes::key(self, st, code),
            Step::Skills => skills::key(self, st, code),
            Step::Summary => {}
        }
    }

    fn footer_keys(&self) -> &'static [(&'static str, &'static str)] {
        match self.step {
            Step::Name => &[("type", "name"), ("enter", "next"), ("esc", "back")],
            Step::Origin | Step::Profession => {
                &[("↑↓", "choose"), ("enter", "next"), ("esc", "back")]
            }
            Step::Attributes => &[
                ("↑↓", "attribute"),
                ("→", "buy"),
                ("←", "refund"),
                ("enter", "next"),
                ("esc", "back"),
            ],
            Step::Skills => &[
                ("↑↓", "row"),
                ("→", "unfold"),
                ("←", "fold"),
                ("space", "buy"),
                ("enter", "next"),
                ("esc", "back"),
            ],
            Step::Summary => &[("enter", "JACK IN"), ("esc", "back")],
        }
    }
}

/// `[key]verb` pairs in the HUD's footer style.
pub(crate) fn keys_line(keys: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (k, verb) in keys {
        spans.push(Span::styled("[", Style::default().fg(FAINT)));
        spans.push(Span::styled(
            k.to_string(),
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled("]", Style::default().fg(FAINT)));
        spans.push(Span::styled(format!("{verb}  "), Style::default().fg(DIM)));
    }
    Line::from(spans)
}

/// The step strip: the current step in a `▌N NAME▐` handle, the others dim.
fn strip(current: Step) -> Line<'static> {
    let mut spans = Vec::new();
    for (i, s) in Step::ALL.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" ▸ ", Style::default().fg(FAINT)));
        }
        let label = format!("{} {}", i + 1, s.label());
        if *s == current {
            spans.push(Span::styled("▌", Style::default().fg(CYAN)));
            spans.push(Span::styled(
                label,
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled("▐", Style::default().fg(CYAN)));
        } else {
            spans.push(Span::styled(label, Style::default().fg(DIM)));
        }
    }
    Line::from(spans)
}

/// The build so far, for the strip's right end.
fn so_far(st: &AppState, b: &Build) -> String {
    let mut parts: Vec<String> = Vec::new();
    let handle = b.handle.trim();
    if !handle.is_empty() {
        parts.push(handle.to_string());
    }
    if let Some(o) = b.origin.as_ref().and_then(|id| st.content.origin(id)) {
        parts.push(o.name.to_lowercase());
    }
    if let Some(p) = b
        .profession
        .as_ref()
        .and_then(|id| st.content.profession(id))
    {
        parts.push(p.name.to_lowercase());
    }
    if parts.is_empty() {
        match &b.template {
            Some(t) => t.to_string(),
            None => "custom build".to_string(),
        }
    } else {
        parts.join(" · ")
    }
}

/// Writes a line right-aligned against `right`.
pub(crate) fn put_right(buf: &mut Buffer, right: u16, y: u16, line: Line<'static>) {
    let w = line.width() as u16;
    let x = right.saturating_sub(w);
    buf.set_line(x, y, &line, w);
}

/// A heavy rule across the width.
fn rule(buf: &mut Buffer, area: Rect, y: u16) {
    let s: String = "━".repeat(area.width as usize);
    buf.set_string(area.x, y, s, Style::default().fg(RULE));
}

/// Greedy word wrap.
pub(crate) fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.width() + 1 + word.width() > width {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

impl Screen for Create {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        if st.build.is_none() {
            return ScreenCmd::Pop {
                fx: TransitionKind::None,
            };
        }
        for k in keys {
            match k.code {
                KeyCode::Esc => return self.back(st),
                KeyCode::Enter => return self.next(st),
                code => self.step_key(st, code),
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let buf = f.buffer_mut();
        solid(buf, area);
        let Some(b) = &st.build else { return };
        let tick = st.tick;
        let tall = area.height >= TALL;

        // Strip and the build so far; a long handle wins over the picks when they collide.
        let strip_line = strip(self.step);
        let room = area.width.saturating_sub(4 + strip_line.width() as u16 + 1) as usize;
        let mut summary = so_far(st, b);
        if summary.width() > room {
            summary = b.handle.trim().to_string();
        }
        buf.set_line(
            area.x + 2,
            area.y,
            &strip_line,
            area.width.saturating_sub(2),
        );
        put_right(
            buf,
            area.right().saturating_sub(2),
            area.y,
            Line::from(Span::styled(summary, Style::default().fg(DIM))),
        );
        rule(buf, area, area.y + 1);

        // Caption and tagline: the step's name, or on the sheet the character's.
        let handle = b.handle.trim().to_uppercase();
        let caption: &str = if self.step == Step::Summary && !handle.is_empty() {
            &handle
        } else {
            self.step.label()
        };
        let w = logo::width(caption);
        let body_y = if tall && logo::covers(caption) && w + 4 <= area.width {
            Logo {
                text: caption,
                tick,
                accent: CYAN_RGB,
                steady: true,
            }
            .render(
                Rect::new(area.x + 2, area.y + 2, w.min(area.width), logo::ROWS),
                buf,
            );
            buf.set_string(
                area.x + 2 + w + 4,
                area.y + 4,
                self.step.tagline(),
                Style::default().fg(DIM),
            );
            area.y + 2 + logo::ROWS + 1
        } else {
            buf.set_string(
                area.x + 2,
                area.y + 2,
                caption,
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            );
            buf.set_string(
                area.x + 2 + caption.width() as u16 + 3,
                area.y + 2,
                self.step.tagline(),
                Style::default().fg(DIM),
            );
            area.y + 4
        };

        // Panes.
        let bottom_rule = area.bottom().saturating_sub(3);
        let body = Rect::new(
            area.x + 2,
            body_y,
            area.width.saturating_sub(4),
            bottom_rule.saturating_sub(body_y),
        );
        // The lists want room for a tagline; the name wants room for the sheet, and the
        // skills' cards room for their art.
        let split = body.width
            * match self.step {
                Step::Name => 40,
                Step::Skills => 44,
                _ => 58,
            }
            / 100;
        let panes = Panes {
            left: Rect::new(body.x, body.y, split.saturating_sub(2), body.height),
            right: Rect::new(
                body.x + split,
                body.y,
                body.width.saturating_sub(split),
                body.height,
            ),
        };
        match self.step {
            Step::Name => name::draw(st, b, &panes, tick, buf),
            Step::Origin => pick::draw(
                &pick::origins(st),
                self.cursor[Step::Origin.index()],
                self.opened,
                self.changed,
                tick,
                &panes,
                buf,
            ),
            Step::Profession => pick::draw(
                &pick::professions(st),
                self.cursor[Step::Profession.index()],
                self.opened,
                self.changed,
                tick,
                &panes,
                buf,
            ),
            Step::Attributes => attributes::draw(self, st, b, &panes, tick, buf),
            Step::Skills => skills::draw(self, st, b, &panes, tick, buf),
            Step::Summary => summary::draw(st, b, body, tick, buf),
        }

        // Footer: keys or the live notice, and the step counter.
        rule(buf, area, bottom_rule);
        let fy = area.bottom().saturating_sub(2);
        match &self.notice {
            Some((msg, until)) if tick < *until => {
                buf.set_string(
                    area.x + 2,
                    fy,
                    msg,
                    Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD),
                );
            }
            _ => {
                buf.set_line(
                    area.x + 2,
                    fy,
                    &keys_line(self.footer_keys()),
                    area.width.saturating_sub(2),
                );
            }
        }
        put_right(
            buf,
            area.right().saturating_sub(2),
            fy,
            Line::from(Span::styled(
                format!("step {} of {}", self.step.index() + 1, Step::ALL.len()),
                Style::default().fg(DIM),
            )),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::testutil::{buffer_text, key};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn draw(s: &dyn Screen, st: &AppState, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| s.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    /// A custom build, landing on ORIGIN.
    fn custom(st: &mut AppState) -> Create {
        let mut ch = chooser::Chooser::new(st, 0);
        ch.update(st, &[key(KeyCode::Down), key(KeyCode::Down)]);
        ch.choose(st)
    }

    /// A custom build with origin and profession picked, opened on `step`.
    fn picked(st: &mut AppState, step: Step) -> Create {
        custom(st);
        let b = st.build.as_mut().unwrap();
        b.origin = Some(game::OriginId::new("creche"));
        b.profession = Some(game::ProfessionId::new("netrunner"));
        Create::new(st, step, 0)
    }

    #[test]
    fn steps_run_origin_to_summary_with_the_name_last() {
        assert_eq!(
            Step::ALL,
            [
                Step::Origin,
                Step::Profession,
                Step::Attributes,
                Step::Skills,
                Step::Name,
                Step::Summary
            ]
        );
    }

    #[test]
    fn chooser_offers_templates_and_custom() {
        let mut st = AppState::new();
        let mut ch = chooser::Chooser::new(&st, 0);
        st.tick = 60;
        let text = draw(&ch, &st, 120, 40);
        assert!(
            text.contains("VEX") && text.contains("KESSLER") && text.contains("CUSTOM BUILD"),
            "{text}"
        );
        assert!(text.contains("crèche kid behind the ICE"), "{text}");
        assert!(matches!(
            ch.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
        assert!(matches!(
            ch.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Push {
                fx: TransitionKind::SlideUp,
                ..
            }
        ));
        assert_eq!(st.build.as_ref().map(|b| b.handle.as_str()), Some("VEX"));
        let c = ch.choose(&mut st);
        assert_eq!(c.step, Step::Summary, "a template lands on the sheet");
        ch.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Down)]);
        let c = ch.choose(&mut st);
        assert_eq!(
            c.step,
            Step::Origin,
            "a custom build starts where you were made"
        );
        assert_eq!(st.build.as_ref().unwrap().handle, "");
    }

    #[test]
    fn name_step_refuses_blank_names_and_shows_the_sheet() {
        let mut st = AppState::new();
        let mut c = picked(&mut st, Step::Name);
        st.tick = 100;
        assert!(matches!(
            c.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Stay
        ));
        let text = draw(&c, &st, 120, 40);
        assert!(text.contains("no name on the sheet"), "{text}");
        assert!(
            text.contains("▮ type a name"),
            "the cursor sits before the placeholder: {text}"
        );
        assert!(
            text.contains("THE SHEET")
                && text.contains("ORIGIN")
                && text.contains("CRÈCHE")
                && text.contains("KIT")
                && text.contains("ATTRIBUTES"),
            "the sheet so far sits beside the name: {text}"
        );
        assert_eq!(c.step, Step::Name);
        c.update(
            &mut st,
            &[
                key(KeyCode::Char(' ')),
                key(KeyCode::Char(' ')),
                key(KeyCode::Enter),
            ],
        );
        assert_eq!(c.step, Step::Name, "spaces are no name");
        c.update(&mut st, &[key(KeyCode::Backspace), key(KeyCode::Backspace)]);
        for ch in "D'Shawn".chars() {
            c.update(&mut st, &[key(KeyCode::Char(ch))]);
        }
        assert_eq!(st.build.as_ref().unwrap().handle, "D'Shawn");
        let text = draw(&c, &st, 120, 40);
        assert!(text.contains("D'Shawn"), "{text}");
        assert!(matches!(
            c.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Flash(_)
        ));
        assert_eq!(c.step, Step::Summary);
        assert!(matches!(
            c.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Flash(_)
        ));
        assert_eq!(c.step, Step::Name);
        c.update(&mut st, &[key(KeyCode::Esc)]);
        assert_eq!(c.step, Step::Skills, "esc on NAME goes back to SKILLS");
    }

    #[test]
    fn origin_step_draws_the_frame_list_and_dossier() {
        let mut st = AppState::new();
        let mut c = custom(&mut st);
        assert_eq!(c.step, Step::Origin);
        st.tick = 200;
        let text = draw(&c, &st, 120, 40);
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines[0].contains(
                "▌1 ORIGIN▐ ▸ 2 PROFESSION ▸ 3 ATTRIBUTES ▸ 4 SKILLS ▸ 5 NAME ▸ 6 SUMMARY"
            ),
            "{}",
            lines[0]
        );
        assert!(
            lines[0].contains("slab rat"),
            "the cursor's pick shows in the strip from the first frame: {}",
            lines[0]
        );
        assert!(lines[1].starts_with('━'), "{}", lines[1]);
        assert!(
            lines[2..7]
                .iter()
                .any(|l| l.contains('▀') || l.contains('▄') || l.contains('█')),
            "block caption at 40 rows: {text}"
        );
        assert!(
            text.contains("where you were made before the city got its hands on you"),
            "{text}"
        );
        for name in ["SLAB RAT", "CRÈCHE", "STOWAWAY", "VAT-GROWN"] {
            assert!(text.contains(name), "{name}: {text}");
        }
        let slab = lines
            .iter()
            .find(|l| l.contains("SLAB RAT") && l.contains("▸"))
            .expect("the selected row");
        assert!(
            slab.contains("BODY COOL") && slab.contains("TECH NET"),
            "cheap and dear at a glance: {slab}"
        );
        assert!(
            text.contains("born between the slabs, knew the alleys before the alphabet"),
            "the whole tagline on its own row: {text}"
        );
        assert!(
            text.contains("DOSSIER")
                && text.contains("AFFINITY")
                && text.contains("cheap")
                && text.contains("dear"),
            "{text}"
        );
        assert!(text.contains("step 1 of 6"), "{text}");
        for l in text.lines().filter(|l| l.contains("│")) {
            let after = l.rsplit('│').next().unwrap_or("");
            assert!(
                after.trim().is_empty(),
                "nothing spills past the dossier: {l:?}"
            );
        }
        assert_eq!(
            st.build
                .as_ref()
                .unwrap()
                .origin
                .as_ref()
                .map(|o| o.0.as_str()),
            Some("slab_rat"),
            "the cursor picks"
        );
        c.update(&mut st, &[key(KeyCode::Down)]);
        assert_eq!(
            st.build
                .as_ref()
                .unwrap()
                .origin
                .as_ref()
                .map(|o| o.0.as_str()),
            Some("creche")
        );
        st.tick = 300;
        let text = draw(&c, &st, 120, 40);
        assert!(text.contains("Filtered air"), "{text}");
        let short = draw(&c, &st, 100, 30);
        let lines: Vec<&str> = short.lines().collect();
        assert!(
            !lines[2..6]
                .iter()
                .any(|l| l.contains('▀') || l.contains('▄')),
            "no block font at 30 rows: {short}"
        );
        assert!(
            short.contains("ORIGIN") && short.contains("DOSSIER"),
            "{short}"
        );
        assert!(
            matches!(
                c.update(&mut st, &[key(KeyCode::Esc)]),
                ScreenCmd::Pop { .. }
            ),
            "esc on the first step leaves the wizard"
        );
    }

    #[test]
    fn summary_wears_the_name_and_jacks_in_a_template() {
        let mut st = AppState::new();
        let ch = chooser::Chooser::new(&st, 0);
        let mut c = ch.choose(&mut st);
        assert_eq!(c.step, Step::Summary);
        st.tick = 100;
        let text = draw(&c, &st, 120, 40);
        let lines: Vec<&str> = text.lines().collect();
        assert!(
            lines[2..7].iter().any(|l| l.contains('█')) && lines[4].contains("the sheet"),
            "the caption is the handle in block font with the tagline: {text}"
        );
        assert!(
            text.contains("JACK IN")
                && text.contains("CRÈCHE")
                && text.contains("NETRUNNER")
                && text.contains("step 6 of 6"),
            "{text}"
        );
        assert!(
            text.contains("BODY") && text.contains("Pawnshop deck"),
            "{text}"
        );
        assert!(
            text.contains("◈ Signal ghost") && text.contains("◈ Street sense"),
            "what the categories grant is on the sheet: {text}"
        );
        let cmd = c.update(&mut st, &[key(KeyCode::Enter)]);
        assert!(
            matches!(
                cmd,
                ScreenCmd::Switch {
                    fx: TransitionKind::Glitch,
                    ..
                }
            ),
            "jack in generates"
        );
        assert_eq!(st.character.handle, "VEX");
        assert!(st.build.is_none());
        assert!(st.character.has_category(&game::CategoryId::new("wrench")));
    }

    #[test]
    fn summary_refuses_an_overspent_build() {
        let mut st = AppState::new();
        let ch = chooser::Chooser::new(&st, 0);
        let mut c = ch.choose(&mut st);
        st.build.as_mut().unwrap().origin = Some(game::OriginId::new("stowaway"));
        st.tick = 100;
        assert!(matches!(
            c.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Stay
        ));
        let text = draw(&c, &st, 120, 40);
        assert!(text.contains("over by"), "{text}");
        assert!(st.build.is_some());
    }

    #[test]
    fn attributes_step_buys_refunds_and_shows_costs() {
        let mut st = AppState::new();
        let ch = chooser::Chooser::new(&st, 0);
        let mut c = ch.choose(&mut st);
        for _ in 0..3 {
            c.update(&mut st, &[key(KeyCode::Esc)]);
        }
        assert_eq!(c.step, Step::Attributes);
        st.tick = 100;
        let text = draw(&c, &st, 120, 40);
        assert!(text.contains("POINTS") && text.contains("0 left"), "{text}");
        assert!(
            text.contains("BODY") && text.contains("next 3 pts") && text.contains("dear"),
            "{text}"
        );
        assert!(text.contains("TECH") && text.contains("cheap"), "{text}");
        c.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Left)]);
        assert_eq!(st.build.as_ref().unwrap().attributes.reflex, 5);
        let text = draw(&c, &st, 120, 40);
        assert!(text.contains("2 left"), "{text}");
        c.update(&mut st, &[key(KeyCode::Right)]);
        assert_eq!(st.build.as_ref().unwrap().attributes.reflex, 6);
        c.update(&mut st, &[key(KeyCode::Right)]);
        assert_eq!(st.build.as_ref().unwrap().attributes.reflex, 6);
        let text = draw(&c, &st, 120, 40);
        assert!(text.contains("no points left"), "{text}");
        st.build.as_mut().unwrap().origin = Some(game::OriginId::new("stowaway"));
        let text = draw(&c, &st, 120, 40);
        assert!(text.contains("over by"), "{text}");
        assert!(matches!(
            c.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Stay
        ));
        assert_eq!(c.step, Step::Attributes);
    }

    /// VEX's build on the SKILLS step with the cursor on `name`, the card typed out.
    fn on_skills_row(st: &mut AppState, name: &str) -> Create {
        let ch = chooser::Chooser::new(st, 0);
        let mut c = ch.choose(st);
        for _ in 0..2 {
            c.update(st, &[key(KeyCode::Esc)]);
        }
        let b = st.build.as_ref().unwrap();
        let list = skills::rows(&c, st, b);
        c.cursor[Step::Skills.index()] = list
            .iter()
            .position(|r| r.line.to_string().contains(name))
            .unwrap();
        st.tick += 100;
        c
    }

    fn cells(s: &dyn Screen, st: &AppState, w: u16, h: u16) -> ratatui::buffer::Buffer {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| s.draw(st, f)).unwrap();
        t.backend().buffer().clone()
    }

    #[test]
    fn skills_step_cards_lead_with_their_art() {
        let mut st = AppState::new();
        let c = on_skills_row(&mut st, "Hacking");
        let text = draw(&c, &st, 100, 30);
        for want in [
            "HACKING",
            "[learned]",
            ":: free ::",
            ":: NET 4 ::",
            "[REQUIRED SKILLS]",
            "Getting into systems",
        ] {
            assert!(text.contains(want), "{want}: {text}");
        }
        // NETRUNNER's card is the tallest; its line still shows at the minimum size.
        let c = on_skills_row(&mut st, "NETRUNNER");
        let text = draw(&c, &st, 100, 30);
        for want in [
            "RUNNER_",
            "[unlocked]",
            "[SKILLS]",
            "the Net is a place and you live",
        ] {
            assert!(text.contains(want), "{want}: {text}");
        }
        let c = on_skills_row(&mut st, "IRON");
        assert!(draw(&c, &st, 100, 30).contains("[locked]"));
        let brass = ratatui::style::Color::Rgb(184, 146, 60);
        let b = cells(&c, &st, 100, 30);
        assert!(
            !b.content.iter().any(|x| x.fg == brass || x.bg == brass),
            "not in the build, the rounds are grey"
        );
    }

    #[test]
    fn skills_step_cards_replay_when_the_cursor_moves_or_something_is_bought() {
        let mut st = AppState::new();
        let mut c = on_skills_row(&mut st, "Hacking");
        c.changed = 0;
        c.update(&mut st, &[key(KeyCode::Down)]);
        assert_eq!(c.changed, st.tick, "a new row, a new card");
        let mut c = on_skills_row(&mut st, "ICE breaker");
        c.changed = 0;
        c.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(c.changed, st.tick, "refunded, the card replays");
    }

    #[test]
    fn skills_step_folds_buys_and_refunds() {
        let mut st = AppState::new();
        let ch = chooser::Chooser::new(&st, 0);
        let mut c = ch.choose(&mut st);
        for _ in 0..2 {
            c.update(&mut st, &[key(KeyCode::Esc)]);
        }
        assert_eq!(c.step, Step::Skills);
        st.tick = 100;
        let text = draw(&c, &st, 120, 40);
        assert!(text.contains("SP") && text.contains("0 left"), "{text}");
        assert!(
            text.contains("◆ NETRUNNER")
                && text.contains("├─ ◆ Hacking")
                && text.contains("◇ IRON"),
            "{text}"
        );
        assert!(
            !text.contains("├─ ◇ Handguns"),
            "unowned categories start folded: {text}"
        );
        c.update(&mut st, &[key(KeyCode::Down); 8]);
        c.update(&mut st, &[key(KeyCode::Right)]);
        let text = draw(&c, &st, 120, 40);
        assert!(text.contains("├─ ◇ Handguns"), "unfolded: {text}");
        c.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(
            !st.build
                .as_ref()
                .unwrap()
                .categories
                .contains(&game::CategoryId::new("iron"))
        );
        assert!(draw(&c, &st, 120, 40).contains("no SP left"));
        c.update(&mut st, &[key(KeyCode::Up); 8]);
        c.update(&mut st, &[key(KeyCode::Char(' '))]);
        let b = st.build.as_ref().unwrap();
        assert!(
            !b.categories.contains(&game::CategoryId::new("netrunner"))
                && !b.skills.contains(&game::SkillId::new("ice_breaker")),
            "refunding the category drops its skills"
        );
        assert_eq!(b.sp_left(&st.content), 300);
        c.update(&mut st, &[key(KeyCode::Char(' '))]);
        let b = st.build.as_ref().unwrap();
        assert!(b.categories.contains(&game::CategoryId::new("netrunner")));
        assert_eq!(b.sp_left(&st.content), 100);
        c.update(&mut st, &[key(KeyCode::Left)]);
        st.tick += 60;
        let text = draw(&c, &st, 120, 40);
        assert!(
            !text.contains("├─ ◆ Hacking") && text.contains("◆ Hacking · ◇ ICE breaker"),
            "folded in the list, still on the card: {text}"
        );
    }

    #[test]
    fn letters_the_block_font_lacks_fall_back_to_a_plain_line() {
        let mut st = AppState::new();
        let mut c = picked(&mut st, Step::Name);
        for ch in "Zoж".chars() {
            c.update(&mut st, &[key(KeyCode::Char(ch))]);
        }
        st.tick = 100;
        let text = draw(&c, &st, 120, 40);
        assert!(text.contains("ZOЖ"), "the sheet keeps every letter: {text}");
        let s = Create::new(&st, Step::Summary, 0);
        let text = draw(&s, &st, 120, 40);
        assert!(
            text.contains("ZOЖ"),
            "and so does the summary caption: {text}"
        );
    }

    #[test]
    fn strip_yields_to_a_long_handle_at_the_minimum_width() {
        let mut st = AppState::new();
        custom(&mut st);
        let b = st.build.as_mut().unwrap();
        b.handle = "Sixteen-char-nam".into();
        b.origin = Some(game::OriginId::new("creche"));
        b.profession = Some(game::ProfessionId::new("netrunner"));
        let c = Create::new(&st, Step::Attributes, 0);
        st.tick = 100;
        let text = draw(&c, &st, 100, 30);
        let first = text.lines().next().unwrap_or("");
        assert!(
            first.contains("6 SUMMARY") && first.contains("Sixteen-char-nam"),
            "{first:?}"
        );
    }

    #[test]
    fn summary_counts_the_talent_point_among_the_unspent() {
        let mut st = AppState::new();
        let ch = chooser::Chooser::new(&st, 0);
        let c = ch.choose(&mut st);
        st.tick = 100;
        let text = draw(&c, &st, 120, 40);
        assert!(
            text.contains("unspent") && text.contains("1 talent point"),
            "{text}"
        );
    }
}
