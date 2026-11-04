//! Command line: interactive app by default, `gen` for headless generation.

use crate::app::{self, AppState};
use crate::save::save_city;
use anyhow::{Context, Result, bail};
use citygen::{GenParams, Generator, SizePreset, WaterMode};
use clap::{Parser, Subcommand};
use std::ffi::OsStr;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "strata",
    about = "Strata Zero: a terminal roguelike in a multi-level cyberpunk city"
)]
pub struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
    /// Start in dev mode: the free camera, the console, the inspector and the debug
    /// starts on the title. `STRATA_DEV=1` does the same.
    #[arg(long)]
    dev: bool,
    /// Play in a window of its own instead of the terminal.
    #[cfg(feature = "window")]
    #[arg(long)]
    window: bool,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Generate a city headlessly and write it to a file.
    Gen {
        /// Seed; random when omitted.
        #[arg(long)]
        seed: Option<u64>,
        /// small, medium, large or WxH.
        #[arg(long, default_value = "medium")]
        size: String,
        #[arg(long)]
        districts: Option<u8>,
        /// none, bay or river.
        #[arg(long)]
        water: Option<String>,
        #[arg(long)]
        density: Option<f32>,
        /// Output path (.city, or .json with --json).
        #[arg(long)]
        out: PathBuf,
        /// Write pretty JSON instead of the binary format.
        #[arg(long)]
        json: bool,
    },
}

pub fn parse_size(s: &str) -> Result<SizePreset> {
    match s {
        "small" => Ok(SizePreset::Small),
        "medium" => Ok(SizePreset::Medium),
        "large" => Ok(SizePreset::Large),
        other => {
            let (w, h) = other
                .split_once('x')
                .with_context(|| format!("size `{other}` is not small/medium/large or WxH"))?;
            Ok(SizePreset::Custom {
                w: w.parse().context("bad width")?,
                h: h.parse().context("bad height")?,
            })
        }
    }
}

pub fn parse_water(s: &str) -> Result<WaterMode> {
    match s {
        "none" => Ok(WaterMode::None),
        "bay" => Ok(WaterMode::Bay),
        "river" => Ok(WaterMode::River),
        other => bail!("water `{other}` is not none/bay/river"),
    }
}

fn random_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    citygen::rng::hash64(nanos, std::process::id() as u64)
}

/// Whether this run asked for dev mode: `--dev`, or `STRATA_DEV` set to anything but
/// empty or `0`.
fn dev_asked(flag: bool, env: Option<&OsStr>) -> bool {
    flag || env.is_some_and(|v| !v.is_empty() && v != OsStr::new("0"))
}

/// The state a game starts from: runs saved in the runs folder, dev mode unlocked and
/// on if it was asked for.
fn start_state(dev: bool) -> AppState {
    let mut st = AppState::new();
    st.saves = Some(crate::runs::default_dir());
    if dev {
        st.unlock_dev();
    }
    st
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let dev = dev_asked(cli.dev, std::env::var_os("STRATA_DEV").as_deref());
    match cli.cmd {
        #[cfg(feature = "window")]
        None if cli.window || crate::window::IS_APP => {
            let st = start_state(dev);
            crate::window::run(crate::screens::initial(&st), st)
        }
        None => {
            let st = start_state(dev);
            app::run(crate::screens::initial(&st), st)
        }
        Some(Cmd::Gen {
            seed,
            size,
            districts,
            water,
            density,
            out,
            json,
        }) => {
            let mut params = GenParams {
                seed: seed.unwrap_or_else(random_seed),
                size: parse_size(&size)?,
                ..Default::default()
            };
            if let Some(d) = districts {
                params.districts = d;
            }
            if let Some(w) = water {
                params.water = parse_water(&w)?;
            }
            if let Some(d) = density {
                params.density = d;
            }
            params.validate()?;
            let city = Generator::run(params)?;
            if json {
                let file = std::fs::File::create(&out)
                    .with_context(|| format!("creating {}", out.display()))?;
                let mut writer = std::io::BufWriter::new(file);
                serde_json::to_writer_pretty(&mut writer, &city)?;
                std::io::Write::flush(&mut writer)?;
            } else {
                save_city(&out, &city)?;
            }
            let bytes = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
            println!(
                "{} seed {} {}x{} districts {} buildings {} nodes {} -> {} ({} bytes)",
                city.name,
                city.seed,
                city.size.w,
                city.size.h,
                city.districts.len(),
                city.buildings.len(),
                city.net.nodes.len(),
                out.display(),
                bytes
            );
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_mode_is_asked_for_by_flag_or_env() {
        assert!(!dev_asked(false, None), "off unless asked");
        assert!(dev_asked(true, None));
        assert!(dev_asked(false, Some(OsStr::new("1"))));
        assert!(!dev_asked(false, Some(OsStr::new("0"))));
        assert!(!dev_asked(false, Some(OsStr::new(""))));
        assert!(Cli::try_parse_from(["strata", "--dev"]).unwrap().dev);
        assert!(!Cli::try_parse_from(["strata"]).unwrap().dev);
    }

    #[test]
    fn a_game_starts_without_dev_mode_unless_asked() {
        let st = start_state(false);
        assert!(!st.dev && !st.dev_unlocked);
        let st = start_state(true);
        assert!(st.dev && st.dev_unlocked);
    }

    #[test]
    fn a_game_saves_its_runs_in_the_runs_folder() {
        assert_eq!(start_state(false).saves, Some(crate::runs::default_dir()));
        assert_eq!(AppState::new().saves, None, "nothing else saves");
    }
}
