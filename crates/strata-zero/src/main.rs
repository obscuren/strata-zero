//! `strata`: Strata Zero, a terminal roguelike in a multi-level cyberpunk city.

// The Windows app is a GUI program: no console opens with it.
#![cfg_attr(all(windows, feature = "app"), windows_subsystem = "windows")]

mod access;
mod actions;
mod anim;
mod app;
mod backdoors;
mod blast;
mod bots;
mod broadcasts;
mod cameras;
mod casts;
mod cli;
mod clinic;
mod clock;
mod combat;
mod constructs;
mod credentials;
mod exposure;
mod foes;
mod grid;
mod helix;
mod holds;
mod home;
mod interact;
mod interiors;
mod jobs;
mod keyjack;
mod look;
mod mail_mark;
mod manual;
mod messages;
mod missions;
mod net;
mod outside;
mod owners;
mod platform;
mod render;
mod rest;
mod ride;
mod runs;
mod save;
mod screens;
mod sheet;
mod term;
mod track;
mod ui;
mod venues;
mod walker;
#[cfg(feature = "window")]
mod window;

fn main() -> anyhow::Result<()> {
    #[cfg(all(windows, feature = "app"))]
    window::fatal::catch_panics();
    let result = cli::run();
    #[cfg(all(windows, feature = "app"))]
    if let Err(e) = &result {
        window::fatal::show(&format!("{e:#}"));
    }
    result
}
