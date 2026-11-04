//! The configure screen: a form over `GenParams` with a live preview card.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::Rgb;
use crate::screens::title::Title;
use crate::ui::form::{Field, Form, FormView};
use crate::ui::panel::boxed;
use citygen::rng::hash64;
use citygen::{GenParams, SizePreset, WaterMode};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

const BTN_RANDOM: usize = 8;
const BTN_GENERATE: usize = 9;

pub struct Configure {
    pub form: Form,
    base: GenParams,
    opened: u64,
    error: Option<String>,
}

fn size_options(base: SizePreset) -> (Vec<String>, usize) {
    let mut opts = vec![
        "small".to_string(),
        "medium".to_string(),
        "large".to_string(),
    ];
    let idx = match base {
        SizePreset::Small => 0,
        SizePreset::Medium => 1,
        SizePreset::Large => 2,
        SizePreset::Custom { w, h } => {
            opts.push(format!("{w}x{h}"));
            3
        }
    };
    (opts, idx)
}

impl Configure {
    pub fn new(params: GenParams, tick: u64) -> Self {
        let (size_opts, size_idx) = size_options(params.size);
        let water_idx = match params.water {
            WaterMode::None => 0,
            WaterMode::Bay => 1,
            WaterMode::River => 2,
        };
        let form = Form {
            fields: vec![
                Field::Text {
                    label: "Seed".into(),
                    value: if params.seed == 0 {
                        String::new()
                    } else {
                        params.seed.to_string()
                    },
                    max: 20,
                    numeric: true,
                },
                Field::Choice {
                    label: "Size".into(),
                    options: size_opts,
                    idx: size_idx,
                },
                Field::Number {
                    label: "Districts".into(),
                    value: params.districts as i64,
                    min: 4,
                    max: 12,
                    step: 1,
                },
                Field::Slider {
                    label: "Density".into(),
                    value: params.density,
                    min: 0.0,
                    max: 1.0,
                    step: 0.05,
                },
                Field::Slider {
                    label: "Wealth skew".into(),
                    value: params.wealth_skew,
                    min: -1.0,
                    max: 1.0,
                    step: 0.1,
                },
                Field::Slider {
                    label: "Decay".into(),
                    value: params.decay,
                    min: 0.0,
                    max: 1.0,
                    step: 0.05,
                },
                Field::Choice {
                    label: "Water".into(),
                    options: vec!["none".into(), "bay".into(), "river".into()],
                    idx: water_idx,
                },
                Field::Choice {
                    label: "Flavor".into(),
                    options: vec!["default".into()],
                    idx: 0,
                },
                Field::Button {
                    label: "RANDOM SEED".into(),
                },
                Field::Button {
                    label: "GENERATE".into(),
                },
            ],
            selected: 0,
        };
        Configure {
            form,
            base: params,
            opened: tick,
            error: None,
        }
    }

    /// The parameters the form currently describes. An empty seed becomes a fresh one.
    pub fn params(&self) -> GenParams {
        let seed = self
            .form
            .text("Seed")
            .and_then(|t| t.parse::<u64>().ok())
            .unwrap_or_else(|| hash64(self.opened, 0xC0FFEE));
        let size = match self.form.choice("Size") {
            Some("small") => SizePreset::Small,
            Some("medium") => SizePreset::Medium,
            Some("large") => SizePreset::Large,
            _ => self.base.size,
        };
        let water = match self.form.choice("Water") {
            Some("none") => WaterMode::None,
            Some("river") => WaterMode::River,
            _ => WaterMode::Bay,
        };
        GenParams {
            seed,
            size,
            districts: self.form.number("Districts").unwrap_or(7) as u8,
            density: self.form.slider("Density").unwrap_or(0.6),
            wealth_skew: self.form.slider("Wealth skew").unwrap_or(0.0),
            decay: self.form.slider("Decay").unwrap_or(0.4),
            water,
            flavor: self.form.choice("Flavor").unwrap_or("default").to_string(),
        }
    }
}

impl Screen for Configure {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        for k in keys {
            if k.code == KeyCode::Esc {
                // The run it was opened over is left, as QUIT TO TITLE leaves it:
                // nothing on the title (the window closing) saves it again.
                st.run = None;
                return ScreenCmd::Switch {
                    to: Box::new(Title::new(st, st.tick)),
                    fx: TransitionKind::SlideUp,
                };
            }
            match self.form.handle(k.code) {
                Some(BTN_RANDOM) => {
                    let seed = hash64(st.tick, 0xC0FFEE) % 1_000_000_007;
                    self.form.set_text("Seed", seed.to_string());
                    self.error = None;
                }
                Some(BTN_GENERATE) => {
                    let params = self.params();
                    if let Err(e) = params.validate() {
                        self.error = Some(e.to_string());
                        st.warn(e.to_string());
                        continue;
                    }
                    match crate::screens::build_city(params) {
                        Ok(city) => return crate::screens::enter_built_city(st, city),
                        Err(e) => {
                            self.error = Some(e.to_string());
                            st.warn(e.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(56), Constraint::Min(20)])
            .split(area);
        let left = boxed("CONFIGURE");
        let inner = left.inner(cols[0]);
        f.render_widget(left, cols[0]);
        let form_area = Rect::new(
            inner.x + 1,
            inner.y + 1,
            inner.width.saturating_sub(2),
            inner.height.saturating_sub(2),
        );
        FormView {
            form: &self.form,
            tick: st.tick,
            accent: Rgb(0, 255, 230),
        }
        .render(form_area, f.buffer_mut());

        let right = boxed("PREVIEW");
        let pinner = right.inner(cols[1]);
        f.render_widget(right, cols[1]);
        let params = self.params();
        let size = params.size.size();
        let est = (size.area() as f32 / 150.0 * (params.density / 0.6)).round() as u32;
        let dim = Style::default().fg(Color::Rgb(160, 170, 185));
        let txt = Style::default().fg(Color::Rgb(205, 212, 222));
        let mut lines = vec![
            Line::from(vec![
                Span::styled(" map       ", dim),
                Span::styled(format!("{}x{} tiles", size.w, size.h), txt),
            ]),
            Line::from(vec![
                Span::styled(" districts ", dim),
                Span::styled(format!("up to {}", params.districts), txt),
            ]),
            Line::from(vec![
                Span::styled(" buildings ", dim),
                Span::styled(format!("≈ {est}"), txt),
            ]),
            Line::from(vec![
                Span::styled(" water     ", dim),
                Span::styled(params.water.label(), txt),
            ]),
            Line::from(vec![
                Span::styled(" seed      ", dim),
                Span::styled(params.seed.to_string(), txt),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                " ↑↓ field · ←→ adjust · enter · esc back",
                dim,
            )),
        ];
        if let Some(e) = &self.error {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!(" {e}"),
                Style::default().fg(Color::Rgb(255, 80, 100)),
            )));
        }
        f.render_widget(
            Paragraph::new(lines).wrap(ratatui::widgets::Wrap { trim: false }),
            pinner,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, Screen, ScreenCmd};
    use crate::screens::testutil::{buffer_text, key};
    use citygen::{GenParams, SizePreset};
    use crossterm::event::KeyCode;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn configure_generate_switches_screens() {
        let mut st = AppState::new();
        let mut cfg = Configure::new(
            GenParams {
                size: SizePreset::Custom { w: 80, h: 40 },
                districts: 4,
                ..Default::default()
            },
            0,
        );
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| cfg.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(
            text.contains("GENERATE") && text.contains("Districts") && text.contains("80x40"),
            "{text}"
        );
        for _ in 0..9 {
            cfg.update(&mut st, &[key(KeyCode::Down)]);
        }
        assert!(matches!(
            cfg.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Switch { .. }
        ));
        assert_eq!(st.params.districts, 4);
    }

    #[test]
    fn configure_rejects_bad_params_and_random_seed_works() {
        let mut st = AppState::new();
        let mut cfg = Configure::new(GenParams::default(), 0);
        // Districts up to 12 on a small map is fine; make it custom-small to force rejection.
        cfg.form.set_text("Seed", "7".into());
        for _ in 0..8 {
            cfg.update(&mut st, &[key(KeyCode::Down)]);
        }
        assert!(matches!(
            cfg.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Stay
        )); // RANDOM SEED
        assert_ne!(cfg.form.text("Seed"), Some("7"));
        assert!(matches!(
            cfg.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Switch { .. }
        ));
        assert_eq!(cfg.params().districts, 7);
    }

    /// Dev: opened over a run, esc goes to the title and leaves the run, as QUIT TO
    /// TITLE does: the window closing on the title saves nothing.
    #[test]
    fn esc_to_the_title_leaves_the_run() {
        let dir = crate::runs::testutil::Dir::new("configure");
        let (mut st, _) = crate::runs::testutil::saving(&dir, crate::runs::testutil::small_city());
        assert!(st.run.is_some());
        let mut cfg = Configure::new(st.params.clone(), st.tick);
        assert!(matches!(
            cfg.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Switch { .. }
        ));
        assert_eq!(st.run, None);
        assert!(!crate::runs::when::save_now(&mut st));
        assert!(crate::runs::list(&dir.0).is_empty());
    }
}
