//! UCI chess engine protocol wrapper.

#![warn(missing_docs)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use gambit_db::{Move, MoveParseError};
use thiserror::Error;

/// Result of a UCI `go` search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    /// Engine's chosen move.
    pub bestmove: Move,
    /// Optional expected reply move for pondering.
    pub ponder: Option<Move>,
}

/// Parsed fields from a UCI `info` line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Info {
    /// Search depth in plies.
    pub depth: Option<u32>,
    /// Selective search depth.
    pub seldepth: Option<u32>,
    /// Principal variation index.
    pub multipv: Option<u32>,
    /// Centipawn score, if reported.
    pub score_cp: Option<i32>,
    /// Mate distance in plies, if reported.
    pub score_mate: Option<i32>,
    /// Nodes searched.
    pub nodes: Option<u64>,
    /// Nodes per second.
    pub nps: Option<u64>,
    /// Search time in milliseconds.
    pub time_ms: Option<u64>,
    /// Principal variation moves.
    pub pv: Vec<Move>,
}

/// Errors from UCI engine communication or parsing.
#[derive(Debug, Error)]
pub enum UciError {
    /// I/O failure talking to the engine process.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Failed to spawn the engine executable.
    #[error("failed to spawn engine process: {0}")]
    Spawn(std::io::Error),
    /// Engine process ended unexpectedly.
    #[error("engine process exited unexpectedly")]
    ProcessExited,
    /// Engine stdin was unavailable after spawn.
    #[error("engine stdin unavailable")]
    NoStdin,
    /// Engine stdout was unavailable after spawn.
    #[error("engine stdout unavailable")]
    NoStdout,
    /// Timed out waiting for an expected UCI response.
    #[error("timed out waiting for '{expected}'")]
    Timeout {
        /// Token or line prefix expected from the engine.
        expected: String,
    },
    /// Received a line that could not be parsed.
    #[error("unexpected engine output: {line}")]
    UnexpectedOutput {
        /// Raw line from the engine.
        line: String,
    },
    /// `bestmove` line was missing or malformed.
    #[error("no bestmove received from engine")]
    NoBestmove,
    /// Engine reported no legal move.
    #[error("engine reported no legal move")]
    NoLegalMove,
    /// Failed to parse a UCI move token.
    #[error("invalid UCI move: {0}")]
    InvalidMove(#[from] MoveParseError),
}

/// Handle to a spawned UCI engine process.
pub struct UciEngine {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    read_timeout: Duration,
}

impl UciEngine {
    /// Default timeout for reading engine responses.
    pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(120);

    /// Spawn an engine executable and complete the UCI handshake (`uci` / `uciok`).
    pub fn spawn(command: impl AsRef<str>, args: &[&str]) -> Result<Self, UciError> {
        let mut child = Command::new(command.as_ref())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(UciError::Spawn)?;

        let stdin = child.stdin.take().ok_or(UciError::NoStdin)?;
        let stdout = child.stdout.take().ok_or(UciError::NoStdout)?;

        let mut engine = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            read_timeout: Self::DEFAULT_READ_TIMEOUT,
        };

        engine.send_line("uci")?;
        engine.wait_for_token("uciok", Self::DEFAULT_READ_TIMEOUT)?;
        Ok(engine)
    }

    /// Override the timeout used when waiting for `bestmove`.
    pub fn set_read_timeout(&mut self, timeout: Duration) {
        self.read_timeout = timeout;
    }

    /// Set the position from FEN and optional move list, then search to the given depth.
    pub fn search_depth(
        &mut self,
        fen: &str,
        moves: &[&str],
        depth: u32,
    ) -> Result<SearchResult, UciError> {
        self.send_line("isready")?;
        self.wait_for_token("readyok", Self::DEFAULT_READ_TIMEOUT)?;

        let position = if moves.is_empty() {
            format!("position fen {fen}")
        } else {
            format!("position fen {fen} moves {}", moves.join(" "))
        };
        self.send_line(&position)?;
        self.send_line(&format!("go depth {depth}"))?;
        self.read_bestmove(self.read_timeout)
    }

    /// Send `quit` and wait for the engine process to exit.
    pub fn quit(mut self) -> Result<(), UciError> {
        let _ = self.send_line("quit");
        let _ = self.child.wait();
        Ok(())
    }

    fn send_line(&mut self, line: &str) -> Result<(), UciError> {
        writeln!(self.stdin, "{line}")?;
        self.stdin.flush()?;
        Ok(())
    }

    fn wait_for_token(&mut self, token: &str, timeout: Duration) -> Result<(), UciError> {
        let deadline = Instant::now() + timeout;
        let mut line = String::new();

        loop {
            if Instant::now() >= deadline {
                return Err(UciError::Timeout {
                    expected: token.to_string(),
                });
            }

            line.clear();
            match self.read_line_with_timeout(&mut line, deadline)? {
                true => {
                    if line_contains_token(&line, token) {
                        return Ok(());
                    }
                }
                false => return Err(UciError::ProcessExited),
            }
        }
    }

    fn read_bestmove(&mut self, timeout: Duration) -> Result<SearchResult, UciError> {
        let deadline = Instant::now() + timeout;
        let mut line = String::new();

        loop {
            if Instant::now() >= deadline {
                return Err(UciError::NoBestmove);
            }

            line.clear();
            match self.read_line_with_timeout(&mut line, deadline)? {
                true => {
                    if let Some(result) = parse_bestmove_line(&line)? {
                        return Ok(result);
                    }
                    let _ = parse_info_line(&line);
                }
                false => return Err(UciError::ProcessExited),
            }
        }
    }

    fn read_line_with_timeout(
        &mut self,
        line: &mut String,
        deadline: Instant,
    ) -> Result<bool, UciError> {
        loop {
            if Instant::now() >= deadline {
                return Ok(false);
            }

            let available = self.stdout.fill_buf()?;
            if !available.is_empty() {
                self.stdout.read_line(line)?;
                return Ok(true);
            }

            if self.child.try_wait()?.is_some() {
                return Ok(false);
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Parse a UCI `bestmove` line. Returns `None` if the line is not a bestmove response.
pub fn parse_bestmove_line(line: &str) -> Result<Option<SearchResult>, UciError> {
    let trimmed = line.trim();
    if !trimmed.starts_with("bestmove ") {
        return Ok(None);
    }

    let rest = trimmed.strip_prefix("bestmove ").expect("prefix checked");
    let mut parts = rest.split_whitespace();

    let best_token = parts.next().ok_or(UciError::UnexpectedOutput {
        line: trimmed.to_string(),
    })?;

    if best_token == "(none)" {
        return Err(UciError::NoLegalMove);
    }

    let bestmove = Move::from_uci(best_token)?;

    let ponder = match parts.next() {
        None => None,
        Some("ponder") => {
            let ponder_token = parts.next().ok_or(UciError::UnexpectedOutput {
                line: trimmed.to_string(),
            })?;
            Some(Move::from_uci(ponder_token)?)
        }
        Some(other) => {
            return Err(UciError::UnexpectedOutput {
                line: format!("unexpected token after bestmove: {other}"),
            });
        }
    };

    Ok(Some(SearchResult { bestmove, ponder }))
}

/// Parse a UCI `info` line. Returns `None` if the line is not an info response.
pub fn parse_info_line(line: &str) -> Result<Option<Info>, UciError> {
    let trimmed = line.trim();
    if !trimmed.starts_with("info ") {
        return Ok(None);
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    let mut info = Info::default();
    let mut index = 1;

    while index < tokens.len() {
        match tokens[index] {
            "depth" => {
                info.depth = Some(parse_u32_token(&tokens, &mut index, "depth")?);
            }
            "seldepth" => {
                info.seldepth = Some(parse_u32_token(&tokens, &mut index, "seldepth")?);
            }
            "multipv" => {
                info.multipv = Some(parse_u32_token(&tokens, &mut index, "multipv")?);
            }
            "score" => {
                index += 1;
                if index >= tokens.len() {
                    return Err(UciError::UnexpectedOutput {
                        line: trimmed.to_string(),
                    });
                }
                match tokens[index] {
                    "cp" => {
                        index += 1;
                        info.score_cp = Some(parse_i32_at(&tokens, &mut index, "score cp")?);
                    }
                    "mate" => {
                        index += 1;
                        info.score_mate = Some(parse_i32_at(&tokens, &mut index, "score mate")?);
                    }
                    other => {
                        return Err(UciError::UnexpectedOutput {
                            line: format!("unknown score kind: {other}"),
                        });
                    }
                }
            }
            "nodes" => {
                info.nodes = Some(parse_u64_token(&tokens, &mut index, "nodes")?);
            }
            "nps" => {
                info.nps = Some(parse_u64_token(&tokens, &mut index, "nps")?);
            }
            "time" => {
                info.time_ms = Some(parse_u64_token(&tokens, &mut index, "time")?);
            }
            "pv" => {
                index += 1;
                while index < tokens.len() {
                    let mv = Move::from_uci(tokens[index])?;
                    info.pv.push(mv);
                    index += 1;
                }
                break;
            }
            _ => {
                index += 1;
            }
        }
    }

    Ok(Some(info))
}

fn parse_i32_at(tokens: &[&str], index: &mut usize, label: &str) -> Result<i32, UciError> {
    let value = tokens
        .get(*index)
        .ok_or_else(|| UciError::UnexpectedOutput {
            line: format!("missing value for {label}"),
        })?;
    let parsed = value.parse().map_err(|_| UciError::UnexpectedOutput {
        line: format!("invalid {label} value: {value}"),
    })?;
    *index += 1;
    Ok(parsed)
}

fn parse_u32_token(tokens: &[&str], index: &mut usize, label: &str) -> Result<u32, UciError> {
    *index += 1;
    let value = tokens
        .get(*index)
        .ok_or_else(|| UciError::UnexpectedOutput {
            line: format!("missing value for {label}"),
        })?;
    value.parse().map_err(|_| UciError::UnexpectedOutput {
        line: format!("invalid {label} value: {value}"),
    })
}

fn parse_u64_token(tokens: &[&str], index: &mut usize, label: &str) -> Result<u64, UciError> {
    *index += 1;
    let value = tokens
        .get(*index)
        .ok_or_else(|| UciError::UnexpectedOutput {
            line: format!("missing value for {label}"),
        })?;
    value.parse().map_err(|_| UciError::UnexpectedOutput {
        line: format!("invalid {label} value: {value}"),
    })
}

fn line_contains_token(line: &str, token: &str) -> bool {
    line.split_whitespace().any(|part| part == token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gambit_db::Square;

    #[test]
    fn parses_bestmove_without_ponder() {
        let result = parse_bestmove_line("bestmove e2e4")
            .expect("parse")
            .expect("bestmove");

        assert_eq!(
            result.bestmove.from,
            Square::from_algebraic("e2").expect("square")
        );
        assert_eq!(
            result.bestmove.to,
            Square::from_algebraic("e4").expect("square")
        );
        assert_eq!(result.ponder, None);
    }

    #[test]
    fn parses_bestmove_with_ponder() {
        let result = parse_bestmove_line("bestmove e2e4 ponder e7e5")
            .expect("parse")
            .expect("bestmove");

        assert_eq!(result.bestmove.to_uci(), "e2e4");
        assert_eq!(result.ponder.expect("ponder").to_uci(), "e7e5");
    }

    #[test]
    fn parses_promotion_bestmove() {
        let result = parse_bestmove_line("bestmove e7e8q ponder d8e7")
            .expect("parse")
            .expect("bestmove");

        assert_eq!(result.bestmove.to_uci(), "e7e8q");
        assert_eq!(result.ponder.expect("ponder").to_uci(), "d8e7");
    }

    #[test]
    fn bestmove_none_is_no_legal_move() {
        let err = parse_bestmove_line("bestmove (none)").expect_err("error");
        assert!(matches!(err, UciError::NoLegalMove));
    }

    #[test]
    fn non_bestmove_line_returns_none() {
        assert_eq!(parse_bestmove_line("info depth 10").expect("parse"), None);
    }

    #[test]
    fn parses_info_line_with_cp_score_and_pv() {
        let info = parse_info_line(
            "info depth 12 seldepth 18 multipv 1 score cp 34 nodes 100000 nps 2000000 time 50 pv e2e4 e7e5 g1f3",
        )
        .expect("parse")
        .expect("info");

        assert_eq!(info.depth, Some(12));
        assert_eq!(info.seldepth, Some(18));
        assert_eq!(info.multipv, Some(1));
        assert_eq!(info.score_cp, Some(34));
        assert_eq!(info.score_mate, None);
        assert_eq!(info.nodes, Some(100_000));
        assert_eq!(info.nps, Some(2_000_000));
        assert_eq!(info.time_ms, Some(50));
        assert_eq!(info.pv.len(), 3);
        assert_eq!(info.pv[0].to_uci(), "e2e4");
        assert_eq!(info.pv[2].to_uci(), "g1f3");
    }

    #[test]
    fn parses_info_line_with_mate_score() {
        let info = parse_info_line("info depth 20 score mate 3 pv h7h8q")
            .expect("parse")
            .expect("info");

        assert_eq!(info.depth, Some(20));
        assert_eq!(info.score_cp, None);
        assert_eq!(info.score_mate, Some(3));
        assert_eq!(info.pv.len(), 1);
        assert_eq!(info.pv[0].to_uci(), "h7h8q");
    }

    #[test]
    fn non_info_line_returns_none() {
        assert_eq!(parse_info_line("bestmove e2e4").expect("parse"), None);
    }

    #[test]
    fn rejects_invalid_move_in_bestmove() {
        let err = parse_bestmove_line("bestmove e2e9").expect_err("error");
        assert!(matches!(err, UciError::InvalidMove(_)));
    }
}
