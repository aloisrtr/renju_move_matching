use std::{path::Path, sync::Arc};

use db::load_database;
use interface::Interface;
use move_matching::MoveMatching;
use plot::{plot_rating_distribution, plot_results, save_results, Performance};
use protocol::Engine;

pub mod db;
pub mod interface;
pub mod move_matching;
pub mod plot;
pub mod protocol;

pub fn move_matching_performance<P: AsRef<Path>>(
    name: &str,
    engine_command: &str,
    database_path: P,
    threads: u32,
    games_count: Option<usize>,
    move_time: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let database_name = database_path
        .as_ref()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap();

    let games = load_database(database_path.as_ref()).unwrap();
    let games = Vec::from(if let Some(i) = games_count {
        &games[0..i]
    } else {
        &games
    });
    log::info!("Loaded database {database_name}");
    log::info!("Saving rating distribution to {name}_rating_distribution.png");
    plot_rating_distribution(format!("{name}_rating_distribution.png"), &games);

    // Open engines
    let checkpoint_path = format!("{name}.csv");
    let matching = Arc::new(if Path::new(&checkpoint_path).exists() {
        MoveMatching::from_checkpoint(&games, &checkpoint_path)
    } else {
        MoveMatching::from_games(&games)
    });

    let terminal = ratatui::init();
    let interface = Interface::new(name.to_string(), matching.clone());

    let interface_handle = { std::thread::spawn(move || interface.render_loop(terminal)) };
    let _workers_handle = (0..(threads as usize).min(games_count.unwrap_or(threads as usize)))
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

    let _ = interface_handle.join().unwrap();
    ratatui::restore();

    log::info!("Saving final results");
    save_results(
        format!("{name}.csv"),
        Performance {
            name: &name,
            matches: matching.snapshot(),
        },
    );
    plot_results(
        format!("{name}.png"),
        std::iter::once(Performance {
            name: &name,
            matches: matching.snapshot(),
        }),
    );

    Ok(())
}
