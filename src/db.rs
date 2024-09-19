use std::path::Path;

use chrono::NaiveDate;
use quick_xml::{events::Event, Reader};
use whr::WhrBuilder;

#[derive(Debug, Clone)]
pub struct Game {
    pub black_elo: u64,
    pub white_elo: u64,
    pub moves: Vec<(u8, u8)>,
}

/// Parses a database of games.
pub fn load_database<P: AsRef<Path>>(data_path: P) -> Result<Vec<Game>, ()> {
    let mut reader = Reader::from_file(data_path).unwrap();
    let mut buffer = vec![];

    let mut games = vec![];
    let mut tournament_timesteps = vec![];

    let mut current_game_is_init = false;
    let mut black = 0;
    let mut white = 0;
    let mut result = 0.5;
    let mut timestep = 0;
    let mut moves = vec![];
    'read: loop {
        match reader.read_event_into(&mut buffer).unwrap() {
            Event::Eof => break,
            Event::Empty(e) => {
                if e.name().as_ref() == b"tournament" {
                    let mut timestep = 0;
                    let mut index: usize = 0;
                    for attr in e.attributes().filter_map(|a| a.ok()) {
                        match attr.key.as_ref() {
                            b"id" => {
                                index = std::str::from_utf8(attr.value.as_ref())
                                    .unwrap()
                                    .parse()
                                    .unwrap()
                            }
                            b"start" => {}
                            b"end" => {
                                let date = std::str::from_utf8(&attr.value).unwrap();
                                let mut parts = date.split('-');
                                let year = parts.next().unwrap().parse().unwrap();
                                let month = parts.next().unwrap().parse().unwrap();
                                let day = parts.next().unwrap().parse().unwrap();
                                timestep = NaiveDate::from_ymd_opt(year, month, day)
                                    .map(|d| {
                                        d.signed_duration_since(NaiveDate::default()).num_days()
                                            as usize
                                    })
                                    .unwrap();
                            }
                            _ => {}
                        }
                    }
                    tournament_timesteps.resize(index, 0);
                    tournament_timesteps[index - 1] = timestep
                }
            }
            Event::End(e) => match e.name().as_ref() {
                b"game" if current_game_is_init => games.push((
                    black,
                    white,
                    if result == 1.0 {
                        Some(black)
                    } else if result == 0.5 {
                        None
                    } else if result == 0.0 {
                        Some(white)
                    } else {
                        panic!()
                    },
                    timestep,
                    moves.clone(),
                )),
                _ => {}
            },
            Event::Start(e) => match e.name().as_ref() {
                b"game" => {
                    current_game_is_init = true;
                    for attr in e.attributes().filter_map(|a| a.ok()) {
                        match attr.key.as_ref() {
                            b"tournament" => {
                                let tournament = std::str::from_utf8(&attr.value)
                                    .unwrap()
                                    .parse::<usize>()
                                    .unwrap();
                                timestep = tournament_timesteps[tournament - 1];
                            }
                            b"rated" => {
                                if std::str::from_utf8(&attr.value)
                                    .unwrap()
                                    .parse::<u8>()
                                    .unwrap()
                                    != 1
                                {
                                    current_game_is_init = false;
                                    continue 'read;
                                }
                            }
                            b"rule" => {
                                if std::str::from_utf8(&attr.value)
                                    .unwrap()
                                    .parse::<u8>()
                                    .unwrap()
                                    != 1
                                {
                                    current_game_is_init = false;
                                    continue 'read;
                                }
                            }
                            b"black" => {
                                black = std::str::from_utf8(&attr.value).unwrap().parse().unwrap()
                            }
                            b"white" => {
                                white = std::str::from_utf8(&attr.value).unwrap().parse().unwrap()
                            }
                            b"bresult" => {
                                result = std::str::from_utf8(&attr.value)
                                    .unwrap()
                                    .parse::<f32>()
                                    .unwrap();
                            }
                            _ => {}
                        }
                    }
                }
                b"move" => match reader.read_event_into(&mut buffer).unwrap() {
                    Event::Text(t) => {
                        moves.clear();
                        let str = t.unescape().unwrap();
                        for m in str.split_whitespace() {
                            let m = m.trim();
                            if m.len() < 2 || m.len() > 3 {
                                return Err(());
                            };
                            let x = m.chars().next().unwrap() as u8 - 'a' as u8;
                            let y = &m[1..].parse::<u8>().unwrap() - 1;

                            moves.push((x, y))
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            _ => (),
        }
    }
    let whr = WhrBuilder::default()
        .with_games(games.iter().map(|(b, w, r, t, _)| {
            assert_ne!(*t, 0);
            (*b, *w, *r, *t, None)
        }))
        .with_w2(19.3)
        .with_virtual_games(2)
        .build();

    Ok(games
        .into_iter()
        .map(|(black, white, _, time, moves)| Game {
            black_elo: (whr.rating(&black, time).unwrap().elo().round() + 1900f64) as u64,
            white_elo: (whr.rating(&white, time).unwrap().elo().round() + 1900f64) as u64,
            moves,
        })
        .collect())
}
