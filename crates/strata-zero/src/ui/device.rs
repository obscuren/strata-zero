//! Diegetic hardware in the city's register: a unit with an edge in the district's
//! colour, dark glass, keys. Every machine wears a theme, a look and a voice kept
//! as data in the style file: municipal gunmetal, megacorp chrome, neon, or the
//! street's cracked plastic. The keypad and the terminal are built from it.

use crate::app::AppState;
use crate::render::Rgb;
use crate::render::style::DeviceStyle;
use crate::ui::chrome::{CYAN, MAGENTA};
use citygen::rng::hash64;
use citygen::{BuildingId, BuildingKind, DistrictId};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

pub const DEVICE_W: u16 = 25;
pub const DEVICE_H: u16 = 19;
/// The terminal: a unit around a 68×20 glass.
pub const CRT_W: u16 = 72;
pub const CRT_H: u16 = 24;
/// How much darker a scanline row is.
const SCANLINE: f32 = 0.88;
/// Ticks a flicker lasts.
const FLICKER_TICKS: u64 = 3;

/// Refusals on the glass.
pub const DENY: Color = MAGENTA;
/// The one operating system every machine in the city runs, by version.
pub const OS_NAME: &str = "HELIX/OS";

/// The status light.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Led {
    Idle,
    Ok,
    Fail,
}

/// How a key is lit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyLit {
    Normal,
    Worn,
    Deny,
    Accept,
}

/// What kind of machine, for picking its theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    Keypad(BuildingId),
    Terminal(DistrictId),
    Rig,
}

/// The theme a district's machines wear.
pub fn theme_name(t: citygen::Theme) -> &'static str {
    use citygen::Theme::*;
    match t {
        CorpCore => "megacorp",
        NightMarket | NeonStrip => "neon",
        CombatZone | OldTown | Docks | Industrial => "street",
        Megablocks => "municipal",
    }
}

/// The built-in look when the style file has none: municipal.
fn municipal() -> DeviceStyle {
    DeviceStyle {
        maker: "CIVITAS".to_string(),
        keypad: "K-40".to_string(),
        terminal: "KIOSK 9".to_string(),
        os_version: "4.1".to_string(),
        alternate: false,
        body: Rgb(40, 46, 54),
        edge: None,
        glass: Rgb(6, 18, 22),
        text: None,
        text_dim: Rgb(0, 120, 135),
        keys_lit: true,
        scanlines: true,
        cracked: false,
        dead: 0,
        flicker: 0,
        cps: 60,
        boot: vec![
            "{OS} · {PLACE} SUBNET".to_string(),
            "MEMORY CHECK OK".to_string(),
            "READY".to_string(),
        ],
        denied: None,
    }
}

/// A unit's look: its theme, the district's accents, and a seed of its own for
/// cracks, dead cells and the flicker's beat.
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub name: String,
    pub style: DeviceStyle,
    pub accent: Rgb,
    pub accent2: Rgb,
    pub seed: u64,
}

impl Look {
    /// A theme by name, with a district's accents when one is given.
    pub fn named(st: &AppState, name: &str, district: Option<DistrictId>, seed: u64) -> Look {
        let style = st.style.device(name).cloned().unwrap_or_else(municipal);
        let (accent, accent2) = district
            .and_then(|d| {
                st.city
                    .as_ref()
                    .map(|c| st.style.theme(c.district(d).theme))
            })
            .map(|t| (t.accent, t.accent2))
            .unwrap_or((Rgb(0, 229, 255), Rgb(255, 0, 200)));
        Look {
            name: name.to_string(),
            style,
            accent,
            accent2,
            seed,
        }
    }

    /// The look a machine wears, by what it is and where it stands.
    pub fn pick(st: &AppState, device: Device) -> Look {
        let Some(city) = st.city.as_ref() else {
            return Look::named(st, "municipal", None, 0);
        };
        match device {
            Device::Keypad(b) => {
                let Some(building) = city.building_opt(b) else {
                    return Look::named(st, "municipal", None, 0);
                };
                let name = if matches!(
                    building.kind,
                    BuildingKind::CorpTower | BuildingKind::Office
                ) {
                    "megacorp"
                } else {
                    theme_name(city.district(building.district).theme)
                };
                Look::named(st, name, Some(building.district), b.index() as u64)
            }
            Device::Terminal(d) => Look::named(
                st,
                theme_name(city.district(d).theme),
                Some(d),
                0x7E12 + d.index() as u64,
            ),
            Device::Rig => {
                let district = st
                    .home
                    .and_then(|r| city.building_opt(r.home.building))
                    .map(|b| b.district);
                Look::named(st, "street", district, 0x5160)
            }
        }
    }

    /// "HELIX/OS 4.1": the city's one OS at this hardware's version.
    pub fn os(&self) -> String {
        format!("{OS_NAME} {}", self.style.os_version)
    }

    /// The maker and model on a keypad: "CIVITAS K-40".
    pub fn keypad_badge(&self) -> String {
        format!("{} {}", self.style.maker, self.style.keypad)
    }

    /// The maker and model on a terminal: "CIVITAS KIOSK 9".
    pub fn terminal_badge(&self) -> String {
        format!("{} {}", self.style.maker, self.style.terminal)
    }

    pub fn body(&self) -> Color {
        self.style.body.to_color()
    }

    /// The hairline edge: the theme's own, else the accent a little under full.
    pub fn edge(&self) -> Color {
        self.style
            .edge
            .unwrap_or_else(|| self.accent.scale(0.75))
            .to_color()
    }

    pub fn glass(&self) -> Color {
        self.style.glass.to_color()
    }

    pub fn text(&self) -> Color {
        self.style.text.map_or(CYAN, |t| t.to_color())
    }

    pub fn text_dim(&self) -> Color {
        self.style.text_dim.to_color()
    }

    /// The badge's ink: the edge colour, or dark ink on a light body.
    fn label(&self) -> Color {
        let b = self.style.body;
        let light = (b.0 as u32 + b.1 as u32 + b.2 as u32) / 3 > 128;
        if light {
            Rgb(30, 34, 40).to_color()
        } else {
            self.style.edge.unwrap_or(self.accent).to_color()
        }
    }

    fn led(&self, led: Led) -> Color {
        match led {
            Led::Idle => self
                .style
                .edge
                .unwrap_or(self.accent)
                .scale(0.45)
                .to_color(),
            Led::Ok => CYAN,
            Led::Fail => MAGENTA,
        }
    }

    /// A key's backlight and label.
    pub fn key_colours(&self, lit: KeyLit) -> (Color, Color) {
        let base = self.style.edge.unwrap_or(self.accent);
        let lit_tint =
            |rgb: Rgb, bg: f32, fg: f32| (rgb.scale(bg).to_color(), rgb.scale(fg).to_color());
        if !self.style.keys_lit {
            // Labels printed on the body: worn ones nearly rubbed off.
            let fg = match lit {
                KeyLit::Normal => base.scale(0.9),
                KeyLit::Worn => base.scale(0.4),
                KeyLit::Deny => Rgb(255, 0, 200).scale(0.8),
                KeyLit::Accept => Rgb(0, 229, 255).scale(0.8),
            };
            return (self.body(), fg.to_color());
        }
        match lit {
            KeyLit::Normal => lit_tint(base, 0.22, 1.0),
            KeyLit::Worn => lit_tint(base, 0.10, 0.45),
            KeyLit::Deny => lit_tint(Rgb(255, 0, 200), 0.22, 1.0),
            KeyLit::Accept => lit_tint(Rgb(0, 229, 255), 0.22, 1.0),
        }
    }

    /// Whether the glass dims at this tick: a few ticks every so many, on a beat of
    /// the unit's own.
    pub fn flickering(&self, tick: u64) -> bool {
        let p = self.style.flicker;
        p > 0 && (tick + hash64(self.seed, 0xF11C) % p) % p < FLICKER_TICKS
    }

    /// Whether an edge cell is missing on a cracked unit.
    fn gap(&self, x: u16, y: u16) -> bool {
        self.style.cracked && hash64(self.seed, (x as u64) * 131 + y as u64).is_multiple_of(7)
    }
}

/// A shell reached from the Net: no unit, a thin frame in the Net's colour on the dark
/// glass, the host on its top edge, who you are on the right, the way out below, a
/// faint lattice behind the text. A refusal turns the frame red while it types. The
/// glass inside is where `crt` puts it, so a page holds the same lines.
pub fn jack(buf: &mut Buffer, area: Rect, title: &str, right: &str, led: Led, look: &Look) -> Rect {
    let glass = look.glass();
    let edge = match led {
        Led::Fail => DENY,
        _ => look.edge(),
    };
    let lattice = look
        .style
        .edge
        .unwrap_or(look.accent)
        .scale(0.18)
        .to_color();
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let cell = &mut buf[(x, y)];
            cell.reset();
            cell.set_char(' ').set_bg(glass);
        }
    }
    let screen = Rect::new(
        area.x + 2,
        area.y + 2,
        area.width.saturating_sub(4),
        area.height.saturating_sub(4),
    );
    for y in (screen.y..screen.bottom()).step_by(2) {
        for x in (screen.x + 1..screen.right()).step_by(6) {
            buf[(x, y)].set_char('·').set_fg(lattice);
        }
    }
    if area.width < 4 || area.height < 3 {
        return screen;
    }
    let (r, b) = (area.right() - 1, area.bottom() - 1);
    for x in area.x..=r {
        for y in [area.y, b] {
            let ch = match (x == area.x, x == r, y == area.y) {
                (true, _, true) => '┌',
                (_, true, true) => '┐',
                (true, _, false) => '└',
                (_, true, false) => '┘',
                _ => '─',
            };
            buf[(x, y)].set_char(ch).set_fg(edge);
        }
    }
    for y in area.y + 1..b {
        for x in [area.x, r] {
            buf[(x, y)].set_char('│').set_fg(edge);
        }
    }
    let mut put = |x: u16, y: u16, s: &str, fg: Color, bold: bool| {
        for (i, ch) in s.chars().enumerate() {
            let cx = x + i as u16;
            if cx >= r {
                break;
            }
            let cell = &mut buf[(cx, y)];
            cell.set_char(ch).set_fg(fg);
            if bold {
                cell.modifier.insert(Modifier::BOLD);
            }
        }
    };
    let head = format!(" ◇ JACK · {title} ");
    put(area.x + 2, area.y, &head, edge, true);
    let right = format!(" {right} ");
    let rw = right.chars().count() as u16;
    let at = r.saturating_sub(rw + 1);
    if at > area.x + 2 + head.chars().count() as u16 {
        put(at, area.y, &right, JACK_SEAT, false);
    }
    put(
        area.x + 2,
        b,
        " esc · back to the grid ",
        look.text_dim(),
        false,
    );
    screen
}

/// Who you are on a jacked host, on the window's edge: the held node's green.
const JACK_SEAT: Color = Color::Rgb(80, 230, 120);

/// The unit: the body, a heavy hairline edge, a badge and an LED.
pub fn bezel(buf: &mut Buffer, area: Rect, badge: &str, led: Led, look: &Look) {
    if area.width < 6 || area.height < 4 {
        return;
    }
    let body = look.body();
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            let c = &mut buf[(x, y)];
            c.reset();
            c.set_char(' ').set_bg(body);
        }
    }
    let edge = Style::default().fg(look.edge()).bg(body);
    let mut put_edge = |x: u16, y: u16, ch: char| {
        if !look.gap(x, y) {
            buf[(x, y)].set_char(ch).set_style(edge);
        }
    };
    for x in area.x + 1..area.right() - 1 {
        put_edge(x, area.y, '━');
        put_edge(x, area.bottom() - 1, '━');
    }
    for y in area.y + 1..area.bottom() - 1 {
        put_edge(area.x, y, '┃');
        put_edge(area.right() - 1, y, '┃');
    }
    buf[(area.x, area.y)].set_char('┏').set_style(edge);
    buf[(area.right() - 1, area.y)]
        .set_char('┓')
        .set_style(edge);
    buf[(area.x, area.bottom() - 1)]
        .set_char('┗')
        .set_style(edge);
    buf[(area.right() - 1, area.bottom() - 1)]
        .set_char('┛')
        .set_style(edge);
    let label = Style::default()
        .fg(look.label())
        .bg(body)
        .add_modifier(Modifier::BOLD);
    // Neon badges alternate the district's two accents, letter by letter.
    let second = label.fg(look.accent2.to_color());
    let first = label.fg(look.accent.to_color());
    let text = format!("▸ {badge}");
    let mut lit = 0usize;
    for (i, ch) in text.chars().enumerate() {
        let x = area.x + 2 + i as u16;
        if x >= area.right() - 4 {
            break;
        }
        let style = if look.style.alternate && i >= 2 && ch != ' ' {
            lit += 1;
            if lit % 2 == 1 { first } else { second }
        } else {
            label
        };
        buf[(x, area.y + 1)].set_char(ch).set_style(style);
    }
    buf[(area.right() - 3, area.y + 1)]
        .set_char('●')
        .set_fg(look.led(led))
        .set_bg(body);
}

/// Dead cells on a worn unit: `▒` in the edge colour, by hash, never over `glass`.
pub fn dead_cells(buf: &mut Buffer, area: Rect, glass: Rect, look: &Look) {
    let n = look.style.dead as u64;
    let inner = Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    if n == 0 || inner.width == 0 || inner.height == 0 {
        return;
    }
    let mut placed = 0u64;
    let mut i = 0u64;
    while placed < n && i < 200 {
        let h = hash64(look.seed, 0xDEAD + i);
        let x = inner.x + (h % inner.width as u64) as u16;
        let y = inner.y + ((h >> 16) % inner.height as u64) as u16;
        i += 1;
        let over_glass = x >= glass.x && x < glass.right() && y >= glass.y && y < glass.bottom();
        if over_glass || buf[(x, y)].symbol() != " " {
            continue;
        }
        buf[(x, y)]
            .set_char('▒')
            .set_fg(look.edge())
            .set_bg(look.body());
        placed += 1;
    }
}

/// The unit around a glass inset two cells; returns the glass rect.
pub fn crt(buf: &mut Buffer, area: Rect, badge: &str, led: Led, look: &Look) -> Rect {
    bezel(buf, area, badge, led, look);
    let screen = Rect::new(
        area.x + 2,
        area.y + 2,
        area.width.saturating_sub(4),
        area.height.saturating_sub(4),
    );
    let glass = look.glass();
    for y in screen.y..screen.bottom() {
        for x in screen.x..screen.right() {
            buf[(x, y)].set_char(' ').set_bg(glass);
        }
    }
    screen
}

/// A strip of glass on the unit: a thin edge, dark inside, lines of text; the text
/// dims on a flicker.
pub fn glass(buf: &mut Buffer, area: Rect, lines: &[(String, Style)], look: &Look, tick: u64) {
    if area.width < 3 || area.height < 3 {
        return;
    }
    let body = look.body();
    let bg = look.glass();
    let edge = Style::default()
        .fg(look.style.edge.unwrap_or(look.accent).scale(0.4).to_color())
        .bg(body);
    for x in area.x..area.right() {
        buf[(x, area.y)].set_char('─').set_style(edge);
        buf[(x, area.bottom() - 1)].set_char('─').set_style(edge);
    }
    for y in area.y..area.bottom() {
        buf[(area.x, y)].set_char('│').set_style(edge);
        buf[(area.right() - 1, y)].set_char('│').set_style(edge);
    }
    buf[(area.x, area.y)].set_char('┌').set_style(edge);
    buf[(area.right() - 1, area.y)]
        .set_char('┐')
        .set_style(edge);
    buf[(area.x, area.bottom() - 1)]
        .set_char('└')
        .set_style(edge);
    buf[(area.right() - 1, area.bottom() - 1)]
        .set_char('┘')
        .set_style(edge);
    for y in area.y + 1..area.bottom() - 1 {
        for x in area.x + 1..area.right() - 1 {
            buf[(x, y)].set_char(' ').set_bg(bg);
        }
    }
    let flicker = look.flickering(tick);
    for (i, (text, style)) in lines.iter().enumerate() {
        let y = area.y + 1 + i as u16;
        if y >= area.bottom() - 1 {
            break;
        }
        let style = if flicker { dimmed(*style) } else { *style };
        for (j, ch) in text.chars().enumerate() {
            let x = area.x + 2 + j as u16;
            if x >= area.right() - 1 {
                break;
            }
            buf[(x, y)].set_char(ch).set_style(style.bg(bg));
        }
    }
}

/// A style at half brightness, for a flicker.
pub fn dimmed(style: Style) -> Style {
    match style.fg {
        Some(Color::Rgb(r, g, b)) => style.fg(Rgb(r, g, b).scale(0.5).to_color()),
        _ => style,
    }
}

/// One key, 5 wide and a row tall, its label in the middle: backlit, or printed on
/// the body when the theme's keys are unlit.
pub fn key(buf: &mut Buffer, x: u16, y: u16, label: char, lit: KeyLit, look: &Look) {
    let (bg, fg) = look.key_colours(lit);
    for i in 0..5 {
        buf[(x + i, y)].set_char(' ').set_bg(bg);
    }
    buf[(x + 2, y)]
        .set_char(label)
        .set_style(Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD));
}

/// Darkens every other row of a glass a shade.
pub fn scanlines(buf: &mut Buffer, area: Rect) {
    for y in (area.y..area.bottom()).skip(1).step_by(2) {
        for x in area.x..area.right() {
            let cell = &mut buf[(x, y)];
            if let Color::Rgb(r, g, b) = cell.fg {
                let d = Rgb(r, g, b).scale(SCANLINE);
                cell.set_fg(d.to_color());
            }
        }
    }
}

/// Garbage scattered over the glass for a refusal, fading with `p` in 0..=1.
pub fn glitch(buf: &mut Buffer, area: Rect, tick: u64, p: f32) {
    for (x, y, ch) in crate::anim::effects::glitch_cells(area, tick, p) {
        if x < area.right() && y < area.bottom() {
            let bg = buf[(x, y)].bg;
            buf[(x, y)].set_char(ch).set_fg(DENY).set_bg(bg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use citygen::{BuildingKind, GenParams, Generator, SizePreset};
    use ratatui::buffer::Buffer;

    fn state() -> AppState {
        let mut st = AppState::new();
        st.enter_city(
            Generator::run(GenParams {
                seed: 6,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 8,
                ..Default::default()
            })
            .unwrap(),
        );
        st
    }

    fn look(name: &str) -> Look {
        let st = state();
        Look::named(&st, name, None, 7)
    }

    #[test]
    fn the_style_file_has_four_device_themes_and_the_rules_pick_them() {
        let st = state();
        for name in ["municipal", "megacorp", "neon", "street"] {
            assert!(st.style.device(name).is_some(), "{name}");
        }
        assert_eq!(theme_name(citygen::Theme::CorpCore), "megacorp");
        assert_eq!(theme_name(citygen::Theme::NightMarket), "neon");
        assert_eq!(theme_name(citygen::Theme::NeonStrip), "neon");
        assert_eq!(theme_name(citygen::Theme::CombatZone), "street");
        assert_eq!(theme_name(citygen::Theme::OldTown), "street");
        assert_eq!(theme_name(citygen::Theme::Docks), "street");
        assert_eq!(theme_name(citygen::Theme::Industrial), "street");
        assert_eq!(theme_name(citygen::Theme::Megablocks), "municipal");
        let city = st.city.as_ref().unwrap();
        if let Some(corp) = city
            .buildings
            .iter()
            .find(|b| matches!(b.kind, BuildingKind::CorpTower | BuildingKind::Office))
        {
            assert_eq!(Look::pick(&st, Device::Keypad(corp.id)).name, "megacorp");
        }
        if let Some(flat) = city
            .buildings
            .iter()
            .find(|b| b.kind == BuildingKind::Apartment)
        {
            let d = city.district(flat.district).theme;
            assert_eq!(Look::pick(&st, Device::Keypad(flat.id)).name, theme_name(d));
            assert_eq!(
                Look::pick(&st, Device::Terminal(flat.district)).name,
                theme_name(d)
            );
        }
        assert_eq!(Look::pick(&st, Device::Rig).name, "street");
    }

    #[test]
    fn a_cracked_edge_has_gaps_and_dead_cells_show_on_the_unit() {
        let area = Rect::new(2, 1, DEVICE_W, DEVICE_H);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 24));
        let street = look("street");
        assert!(street.style.cracked && street.style.dead > 0);
        bezel(&mut buf, area, &street.keypad_badge(), Led::Idle, &street);
        let edge_cells = (area.x..area.right())
            .filter(|x| buf[(*x, area.y)].symbol() == "━")
            .count();
        assert!(edge_cells < area.width as usize, "the top edge has gaps");
        let glass = Rect::new(area.x + 2, area.y + 3, area.width - 4, 4);
        dead_cells(&mut buf, area, glass, &street);
        let dead = (area.y..area.bottom())
            .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
            .filter(|(x, y)| buf[(*x, *y)].symbol() == "▒")
            .count();
        assert_eq!(dead, street.style.dead as usize);
        assert!(
            !(glass.y..glass.bottom())
                .flat_map(|y| (glass.x..glass.right()).map(move |x| (x, y)))
                .any(|(x, y)| buf[(x, y)].symbol() == "▒"),
            "never over the glass"
        );
        let mut clean = Buffer::empty(Rect::new(0, 0, 40, 24));
        let corp = look("megacorp");
        bezel(&mut clean, area, &corp.keypad_badge(), Led::Idle, &corp);
        let full = (area.x..area.right())
            .filter(|x| clean[(*x, area.y)].symbol() == "━")
            .count();
        assert_eq!(full, area.width as usize - 2, "chrome is unbroken");
    }

    #[test]
    fn unlit_keys_sit_on_the_body_and_flicker_dims_on_its_beat() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 24));
        let street = look("street");
        assert!(!street.style.keys_lit);
        key(&mut buf, 10, 10, '7', KeyLit::Normal, &street);
        assert_eq!(buf[(10, 10)].bg, street.body());
        let corp = look("megacorp");
        key(&mut buf, 10, 12, '7', KeyLit::Normal, &corp);
        assert_ne!(buf[(10, 12)].bg, corp.body(), "a lit key has a backlight");
        let p = street.style.flicker;
        assert!(p > 0);
        let on = (0..p * 2)
            .find(|t| street.flickering(*t))
            .expect("a flicker");
        assert!(!street.flickering(on + 3), "three ticks long");
        assert!(!corp.flickering(on), "megacorp never flickers");
        assert!(!corp.style.scanlines && street.style.scanlines);
    }

    #[test]
    fn the_crt_has_dark_glass_inside_the_unit_and_scanlines_dim_alternate_rows() {
        let area = Rect::new(3, 2, CRT_W, CRT_H);
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        let muni = look("municipal");
        let screen = crt(&mut buf, area, &muni.terminal_badge(), Led::Idle, &muni);
        assert_eq!(screen, Rect::new(5, 4, 68, 20));
        assert_eq!(buf[(5, 4)].bg, muni.glass());
        assert_eq!(buf[(3, 2)].symbol(), "┏");
        let cyan = Style::default().fg(muni.text()).bg(muni.glass());
        buf[(10, 4)].set_char('A').set_style(cyan);
        buf[(10, 5)].set_char('B').set_style(cyan);
        scanlines(&mut buf, screen);
        assert_eq!(buf[(10, 4)].fg, muni.text());
        assert_ne!(buf[(10, 5)].fg, muni.text(), "row 5 is darker");
        glitch(&mut buf, screen, 7, 0.0);
        let magenta = (screen.y..screen.bottom())
            .flat_map(|y| (screen.x..screen.right()).map(move |x| (x, y)))
            .filter(|(x, y)| buf[(*x, *y)].fg == DENY)
            .count();
        assert!(magenta > 0, "the glitch scatters magenta garbage");
        let row: String = (area.x..area.right())
            .map(|x| buf[(x, 3)].symbol().to_string())
            .collect();
        assert!(row.contains("▸ CIVITAS KIOSK 9"), "{row}");
        bezel(&mut buf, area, &muni.terminal_badge(), Led::Fail, &muni);
        assert_eq!(buf[(area.right() - 3, 3)].fg, MAGENTA);
        bezel(&mut buf, area, &muni.terminal_badge(), Led::Ok, &muni);
        assert_eq!(buf[(area.right() - 3, 3)].fg, CYAN);
    }

    #[test]
    fn every_device_runs_helix_under_its_makers_name() {
        for (name, keypad, terminal, version) in [
            ("municipal", "CIVITAS K-40", "CIVITAS KIOSK 9", "4.1"),
            ("megacorp", "HALCYON SG-7", "HALCYON NODE 9", "9.4"),
            ("neon", "LOTUS LUXPAD", "LOTUS MARQUEE", "8.0"),
            ("street", "ZENBO ZK-2", "ZENBO 64", "2.2"),
        ] {
            let l = look(name);
            assert_eq!(l.keypad_badge(), keypad);
            assert_eq!(l.terminal_badge(), terminal);
            assert_eq!(l.os(), format!("{OS_NAME} {version}"));
            assert!(
                l.keypad_badge().chars().count() + 2 <= DEVICE_W as usize - 6,
                "{keypad} fits the keypad"
            );
        }
        assert_eq!(OS_NAME, "HELIX/OS");
    }

    #[test]
    fn the_neon_badge_alternates_two_colours_and_the_others_do_not() {
        let area = Rect::new(2, 1, DEVICE_W, DEVICE_H);
        let badge_colours = |l: &Look| {
            let mut buf = Buffer::empty(Rect::new(0, 0, 40, 24));
            bezel(&mut buf, area, &l.keypad_badge(), Led::Idle, l);
            (area.x + 4..area.x + 4 + l.keypad_badge().chars().count() as u16)
                .filter(|x| buf[(*x, area.y + 1)].symbol() != " ")
                .map(|x| buf[(x, area.y + 1)].fg)
                .collect::<Vec<_>>()
        };
        let neon = badge_colours(&look("neon"));
        assert!(neon.len() > 4);
        assert!(neon.windows(2).all(|w| w[0] != w[1]), "{neon:?}");
        assert_eq!(neon[0], neon[2]);
        let muni = badge_colours(&look("municipal"));
        assert!(muni.windows(2).all(|w| w[0] == w[1]), "{muni:?}");
    }
}
