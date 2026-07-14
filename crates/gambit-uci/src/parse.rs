//! UCI line parsing.

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
    /// Invalid position command.
    #[error("invalid position: {0}")]
    InvalidPosition(String),
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

/// Parsed `go` command limits.
#[derive(Debug, Default, Clone, Copy)]
pub struct GoParams {
    /// Search depth in plies.
    pub depth: Option<u32>,
    /// Movetime in milliseconds.
    pub movetime_ms: Option<u64>,
}

/// Parse `go depth N` or `go movetime N`.
pub fn parse_go_line(line: &str) -> GoParams {
    let mut params = GoParams::default();
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let mut i = 1;
    while i < tokens.len() {
        match tokens[i] {
            "depth" => {
                i += 1;
                if i < tokens.len() {
                    if let Ok(d) = tokens[i].parse() {
                        params.depth = Some(d);
                    }
                }
            }
            "movetime" => {
                i += 1;
                if i < tokens.len() {
                    if let Ok(t) = tokens[i].parse() {
                        params.movetime_ms = Some(t);
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    params
}

/// Build a position from `position fen ... [moves ...]` or `position startpos [moves ...]`.
pub fn parse_position_line(line: &str) -> Result<gambit_db::Position, UciError> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 2 || tokens[0] != "position" {
        return Err(UciError::InvalidPosition(
            "expected position command".into(),
        ));
    }

    let mut pos = if tokens[1] == "startpos" {
        gambit_db::Position::starting_position()
    } else if tokens[1] == "fen" {
        if tokens.len() < 3 {
            return Err(UciError::InvalidPosition("fen requires fields".into()));
        }
        let fen_end = tokens
            .iter()
            .position(|&t| t == "moves")
            .unwrap_or(tokens.len());
        let fen = tokens[2..fen_end].join(" ");
        gambit_db::Position::from_fen(&fen).map_err(|e| UciError::InvalidPosition(e.to_string()))?
    } else {
        return Err(UciError::InvalidPosition("expected fen or startpos".into()));
    };

    if let Some(moves_idx) = tokens.iter().position(|&t| t == "moves") {
        for uci in &tokens[moves_idx + 1..] {
            let mv = Move::from_uci(uci)?;
            pos = pos
                .apply_move(mv)
                .map_err(|e| UciError::InvalidPosition(e.to_string()))?;
        }
    }

    Ok(pos)
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
        assert_eq!(info.score_cp, Some(34));
        assert_eq!(info.pv.len(), 3);
    }

    #[test]
    fn parses_position_startpos() {
        let pos = parse_position_line("position startpos").expect("parse");
        assert_eq!(pos, gambit_db::Position::starting_position());
    }

    #[test]
    fn parses_position_with_moves() {
        let pos = parse_position_line("position startpos moves e2e4 e7e5").expect("parse");
        assert_eq!(
            pos.to_fen(),
            "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2"
        );
    }

    #[test]
    fn parses_go_depth() {
        let go = parse_go_line("go depth 8");
        assert_eq!(go.depth, Some(8));
    }
}
