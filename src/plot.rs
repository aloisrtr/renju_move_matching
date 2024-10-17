use plotters::{
    backend::{BitMapBackend, SVGBackend},
    chart::ChartBuilder,
    coord::{combinators::IntoLinspace, ranged1d::IntoSegmentedCoord},
    drawing::IntoDrawingArea,
    element::Rectangle,
    series::{DashedLineSeries, Histogram, LineSeries},
    style::*,
};
use std::path::Path;

use crate::db::Game;

type Backend<'a> = SVGBackend<'a>;

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
                .style(RED.mix(0.5).filled().stroke_width(10))
                .data(games.iter().map(|g| (g.black_elo as u32, 1))),
        )
        .unwrap();
    rating_distribution_file
        .present()
        .expect("Could not open file");
}

pub struct Performance<'a, I: Iterator<Item = (bool, u64, u32, u32)>> {
    pub name: &'a str,
    pub matches: I,
}
pub fn plot_results<'a, P: AsRef<Path>, I: Iterator<Item = (bool, u64, u32, u32)>>(
    path: P,
    perfs: impl Iterator<Item = Performance<'a, I>>,
) {
    const PALETTE: [RGBColor; 4] = [
        RGBColor(255, 215, 0),
        RGBColor(250, 135, 117),
        RGBColor(205, 52, 181),
        RGBColor(0, 0, 255),
    ];
    let file_name = path.as_ref().file_stem().unwrap().to_str().unwrap();
    let file_extension = path.as_ref().extension().unwrap().to_str().unwrap();
    let both_sides_file = Backend::new(&path, (1024, 720)).into_drawing_area();
    let by_colour_file_name = format!("{file_name}_colours.{file_extension}");
    let side_by_side_file = Backend::new(&by_colour_file_name, (1024, 720)).into_drawing_area();

    const FONT: &'static str = "sans-serif";
    let mut both_sides_chart = ChartBuilder::on(&both_sides_file)
        .x_label_area_size(80)
        .y_label_area_size(80)
        .margin(5)
        .caption("Move matching performance (%)", (FONT, 60))
        .build_cartesian_2d(1400u32..2900u32, (0f64..80f64).step(5f64))
        .unwrap();
    both_sides_chart
        .configure_mesh()
        .disable_x_mesh()
        .bold_line_style(WHITE.mix(0.3))
        .x_desc("Rating bucket (WHR)")
        .x_label_style((FONT, 30))
        .y_label_style((FONT, 30))
        .axis_desc_style((FONT, 40))
        .draw()
        .unwrap();
    let mut side_by_side_chart = ChartBuilder::on(&side_by_side_file)
        .x_label_area_size(80)
        .y_label_area_size(80)
        .margin(5)
        .caption("Move matching performance by side (%)", (FONT, 60))
        .build_cartesian_2d(1400u32..2900u32, (0f64..80f64).step(5f64))
        .unwrap();
    side_by_side_chart
        .configure_mesh()
        .disable_x_mesh()
        .bold_line_style(WHITE.mix(0.3))
        .x_desc("Rating bucket (WHR)")
        .x_label_style((FONT, 30))
        .y_label_style((FONT, 30))
        .axis_desc_style((FONT, 40))
        .draw()
        .unwrap();

    for (i, Performance { name, matches }) in perfs.into_iter().enumerate() {
        let mut brackets_performance = vec![[(0, 0); 2]; 18];
        for (side, elo, matches, total) in matches {
            let bracket_index = (elo / 100) - 11;
            brackets_performance[bracket_index as usize][side as usize].0 += matches;
            brackets_performance[bracket_index as usize][side as usize].1 += total;
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
                    brackets_performance
                        .iter()
                        .enumerate()
                        .filter_map(|(i, v)| {
                            let bracket = (i as u32 + 11) * 100;
                            let accuracy = ((v[0] + v[1]) / 2f64) * 100f64;
                            if bracket < 1500 {
                                None
                            } else {
                                Some((bracket, accuracy))
                            }
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
                    brackets_performance
                        .iter()
                        .enumerate()
                        .filter_map(|(i, v)| {
                            let bracket = (i as u32 + 11) * 100;
                            let accuracy = v[0] * 100f64;
                            if bracket < 1500 {
                                None
                            } else {
                                Some((bracket, accuracy))
                            }
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
                brackets_performance
                    .into_iter()
                    .enumerate()
                    .filter_map(|(i, v)| {
                        let bracket = (i as u32 + 11) * 100;
                        let accuracy = v[1] * 100f64;
                        if bracket < 1500 {
                            None
                        } else {
                            Some((bracket, accuracy))
                        }
                    }),
                5,
                10,
                PALETTE[i].filled().stroke_width(5),
            ))
            .unwrap()
            .label("White".to_string())
            .legend(move |(x, y)| {
                Rectangle::new(
                    [(x - 30, y + 3), (x, y)],
                    PALETTE[i].filled().stroke_width(5),
                )
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
    side_by_side_file.present().expect("Could not open file");
}

pub fn save_results<'a, P: AsRef<Path>, I: Iterator<Item = (bool, u64, u32, u32)>>(
    path: P,
    Performance { matches, .. }: Performance<'a, I>,
) {
    let mut csv = csv::Writer::from_path(path).unwrap();

    for (side, elo, matches, total) in matches {
        csv.write_record(&[
            &side.to_string(),
            &elo.to_string(),
            &matches.to_string(),
            &total.to_string(),
        ])
        .unwrap();
        csv.flush().unwrap();
    }
}
