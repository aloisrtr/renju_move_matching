use std::{
    collections::HashMap,
    path::Path,
    sync::{
        atomic::{AtomicU32, AtomicU64, AtomicUsize},
        Arc,
    },
    time::{Duration, Instant},
};

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use plotters::{
    backend::BitMapBackend,
    chart::ChartBuilder,
    coord::{combinators::IntoLinspace, ranged1d::IntoSegmentedCoord},
    drawing::IntoDrawingArea,
    element::Rectangle,
    series::{Histogram, LineSeries},
    style::{Color, RGBColor, BLACK, BLUE, GREEN, RED, WHITE},
};

use crate::{
    db::{load_database, Game},
    protocol::{Command, Engine, EngineError, Response},
};

pub struct MoveMatchingTask<'a> {
    moves: &'a [(u8, u8)],
    idx: usize,
    black_matches: &'a (AtomicU32, AtomicU32),
    white_matches: &'a (AtomicU32, AtomicU32),
    completed: &'a AtomicUsize,
    completed_matches: &'a AtomicU64,
}
impl<'a> MoveMatchingTask<'a> {
    pub fn match_challenge(&mut self, engine: &mut Engine) -> Result<(), EngineError> {
        // Loop over moves and try to match them
        let mut black_matches = (0, 0);
        let mut white_matches = (0, 0);
        let mut result = Ok(());
        while self.idx < self.moves.len() - 2 {
            std::thread::sleep(Duration::from_millis(500));
            let matches = if self.idx % 2 == 0 {
                &mut black_matches
            } else {
                &mut white_matches
            };
            match engine.send_command(Command::Board(&self.moves[0..self.idx]))? {
                Response::Move((x, y)) => {
                    log::trace!("[{}] Move: {:?}", engine.id, (x, y));
                    if (x, y) == self.moves[self.idx] {
                        matches.0 += 1;
                    }
                }
                x => {
                    log::error!("Unexpected answer from engine {x:?}");
                    result = Err(EngineError::Error(
                        "Unexpected answer from engine {x:?}".to_string(),
                    ));
                    break;
                }
            }
            self.completed_matches
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            matches.1 += 1;
            self.idx += 1
        }
        self.black_matches
            .0
            .fetch_add(black_matches.0, std::sync::atomic::Ordering::Relaxed);
        self.white_matches
            .0
            .fetch_add(white_matches.0, std::sync::atomic::Ordering::Relaxed);
        self.black_matches
            .1
            .fetch_add(black_matches.1, std::sync::atomic::Ordering::Relaxed);
        self.white_matches
            .1
            .fetch_add(white_matches.1, std::sync::atomic::Ordering::Relaxed);
        self.completed
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        result
    }
}

pub struct MoveMatching {
    games: Vec<Game>,
    matches: HashMap<u64, (AtomicU32, AtomicU32)>,
    next: AtomicUsize,
    completed: AtomicUsize,
    completed_matches: AtomicU64,
}
impl MoveMatching {
    pub fn get_next_task<'a>(&'a self) -> Option<MoveMatchingTask<'a>> {
        let next = self.next.fetch_add(1, std::sync::atomic::Ordering::Acquire);
        if let Some(game) = self.games.get(next) {
            let black_matches = self.matches.get(&game.black_elo).unwrap();
            let white_matches = self.matches.get(&game.white_elo).unwrap();
            Some(MoveMatchingTask {
                moves: &game.moves,
                idx: 5,
                black_matches,
                white_matches,
                completed: &self.completed,
                completed_matches: &self.completed_matches,
            })
        } else {
            None
        }
    }
}

pub fn move_matching_performance<P: AsRef<Path>>(
    name: &str,
    engine_path: P,
    args: Vec<String>,
    database_path: P,
    threads: u32,
    games_count: Option<usize>,
    move_time: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let engine_name = engine_path.as_ref().file_name().unwrap().to_str().unwrap();
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
    let positions: u64 = games
        .iter()
        .map(|g| g.moves.len().saturating_sub(6) as u64)
        .sum();
    log::info!("Positions for move matching: {positions}");

    let rating_distribution_file =
        BitMapBackend::new("rating_distribution.png", (1024, 720)).into_drawing_area();
    rating_distribution_file.fill(&WHITE).unwrap();

    let mut rating_distribution_chart = ChartBuilder::on(&rating_distribution_file)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .margin(5)
        .caption("Renju ratings distribution", ("sans-serif", 50.0))
        .build_cartesian_2d((1400u32..2900u32).into_segmented(), 0u32..300u32)
        .unwrap();
    rating_distribution_chart
        .configure_mesh()
        .disable_x_mesh()
        .bold_line_style(WHITE.mix(0.3))
        .y_desc("Number of games")
        .x_desc("Rating")
        .axis_desc_style(("sans-serif", 15))
        .draw()
        .unwrap();
    rating_distribution_chart
        .draw_series(
            Histogram::vertical(&rating_distribution_chart)
                .style(RED.mix(0.5).filled())
                .data(
                    games
                        .iter()
                        .map(|g| (g.black_elo as u32, 1))
                        .chain(games.iter().map(|g| (g.white_elo as u32, 1))),
                ),
        )
        .unwrap();
    rating_distribution_file
        .present()
        .expect("Could not open file");

    // Open engines
    let matching = Arc::new(MoveMatching {
        matches: HashMap::from_iter(
            games
                .iter()
                .map(|g| (g.white_elo, (AtomicU32::new(0), AtomicU32::new(0))))
                .chain(
                    games
                        .iter()
                        .map(|g| (g.black_elo, (AtomicU32::new(0), AtomicU32::new(0)))),
                ),
        ),
        games,
        next: AtomicUsize::new(0),
        completed: AtomicUsize::new(0),
        completed_matches: AtomicU64::new(0),
    });

    let progress_handle = {
        let name = name.to_string();
        let matching = matching.clone();
        let engine_name = engine_name.to_string();
        let database_name = database_name.to_string();
        std::thread::spawn(move || {
            let bar = ProgressBar::new(positions as u64).with_style(
                ProgressStyle::default_bar()
                    .template("Processed games: {human_pos}/{human_len}\n{bard:40.cyan/blue}")
                    .unwrap()
                    .progress_chars("##-"),
            );
            bar.set_draw_target(ProgressDrawTarget::stdout());

            let mut last_save = Instant::now();
            loop {
                let completed = matching
                    .completed_matches
                    .load(std::sync::atomic::Ordering::Relaxed);
                if completed >= positions {
                    break;
                }
                bar.set_position(completed);

                if last_save.elapsed().as_secs() >= 900 {
                    last_save = Instant::now();
                    let perf = Performance {
                        name: &name,
                        matches: HashMap::from_iter(matching.matches.iter().map(
                            |(elo, (matches, total))| {
                                (
                                    *elo,
                                    (
                                        matches.load(std::sync::atomic::Ordering::Relaxed),
                                        total.load(std::sync::atomic::Ordering::Relaxed),
                                    ),
                                )
                            },
                        )),
                    };
                    save_results(&format!("{engine_name}_{database_name}.csv"), &perf);
                    plot_results(&format!("{engine_name}_{database_name}.png"), vec![&perf]);
                }

                std::thread::sleep(Duration::from_secs(1));
            }
        })
    };
    let handles = (0..(threads as usize).min(games_count.unwrap_or(threads as usize)))
        .map(|i| {
            let engine_path = engine_path.as_ref().to_path_buf();
            let engine_args = args.clone();
            let matching = matching.clone();
            std::thread::spawn(move || {
                let mut engine =
                    Engine::open_engine(i as usize, &engine_path, &engine_args, move_time).unwrap();
                log::trace!("thread {i} waiting for next task");
                while let Some(mut task) = matching.get_next_task() {
                    task.match_challenge(&mut engine)
                        .unwrap_or_else(|e| panic!("Engine failed in thread {i}: {e:?}"));
                    log::info!("[{i}] Completed a move matching task");
                }
                engine.close_engine()
            })
        })
        .collect::<Vec<_>>();

    for handle in handles {
        handle.join().unwrap()
    }
    progress_handle.join().unwrap();

    log::info!("Saving final results");
    let perf = Performance {
        name,
        matches: HashMap::from_iter(matching.matches.iter().map(|(elo, (matches, total))| {
            (
                *elo,
                (
                    matches.load(std::sync::atomic::Ordering::Relaxed),
                    total.load(std::sync::atomic::Ordering::Relaxed),
                ),
            )
        })),
    };
    save_results(&format!("{engine_name}_{database_name}.csv"), &perf);
    plot_results(&format!("{engine_name}_{database_name}.png"), vec![&perf]);

    Ok(())
}

pub struct Performance<'a> {
    pub name: &'a str,
    pub matches: HashMap<u64, (u32, u32)>,
}
pub fn plot_results<'a, P: AsRef<Path>>(path: P, perfs: Vec<&Performance<'a>>) {
    const PALETTE: [RGBColor; 3] = [GREEN, BLUE, RED];
    let move_matching_file = BitMapBackend::new(&path, (1024, 720)).into_drawing_area();
    move_matching_file.fill(&WHITE).unwrap();

    let mut move_matching_chart = ChartBuilder::on(&move_matching_file)
        .x_label_area_size(80)
        .y_label_area_size(80)
        .margin(5)
        .caption("Move matching performance", ("Calibri", 60))
        .build_cartesian_2d(1400u32..2900u32, (0f64..80f64).step(5f64))
        .unwrap();
    move_matching_chart
        .configure_mesh()
        .disable_x_mesh()
        .bold_line_style(WHITE.mix(0.3))
        .x_desc("Rating")
        .x_label_style(("Calibri", 30))
        .y_label_style(("Calibri", 30))
        .axis_desc_style(("Calibri", 40))
        .draw()
        .unwrap();

    for (i, Performance { name, matches }) in perfs.into_iter().enumerate() {
        let mut brackets_performance = vec![(0, 0); 18];
        for (elo, (matches, total)) in matches.iter() {
            let bracket_index = (elo / 100) - 11;
            brackets_performance[bracket_index as usize].0 += matches;
            brackets_performance[bracket_index as usize].1 += total;
        }
        let brackets_performance = brackets_performance
            .into_iter()
            .map(|(matches, total)| matches as f64 / total as f64)
            .collect::<Vec<_>>();

        move_matching_chart
            .draw_series(
                LineSeries::new(
                    brackets_performance
                        .into_iter()
                        .enumerate()
                        .filter_map(|(i, v)| {
                            let bracket = (i as u32 + 11) * 100;
                            let accuracy = v * 100f64;
                            if bracket < 1500 {
                                None
                            } else {
                                Some((bracket, accuracy))
                            }
                        }),
                    PALETTE[i].filled().stroke_width(3),
                )
                .point_size(5),
            )
            .unwrap()
            .label(name.to_string())
            .legend(move |(x, y)| {
                Rectangle::new(
                    [(x - 30, y + 3), (x, y)],
                    PALETTE[i].filled().stroke_width(3),
                )
            });
    }

    move_matching_chart
        .configure_series_labels()
        .position(plotters::chart::SeriesLabelPosition::UpperRight)
        .margin(40)
        .legend_area_size(10)
        .border_style(BLACK.mix(0.1))
        .background_style(WHITE)
        .label_font(("Calibri", 30))
        .draw()
        .unwrap();
    move_matching_file.present().expect("Could not open file");
}

pub fn save_results<'a, P: AsRef<Path>>(path: P, Performance { matches, .. }: &Performance<'a>) {
    // Save results
    let mut csv = csv::Writer::from_path(path).unwrap();

    for (elo, (matches, total)) in matches.iter() {
        csv.write_record(&[&elo.to_string(), &matches.to_string(), &total.to_string()])
            .unwrap();
        csv.flush().unwrap();
    }
}
