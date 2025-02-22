use std::path::PathBuf;

use clap::{command, Parser, Subcommand};
use renju_move_matching::{
    move_matching::MatchesSnapshot, move_matching_performance, plot::plot_results,
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Match {
        name: String,
        engine_command: String,
        database_path: PathBuf,
        games: u32,
        bucket_size: Option<u32>,

        #[arg(short, long)]
        threads: Option<u32>,

        #[arg(short, long)]
        move_time: Option<u32>,
    },
    Plot {
        name: String,

        #[arg(short, long, num_args = 1..)]
        names: Vec<String>,

        #[arg(short, long, num_args = 1..)]
        perfs: Vec<PathBuf>,
    },
}

fn main() {
    env_logger::init();

    let args = Arguments::parse();
    match args.command {
        Command::Plot { name, names, perfs } => {
            if names.len() != perfs.len() {
                panic!()
            }
            let perfs = names
                .iter()
                .zip(perfs.iter())
                .map(|(name, perf_path)| {
                    let csv = csv::ReaderBuilder::new()
                        .has_headers(true)
                        .from_path(perf_path)
                        .unwrap()
                        .into_deserialize();
                    let matches = csv
                        .filter_map(|e| e.ok())
                        .map(
                            |(
                                elo,
                                black_positions,
                                black_matches,
                                white_positions,
                                white_matches,
                            )| {
                                (
                                    elo,
                                    MatchesSnapshot {
                                        black_positions,
                                        black_matches,
                                        white_positions,
                                        white_matches,
                                    },
                                )
                            },
                        )
                        .collect();
                    (name.as_str(), matches)
                })
                .collect();
            plot_results(&name, perfs)
        }
        Command::Match {
            name,
            engine_command,
            database_path,
            threads,
            games,
            bucket_size,
            move_time,
        } => {
            move_matching_performance(
                &name,
                &engine_command,
                database_path,
                threads.unwrap_or(1),
                games,
                bucket_size.unwrap_or(200),
                move_time.unwrap_or(10000),
            )
            .unwrap();
        }
    }
}
