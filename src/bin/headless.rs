use std::env;
use std::fmt::Write;
use std::process::ExitCode;

use cargo_stranding_again::{
    headless::{HeadlessCommand, HeadlessGame, HeadlessSnapshot},
    logging,
};

fn main() -> ExitCode {
    logging::init();

    let mut args = env::args().skip(1).peekable();
    if args.peek().is_none() {
        tracing::info!("headless invoked without commands");
        print_usage();
        return ExitCode::SUCCESS;
    }

    tracing::info!("starting headless run");
    let mut game = HeadlessGame::new();
    print_snapshot("start", game.snapshot());

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
        "turn={} time={} player=({}, {}) stamina={:.1} cargo={:.1} parcels=loose:{},assigned:{},carried:{} delivered={}",
        snapshot.turn,
        snapshot.timeline,
        snapshot.player_position.x,
        snapshot.player_position.y,
        snapshot.player_stamina,
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
    println!("Commands: north south east west wait pickup sprint");
    println!("Also accepted: n s e w, up down left right, move <direction>");
    println!("Example: cargo run --bin headless -- move east wait pickup");
}
