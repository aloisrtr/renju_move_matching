//! # Explicit graph representation of a set of played games.
//! Nodes of the graph are game states, while edges are moves annotated with the
//! probability of this branch being followed in the dataset.

use std::{
    collections::{hash_map::Entry, HashMap},
    str::FromStr,
};

use romoku::game::action::Action;

#[derive(Hash, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PositionKey {
    hash: u64,
    played: Action,
}
#[derive(Hash, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PositionData {
    visits: u32,
    forbidden: bool,
}

#[derive(Clone, Default, Debug)]
pub struct Position {
    is_opening: bool,
    black_stones: bool,
    trajectories: Vec<Vec<Action>>,
    next: HashMap<PositionKey, PositionData>,
    total_visits: u32,
}
impl Position {
    pub fn new(is_opening: bool, black_stones: bool) -> Self {
        Self {
            is_opening,
            black_stones,
            ..Default::default()
        }
    }

    pub fn add_next(&mut self, hash: u64, played: Action, forbidden: bool) {
        let key = PositionKey { hash, played };
        if self.next.contains_key(&key) {
            self.next.get_mut(&key).unwrap().visits += 1;
            assert_eq!(self.next.get_mut(&key).unwrap().forbidden, forbidden)
        } else {
            self.next.insert(
                key,
                PositionData {
                    visits: 1,
                    forbidden,
                },
            );
        }
        self.total_visits += 1
    }

    pub fn black_stones_turn(&self) -> bool {
        self.black_stones
    }

    pub fn policy(&self) -> HashMap<Action, f64> {
        let mut probabilities = HashMap::with_capacity(self.next.len());
        for (PositionKey { played, .. }, PositionData { visits, .. }) in self.next.iter() {
            probabilities.insert(*played, *visits as f64 / self.total_visits as f64);
        }
        probabilities
    }
}

#[derive(Clone, Debug)]
pub struct PositionGraph {
    nodes: HashMap<u64, Position>,
    root: u64,
}
impl PositionGraph {
    /// Constructs the position graph from a set of trajectories.
    pub fn from_games(games: Vec<Vec<Action>>) -> Self {
        let mut nodes = HashMap::new();

        let mut state = romoku::game::board::Board::empty();
        let mut current_hash = state.hash();
        let root = current_hash;
        nodes.insert(root, Position::new(true, true));

        for moves in games {
            state = romoku::game::board::Board::empty();
            let mut prev_hash = state.hash();
            for (i, &m) in moves.iter().enumerate() {
                let forbidden = state.play(m).is_err();
                current_hash = state.hash();
                nodes
                    .get_mut(&prev_hash)
                    .unwrap()
                    .add_next(current_hash, m, forbidden);
                match nodes.entry(current_hash) {
                    Entry::Occupied(_) => {}
                    Entry::Vacant(e) => {
                        e.insert(Position::new(i <= 4, i % 2 == 1));
                    }
                }
                prev_hash = current_hash
            }
        }

        PositionGraph { nodes, root }
    }

    pub fn positions_count(&self, count_openings: bool) -> usize {
        if count_openings {
            self.nodes.len()
        } else {
            self.nodes.values().filter(|p| !p.is_opening).count()
        }
    }

    pub fn positions(&self, count_openings: bool) -> impl Iterator<Item = &Position> {
        self.nodes
            .values()
            .filter(move |p| count_openings || !p.is_opening)
    }

    pub fn get(&self, hash: u64) -> Option<&Position> {
        self.nodes.get(&hash)
    }

    pub fn root(&self) -> &Position {
        self.nodes.get(&self.root).unwrap()
    }

    pub fn self_move_matching(&self, count_openings: bool) -> f64 {
        (self
            .nodes
            .values()
            .filter_map(|p| {
                let prob = p.next.values().map(|p| p.visits).max().unwrap_or(0) as f64
                    / p.total_visits as f64;
                if prob.is_nan() || (!count_openings && p.is_opening) {
                    None
                } else {
                    Some(prob)
                }
            })
            .sum::<f64>()
            / self.nodes.len() as f64)
            * 100.
    }

    /// Outputs the DOT representation of this graph for vizualisation purposes
    pub fn to_dot(&self) -> String {
        let mut dot_output = String::from_str("strict digraph {\n").unwrap();

        for hash in self.nodes.keys() {
            dot_output += &format!("\t{hash} [label={hash:0x}];\n");
        }

        for v in self.nodes.iter() {
            let visits = v.1.total_visits;
            for (key, value) in v.1.next.iter() {
                dot_output += &format!(
                    "\t{} -> {} [label={} {}%];",
                    v.0,
                    key.hash,
                    key.played,
                    (value.visits as f32 / visits as f32) * 100.
                )
            }
        }
        dot_output += "}";
        dot_output
    }
}
