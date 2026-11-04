//! THROWAWAY (tiles preview spike): a generated world as plain text for the Python mock.
use citygen::overworld::{Overworld, WorldParams};
use std::io::Write;

fn main() {
    let seed: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let w = Overworld::generate(WorldParams { seed, ..Default::default() }).unwrap();
    let mut o = std::io::BufWriter::new(std::io::stdout());
    writeln!(o, "world {} {} {}", w.seed, w.size.w, w.size.h).unwrap();
    for (p, t) in w.tiles.iter() {
        let (c, d, th) = t.urban.map_or((-1i32, 0u8, String::from("-")), |u| (u.city as i32, u.density, format!("{:?}", u.theme)));
        writeln!(o, "t {} {} {:?} {:?} {} {} {} {} {} {}", p.x, p.y, t.water, t.ground, t.elevation, t.road, t.river, c, d, th).unwrap();
    }
    for c in &w.cities {
        writeln!(o, "c {} {} {} {} {}", c.centre.x, c.centre.y, c.size, c.home, c.name).unwrap();
    }
    for r in &w.routes {
        let path: Vec<String> = r.path.iter().map(|p| format!("{},{}", p.x, p.y)).collect();
        writeln!(o, "r {}|{}", r.name, path.join(" ")).unwrap();
    }
    for r in &w.rivers {
        let path: Vec<String> = r.iter().map(|p| format!("{},{}", p.x, p.y)).collect();
        writeln!(o, "v {}", path.join(" ")).unwrap();
    }
    for p in &w.pois {
        writeln!(o, "p {} {} {:?} {}", p.at.x, p.at.y, p.kind, p.name).unwrap();
    }
    for r in &w.regions {
        writeln!(o, "g {} {} {:?} {}", r.at.x, r.at.y, r.kind, r.name).unwrap();
    }
}
