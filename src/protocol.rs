//! # Yixin Protocol
//! Implementation of the [Yixin Protocol](https://github.com/accreator/Yixin-protocol/blob/master/protocol.pdf)
//! (which is itself an extension of the [Gomocup protocol](http://web.quick.cz/lastp/protocl2en.htm))
//! to interface with various engines easily.

use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Stdio},
};

#[derive(Debug)]
pub enum EngineError {
    Error(String),
    Unknown(String),
    ResponseParseError(ResponseParseErr),
    IoError(std::io::Error),
    UnexpectedResponse(Response),
}

pub struct Engine {
    pub id: usize,
    process: Child,
}
impl Engine {
    /// Opens a new engine.
    pub fn open_engine(id: usize, command: &str, move_time: u32) -> Result<Self, std::io::Error> {
        let mut command_parts = command.split_whitespace();
        let mut command = std::process::Command::new(command_parts.next().unwrap());
        command.args(command_parts);

        let process = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let mut engine = Self { process, id };

        engine.send_command(Command::Start(15)).unwrap();
        engine
            .send_command(Command::Info {
                key: "timeout_turn",
                value: &move_time.to_string(),
            })
            .unwrap();
        engine
            .send_command(Command::Info {
                key: "thread_num",
                value: "1",
            })
            .unwrap();
        engine
            .send_command(Command::Info {
                key: "rule",
                value: "2",
            })
            .unwrap();
        Ok(engine)
    }

    pub fn close_engine(mut self) {
        self.send_command(Command::End).unwrap();
        self.process.kill().unwrap();
    }

    pub fn send_command<'a>(&mut self, command: Command<'a>) -> Result<Response, EngineError> {
        write!(self.process.stdin.as_mut().unwrap(), "{command}")
            .map_err(|e| EngineError::IoError(e))?;

        log::trace!("[{}] Sent: {command}", self.id);
        if matches!(
            command,
            Command::Info { .. }
                | Command::End
                | Command::HashClear
                | Command::Stop
                | Command::YixinBoard(_)
        ) {
            return Ok(Response::None);
        }

        let response = &mut String::new();
        let mut reader = BufReader::new(self.process.stdout.as_mut().unwrap());
        loop {
            reader
                .read_line(response)
                .map_err(|e| EngineError::IoError(e))?;
            match response
                .parse::<Response>()
                .map_err(EngineError::ResponseParseError)?
            {
                Response::Ok => {
                    return Ok(Response::Ok);
                }
                Response::Move((x, y)) => {
                    return Ok(Response::Move((x, y)));
                }
                Response::Suggest((x, y)) => {
                    return Ok(Response::Move((x, y)));
                }
                Response::Debug(s) => {
                    log::debug!("[{}] {s}", self.id)
                }
                Response::Error(s) => {
                    log::error!("[{}] {s}", self.id);
                    return Err(EngineError::Error(s));
                }
                Response::Unknown(s) => {
                    log::warn!("[{}] {s}", self.id);
                    return Err(EngineError::Unknown(s));
                }
                Response::Message(s) => {
                    log::trace!("[{}] {s}", self.id)
                }
                Response::None => {
                    return Ok(Response::None);
                }
            }
            response.clear()
        }
    }
}

/// Commands sent by the manager to the Renju engine.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Command<'a> {
    Start(u8),
    Begin,
    Stop,
    ShowForbidden,
    HashClear,
    Turn((u8, u8)),
    Board(&'a [(u8, u8)]),
    YixinBoard(&'a [(u8, u8)]),
    Info { key: &'a str, value: &'a str },
    End,
    Restart,
}
impl<'a> std::fmt::Display for Command<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Start(size) => write!(f, "START {size}\r\n"),
            Self::Begin => write!(f, "BEGIN\r\n"),
            Self::Stop => write!(f, "yxstop\r\n"),
            Self::ShowForbidden => write!(f, "yxshowforbid\r\n"),
            Self::HashClear => write!(f, "yxhashclear\r\n"),
            Self::Turn((x, y)) => write!(f, "TURN {x},{y}\r\n"),
            Self::Board(moves) => {
                write!(f, "BOARD\r\n")?;
                for (i, (x, y)) in moves.iter().enumerate() {
                    write!(f, "{x},{y},{}\r\n", if i % 2 == 0 { 1 } else { 2 })?;
                }
                write!(f, "DONE\r\n")
            }
            Self::YixinBoard(moves) => {
                write!(f, "yxboard\r\n")?;
                for (i, (x, y)) in moves.iter().enumerate() {
                    write!(f, "{x},{y},{}\r\n", if i % 2 == 0 { 1 } else { 2 })?;
                }
                write!(f, "DONE\r\n")
            }
            Self::Info { key, value } => write!(f, "INFO {key} {value}\r\n"),
            Self::End => write!(f, "END\r\n"),
            Self::Restart => write!(f, "RESTART\r\n"),
        }
    }
}

#[derive(Debug)]
pub enum ResponseParseErr {
    MissingCommand,
    MissingArgument,
    MissingCoordinate,
    InvalidCoordinate(String),
}

/// Responses from the Renju engine to the manager.
#[derive(Clone, Debug)]
pub enum Response {
    Ok,
    Move((u8, u8)),
    Suggest((u8, u8)),
    Debug(String),
    Error(String),
    Unknown(String),
    Message(String),
    None,
}
impl std::str::FromStr for Response {
    type Err = ResponseParseErr;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut tokens = s.split_whitespace();
        let command = tokens.next().ok_or(ResponseParseErr::MissingCommand)?;
        Ok(match command.to_lowercase().as_str() {
            "ok" => Self::Ok,
            "suggest" => {
                let coords = tokens.next().ok_or(ResponseParseErr::MissingArgument)?;
                let mut coords = coords.split(',');
                let x = coords.next().ok_or(ResponseParseErr::MissingCoordinate)?;
                let y = coords.next().ok_or(ResponseParseErr::MissingCoordinate)?;
                let x = x
                    .parse::<u8>()
                    .map_err(|_| ResponseParseErr::InvalidCoordinate(x.to_string()))?;
                let y = y
                    .parse::<u8>()
                    .map_err(|_| ResponseParseErr::InvalidCoordinate(y.to_string()))?;
                Self::Suggest((x, y))
            }
            "debug" => Self::Debug(tokens.collect::<Vec<_>>().join(" ")),
            "error" => Self::Error(tokens.collect::<Vec<_>>().join(" ")),
            "unknown" => Self::Unknown(tokens.collect::<Vec<_>>().join(" ")),
            "message" => Self::Message(tokens.collect::<Vec<_>>().join(" ")),
            "" => Self::None,
            coords => {
                let mut coords = coords.split(',');
                let x = coords.next().ok_or(ResponseParseErr::MissingCoordinate)?;
                let y = coords.next().ok_or(ResponseParseErr::MissingCoordinate)?;
                let x = x
                    .parse::<u8>()
                    .map_err(|_| ResponseParseErr::InvalidCoordinate(x.to_string()))?;
                let y = y
                    .parse::<u8>()
                    .map_err(|_| ResponseParseErr::InvalidCoordinate(y.to_string()))?;
                Self::Move((x, y))
            }
        })
    }
}
