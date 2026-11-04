//! Command line: interactive app by default, `gen` for headless generation.

use crate::app::{self, AppState};
use crate::save::save_city;
use anyhow::{Context, Result, bail};
use citygen::overworld::{Overworld, WorldParams, WorldSize};
use citygen::{GenParams, Generator, SizePreset, WaterMode};
use clap::{Parser, Subcommand};
use std::ffi::OsStr;
use std::ops::Range;
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
    /// Generate overworlds and write them to one HTML page, to look at in a browser.
    World {
        /// One seed.
        #[arg(long, conflicts_with = "seeds")]
        seed: Option<u64>,
        /// A range of seeds, `A..B` (B excluded); 0..12 when neither is given.
        #[arg(long)]
        seeds: Option<String>,
        /// small, medium, large or WxH.
        #[arg(long, default_value = "medium")]
        size: String,
        /// How many cities, 3 to 8.
        #[arg(long)]
        cities: Option<u8>,
        /// Draw one level down instead: the detail map round overworld tile `TX,TY`
        /// (needs `--seed`).
        #[arg(long, requires = "seed", conflicts_with = "seeds")]
        detail: Option<String>,
        /// How many overworld tiles `--detail` draws, `WxH`.
        #[arg(long, default_value = "10x4")]
        span: String,
        /// The page to write.
        #[arg(long, default_value = "world-sheet.html")]
        out: PathBuf,
    },
}

/// An overworld tile, `TX,TY`.
pub fn parse_tile(s: &str) -> Result<citygen::Pos> {
    let (x, y) = s
        .split_once(',')
        .with_context(|| format!("tile `{s}` is not TX,TY"))?;
    Ok(citygen::Pos::new(
        x.trim()
            .parse()
            .with_context(|| format!("bad tile x `{x}`"))?,
        y.trim()
            .parse()
            .with_context(|| format!("bad tile y `{y}`"))?,
    ))
}

/// A span of tiles, `WxH`, each at least 1.
pub fn parse_span(s: &str) -> Result<(u32, u32)> {
    let (w, h) = s
        .split_once('x')
        .with_context(|| format!("span `{s}` is not WxH"))?;
    let (w, h): (u32, u32) = (
        w.parse().context("bad span width")?,
        h.parse().context("bad span height")?,
    );
    if w == 0 || h == 0 {
        bail!("span `{s}` is empty");
    }
    Ok((w, h))
}

/// `A..B` (B excluded, A below B) or a single seed `N`.
pub fn parse_seeds(s: &str) -> Result<Range<u64>> {
    match s.split_once("..") {
        Some((a, b)) => {
            let a: u64 = a
                .trim()
                .parse()
                .with_context(|| format!("bad seed `{a}`"))?;
            let b: u64 = b
                .trim()
                .parse()
                .with_context(|| format!("bad seed `{b}`"))?;
            if a >= b {
                bail!("seeds `{s}` run backwards or are empty");
            }
            Ok(a..b)
        }
        None => {
            let n: u64 = s
                .trim()
                .parse()
                .with_context(|| format!("bad seed `{s}`"))?;
            one_seed(n)
        }
    }
}

fn one_seed(n: u64) -> Result<Range<u64>> {
    let end = n
        .checked_add(1)
        .with_context(|| format!("seed {n} is the last there is; use one below it"))?;
    Ok(n..end)
}

/// The seeds `strata world` draws: `--seed`, else `--seeds`, else 0..12.
pub fn seed_range(seed: Option<u64>, seeds: Option<&str>) -> Result<Range<u64>> {
    match (seed, seeds) {
        (Some(_), Some(_)) => bail!("--seed and --seeds both given; pick one"),
        (Some(n), None) => one_seed(n),
        (None, Some(s)) => parse_seeds(s),
        (None, None) => Ok(0..12),
    }
}

pub fn parse_world_size(s: &str) -> Result<WorldSize> {
    match s {
        "small" => Ok(WorldSize::Small),
        "medium" => Ok(WorldSize::Medium),
        "large" => Ok(WorldSize::Large),
        other => {
            let (w, h) = other
                .split_once('x')
                .with_context(|| format!("size `{other}` is not small/medium/large or WxH"))?;
            Ok(WorldSize::Custom {
                w: w.parse().context("bad width")?,
                h: h.parse().context("bad height")?,
            })
        }
    }
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

/// The window's game: its first screen and its state, once `enter` has put the desktop
/// app in its app folder (`window::enter_app_dir`), so the title finds the runs kept
/// there to CONTINUE.
#[cfg(feature = "window")]
fn window_start(
    dev: bool,
    enter: impl FnOnce() -> Result<Option<PathBuf>>,
) -> Result<(Box<dyn crate::app::Screen>, AppState)> {
    let entered = enter()?;
    let mut st = start_state(dev);
    if let Some(dir) = entered {
        // The folder `runs` names from there, by its whole path.
        st.saves = Some(dir.join(crate::runs::default_dir()));
    }
    Ok((crate::screens::initial(&st), st))
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let dev = dev_asked(cli.dev, std::env::var_os("STRATA_DEV").as_deref());
    match cli.cmd {
        #[cfg(feature = "window")]
        None if cli.window || crate::window::IS_APP => {
            let (screen, st) = window_start(dev, crate::window::enter_app_dir)?;
            crate::window::run(screen, st)
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
        Some(Cmd::World {
            seed,
            seeds,
            size,
            cities,
            detail,
            span,
            out,
        }) => {
            let range = seed_range(seed, seeds.as_deref())?;
            let size = parse_world_size(&size)?;
            let style = crate::render::style::StylePack::load()?;
            if let Some(tile) = detail {
                let mut params = WorldParams {
                    seed: range.start,
                    size,
                    ..Default::default()
                };
                if let Some(c) = cities {
                    params.cities = c;
                }
                let world = Overworld::generate(params)?;
                let (centre, span) = (parse_tile(&tile)?, parse_span(&span)?);
                if world.tile(centre).is_none() {
                    bail!(
                        "tile {tile} is outside the {}×{} world",
                        world.size.w,
                        world.size.h
                    );
                }
                std::fs::write(
                    &out,
                    crate::sheet::detail_sheet(&world, centre, span, &style),
                )
                .with_context(|| format!("writing {}", out.display()))?;
                println!("-> {}", out.display());
                return Ok(());
            }
            let mut worlds = Vec::new();
            for seed in range {
                let mut params = WorldParams {
                    seed,
                    size,
                    ..Default::default()
                };
                if let Some(c) = cities {
                    params.cities = c;
                }
                let start = std::time::Instant::now();
                let world = Overworld::generate(params)?;
                let took = start.elapsed();
                println!(
                    "seed {seed}: {} cities (home {}), {} routes, {} POIs, {} ms",
                    world.cities.len(),
                    world.home().name,
                    world.routes.len(),
                    world.pois.len(),
                    took.as_millis()
                );
                worlds.push((world, took));
            }
            std::fs::write(&out, crate::sheet::world_sheet(&worlds, &style))
                .with_context(|| format!("writing {}", out.display()))?;
            println!("-> {}", out.display());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tile_parses() {
        assert_eq!(parse_tile("50,36").unwrap(), citygen::Pos::new(50, 36));
        assert!(parse_tile("50").is_err() && parse_tile("a,b").is_err());
    }

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
    fn seeds_parse_as_ranges() {
        assert_eq!(parse_seeds("0..12").unwrap(), 0..12);
        assert_eq!(parse_seeds("7").unwrap(), 7..8);
        assert!(parse_seeds("5..2").is_err());
        assert!(parse_seeds("abc").is_err());
        assert!(parse_seeds("3..x").is_err());
    }

    #[test]
    fn one_seed_or_a_range_but_not_both() {
        assert!(
            Cli::try_parse_from(["strata", "world", "--seed", "1", "--seeds", "0..3"]).is_err()
        );
        assert_eq!(seed_range(Some(4), None).unwrap(), 4..5);
        assert_eq!(seed_range(None, Some("2..5")).unwrap(), 2..5);
        assert_eq!(seed_range(None, None).unwrap(), 0..12);
        assert!(seed_range(Some(u64::MAX), None).is_err());
        assert!(parse_seeds(&u64::MAX.to_string()).is_err());
    }

    #[test]
    fn world_sizes_parse() {
        assert_eq!(parse_world_size("medium").unwrap(), WorldSize::Medium);
        assert_eq!(
            parse_world_size("200x60").unwrap(),
            WorldSize::Custom { w: 200, h: 60 }
        );
        assert!(parse_world_size("huge").is_err());
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

    /// The desktop app's title offers CONTINUE for a run in its app folder: the folder
    /// is entered before the title looks. The run lands there only as the folder is
    /// entered, so a title made before it would find nothing.
    #[cfg(feature = "window")]
    #[test]
    fn the_window_enters_the_app_folder_before_the_title_looks() {
        use crate::runs::testutil::{Dir, saving, small_city};
        use crate::screens::testutil::buffer_text;
        let app = Dir::new("app-folder");
        let (mut run, _) = saving(&app, small_city());
        run.saves = Some(app.0.join("runs"));
        let enter = || {
            assert!(crate::runs::when::save_now(&mut run));
            Ok(Some(app.0.clone()))
        };
        let (mut splash, mut st) = window_start(false, enter).unwrap();
        assert_eq!(
            st.saves,
            Some(app.0.join("runs")),
            "runs/ in the app folder"
        );
        // The splash first, then the title it hands over to.
        let mut title = loop {
            st.tick += 1;
            if let crate::app::ScreenCmd::Switch { to, .. } = splash.update(&mut st, &[]) {
                break to;
            }
            assert!(st.tick < 1_000, "the splash hands over");
        };
        for _ in 0..90 {
            st.tick += 1;
            title.update(&mut st, &[]);
        }
        let text = buffer_text(&crate::screens::shots::draw_stack(
            &st,
            &[title.as_ref()],
            (120, 40),
        ));
        assert!(text.contains("▸ CONTINUE"), "{text}");
    }
}
