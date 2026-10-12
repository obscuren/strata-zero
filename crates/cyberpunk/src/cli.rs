//! Command line: interactive app by default, `gen` for headless generation.

use crate::app::{self, AppState};
use crate::save::save_city;
use anyhow::{Context, Result, bail};
use citygen::{GenParams, Generator, SizePreset, WaterMode};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "cyberpunk",
    about = "A terminal roguelike in a multi-level cyberpunk city"
)]
pub struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
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

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        None => app::run(Box::new(crate::screens::initial()), AppState::new()),
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
