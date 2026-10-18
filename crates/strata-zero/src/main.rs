//! `strata`: Strata Zero, a terminal roguelike in a multi-level cyberpunk city.

mod access;
mod actions;
mod anim;
mod app;
mod backdoors;
mod cli;
mod constructs;
mod credentials;
mod helix;
mod home;
mod interact;
mod look;
mod interiors;
mod messages;
mod net;
mod render;
mod save;
mod screens;
mod term;
mod ui;
mod walker;

fn main() -> anyhow::Result<()> {
    cli::run()
}
