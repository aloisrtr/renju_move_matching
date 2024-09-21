use std::{
    io::Result,
    sync::Arc,
    time::{Duration, Instant},
};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Style, Stylize},
    text::Span,
    widgets::{block::Title, Axis, Block, Borders, Chart, Dataset, Gauge, Widget},
    DefaultTerminal,
};

use crate::{
    move_matching::MoveMatching,
    plot::{plot_results, save_results, Performance},
};

pub struct Interface {
    experiment_name: String,
    move_matching: Arc<MoveMatching>,
    exit_requested: bool,
}
impl Interface {
    pub fn new(experiment_name: String, move_matching: Arc<MoveMatching>) -> Self {
        Self {
            experiment_name,
            move_matching,
            exit_requested: false,
        }
    }

    pub fn render_loop(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let mut last_update = Instant::now();
        let mut last_checkpoint = Instant::now();
        while !self.exit_requested && !self.move_matching.is_completed() {
            if last_update.elapsed() > Duration::from_secs_f32(1. / 10.) {
                terminal.draw(|frame| frame.render_widget(&self, frame.area()))?;
                last_update = Instant::now()
            }
            if last_checkpoint.elapsed() > Duration::from_secs(900) {
                self.save_checkpoint();
                last_checkpoint = Instant::now()
            }
            self.handle_events()?;
        }
        Ok(())
    }

    fn handle_events(&mut self) -> Result<()> {
        let timeout = Duration::from_secs_f32(1. / 20.);
        if event::poll(timeout)? {
            if let Event::Key(k) = event::read()? {
                log::trace!("Read input {k:?}");
                if k.kind == KeyEventKind::Press {
                    match k.code {
                        KeyCode::Char('q') | KeyCode::Esc => self.exit_requested = true,
                        KeyCode::Char('s') | KeyCode::Enter => self.save_checkpoint(),
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    fn save_checkpoint(&mut self) {
        save_results(
            format!("{}.csv", self.experiment_name),
            Performance {
                name: &self.experiment_name,
                matches: self.move_matching.snapshot(),
            },
        );
        plot_results(
            format!("{}.png", self.experiment_name),
            std::iter::once(Performance {
                name: &self.experiment_name,
                matches: self.move_matching.snapshot(),
            }),
        );
    }

    fn draw_progress(&self, area: Rect, buffer: &mut Buffer) {
        let completed_positions = self.move_matching.completed_positions();
        let total_positions = self.move_matching.total_positions();
        Gauge::default()
            .block(
                Block::new()
                    .borders(Borders::all())
                    .title(Title::from("Progress").alignment(Alignment::Left))
                    .fg(Color::White),
            )
            .gauge_style(Color::Green)
            .ratio(completed_positions as f64 / total_positions as f64)
            .label(Span::styled(
                format!("{completed_positions}/{total_positions} positions"),
                Style::new().fg(Color::White),
            ))
            .render(area, buffer);
    }

    fn draw_plot(&self, area: Rect, buffer: &mut Buffer) {
        let mut brackets_performance = [(0, 0); 18];
        for (elo, matches, total) in self.move_matching.snapshot() {
            let bracket_index = (elo / 100) - 11;
            brackets_performance[bracket_index as usize].0 += matches;
            brackets_performance[bracket_index as usize].1 += total;
        }
        let mut plot_data = [(0., 0.); 18];
        for (i, (matches, total)) in brackets_performance.into_iter().enumerate() {
            let bracket = (i as u32 + 11) * 100;
            let accuracy = if total == 0 {
                0.
            } else {
                (matches as f64 / total as f64) * 100f64
            };
            plot_data[i] = (bracket as f64, accuracy)
        }

        let dataset = Dataset::default()
            .name(self.experiment_name.as_str().italic())
            .marker(ratatui::symbols::Marker::Braille)
            .style(Style::default().fg(Color::Red))
            .graph_type(ratatui::widgets::GraphType::Line)
            .data(&plot_data);

        Chart::new(vec![dataset])
            .block(
                Block::bordered()
                    .title(Title::from("Performance").alignment(Alignment::Left))
                    .fg(Color::White),
            )
            .x_axis(
                Axis::default()
                    .title("Rating")
                    .style(Style::default().white())
                    .bounds([1400., 3000.])
                    .labels([
                        "1400", "1600", "1800", "2000", "2200", "2400", "2600", "2800", "3000",
                    ]),
            )
            .y_axis(
                Axis::default()
                    .title("Move matching %")
                    .style(Style::default().white())
                    .bounds([0., 80.])
                    .labels(["0", "10", "20", "30", "40", "50", "60", "70", "80"]),
            )
            .legend_position(Some(ratatui::widgets::LegendPosition::TopLeft))
            .render(area, buffer);
    }
}
impl Widget for &Interface {
    fn render(self, area: Rect, buffer: &mut Buffer)
    where
        Self: Sized,
    {
        let [progress, plot] =
            Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(area);
        self.draw_progress(progress, buffer);
        self.draw_plot(plot, buffer);
    }
}
