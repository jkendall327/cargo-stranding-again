use std::env;
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cargo_stranding_again::{
    headless::{
        run_scenario, ExpectationFailure, HeadlessCommand, HeadlessGame, HeadlessScenario,
        HeadlessScenarioError, HeadlessScenarioReport, HeadlessSnapshot,
    },
    logging,
};

const DEFAULT_SCENARIO_DIR: &str = "scenarios/headless";

fn main() -> ExitCode {
    logging::init();

    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() {
        tracing::info!("headless invoked without commands");
        print_usage();
        return ExitCode::SUCCESS;
    }

    match args[0].as_str() {
        "--scenario" | "scenario" => {
            let Some(path) = args.get(1) else {
                eprintln!("missing scenario path");
                print_usage();
                return ExitCode::from(2);
            };
            run_scenario_file(Path::new(path))
        }
        "--scenario-dir" | "scenarios" => {
            let path = args
                .get(1)
                .map_or_else(|| PathBuf::from(DEFAULT_SCENARIO_DIR), PathBuf::from);
            run_scenario_dir(&path)
        }
        "all" => run_scenario_dir(Path::new(DEFAULT_SCENARIO_DIR)),
        _ => run_command_args(args),
    }
}

fn run_command_args(args: Vec<String>) -> ExitCode {
    tracing::info!("starting headless run");
    let mut game = HeadlessGame::new();
    print_snapshot("start", game.snapshot());

    let mut args = args.into_iter().peekable();
    while let Some(token) = args.next() {
        let command_token = if token == "move" {
            match args.next() {
                Some(direction) => direction,
                None => {
                    tracing::warn!("headless command parse failed: missing direction after move");
                    eprintln!("missing direction after 'move'");
                    return ExitCode::from(2);
                }
            }
        } else {
            token
        };

        let Some(command) = HeadlessCommand::from_token(&command_token) else {
            tracing::warn!(command = command_token, "unknown headless command");
            eprintln!("unknown headless command: {command_token}");
            print_usage();
            return ExitCode::from(2);
        };

        match command {
            HeadlessCommand::Action(action) => {
                tracing::debug!(?action, "running headless action");
                game.step(action);
                print_snapshot(&format!("{action:?}"), game.snapshot());
            }
        }
    }

    tracing::info!("finished headless run");
    ExitCode::SUCCESS
}

fn run_scenario_file(path: &Path) -> ExitCode {
    match load_scenario(path).and_then(|scenario| run_loaded_scenario(path, &scenario)) {
        Ok(report) => print_scenario_report(path, &report),
        Err(error) => {
            eprintln!("{}: {error}", path.display());
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}

fn run_scenario_dir(path: &Path) -> ExitCode {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) => {
            eprintln!("{}: {error}", path.display());
            return ExitCode::from(2);
        }
    };

    let mut paths = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                eprintln!("{}: {error}", path.display());
                return ExitCode::from(2);
            }
        };
        let entry_path = entry.path();
        if entry_path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            paths.push(entry_path);
        }
    }
    paths.sort();

    if paths.is_empty() {
        eprintln!("{}: no .json scenarios found", path.display());
        return ExitCode::from(2);
    }

    let mut failed = false;
    for scenario_path in paths {
        let report = match load_scenario(&scenario_path)
            .and_then(|scenario| run_loaded_scenario(&scenario_path, &scenario))
        {
            Ok(report) => report,
            Err(error) => {
                eprintln!("{}: {error}", scenario_path.display());
                failed = true;
                continue;
            }
        };

        print_scenario_report(&scenario_path, &report);
        failed |= !report.failures.is_empty();
    }

    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn load_scenario(path: &Path) -> Result<HeadlessScenario, String> {
    let contents = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&contents).map_err(|error| error.to_string())
}

fn run_loaded_scenario(
    path: &Path,
    scenario: &HeadlessScenario,
) -> Result<HeadlessScenarioReport, String> {
    run_scenario(scenario).map_err(|error| scenario_error_message(path, error))
}

fn scenario_error_message(path: &Path, error: HeadlessScenarioError) -> String {
    match error {
        HeadlessScenarioError::UnknownCommand { command } => {
            format!("unknown command in scenario: {command}")
        }
        HeadlessScenarioError::NoPlayer => {
            format!("{} produced no player snapshot", path.display())
        }
    }
}

fn print_scenario_report(path: &Path, report: &HeadlessScenarioReport) {
    if report.failures.is_empty() {
        println!(
            "PASS {} ({}) final: {}",
            report.name,
            path.display(),
            format_snapshot(report.final_snapshot)
        );
        if report.show_view {
            print_ascii_view(&report.final_view);
        }
        return;
    }

    println!(
        "FAIL {} ({}) final: {}",
        report.name,
        path.display(),
        format_snapshot(report.final_snapshot)
    );
    for failure in &report.failures {
        print_expectation_failure(failure);
    }
    print_ascii_view(&report.final_view);
}

fn print_expectation_failure(failure: &ExpectationFailure) {
    println!(
        "  expected {}={} actual={}",
        failure.field, failure.expected, failure.actual
    );
}

fn print_ascii_view(view: &str) {
    println!("  view:");
    for line in view.lines() {
        println!("    {line}");
    }
}

fn print_snapshot(label: &str, snapshot: Option<HeadlessSnapshot>) {
    match snapshot {
        Some(snapshot) => println!("{label}: {}", format_snapshot(snapshot)),
        None => println!("{label}: no player found"),
    }
}

fn format_snapshot(snapshot: HeadlessSnapshot) -> String {
    let mut output = String::new();
    let _ = write!(
        output,
        "turn={} time={} player=({}, {}) stamina={:.1} mode={} cargo={:.1} parcels=loose:{},assigned:{},carried:{} delivered={}",
        snapshot.turn,
        snapshot.timeline,
        snapshot.player_position.x,
        snapshot.player_position.y,
        snapshot.player_stamina,
        snapshot.player_movement_mode.label(),
        snapshot.player_cargo,
        snapshot.loose_parcels,
        snapshot.assigned_parcels,
        snapshot.carried_parcels,
        snapshot.delivered_parcels
    );
    output
}

fn print_usage() {
    println!("Usage: cargo run --bin headless -- <commands>");
    println!("       cargo run --bin headless -- --scenario <path.json>");
    println!("       cargo run --bin headless -- --scenario-dir <dir>");
    println!("       cargo run --bin headless -- all");
    println!("Commands: north south east west wait pickup mode");
    println!("Also accepted: n s e w, up down left right, move <direction>");
    println!("Example: cargo run --bin headless -- mode move east wait pickup");
}
