use plotters::{
    backend::BitMapBackend,
    chart::ChartBuilder,
    coord::{combinators::IntoLinspace, ranged1d::IntoSegmentedCoord},
    drawing::IntoDrawingArea,
    element::Rectangle,
    series::{Histogram, LineSeries},
    style::*,
};
use std::path::Path;

use crate::db::Game;

pub fn plot_rating_distribution<'a, P: AsRef<Path>>(path: P, games: &[Game]) {
    let rating_distribution_file = BitMapBackend::new(&path, (1024, 720)).into_drawing_area();
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
}

pub struct Performance<'a, I: Iterator<Item = (u64, u32, u32)>> {
    pub name: &'a str,
    pub matches: I,
}
pub fn plot_results<'a, P: AsRef<Path>, I: Iterator<Item = (u64, u32, u32)>>(
    path: P,
    perfs: impl Iterator<Item = Performance<'a, I>>,
) {
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
        for (elo, matches, total) in matches {
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

pub fn save_results<'a, P: AsRef<Path>, I: Iterator<Item = (u64, u32, u32)>>(
    path: P,
    Performance { matches, .. }: Performance<'a, I>,
) {
    let mut csv = csv::Writer::from_path(path).unwrap();

    for (elo, matches, total) in matches {
        csv.write_record(&[&elo.to_string(), &matches.to_string(), &total.to_string()])
            .unwrap();
        csv.flush().unwrap();
    }
}
