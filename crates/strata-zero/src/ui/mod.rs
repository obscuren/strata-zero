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
// The desk (`screens/desk.rs`) draws the rack; until it does, only the tests do.
#[cfg_attr(not(test), allow(dead_code))]
pub mod rack;
pub mod sidebar;
pub mod skill_art;
pub mod skill_card;
pub mod slab;
pub mod stickers;
pub mod talents;
pub mod transition;
