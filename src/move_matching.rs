use std::{
    collections::HashMap,
    sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering},
    time::Duration,
};

use romoku::game::action::Action;

use crate::{
    db::{Bucket, Game},
    game_graph::{Position, PositionGraph},
    protocol::{Command, Engine, EngineError, Response},
};

#[derive(Default)]
pub struct Matches {
    pub black_positions: AtomicU32,
    pub black_matches: AtomicU32,
    pub white_positions: AtomicU32,
    pub white_matches: AtomicU32,
}
impl Matches {
    pub fn snapshot(&self) -> MatchesSnapshot {
        MatchesSnapshot {
            black_positions: self.black_positions.load(Ordering::Relaxed),
            black_matches: self.black_matches.load(Ordering::Relaxed),
            white_positions: self.white_positions.load(Ordering::Relaxed),
            white_matches: self.white_matches.load(Ordering::Relaxed),
        }
    }
}
pub struct MatchesSnapshot {
    pub black_positions: u32,
    pub black_matches: u32,
    pub white_positions: u32,
    pub white_matches: u32,
}

pub struct MoveMatching<'a> {
    positions: Vec<(u32, &'a Position)>,
    matches: HashMap<u32, Matches>,
    next: AtomicUsize,
    total_positions: u64,
    completed_positions: AtomicU64,
    chunk_size: usize,
}
impl<'a> MoveMatching<'a> {
    pub fn from_games(buckets: &'a [Bucket], chunk_size: usize) -> Self {
        Self {
            matches: buckets
                .iter()
                .map(|b| (b.elo, Default::default()))
                .collect(),
            total_positions: buckets
                .iter()
                .map(|b| b.position_graph.positions_count(false))
                .sum::<usize>() as u64,
            positions: buckets
                .iter()
                .flat_map(|b| b.position_graph.positions(false).map(|p| (b.elo, p)))
                .collect(),
            next: AtomicUsize::new(0),
            completed_positions: AtomicU64::new(0),
            chunk_size,
        }
    }

    pub fn completed_positions(&self) -> u64 {
        self.completed_positions
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn total_positions(&self) -> u64 {
        self.total_positions
    }

    pub fn is_completed(&self) -> bool {
        self.completed_positions() == self.total_positions
    }

    pub fn snapshot(&self) -> Vec<(u32, MatchesSnapshot)> {
        self.matches
            .iter()
            .map(|(b, m)| (*b, m.snapshot()))
            .collect()
    }

    pub fn get_next_task(&'a self) -> Option<MoveMatchingTask<'a>> {
        let next = self
            .next
            .fetch_add(self.chunk_size, std::sync::atomic::Ordering::Acquire);
        if next >= self.positions.len() {
            return None;
        }

        // Get a chunk of positions
        let positions = &self.positions[next..self.positions.len().min(next + self.chunk_size)];
        Some(MoveMatchingTask {
            positions: positions
                .iter()
                .map(|(bucket, pos)| (*pos, self.matches.get(bucket).unwrap()))
                .collect(),
            completed_positions: &self.completed_positions,
        })
    }
}

pub struct MoveMatchingTask<'a> {
    positions: Vec<(&'a Position, &'a Matches)>,
    completed_positions: &'a AtomicU64,
}
impl<'a> MoveMatchingTask<'a> {
    pub fn match_challenge(&mut self, engine: &mut Engine) -> Result<(), EngineError> {
        // Loop over moves and try to match them
        let mut black_positions = 0;
        let mut black_matches = 0;
        let mut white_positions = 0;
        let mut white_matches = 0;

        let mut result = Ok(());
        for (position, matches) in &self.positions {
            std::thread::sleep(Duration::from_millis(500));
            let (matches, positions) = if position.black_stones_turn() {
                (&mut black_matches, &mut black_positions)
            } else {
                (&mut white_matches, &mut white_positions)
            };
            match engine.send_command(Command::Board())
        }

        while self.idx < self.moves.len() {
            std::thread::sleep(Duration::from_millis(500));
            let (matches, positions) = if self.idx % 2 == 0 {
                (&mut black_matches, &mut black_positions)
            } else {
                (&mut white_matches, &mut white_positions)
            };
            match engine.send_command(Command::Board(&self.moves[0..self.idx])) {
                Ok(Response::Move(a)) => {
                    log::trace!("[{}] Move: {a}", engine.id);
                    if a == self.moves[self.idx] {
                        *matches += 1;
                    }
                }
                Ok(r) => {
                    log::error!("Unexpected response from engine: {r:?}");
                    result = Err(EngineError::UnexpectedResponse(r));
                    break;
                }
                Err(e) => {
                    log::error!("Error when receiving response: {e:?}");
                    result = Err(e);
                    break;
                }
            }
            self.completed_positions
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            *positions += 1;
            self.idx += 1
        }
        self.matches
            .black_matches
            .fetch_add(black_matches, Ordering::Relaxed);
        self.matches
            .white_matches
            .fetch_add(white_matches, Ordering::Relaxed);
        self.matches
            .black_positions
            .fetch_add(black_positions, Ordering::Relaxed);
        self.matches
            .white_positions
            .fetch_add(white_positions, Ordering::Relaxed);

        self.completed_games
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        result
    }
}
