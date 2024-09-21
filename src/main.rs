use std::path::PathBuf;

use clap::{command, Parser, Subcommand};
use renju_move_matching::{
    move_matching_performance,
    plot::{plot_results, Performance},
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

        #[arg(short, long)]
        threads: Option<u32>,

        #[arg(short, long)]
        games: Option<usize>,

        #[arg(short, long)]
        move_time: Option<u32>,
    },
    Plot {
        output_path: PathBuf,

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
        Command::Plot {
            output_path,
            names,
            perfs,
        } => {
            if names.len() != perfs.len() {
                panic!()
            }
            let perfs = names.iter().zip(perfs.iter()).map(|(name, perf_path)| {
                let csv = csv::Reader::from_path(&perf_path)
                    .unwrap()
                    .into_deserialize();
                Performance {
                    name,
                    matches: csv.filter_map(|e| e.ok()),
                }
            });
            plot_results(output_path, perfs)
        }
        Command::Match {
            name,
            engine_command,
            database_path,
            threads,
            games,
            move_time,
        } => {
            move_matching_performance(
                &name,
                &engine_command,
                database_path,
                threads.unwrap_or(1),
                games,
                move_time.unwrap_or(5000),
            )
            .unwrap();
        }
    }
}
