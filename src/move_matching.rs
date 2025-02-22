use std::{
    sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering},
    time::Duration,
};

use crate::{
    db::{Bucket, Game},
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

pub struct MoveMatching {
    games: Vec<(u32, Game)>,
    matches: Vec<(u32, Matches)>,
    next: AtomicUsize,
    total_positions: u64,
    completed_games: AtomicUsize,
    completed_positions: AtomicU64,
}
impl MoveMatching {
    pub fn from_games(buckets: &[Bucket]) -> Self {
        Self {
            matches: Vec::from_iter(buckets.iter().map(|b| (b.elo, Default::default()))),
            total_positions: buckets
                .iter()
                .map(|b| {
                    b.games
                        .iter()
                        .map(|g| g.moves.len().saturating_sub(6) as u64)
                        .sum::<u64>()
                })
                .sum(),
            games: buckets
                .into_iter()
                .flat_map(|b| b.games.iter().map(move |g| (b.elo, g.clone())))
                .collect(),
            next: AtomicUsize::new(0),
            completed_games: AtomicUsize::new(0),
            completed_positions: AtomicU64::new(0),
        }
    }

    /* pub fn from_checkpoint<P: AsRef<Path>>(games: &[Game], path: P) -> Self {
        let mut matching = Self::from_games(games);

        let csv = csv::Reader::from_path(&path).unwrap().into_deserialize();
        for (white, elo, matches, total) in csv.filter_map(|e| e.ok()) {
            if white {
                &mut matching.white_matches
            } else {
                &mut matching.black_matches
            }
            .entry(elo)
            .and_modify(|e| *e = (AtomicU32::new(matches), AtomicU32::new(total)))
            .or_insert((AtomicU32::new(matches), AtomicU32::new(total)));
        }
        let mut positions: u64 = matching
            .black_matches
            .values()
            .chain(matching.white_matches.values())
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
    } */

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

    pub fn snapshot(&self) -> Vec<(u32, MatchesSnapshot)> {
        self.matches
            .iter()
            .map(|(b, m)| (*b, m.snapshot()))
            .collect()
    }

    pub fn get_next_task<'a>(&'a self) -> Option<MoveMatchingTask<'a>> {
        let next = self.next.fetch_add(1, std::sync::atomic::Ordering::Acquire);
        if let Some((elo, game)) = self.games.get(next) {
            let matches_index = self.matches.binary_search_by_key(elo, |(e, _)| *e).unwrap();
            Some(MoveMatchingTask {
                moves: &game.moves,
                idx: 5,
                matches: &self.matches[matches_index].1,
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
    matches: &'a Matches,
    completed_games: &'a AtomicUsize,
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
        while self.idx < self.moves.len() {
            std::thread::sleep(Duration::from_millis(500));
            let (matches, positions) = if self.idx % 2 == 0 {
                (&mut black_matches, &mut black_positions)
            } else {
                (&mut white_matches, &mut white_positions)
            };
            match engine.send_command(Command::Board(&self.moves[0..self.idx])) {
                Ok(Response::Move((x, y))) => {
                    log::trace!("[{}] Move: {:?}", engine.id, (x, y));
                    if (x, y) == self.moves[self.idx] {
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
