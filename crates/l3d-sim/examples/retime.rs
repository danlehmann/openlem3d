//! Re-times the stored solutions that no longer save enough: shifts one
//! assignment, or it and all after it, by up to SPREAD ticks either way,
//! keeping each shift that saves more, until the solution saves enough or
//! no shift helps, and prints the new assignment lists:
//! `cargo run --release -p l3d-sim --example retime -- [SPREAD] [PASSES] [THREADS]`.

use l3d_formats::gamedata::{GameData, locate_data_dir};
use l3d_sim::demos::{Plan, RateStep, SOLUTIONS, Step, play, play_steps};

fn main() {
    let a: Vec<i64> = std::env::args()
        .skip(1)
        .map(|s| s.parse().expect("numbers"))
        .collect();
    let arg = |i: usize, d: i64| a.get(i).copied().unwrap_or(d);
    let (spread, passes, threads) = (arg(0, 40), arg(1, 4), arg(2, 4) as usize);
    let mut data = GameData::open(&locate_data_dir(None)).expect("open game data");
    let mut jobs = Vec::new();
    for (k, s) in SOLUTIONS.iter().enumerate() {
        let level = data.level(s.level).expect("level");
        let blocks = data.blocks(s.level).expect("blocks");
        let need = s.need(&level);
        if play(s, &level, &blocks).counts.saved < need {
            jobs.push((k, level, blocks, need));
        }
    }
    let next = std::sync::Mutex::new(jobs.into_iter());
    // Levels run one per worker; each worker tries its shifts on `inner`
    // threads of its own.
    let inner = (threads / 3).max(1);
    std::thread::scope(|scope| {
        for _ in 0..threads.div_ceil(inner) {
            scope.spawn(|| {
                loop {
                    let job = next.lock().unwrap().next();
                    let Some((k, level, blocks, need)) = job else {
                        break;
                    };
                    let s = &SOLUTIONS[k];
                    let (rates, mut steps): (&[RateStep], Vec<Step>) = match s.plan {
                        Plan::At(st) => (&[], st.to_vec()),
                        Plan::Rated(r, st) => (r, st.to_vec()),
                        Plan::Each(_) => continue,
                    };
                    // Saves first, then assignments taken, then fewer dead: a
                    // route that saves none yet still gets nearer.
                    let saved = |steps: &[Step]| {
                        let sim = play_steps(rates, steps, &level, &blocks);
                        (sim.counts.saved, sim.log.len(), -(sim.counts.dead as i64))
                    };
                    let mut best = saved(&steps);
                    'search: for _ in 0..passes {
                        let mut improved = false;
                        for j in 0..steps.len() {
                            let t0 = steps[j].0 as i64;
                            let trials: Vec<Vec<Step>> = (1..=spread)
                                .flat_map(|d| [-d, d])
                                .flat_map(|d| [(d, false), (d, true)])
                                .filter(|&(d, _)| t0 + d >= 0)
                                .map(|(d, rest)| {
                                    let mut trial = steps.clone();
                                    for (m, s) in trial.iter_mut().enumerate() {
                                        if m == j || (rest && m > j) {
                                            s.0 = (s.0 as i64 + d).max(0) as u64;
                                        }
                                    }
                                    trial.sort_by_key(|s| s.0);
                                    trial
                                })
                                .collect();
                            // Saves per trial, in trial order, so the pick
                            // is the same however the threads interleave.
                            let chunk = trials.len().div_ceil(inner).max(1);
                            let counts: Vec<(u32, usize, i64)> = std::thread::scope(|s| {
                                let parts: Vec<_> = trials
                                    .chunks(chunk)
                                    .map(|c| {
                                        s.spawn(move || {
                                            c.iter().map(|t| saved(t)).collect::<Vec<_>>()
                                        })
                                    })
                                    .collect();
                                parts.into_iter().flat_map(|p| p.join().unwrap()).collect()
                            });
                            let mut found: Option<((u32, usize, i64), usize)> = None;
                            for (i, &n) in counts.iter().enumerate() {
                                if n > best && found.is_none_or(|(m, _)| n > m) {
                                    found = Some((n, i));
                                }
                            }
                            if let Some((n, i)) = found {
                                best = n;
                                steps = trials[i].clone();
                                improved = true;
                                if best.0 >= need {
                                    break 'search;
                                }
                            }
                        }
                        if !improved {
                            break;
                        }
                    }
                    let mark = if best.0 >= need { "SOLVED" } else { "short" };
                    println!(
                        "solution {k} LEVEL.{:03}: {} of {need} {mark}\n{steps:?}",
                        s.level, best.0
                    );
                }
            });
        }
    });
}
