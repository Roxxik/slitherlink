use std::env;
use std::fs;
use std::process::ExitCode;

use slitherlink_core::{propagate, Puzzle};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let mut propagate_mode = false;
    let mut path: Option<&str> = None;
    for arg in &args[1..] {
        match arg.as_str() {
            "--propagate" => propagate_mode = true,
            other if other.starts_with("--") => {
                eprintln!("unknown flag: {other}");
                return ExitCode::from(2);
            }
            other => path = Some(other),
        }
    }
    let Some(path) = path else {
        eprintln!("usage: {} [--propagate] <puzzle-file>", args.first().map(String::as_str).unwrap_or("slitherlink"));
        return ExitCode::from(2);
    };
    let contents = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            return ExitCode::from(1);
        }
    };
    let puzzle: Puzzle = match contents.parse() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("parse error in {path}: {e}");
            return ExitCode::from(1);
        }
    };
    if propagate_mode {
        let solution = propagate(&puzzle);
        print!("{}", puzzle.overlay(&solution));
    } else {
        print!("{puzzle}");
    }
    ExitCode::SUCCESS
}
