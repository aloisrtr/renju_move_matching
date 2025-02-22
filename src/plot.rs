use plotters::{
    backend::SVGBackend,
    chart::ChartBuilder,
    coord::combinators::IntoLinspace,
    drawing::IntoDrawingArea,
    element::Rectangle,
    prelude::BitMapBackend,
    series::{DashedLineSeries, Histogram, LineSeries},
    style::*,
};

use crate::{db::Bucket, move_matching::MatchesSnapshot};

type Backend<'a> = SVGBackend<'a>;

const ARTIFACTS_DIR: &str = "./artifacts";

fn create_artifacts_dir() {
    let _ = std::fs::create_dir(ARTIFACTS_DIR);
}
fn create_experiment_dir(experiment_name: &str) {
    create_artifacts_dir();
    let _ = std::fs::create_dir(format!("{ARTIFACTS_DIR}/{experiment_name}"));
}

pub enum PlotError {
    NotEnoughRecords,
}

pub fn plot_rating_distribution(experiment_name: &str, buckets: &[Bucket]) {
    create_artifacts_dir();
    create_experiment_dir(experiment_name);
    let path = format!("{ARTIFACTS_DIR}/{experiment_name}/ratings.png");
    let rating_distribution_file = BitMapBackend::new(&path, (1024, 720)).into_drawing_area();
    rating_distribution_file.fill(&WHITE).unwrap();

    let min_bucket = buckets.iter().map(|b| b.elo).min().unwrap();
    let max_bucket = buckets.iter().map(|b| b.elo).max().unwrap();
    let elo_range = min_bucket..(max_bucket + (max_bucket - min_bucket) / buckets.len() as u32);
    let max_games_per_bracket = buckets.iter().map(|b| b.games.len() as u32).max().unwrap();

    let mut rating_distribution_chart = ChartBuilder::on(&rating_distribution_file)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .margin(5)
        .caption("Renju ratings distribution", ("sans-serif", 50.0))
        .build_cartesian_2d(elo_range, 0u32..max_games_per_bracket)
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
                .style(RED.filled())
                .data(buckets.iter().map(|b| (b.elo, b.games.len() as u32))),
        )
        .unwrap();

    rating_distribution_file
        .present()
        .expect("Could not open file");
}

pub fn plot_results(experiment_name: &str, perfs: Vec<(&str, Vec<(u32, MatchesSnapshot)>)>) {
    const PALETTE: [RGBColor; 4] = [
        RGBColor(255, 215, 0),
        RGBColor(250, 135, 117),
        RGBColor(205, 52, 181),
        RGBColor(0, 0, 255),
    ];

    let min_bucket = perfs
        .iter()
        .map(|(_, m)| m.iter().map(|(elo, _)| *elo).min().unwrap_or(0))
        .min()
        .unwrap_or(0);
    let max_bucket = perfs
        .iter()
        .map(|(_, m)| m.iter().map(|(elo, _)| *elo).max().unwrap_or(3000))
        .max()
        .unwrap();
    let buckets_count = perfs.iter().map(|(_, m)| m.len()).max().unwrap_or(1);
    let bucket_size = (max_bucket - min_bucket) / buckets_count as u32;
    let elo_range = min_bucket..(max_bucket + bucket_size);

    create_artifacts_dir();
    create_experiment_dir(experiment_name);
    let both_sides_path = format!("{ARTIFACTS_DIR}/{experiment_name}/matching.svg");
    let both_sides_file = Backend::new(&both_sides_path, (1024, 720)).into_drawing_area();
    let by_side_path = format!("{ARTIFACTS_DIR}/{experiment_name}/matching_by_side.svg");
    let by_side_file = Backend::new(&by_side_path, (1024, 720)).into_drawing_area();

    const FONT: &'static str = "sans-serif";
    let mut both_sides_chart = ChartBuilder::on(&both_sides_file)
        .x_label_area_size(80)
        .y_label_area_size(80)
        .margin(5)
        .caption("Move matching performance (%)", (FONT, 60))
        .build_cartesian_2d(
            elo_range.clone().step(bucket_size),
            (0f64..80f64).step(5f64),
        )
        .unwrap();
    both_sides_chart
        .configure_mesh()
        .bold_line_style(WHITE.mix(0.3))
        .x_desc("Rating bucket (WHR)")
        .x_label_style((FONT, 30))
        .y_label_style((FONT, 30))
        .axis_desc_style((FONT, 40))
        .draw()
        .unwrap();
    let mut side_by_side_chart = ChartBuilder::on(&by_side_file)
        .x_label_area_size(80)
        .y_label_area_size(80)
        .margin(5)
        .caption("Move matching performance by side (%)", (FONT, 60))
        .build_cartesian_2d(elo_range.step(bucket_size), (0f64..80f64).step(5f64))
        .unwrap();
    side_by_side_chart
        .configure_mesh()
        .bold_line_style(WHITE.mix(0.3))
        .x_desc("Rating bucket (WHR)")
        .x_label_style((FONT, 30))
        .y_label_style((FONT, 30))
        .axis_desc_style((FONT, 40))
        .draw()
        .unwrap();

    for (i, (name, matches)) in perfs.iter().enumerate() {
        let mut brackets_performance = vec![[(0, 0); 2]; buckets_count + 1];
        for (
            elo,
            MatchesSnapshot {
                black_positions,
                black_matches,
                white_positions,
                white_matches,
            },
        ) in matches
        {
            let bracket_index = (elo / bucket_size) - (min_bucket / bucket_size);
            brackets_performance[bracket_index as usize][0].0 += black_matches;
            brackets_performance[bracket_index as usize][0].1 += black_positions;
            brackets_performance[bracket_index as usize][1].0 += white_matches;
            brackets_performance[bracket_index as usize][1].1 += white_positions;
        }

        let brackets_performance = brackets_performance
            .into_iter()
            .map(
                |[(black_matches, black_total), (white_matches, white_total)]| {
                    [
                        black_matches as f64 / black_total as f64,
                        white_matches as f64 / white_total as f64,
                    ]
                },
            )
            .collect::<Vec<_>>();

        both_sides_chart
            .draw_series(
                LineSeries::new(
                    brackets_performance.iter().enumerate().map(|(i, v)| {
                        let bracket = (i as u32 + (min_bucket * bucket_size)) * bucket_size;
                        let accuracy = ((v[0] + v[1]) / 2f64) * 100f64;
                        (bracket, accuracy)
                    }),
                    PALETTE[i].filled().stroke_width(5),
                )
                .point_size(0),
            )
            .unwrap()
            .label(name.to_string())
            .legend(move |(x, y)| {
                Rectangle::new(
                    [(x - 30, y + 3), (x, y)],
                    PALETTE[i].filled().stroke_width(5),
                )
            });
        side_by_side_chart
            .draw_series(
                LineSeries::new(
                    brackets_performance.iter().enumerate().map(|(i, v)| {
                        let bracket = (i as u32 + (min_bucket * bucket_size)) * bucket_size;
                        let accuracy = v[0] * 100f64;
                        (bracket, accuracy)
                    }),
                    PALETTE[i].filled().stroke_width(5),
                )
                .point_size(0),
            )
            .unwrap()
            .label("Black".to_string())
            .legend(move |(x, y)| {
                Rectangle::new(
                    [(x - 30, y + 3), (x, y)],
                    PALETTE[i].filled().stroke_width(5),
                )
            });
        side_by_side_chart
            .draw_series(DashedLineSeries::new(
                brackets_performance.into_iter().enumerate().map(|(i, v)| {
                    let bracket = (i as u32 + (min_bucket * bucket_size)) * bucket_size;
                    let accuracy = v[1] * 100f64;
                    (bracket, accuracy)
                }),
                5,
                10,
                PALETTE[i].filled().stroke_width(5),
            ))
            .unwrap()
            .label("White".to_string())
            .legend(move |(x, y)| {
                Rectangle::new([(x - 30, y + 3), (x, y)], PALETTE[i].stroke_width(5))
            });
    }

    both_sides_chart
        .configure_series_labels()
        .position(plotters::chart::SeriesLabelPosition::UpperRight)
        .margin(40)
        .legend_area_size(10)
        .border_style(BLACK.mix(0.1))
        .background_style(WHITE)
        .label_font((FONT, 30))
        .draw()
        .unwrap();
    both_sides_file.present().expect("Could not open file");

    side_by_side_chart
        .configure_series_labels()
        .position(plotters::chart::SeriesLabelPosition::UpperRight)
        .margin(40)
        .legend_area_size(10)
        .border_style(BLACK.mix(0.1))
        .background_style(WHITE)
        .label_font((FONT, 30))
        .draw()
        .unwrap();
    by_side_file.present().expect("Could not open file");
}

pub fn save_results<'a, I: IntoIterator<Item = (u32, MatchesSnapshot)>>(
    experiment_name: &str,
    matches: I,
) {
    create_experiment_dir(experiment_name);
    let path = format!("{ARTIFACTS_DIR}/{experiment_name}/results.csv");
    let mut csv = csv::Writer::from_path(path).unwrap();

    csv.write_record(&[
        "Elo bucket",
        "Black positions",
        "Black matches",
        "White positions",
        "White matches",
    ])
    .unwrap();
    for (
        elo,
        MatchesSnapshot {
            black_positions,
            black_matches,
            white_positions,
            white_matches,
        },
    ) in matches
    {
        csv.write_record(&[
            &elo.to_string(),
            &black_positions.to_string(),
            &black_matches.to_string(),
            &white_positions.to_string(),
            &white_matches.to_string(),
        ])
        .unwrap();
    }
    csv.flush().unwrap();
}
