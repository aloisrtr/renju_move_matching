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
    plot::{plot_results, save_results},
};

pub struct Interface {
    experiment_name: String,
    move_matching: Arc<MoveMatching>,
    min_bracket: u32,
    max_bracket: u32,
    bracket_size: u32,
    buckets_self_move_matching: Vec<(f64, f64)>,
    exit_requested: bool,
}
impl Interface {
    pub fn new(
        experiment_name: String,
        min_bracket: u32,
        max_bracket: u32,
        bracket_size: u32,
        move_matching: Arc<MoveMatching>,
        buckets_self_move_matching: Vec<(f64, f64)>,
    ) -> Self {
        Self {
            min_bracket,
            max_bracket,
            bracket_size,
            experiment_name,
            move_matching,
            buckets_self_move_matching,
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
        save_results(&self.experiment_name, self.move_matching.snapshot());
        plot_results(
            &self.experiment_name,
            vec![(&self.experiment_name, self.move_matching.snapshot())],
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
                    .fg(Color::Black),
            )
            .gauge_style(Color::Green)
            .ratio((completed_positions as f64 / total_positions as f64).min(0.99))
            .label(Span::styled(
                format!("{completed_positions}/{total_positions} positions"),
                Style::new().fg(Color::Black),
            ))
            .render(area, buffer);
    }

    fn draw_plot(&self, area: Rect, buffer: &mut Buffer) {
        let brackets_count = ((self.max_bracket - self.min_bracket) / self.bracket_size) + 1;
        let mut brackets_performance = vec![[(0, 0); 2]; brackets_count as usize];
        for (elo, matches) in self.move_matching.snapshot() {
            let bracket_index = (elo / self.bracket_size) - (self.min_bracket / self.bracket_size);
            brackets_performance[bracket_index as usize][0].0 += matches.black_matches;
            brackets_performance[bracket_index as usize][0].1 += matches.black_positions;
            brackets_performance[bracket_index as usize][1].0 += matches.white_matches;
            brackets_performance[bracket_index as usize][1].1 += matches.white_positions;
        }
        let mut whole_plot_data = vec![(0., 0.); brackets_count as usize];
        let mut black_plot_data = vec![(0., 0.); brackets_count as usize];
        let mut white_plot_data = vec![(0., 0.); brackets_count as usize];
        for (i, data) in brackets_performance.into_iter().enumerate() {
            let bracket = (i as u32 + (self.min_bracket / self.bracket_size)) * self.bracket_size;
            let general_accuracy = if data[0].1 + data[1].1 == 0 {
                0.
            } else {
                ((data[0].0 + data[1].0) as f64 / (data[0].1 + data[1].1) as f64) * 100f64
            };
            let black_accuracy = if data[0].1 == 0 {
                0.
            } else {
                (data[0].0 as f64 / data[0].1 as f64) * 100f64
            };
            let white_accuracy = if data[1].1 == 0 {
                0.
            } else {
                (data[1].0 as f64 / data[1].1 as f64) * 100f64
            };
            whole_plot_data[i] = (bracket as f64, general_accuracy);
            black_plot_data[i] = (bracket as f64, black_accuracy);
            white_plot_data[i] = (bracket as f64, white_accuracy)
        }

        let whole_dataset = Dataset::default()
            .name(self.experiment_name.as_str().italic())
            .marker(ratatui::symbols::Marker::Dot)
            .style(Style::default().fg(Color::Green))
            .graph_type(ratatui::widgets::GraphType::Line)
            .bold()
            .data(&whole_plot_data);
        let black_dataset = Dataset::default()
            .name("Black stones".italic())
            .marker(ratatui::symbols::Marker::Dot)
            .style(Style::default().fg(Color::Black))
            .graph_type(ratatui::widgets::GraphType::Line)
            .data(&black_plot_data);
        let white_dataset = Dataset::default()
            .name("White stones".italic())
            .marker(ratatui::symbols::Marker::Dot)
            .style(Style::default().fg(Color::Yellow))
            .graph_type(ratatui::widgets::GraphType::Line)
            .data(&white_plot_data);
        let self_move_matching = Dataset::default()
            .name("Peak performance".italic())
            .marker(ratatui::symbols::Marker::Dot)
            .style(Style::default().fg(Color::Red))
            .graph_type(ratatui::widgets::GraphType::Line)
            .data(&self.buckets_self_move_matching);

        Chart::new(vec![
            black_dataset,
            white_dataset,
            whole_dataset,
            self_move_matching,
        ])
        .block(
            Block::bordered()
                .title(Title::from("Performance").alignment(Alignment::Left))
                .fg(Color::Black),
        )
        .x_axis(
            Axis::default()
                .title("Rating")
                .style(Style::default().black())
                .bounds([self.min_bracket as f64, self.max_bracket as f64])
                .labels(
                    (self.min_bracket..=self.max_bracket)
                        .step_by(self.bracket_size as usize)
                        .map(|i| i.to_string()),
                ),
        )
        .y_axis(
            Axis::default()
                .title("Move matching %")
                .style(Style::default().black())
                .bounds([0., 100.])
                .labels([
                    "0", "10", "20", "30", "40", "50", "60", "70", "80", "90", "100",
                ]),
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
