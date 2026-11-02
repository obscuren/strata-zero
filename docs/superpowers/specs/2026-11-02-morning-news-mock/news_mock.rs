//! The broadcast mock Jeffrey agreed (2026-11-02): a throwaway shot test, appended to
//! `crates/strata-zero/src/screens/shots.rs` and run with
//! `SHOT_DIR=<dir> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored news_mock`.
//! It draws the broadcast screen over the map at 07:14; the text, the still and the
//! SOUGHT lines are hard-coded here and come from the story and the player in the
//! game (spec §2, §4, §7). Kept for reference, not compiled.


const NEWS_P: [&str; 2] = [
    "A noodle cook was found dead late last night outside his stall at Old Kowloon Row Station. He was Marlo, 41. Sources at the precinct say he was a paid informer.",
    "Officers want to speak to a courier the station's cameras caught leaving soon after, carrying a package. Anyone who knows them is asked to call it in.",
];
const NEWS_TICKER: &str = " ▸ MARLO, 41, NAMED AS POLICE INFORMER   ▸ COURIER SOUGHT IN KOWLOON ROW   ▸ LIFT 9 OUT OF SERVICE   ▸ RAIN BY NOON   ▸ ARCOLOGY RENTS UP 4%";

/// The camera still: 18 pixels wide, 18 high, levels 0..=6, 9 for the red bracket.
fn news_still() -> [[u8; 18]; 18] {
    let mut p: [[u8; 18]; 18] = [
        [1, 5, 5, 5, 5, 5, 5, 1, 1, 1, 1, 1, 1, 1, 6, 6, 6, 1],
        [1, 4, 4, 4, 4, 4, 4, 1, 1, 0, 1, 1, 0, 1, 1, 2, 1, 1],
        [3, 2, 3, 2, 3, 2, 3, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1],
        [2, 4, 4, 4, 4, 4, 4, 2, 1, 1, 0, 1, 1, 1, 1, 2, 1, 1],
        [2, 4, 5, 5, 5, 5, 4, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1],
        [2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1],
        [3, 3, 3, 3, 3, 3, 3, 3, 2, 2, 3, 3, 0, 3, 4, 4, 3, 3],
        [3, 3, 3, 3, 3, 3, 3, 3, 2, 2, 3, 0, 0, 0, 4, 4, 4, 3],
        [3, 0, 0, 3, 0, 0, 3, 3, 2, 2, 3, 0, 0, 6, 6, 4, 4, 3],
        [3, 0, 0, 0, 0, 0, 3, 3, 2, 2, 3, 0, 3, 0, 4, 4, 3, 3],
        [3, 3, 1, 1, 1, 3, 3, 3, 2, 2, 3, 0, 3, 0, 3, 3, 3, 3],
        [2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 3, 3, 3, 2],
        [2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2],
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 1, 1, 1, 1, 1, 1, 1, 1],
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 1, 1, 1, 1, 1, 1, 1, 1],
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 1, 1, 1, 1, 1, 1, 1, 1],
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 1, 1, 1, 1, 1, 1, 1, 1],
    ];
    // grain
    let mut seed: u32 = 0x5eed_1234;
    for row in p.iter_mut() {
        for v in row.iter_mut() {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            let r = (seed >> 16) % 100;
            if r < 5 && *v < 6 {
                *v += 1;
            } else if r > 95 && *v > 0 {
                *v -= 1;
            }
        }
    }
    // the bracket the newsroom drew round them
    for (x, y) in [
        (10, 5), (11, 5), (10, 6), (15, 5), (14, 5), (15, 6),
        (10, 12), (11, 12), (10, 11), (15, 12), (14, 12), (15, 11),
    ] {
        p[y][x] = 9;
    }
    p
}

fn news_colour(v: u8, row: usize) -> Color {
    const GREENS: [(u8, u8, u8); 7] = [
        (4, 8, 5),
        (11, 23, 16),
        (21, 41, 27),
        (36, 67, 44),
        (63, 112, 73),
        (106, 168, 119),
        (182, 240, 194),
    ];
    if v == 9 {
        return Color::Rgb(255, 48, 72);
    }
    let (r, g, b) = GREENS[usize::from(v.min(6))];
    let k = if row % 2 == 1 { 0.8 } else { 1.0 };
    let f = |c: u8| (f32::from(c) * k) as u8;
    Color::Rgb(f(r), f(g), f(b))
}

struct NewsMock;

impl Screen for NewsMock {
    fn update(
        &mut self,
        _: &mut AppState,
        _: &[crossterm::event::KeyEvent],
    ) -> crate::app::ScreenCmd {
        crate::app::ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut ratatui::Frame) {
        use crate::missions::face;
        use crate::ui::chrome::{DIM, FAINT, MAGENTA, PANEL_BG, TEXT, put};
        use ratatui::style::Style;
        const RED: Color = Color::Rgb(255, 48, 72);
        let area = f.area();
        let w = 90.min(area.width.saturating_sub(4));
        let room = usize::from(w - 26);
        let mut lines: Vec<String> = Vec::new();
        for (i, para) in NEWS_P.iter().enumerate() {
            if i > 0 {
                lines.push(String::new());
            }
            lines.extend(crate::ui::sidebar::wrap_text(para, room));
        }
        let h = (lines.len() as u16 + 11).max(17).min(area.height - 2);
        let r = Rect::new(
            area.x + (area.width - w) / 2,
            area.y + (area.height - h) / 2,
            w,
            h,
        );
        let buf = f.buffer_mut();
        face::bracket_frame(buf, r);
        let right = r.right();
        let bold = |c| {
            Style::default()
                .fg(c)
                .bg(PANEL_BG)
                .add_modifier(Modifier::BOLD)
        };
        let plain = |c| Style::default().fg(c).bg(PANEL_BG);
        put(buf, r.x + 3, r.y + 1, "HOLO", bold(MAGENTA), right);
        put(buf, r.x + 8, r.y + 1, "▸ news", plain(DIM), right);
        put(buf, r.x + 15, r.y + 1, "· VELFORM NEWSLINE", plain(FAINT), right);
        put(buf, right - 18, r.y + 1, "●", plain(RED), right);
        put(buf, right - 16, r.y + 1, "LIVE", bold(RED), right);
        let (hh, mm) = st.clock();
        put(
            buf,
            right - 11,
            r.y + 1,
            &format!("{hh:02}:{mm:02}"),
            plain(DIM),
            right,
        );
        // the still, then what it is
        let still = news_still();
        for j in 0..9u16 {
            for i in 0..18u16 {
                let (top, bottom) = (
                    still[usize::from(2 * j)][usize::from(i)],
                    still[usize::from(2 * j + 1)][usize::from(i)],
                );
                let cell = &mut buf[(r.x + 3 + i, r.y + 3 + j)];
                cell.set_symbol("▀");
                cell.set_fg(news_colour(top, usize::from(2 * j)));
                cell.set_bg(news_colour(bottom, usize::from(2 * j + 1)));
            }
        }
        put(buf, r.x + 3, r.y + 13, "●", plain(RED), right);
        put(buf, r.x + 5, r.y + 13, "CAM 14", bold(DIM), right);
        put(buf, r.x + 12, r.y + 13, "· 22:41", plain(FAINT), right);
        put(buf, r.x + 3, r.y + 14, "Kowloon Row", plain(FAINT), right);
        let tx = r.x + 22;
        for (j, line) in lines.iter().enumerate() {
            put(buf, tx, r.y + 3 + j as u16, line, plain(TEXT), right);
        }
        // the description, as a lower third
        let cy = r.y + 3 + lines.len() as u16 + 1;
        let band = Color::Rgb(26, 8, 14);
        let on = |c| Style::default().fg(c).bg(band);
        for dy in 0..2 {
            put(buf, tx, cy + dy, &" ".repeat(room), on(TEXT), right - 3);
            put(buf, tx, cy + dy, "▌", on(RED), right);
        }
        put(
            buf,
            tx + 2,
            cy,
            "SOUGHT",
            on(RED).add_modifier(Modifier::BOLD),
            right,
        );
        put(
            buf,
            tx + 10,
            cy,
            "a courier · medium build · salvage jacket",
            on(TEXT),
            right,
        );
        put(
            buf,
            tx + 10,
            cy + 1,
            "carrying a taped package · 22:41, heading north",
            on(TEXT),
            right,
        );
        let key_y = r.bottom() - 4;
        put(buf, tx, key_y, "space", bold(MAGENTA), right);
        put(buf, tx + 6, key_y, "turn it off", plain(DIM), right);
        // the crawl along the foot
        let ty = r.bottom() - 2;
        let crawl = Color::Rgb(40, 0, 32);
        put(
            buf,
            r.x + 1,
            ty,
            &" ".repeat(usize::from(w - 2)),
            Style::default().bg(crawl),
            right - 1,
        );
        put(
            buf,
            r.x + 1,
            ty,
            " NEWSLINE ",
            Style::default()
                .fg(PANEL_BG)
                .bg(MAGENTA)
                .add_modifier(Modifier::BOLD),
            right - 1,
        );
        put(
            buf,
            r.x + 11,
            ty,
            NEWS_TICKER,
            Style::default().fg(DIM).bg(crawl),
            right - 1,
        );
    }
}

#[test]
#[ignore]
fn news_mock() {
    let mut st = state_home();
    let want = 7 * 60 + 14;
    let ahead = (want + 1440 - st.minute_of_day() as u64) % 1440;
    {
        let (city, world) = (st.city.as_ref().unwrap(), st.world.as_mut().unwrap());
        world.advance(city, world.tick() + ahead * sim::TICKS_PER_MINUTE);
    }
    let map = MapScreen::walking(&mut st);
    for (w, h) in HOLO_SIZES {
        shot_stack(&format!("news_mock_{w}x{h}"), &st, &[&map, &NewsMock], (w, h));
    }
}
