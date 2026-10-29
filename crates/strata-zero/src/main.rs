//! `strata`: Strata Zero, a terminal roguelike in a multi-level cyberpunk city.

mod access;
mod actions;
mod anim;
mod app;
mod backdoors;
mod blast;
mod casts;
mod cli;
mod clinic;
mod combat;
mod constructs;
mod credentials;
mod exposure;
mod foes;
mod helix;
mod home;
mod interact;
mod interiors;
mod jobs;
mod look;
mod messages;
mod net;
mod render;
mod save;
mod screens;
mod term;
mod ui;
mod walker;
#[cfg(feature = "window")]
mod window;

fn main() -> anyhow::Result<()> {
    cli::run()
}
