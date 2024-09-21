use std::{
    collections::HashMap,
    path::Path,
    sync::atomic::{AtomicU32, AtomicU64, AtomicUsize},
    time::Duration,
};

use crate::{
    db::Game,
    protocol::{Command, Engine, EngineError, Response},
};

pub struct MoveMatching {
    games: Vec<Game>,
    matches: HashMap<u64, (AtomicU32, AtomicU32)>,
    next: AtomicUsize,
    total_positions: u64,
    completed_games: AtomicUsize,
    completed_positions: AtomicU64,
}
impl MoveMatching {
    pub fn from_games(games: &[Game]) -> Self {
        Self {
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
            games: games.to_vec(),
            next: AtomicUsize::new(0),
            total_positions: games
                .iter()
                .map(|g| g.moves.len().saturating_sub(6) as u64)
                .sum(),
            completed_games: AtomicUsize::new(0),
            completed_positions: AtomicU64::new(0),
        }
    }

    pub fn from_checkpoint<P: AsRef<Path>>(games: &[Game], path: P) -> Self {
        let mut matching = Self::from_games(games);

        let csv = csv::Reader::from_path(&path).unwrap().into_deserialize();
        for (elo, matches, total) in csv.filter_map(|e| e.ok()) {
            matching
                .matches
                .entry(elo)
                .and_modify(|e| *e = (AtomicU32::new(matches), AtomicU32::new(total)))
                .or_insert((AtomicU32::new(matches), AtomicU32::new(total)));
        }
        let mut positions: u64 = matching
            .matches
            .values()
            .map(|(_, total)| total.load(std::sync::atomic::Ordering::Relaxed) as u64)
            .sum();

        let mut completed_games = 0;
        let mut completed_positions = 0;
        for g in games {
            if let Some(p) = positions.checked_sub(g.moves.len().saturating_sub(7) as u64) {
                positions = p;
                completed_positions += g.moves.len().saturating_sub(7) as u64;
                completed_games += 1
            } else {
                break;
            }
        }
        matching.next = AtomicUsize::new(completed_games);
        matching.completed_games = AtomicUsize::new(completed_games);
        matching.completed_positions = AtomicU64::new(completed_positions);

        matching
    }

    pub fn completed_games(&self) -> u64 {
        self.completed_games
            .load(std::sync::atomic::Ordering::Relaxed) as u64
    }

    pub fn completed_positions(&self) -> u64 {
        self.completed_positions
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn total_games(&self) -> u64 {
        self.games.len() as u64
    }

    pub fn total_positions(&self) -> u64 {
        self.total_positions
    }

    pub fn is_completed(&self) -> bool {
        self.completed_games() == self.games.len() as u64
    }

    pub fn snapshot(&self) -> impl Iterator<Item = (u64, u32, u32)> + '_ {
        self.matches.iter().map(|(elo, (matches, total))| {
            (
                *elo,
                matches.load(std::sync::atomic::Ordering::Relaxed),
                total.load(std::sync::atomic::Ordering::Relaxed),
            )
        })
    }

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
                completed_games: &self.completed_games,
                completed_positions: &self.completed_positions,
            })
        } else {
            None
        }
    }
}

pub struct MoveMatchingTask<'a> {
    moves: &'a [(u8, u8)],
    idx: usize,
    black_matches: &'a (AtomicU32, AtomicU32),
    white_matches: &'a (AtomicU32, AtomicU32),
    completed_games: &'a AtomicUsize,
    completed_positions: &'a AtomicU64,
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
            match engine.send_command(Command::Board(&self.moves[0..self.idx])) {
                Ok(Response::Move((x, y))) => {
                    log::trace!("[{}] Move: {:?}", engine.id, (x, y));
                    if (x, y) == self.moves[self.idx] {
                        matches.0 += 1;
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
        self.completed_games
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        result
    }
}
