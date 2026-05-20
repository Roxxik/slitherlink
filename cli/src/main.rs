use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use slitherlink_core::{
    generate_seeded, generate_with, is_solved, propagate, LotteryRow, MetropolisBuilder,
    ProposalAction, Puzzle, RegionAlgo, WalkBuilder,
};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let prog = args.first().map(String::as_str).unwrap_or("slitherlink");

    let mut propagate_mode = false;
    let mut generate_spec: Option<String> = None;
    let mut show_region_spec: Option<String> = None;
    let mut seed: Option<u64> = None;
    // None means "let the seed pick the generator", matching the UI (generate_seeded).
    let mut algo: Option<RegionAlgo> = None;
    let mut path: Option<String> = None;
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--propagate" => propagate_mode = true,
            "--generate" => {
                let Some(spec) = iter.next() else {
                    eprintln!("--generate requires a WxH argument");
                    return ExitCode::from(2);
                };
                generate_spec = Some(spec.clone());
            }
            "--show-region" => {
                let Some(spec) = iter.next() else {
                    eprintln!("--show-region requires a WxH argument");
                    return ExitCode::from(2);
                };
                show_region_spec = Some(spec.clone());
            }
            "--algo" => {
                let Some(v) = iter.next() else {
                    eprintln!("--algo requires a value (walk|metropolis)");
                    return ExitCode::from(2);
                };
                algo = Some(match v.as_str() {
                    "walk" => RegionAlgo::Walk,
                    "metropolis" => RegionAlgo::Metropolis,
                    other => {
                        eprintln!("unknown algo {other:?} (expected walk|metropolis)");
                        return ExitCode::from(2);
                    }
                });
            }
            "--seed" => {
                let Some(v) = iter.next() else {
                    eprintln!("--seed requires a value");
                    return ExitCode::from(2);
                };
                seed = match v.parse() {
                    Ok(n) => Some(n),
                    Err(_) => {
                        eprintln!("invalid seed: {v}");
                        return ExitCode::from(2);
                    }
                };
            }
            other if other.starts_with("--") => {
                eprintln!("unknown flag: {other}");
                return ExitCode::from(2);
            }
            other => path = Some(other.to_string()),
        }
    }

    if let Some(spec) = show_region_spec {
        if generate_spec.is_some() || path.is_some() {
            eprintln!("--show-region is mutually exclusive with --generate / puzzle file");
            return ExitCode::from(2);
        }
        let Some((w, h)) = parse_size(&spec) else {
            eprintln!("invalid --show-region spec: {spec:?} (expected WxH)");
            return ExitCode::from(2);
        };
        let chosen = seed.unwrap_or_else(random_seed);
        eprintln!("seed: {chosen}");
        return match algo.unwrap_or(RegionAlgo::Walk) {
            RegionAlgo::Walk => show_walk(w, h, chosen),
            RegionAlgo::Metropolis => show_metropolis(w, h, chosen),
        };
    }

    let (puzzle, generated) = match (generate_spec, path) {
        (Some(spec), None) => {
            let Some((w, h)) = parse_size(&spec) else {
                eprintln!("invalid --generate spec: {spec:?} (expected WxH)");
                return ExitCode::from(2);
            };
            let chosen = seed.unwrap_or_else(random_seed);
            eprintln!("seed: {chosen}");
            // No --algo: reproduce exactly what the UI shows (seed picks the generator).
            let puzzle = match algo {
                Some(a) => generate_with(w, h, chosen, a),
                None => generate_seeded(w, h, chosen),
            };
            if !is_solved(&puzzle, &propagate(&puzzle)) {
                eprintln!("seed {chosen} did not produce a propagate-solvable puzzle");
                return ExitCode::from(3);
            }
            (puzzle, true)
        }
        (None, Some(p)) => match load_puzzle(&p) {
            Ok(puz) => (puz, false),
            Err(code) => return code,
        },
        (Some(_), Some(_)) => {
            eprintln!("--generate and a puzzle file are mutually exclusive");
            return ExitCode::from(2);
        }
        (None, None) => {
            eprintln!(
                "usage: {prog} [--propagate] <puzzle-file>\n       {prog} --generate WxH [--seed N] [--algo walk|metropolis] [--propagate]\n       {prog} --show-region WxH [--seed N] [--algo walk|metropolis]"
            );
            return ExitCode::from(2);
        }
    };

    if propagate_mode {
        let solution = propagate(&puzzle);
        print!("{}", puzzle.overlay(&solution));
    } else if generated {
        print!("{}", puzzle.to_storage_string());
    } else {
        print!("{puzzle}");
    }
    ExitCode::SUCCESS
}

fn parse_size(spec: &str) -> Option<(usize, usize)> {
    let (w, h) = spec.split_once('x')?;
    Some((w.parse().ok()?, h.parse().ok()?))
}

fn random_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0xCAFE_BABE_DEAD_BEEF)
}

fn show_walk(w: usize, h: usize, seed: u64) -> ExitCode {
    let mut builder = WalkBuilder::new(w, h, seed);
    print_walk(&mut builder);
    let stdin = io::stdin();
    let mut lock = stdin.lock();
    let mut line = String::new();
    while !builder.done() {
        eprint!("[Enter to step, q to quit] ");
        let _ = io::stderr().flush();
        line.clear();
        if lock.read_line(&mut line).is_err() {
            break;
        }
        if line.trim() == "q" {
            break;
        }
        if !builder.step() {
            eprintln!("(no further progress: stuck)");
            break;
        }
        print_walk(&mut builder);
    }
    ExitCode::SUCCESS
}

fn show_metropolis(w: usize, h: usize, seed: u64) -> ExitCode {
    let mut builder = MetropolisBuilder::new(w, h, seed);
    print_metropolis(&builder, 0, 0);
    let stdin = io::stdin();
    let mut lock = stdin.lock();
    let mut line = String::new();
    while !builder.done() {
        eprint!("[Enter to advance to next add, q to quit] ");
        let _ = io::stderr().flush();
        line.clear();
        if lock.read_line(&mut line).is_err() {
            break;
        }
        if line.trim() == "q" {
            break;
        }
        let mut skipped_rejects = 0usize;
        let mut skipped_removes = 0usize;
        let mut exhausted = false;
        loop {
            if !builder.step() {
                exhausted = true;
                break;
            }
            let p = builder.last_proposal().expect("step recorded a proposal");
            if p.accepted && p.action == ProposalAction::Add {
                break;
            }
            if p.accepted {
                skipped_removes += 1;
            } else {
                skipped_rejects += 1;
            }
        }
        print_metropolis(&builder, skipped_rejects, skipped_removes);
        if exhausted {
            eprintln!("(no further successful adds; iterations exhausted)");
            break;
        }
    }
    ExitCode::SUCCESS
}

fn print_walk(b: &mut WalkBuilder) {
    let w = b.width();
    let h = b.height();
    let total = w * h;
    let count = b.count();
    let tip = b.tip();
    let inside = b.inside().to_vec();
    let top = (0..w).any(|x| inside[x]);
    let bot = (0..w).any(|x| inside[(h - 1) * w + x]);
    let left = (0..h).any(|y| inside[y * w]);
    let right = (0..h).any(|y| inside[y * w + w - 1]);
    println!(
        "count {count}/{tgt} ({pct}%/{tpct}% of grid)  borders: {t}{b_}{l}{r}  tip: {tip:?}",
        tgt = b.target(),
        pct = count * 100 / total,
        tpct = b.target() * 100 / total,
        t = if top { "T" } else { "-" },
        b_ = if bot { "B" } else { "-" },
        l = if left { "L" } else { "-" },
        r = if right { "R" } else { "-" },
    );
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let ch = if Some((x, y)) == tip {
                '*'
            } else if inside[i] {
                'X'
            } else {
                '.'
            };
            print!("{ch} ");
        }
        println!();
    }
    if let Some(rows) = b.next_lottery() {
        let total_weight: u32 = rows.iter().map(|r| r.weight).sum();
        println!("lottery (total weight {total_weight}):");
        for row in rows {
            print_lottery_row(&row, total_weight);
        }
    }
    println!();
}

fn print_metropolis(b: &MetropolisBuilder, skipped_rejects: usize, skipped_removes: usize) {
    let w = b.width();
    let h = b.height();
    let total = w * h;
    let count = b.count();
    let inside = b.inside();
    let last = b.last_proposal();
    let highlight = last.map(|p| p.cell);
    let top = (0..w).any(|x| inside[x]);
    let bot = (0..w).any(|x| inside[(h - 1) * w + x]);
    let left = (0..h).any(|y| inside[y * w]);
    let right = (0..h).any(|y| inside[y * w + w - 1]);
    println!(
        "iter {it}/{maxit}  count {count}/{tgt} ({pct}%/{tpct}%)  borders: {t}{b_}{l}{r}",
        it = b.iteration(),
        maxit = b.max_iterations(),
        tgt = b.target(),
        pct = count * 100 / total,
        tpct = b.target() * 100 / total,
        t = if top { "T" } else { "-" },
        b_ = if bot { "B" } else { "-" },
        l = if left { "L" } else { "-" },
        r = if right { "R" } else { "-" },
    );
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let ch = if Some((x, y)) == highlight {
                '*'
            } else if inside[i] {
                'X'
            } else {
                '.'
            };
            print!("{ch} ");
        }
        println!();
    }
    match last {
        Some(p) => {
            let action = match p.action {
                ProposalAction::Add => "add",
                ProposalAction::Remove => "remove",
            };
            let verdict = if p.accepted { "ACCEPT" } else { "reject" };
            println!(
                "last: ({x},{y}) {action:>6} -> {verdict}  ({note})",
                x = p.cell.0,
                y = p.cell.1,
                note = p.note,
            );
        }
        None => println!("[no proposals yet]"),
    }
    if skipped_rejects > 0 || skipped_removes > 0 {
        println!(
            "skipped since last add: {skipped_rejects} rejects, {skipped_removes} accepted-removes",
        );
    }
    println!();
}

fn print_lottery_row(row: &LotteryRow, total: u32) {
    let pct = if total == 0 {
        0.0
    } else {
        100.0 * row.weight as f64 / total as f64
    };
    let target = match row.target {
        Some((x, y)) => format!("-> ({x},{y})"),
        None => String::from("        "),
    };
    println!(
        "  {label:>4}  w={w:<3} {pct:>5.1}%  {target}  {note}",
        label = row.label,
        w = row.weight,
        pct = pct,
        note = row.note,
    );
}

fn load_puzzle(path: &str) -> Result<Puzzle, ExitCode> {
    let contents = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return Err(ExitCode::from(1));
        }
    };
    contents.parse().map_err(|e| {
        eprintln!("parse error in {path}: {e}");
        ExitCode::from(1)
    })
}
