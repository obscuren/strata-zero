//! Widgets built on the animation primitives: logo, menu, form, panels, transitions.

pub mod art;
pub mod board;
pub mod chrome;
pub mod console;
pub mod cutaway;
pub mod cyberdeck;
pub mod device;
pub mod dim;
pub mod form;
pub mod hardware;
pub mod hud;
pub mod implant_art;
pub mod logo;
pub mod looks;
pub mod menu;
pub mod meter;
pub mod panel;
pub mod picture;
pub mod pixels;
pub mod preview;
pub mod rack;
pub mod sidebar;
pub mod skill_art;
pub mod skill_card;
pub mod slab;
pub mod stickers;
pub mod talents;
pub mod transition;

/// A number with `sep` between each three digits from the right: `1,240`, `12 500`.
pub fn grouped(n: u64, sep: char) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(sep);
        }
        out.push(c);
    }
    out
}

/// A sum as the game writes it, its thousands set off with commas: `4,800`.
pub fn commas(n: u32) -> String {
    grouped(u64::from(n), ',')
}
