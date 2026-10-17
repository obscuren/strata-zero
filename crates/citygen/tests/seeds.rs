//! Determinism and invariant sweeps over many seeds.

use citygen::*;

fn sweep(preset: SizePreset, seeds: std::ops::Range<u64>) {
    for seed in seeds {
        let p = GenParams {
            seed,
            size: preset,
            water: match seed % 3 {
                0 => WaterMode::Bay,
                1 => WaterMode::River,
                _ => WaterMode::None,
            },
            districts: 4 + (seed % 9) as u8,
            ..Default::default()
        };
        if p.validate().is_err() {
            continue;
        }
        let a = Generator::run(p.clone()).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
        let b = Generator::run(p).unwrap();
        assert_eq!(
            postcard::to_allocvec(&a).unwrap(),
            postcard::to_allocvec(&b).unwrap(),
            "seed {seed} not deterministic"
        );
        a.validate()
            .unwrap_or_else(|v| panic!("seed {seed}: {v:?}"));
    }
}

#[test]
fn small_seeds() {
    sweep(SizePreset::Small, 0..50);
}

#[test]
fn medium_seeds() {
    sweep(SizePreset::Medium, 1000..1010);
}

/// Every level of every atrium megablock validates, on the Medium cities that have
/// one.
#[test]
fn medium_atrium_floors_validate() {
    let mut seen = 0;
    for seed in [1u64, 2, 8, 11] {
        let c = Generator::run(GenParams {
            seed,
            size: SizePreset::Medium,
            districts: 7,
            ..Default::default()
        })
        .unwrap();
        for b in &c.buildings {
            let Some(plan) = FloorPlan::new(&c, b.id).filter(|p| p.atrium.is_some()) else {
                continue;
            };
            seen += 1;
            for level in plan.levels() {
                let f = generate_floor(&c, &plan, level);
                validate_floor(&plan, &f)
                    .unwrap_or_else(|e| panic!("seed {seed} {:?} level {level}: {e}", b.id));
            }
        }
    }
    assert!(seen >= 4, "{seen} atrium megablocks");
}

#[test]
fn large_seeds() {
    sweep(SizePreset::Large, 2000..2003);
}

#[test]
#[ignore]
fn extended_seeds() {
    sweep(SizePreset::Small, 100..250);
    sweep(SizePreset::Medium, 3000..3050);
}

#[test]
fn medium_generates_under_two_seconds() {
    let t = std::time::Instant::now();
    Generator::run(GenParams {
        seed: 77,
        ..Default::default()
    })
    .unwrap();
    assert!(t.elapsed().as_secs_f32() < 2.0, "took {:?}", t.elapsed());
}
