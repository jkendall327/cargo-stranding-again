use std::env;
use std::process::{Command, ExitCode};

/// Small app for coding agents to run multiple cargo verification steps against the codebase.
fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("verify") => run_steps(&[
            ("format", &["fmt", "--check"][..]),
            ("clippy", &["clippy", "--all-targets"][..]),
            ("tests", &["test"][..]),
        ]),
        Some(command) => {
            eprintln!("unknown xtask command: {command}");
            print_usage();
            ExitCode::FAILURE
        }
        None => {
            print_usage();
            ExitCode::FAILURE
        }
    }
}

fn run_steps(steps: &[(&str, &[&str])]) -> ExitCode {
    for (label, args) in steps {
        println!("==> cargo {}", args.join(" "));
        let status = Command::new("cargo")
            .args(*args)
            .status()
            .unwrap_or_else(|error| panic!("failed to run {label}: {error}"));

        if !status.success() {
            return ExitCode::from(status.code().unwrap_or(1) as u8);
        }
    }

    ExitCode::SUCCESS
}

fn print_usage() {
    eprintln!("usage: cargo run --bin xtask -- verify");
}
