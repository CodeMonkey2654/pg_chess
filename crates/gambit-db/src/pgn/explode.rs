//! Ingest-oriented mainline explosion: positions and plies without cloning.

use crate::fen::Position;
use crate::game::ChessGame;
use crate::movement::Move;
use crate::pgn::error::PgnError;
use crate::pgn::parse::PgnGame;
use chrono::NaiveDate;
use std::collections::HashMap;

/// Normalized game metadata from PGN headers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameHeaders {
    /// White player name.
    pub white: Option<String>,
    /// Black player name.
    pub black: Option<String>,
    /// White Elo rating.
    pub white_elo: Option<i32>,
    /// Black Elo rating.
    pub black_elo: Option<i32>,
    /// Event name.
    pub event: Option<String>,
    /// Site.
    pub site: Option<String>,
    /// Round.
    pub round: Option<String>,
    /// Game date.
    pub date: Option<NaiveDate>,
    /// Game result.
    pub result: String,
    /// ECO opening code.
    pub eco: Option<String>,
}

impl GameHeaders {
    /// Build typed headers from raw PGN tag pairs.
    pub fn from_pgn(headers: &HashMap<String, String>) -> Self {
        Self {
            white: headers.get("White").cloned(),
            black: headers.get("Black").cloned(),
            white_elo: parse_elo(headers.get("WhiteElo")),
            black_elo: parse_elo(headers.get("BlackElo")),
            event: headers.get("Event").cloned(),
            site: headers.get("Site").cloned(),
            round: headers.get("Round").cloned(),
            date: parse_date(headers.get("Date")),
            result: normalize_result(headers.get("Result")),
            eco: headers.get("ECO").cloned(),
        }
    }
}

fn parse_elo(value: Option<&String>) -> Option<i32> {
    let v = value?;
    v.parse().ok()
}

fn parse_date(value: Option<&String>) -> Option<NaiveDate> {
    let v = value?;
    let trimmed = v.trim();
    if trimmed == "????.??.??" || trimmed.is_empty() {
        return None;
    }
    // PGN dates: YYYY.MM.DD or YYYY.MM or YYYY
    let parts: Vec<&str> = trimmed.split('.').collect();
    let year: i32 = parts.first()?.parse().ok()?;
    let month: u32 = parts.get(1).and_then(|m| m.parse().ok()).unwrap_or(1);
    let day: u32 = parts.get(2).and_then(|d| d.parse().ok()).unwrap_or(1);
    NaiveDate::from_ymd_opt(year, month, day)
}

fn normalize_result(value: Option<&String>) -> String {
    match value.map(String::as_str) {
        Some("1-0") => "1-0".to_string(),
        Some("0-1") => "0-1".to_string(),
        Some("1/2-1/2") => "1/2-1/2".to_string(),
        _ => "*".to_string(),
    }
}

/// One position row at a given ply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionRow {
    /// Half-move index from game start (0 = initial position).
    pub ply: u32,
    /// Zobrist hash of this position.
    pub hash: u64,
    /// FEN representation.
    pub fen: String,
}

/// One played move at a given ply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlyRow {
    /// Half-move index (1 = first move played).
    pub ply: u32,
    /// SAN notation.
    pub san: String,
    /// UCI notation.
    pub uci: String,
    /// Resolved move.
    pub resolved: Move,
}

/// Mainline positions and plies extracted for database ingest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplodedGame {
    /// Normalized headers.
    pub headers: GameHeaders,
    /// Starting FEN.
    pub start_fen: String,
    /// Positions from ply 0 through final ply.
    pub positions: Vec<PositionRow>,
    /// Mainline moves from ply 1 through final ply.
    pub plies: Vec<PlyRow>,
}

/// Explode mainline positions and plies from a parsed PGN game.
///
/// Uses incremental replay via [`ChessGame::play`] — no O(n²) position cloning.
pub fn explode_mainline(pgn: &PgnGame) -> Result<ExplodedGame, PgnError> {
    let start = pgn.starting_position()?;
    let start_fen = start.to_fen();
    let headers = GameHeaders::from_pgn(&pgn.headers);

    let mut game = ChessGame::from_position(start);
    let mut positions = Vec::with_capacity(pgn.movetext.moves.len() + 1);
    let mut plies = Vec::with_capacity(pgn.movetext.moves.len());

    positions.push(position_row(0, game.current()));

    for (i, pm) in pgn.movetext.moves.iter().enumerate() {
        game.play(pm.resolved)
            .map_err(|_| PgnError::IllegalMove(pm.san.clone()))?;
        let ply = (i + 1) as u32;
        plies.push(PlyRow {
            ply,
            san: pm.san.clone(),
            uci: pm.resolved.to_uci(),
            resolved: pm.resolved,
        });
        positions.push(position_row(ply, game.current()));
    }

    Ok(ExplodedGame {
        headers,
        start_fen,
        positions,
        plies,
    })
}

fn position_row(ply: u32, pos: &Position) -> PositionRow {
    PositionRow {
        ply,
        hash: pos.hash,
        fen: pos.to_fen(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_pgn;

    const SAMPLE: &str = r#"[Event "Example"]
[White "Alice"]
[Black "Bob"]
[WhiteElo "2400"]
[BlackElo "2300"]
[Date "2024.01.15"]
[Result "1-0"]

1. e4 e5 2. Nf3 1-0
"#;

    #[test]
    fn explode_mainline_counts() {
        let pgn = parse_pgn(SAMPLE).expect("parse");
        let exploded = explode_mainline(&pgn).expect("explode");
        assert_eq!(exploded.plies.len(), 3);
        assert_eq!(exploded.positions.len(), 4);
        assert_eq!(exploded.positions[0].ply, 0);
        assert_eq!(exploded.plies[0].san, "e4");
        assert_eq!(exploded.plies[0].uci, "e2e4");
    }

    #[test]
    fn headers_parsed() {
        let pgn = parse_pgn(SAMPLE).expect("parse");
        let exploded = explode_mainline(&pgn).expect("explode");
        assert_eq!(exploded.headers.white.as_deref(), Some("Alice"));
        assert_eq!(exploded.headers.white_elo, Some(2400));
        assert_eq!(exploded.headers.date, NaiveDate::from_ymd_opt(2024, 1, 15));
        assert_eq!(exploded.headers.result, "1-0");
    }

    #[test]
    fn hashes_are_incremental() {
        let pgn = parse_pgn(SAMPLE).expect("parse");
        let exploded = explode_mainline(&pgn).expect("explode");
        assert_ne!(exploded.positions[0].hash, exploded.positions[1].hash);
        for row in &exploded.positions {
            assert!(!row.fen.is_empty());
        }
    }
}
