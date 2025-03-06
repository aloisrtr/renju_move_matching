use std::{path::Path, sync::Arc};

use db::load_database;
use game_graph::PositionGraph;
use interface::Interface;
use move_matching::MoveMatching;
use plot::{plot_rating_distribution, plot_results, save_results};
use protocol::Engine;

pub mod db;
pub mod game_graph;
pub mod interface;
pub mod move_matching;
pub mod plot;
pub mod protocol;

pub fn move_matching_performance<P: AsRef<Path>>(
    name: &str,
    engine_command: &str,
    database_path: P,
    threads: u32,
    games_per_bucket: u32,
    bucket_size: u32,
    move_time: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let database_name = database_path
        .as_ref()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();

    let buckets = load_database(database_path.as_ref(), bucket_size, games_per_bucket).unwrap();
    log::info!("Loaded database {database_name}");
    log::info!("Saving rating distribution to rating_distribution.png");
    plot_rating_distribution(name, &buckets);

    // Open engines
    let matching = Arc::new(MoveMatching::from_games(&buckets, 100));

    let terminal = ratatui::init();
    let min_bracket = buckets.iter().map(|b| b.elo).min().unwrap();
    let max_bracket = buckets.iter().map(|b| b.elo).max().unwrap();
    let self_move_matching = buckets
        .iter()
        .map(|b| {
            (
                b.elo as f64,
                PositionGraph::from_games(&b.games).self_move_matching(false),
            )
        })
        .collect();
    let interface = Interface::new(
        name.to_string(),
        min_bracket,
        max_bracket,
        bucket_size,
        matching.clone(),
        self_move_matching,
    );

    let interface_handle = { std::thread::spawn(move || interface.render_loop(terminal)) };
    let _workers_handle = (0..(threads as usize).min(games_per_bucket as usize))
        .map(|i| {
            let matching = matching.clone();
            let engine_command = engine_command.to_string();
            std::thread::spawn(move || {
                let mut engine =
                    Engine::open_engine(i as usize, &engine_command, move_time).unwrap();
                log::trace!("thread {i} waiting for next task");
                while let Some(mut task) = matching.get_next_task() {
                    if let Err(e) = task.match_challenge(&mut engine) {
                        log::error!("[{i}] Error when matching: {e:?}")
                    }
                    log::info!("[{i}] Completed a move matching task");
                }
                engine.close_engine()
            })
        })
        .collect::<Vec<_>>();

    if let Err(e) = interface_handle.join().unwrap() {
        log::error!("Error: interface failed with {e:?}")
    }
    ratatui::restore();

    log::info!("Saving final results");
    save_results(name, matching.snapshot());
    plot_results(name, vec![(name, matching.snapshot())]);

    Ok(())
}
