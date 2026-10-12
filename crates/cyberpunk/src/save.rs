//! The `.city` file format: magic, version, header, postcard body.

use anyhow::{Context, Result, bail};
use citygen::{City, Size};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const MAGIC: &[u8; 4] = b"CITY";
pub const FORMAT_VERSION: u16 = 2;
/// Versions whose header this build can read (for listing); only `FORMAT_VERSION` loads.
const READABLE_VERSIONS: [u16; 2] = [1, 2];

#[derive(Debug, Serialize, Deserialize)]
struct Header {
    name: String,
    seed: u64,
    w: u32,
    h: u32,
}

/// What the file list shows for one saved city.
#[derive(Clone, Debug)]
pub struct CityFileInfo {
    pub path: PathBuf,
    pub name: String,
    pub seed: u64,
    pub size: Size,
    pub modified: SystemTime,
}

/// Where cities are saved by default.
pub fn default_dir() -> PathBuf {
    PathBuf::from("cities")
}

pub fn save_city(path: &Path, city: &City) -> Result<()> {
    let header = Header {
        name: city.name.clone(),
        seed: city.seed,
        w: city.size.w,
        h: city.size.h,
    };
    let header_bytes = postcard::to_allocvec(&header)?;
    let body = postcard::to_allocvec(city)?;
    let mut out = Vec::with_capacity(10 + header_bytes.len() + body.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&(header_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&header_bytes);
    out.extend_from_slice(&body);
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(path, out).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Splits a file into (header, body) after checking magic and version.
fn split<'a>(bytes: &'a [u8], path: &Path) -> Result<(Header, u16, &'a [u8])> {
    if bytes.len() < 10 || &bytes[0..4] != MAGIC {
        bail!("{} is not a .city file", path.display());
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if !READABLE_VERSIONS.contains(&version) {
        bail!(
            "{} is .city version {version}; this build reads version {FORMAT_VERSION}",
            path.display()
        );
    }
    let header_len = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
    let header_end = 10 + header_len;
    if bytes.len() < header_end {
        bail!("{} is truncated", path.display());
    }
    let header: Header = postcard::from_bytes(&bytes[10..header_end])
        .with_context(|| format!("{}: corrupt header", path.display()))?;
    Ok((header, version, &bytes[header_end..]))
}

pub fn load_city(path: &Path) -> Result<City> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let (header, version, body) = split(&bytes, path)?;
    if version != FORMAT_VERSION {
        bail!(
            "{} was saved by an older version ({version}); regenerate it from seed {}",
            path.display(),
            header.seed
        );
    }
    let city: City = postcard::from_bytes(body)
        .with_context(|| format!("{}: corrupt or truncated city data", path.display()))?;
    if (header.w, header.h) != (city.size.w, city.size.h) {
        bail!(
            "{}: header says {}x{} but the city is {}x{}",
            path.display(),
            header.w,
            header.h,
            city.size.w,
            city.size.h
        );
    }
    let strata_ok = city.strata.len() == citygen::StratumKind::ALL.len()
        && city
            .strata
            .iter()
            .all(|s| s.tiles.size() == city.size && s.light.size() == city.size);
    if !strata_ok {
        bail!("{}: strata do not match the city size", path.display());
    }
    if let Err(violations) = city.validate() {
        let first: Vec<String> = violations.iter().take(3).map(|v| v.to_string()).collect();
        bail!(
            "{}: city violates its invariants ({} problems, e.g. {})",
            path.display(),
            violations.len(),
            first.join("; ")
        );
    }
    Ok(city)
}

/// Lists `.city` files in `dir`, newest first. A missing directory lists as empty.
pub fn list_cities(dir: &Path) -> Result<Vec<CityFileInfo>> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(out),
        Err(e) => return Err(e).with_context(|| format!("listing {}", dir.display())),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("city") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok((header, _, _)) = split(&bytes, &path) else {
            continue;
        };
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        out.push(CityFileInfo {
            path,
            name: header.name,
            seed: header.seed,
            size: Size {
                w: header.w,
                h: header.h,
            },
            modified,
        });
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.path.cmp(&b.path)));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, SizePreset};

    fn tiny() -> citygen::City {
        Generator::run(GenParams {
            seed: 5,
            size: SizePreset::Custom { w: 80, h: 40 },
            districts: 4,
            ..Default::default()
        })
        .unwrap()
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("city-test-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn roundtrip_and_header() {
        let c = tiny();
        let dir = tmp("rt");
        let path = dir.join("a.city");
        save_city(&path, &c).unwrap();
        let back = load_city(&path).unwrap();
        assert_eq!(
            postcard::to_allocvec(&c).unwrap(),
            postcard::to_allocvec(&back).unwrap()
        );
        let list = list_cities(&dir).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].seed, 5);
        assert_eq!(list[0].name, c.name);
        assert_eq!(list[0].size.w, 80);
        assert!(list_cities(&dir.join("missing")).unwrap().is_empty());
    }

    #[test]
    fn rejects_structurally_inconsistent_cities() {
        let dir = tmp("inconsistent");
        let p = dir.join("x.city");
        // Header disagrees with the body.
        let c = tiny();
        save_city(&p, &c).unwrap();
        let mut bytes = std::fs::read(&p).unwrap();
        let header_len = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
        let mut header: Header = postcard::from_bytes(&bytes[10..10 + header_len]).unwrap();
        header.w = 96;
        let hb = postcard::to_allocvec(&header).unwrap();
        let body = bytes.split_off(10 + header_len);
        bytes.truncate(6);
        bytes.extend_from_slice(&(hb.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&hb);
        bytes.extend_from_slice(&body);
        std::fs::write(&p, &bytes).unwrap();
        let err = load_city(&p).unwrap_err().to_string();
        assert!(err.contains("header"), "{err}");
        // Body violates the city invariants (tiles reference districts that do not exist).
        let mut broken = tiny();
        broken.districts.clear();
        save_city(&p, &broken).unwrap();
        let err = load_city(&p).unwrap_err().to_string();
        assert!(
            err.contains("invariant") || err.contains("missing"),
            "{err}"
        );
        // A grid whose data length disagrees with its size never gets past deserialization.
        let mut odd = tiny();
        odd.strata.pop();
        save_city(&p, &odd).unwrap();
        assert!(load_city(&p).is_err());
    }

    #[test]
    fn rejects_bad_magic_version_and_truncation() {
        let dir = tmp("bad");
        let p = dir.join("x.city");
        std::fs::write(&p, b"NOPE\x01\x00").unwrap();
        assert!(
            load_city(&p)
                .unwrap_err()
                .to_string()
                .contains("not a .city file")
        );
        let mut bytes = b"CITY".to_vec();
        bytes.extend_from_slice(&99u16.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        std::fs::write(&p, &bytes).unwrap();
        assert!(
            load_city(&p)
                .unwrap_err()
                .to_string()
                .contains("version 99")
        );
        let c = tiny();
        save_city(&p, &c).unwrap();
        let full = std::fs::read(&p).unwrap();
        std::fs::write(&p, &full[..full.len() / 2]).unwrap();
        assert!(load_city(&p).is_err());
        std::fs::write(&p, b"CI").unwrap();
        assert!(load_city(&p).is_err());
    }

    #[test]
    fn version_one_files_list_but_refuse_to_load() {
        let dir = std::env::temp_dir().join(format!("cyberpunk-v1-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let header = postcard::to_allocvec(&Header {
            name: "Old Town".into(),
            seed: 42,
            w: 80,
            h: 40,
        })
        .unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&(header.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(&[0u8; 8]);
        let path = dir.join("old.city");
        std::fs::write(&path, &bytes).unwrap();
        let listed = list_cities(&dir).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].seed, 42);
        let err = load_city(&path).unwrap_err().to_string();
        assert!(
            err.contains("older version") && err.contains("seed 42"),
            "{err}"
        );
        assert_eq!(FORMAT_VERSION, 2);
        std::fs::remove_dir_all(&dir).ok();
    }
}
