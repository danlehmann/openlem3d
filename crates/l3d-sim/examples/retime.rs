//! Re-times the stored solutions that no longer save enough: shifts one
//! assignment, or it and all after it, by up to SPREAD ticks either way,
//! keeping each shift that saves more, until the solution saves enough or
//! no shift helps, and prints the new assignment lists:
//! `cargo run --release -p l3d-sim --example retime -- [SPREAD] [PASSES] [THREADS]`.

use l3d_formats::gamedata::{GameData, locate_data_dir};
use l3d_sim::demos::{Plan, SOLUTIONS, Solution, play};

type Step = (u64, usize, u8, Option<bool>);

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
        let need = if s.saves > 0 {
            s.saves
        } else {
            level.save_requirement as u32
        };
        if play(s, &level, &blocks).counts.saved < need {
            jobs.push((k, level, blocks, need));
        }
    }
    let next = std::sync::Mutex::new(jobs.into_iter());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                while let Some((k, level, blocks, need)) = next.lock().unwrap().next() {
                    let s = &SOLUTIONS[k];
                    let (rates, mut steps): (&'static [(u64, i32)], Vec<Step>) = match s.plan {
                        Plan::At(st) => (&[], st.to_vec()),
                        Plan::Rated(r, st) => (r, st.to_vec()),
                        Plan::Each(_) => continue,
                    };
                    let saved = |steps: &[Step]| {
                        let steps: &'static [Step] = Box::leak(steps.to_vec().into_boxed_slice());
                        let plan = if rates.is_empty() {
                            Plan::At(steps)
                        } else {
                            Plan::Rated(rates, steps)
                        };
                        let t = Solution {
                            level: s.level,
                            plan,
                            saves: s.saves,
                        };
                        play(&t, &level, &blocks).counts.saved
                    };
                    let mut best = saved(&steps);
                    'search: for _ in 0..passes {
                        let mut improved = false;
                        for j in 0..steps.len() {
                            let t0 = steps[j].0 as i64;
                            let mut found = None;
                            let tries = (1..=spread)
                                .flat_map(|d| [-d, d])
                                .flat_map(|d| [(d, false), (d, true)]);
                            for (d, rest) in tries {
                                if t0 + d < 0 {
                                    continue;
                                }
                                let mut trial = steps.clone();
                                for (m, s) in trial.iter_mut().enumerate() {
                                    if m == j || (rest && m > j) {
                                        s.0 = (s.0 as i64 + d).max(0) as u64;
                                    }
                                }
                                trial.sort_by_key(|s| s.0);
                                let n = saved(&trial);
                                if n > best && found.as_ref().is_none_or(|(m, _)| n > *m) {
                                    found = Some((n, trial));
                                }
                            }
                            if let Some((n, trial)) = found {
                                best = n;
                                steps = trial;
                                improved = true;
                                if best >= need {
                                    break 'search;
                                }
                            }
                        }
                        if !improved {
                            break;
                        }
                    }
                    let mark = if best >= need { "SOLVED" } else { "short" };
                    println!(
                        "solution {k} LEVEL.{:03}: {best} of {need} {mark}\n{steps:?}",
                        s.level
                    );
                }
            });
        }
    });
}
