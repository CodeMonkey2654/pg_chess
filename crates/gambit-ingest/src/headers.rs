//! Map exploded PGN games to database row values.

use chrono::NaiveDate;
use gambit_db::ExplodedGame;

/// Staging row for `gambit.staging_games`.
#[derive(Debug, Clone)]
pub struct StagingGameRow {
    /// Batch-local sequence number.
    pub batch_seq: i32,
    /// Optional full PGN text.
    pub pgn_text: Option<String>,
    /// SHA-256 of source archive (optional).
    pub pgn_sha256: Option<Vec<u8>>,
    /// Byte offset in source archive (optional).
    pub pgn_byte_offset: Option<i64>,
    /// White player.
    pub white: Option<String>,
    /// Black player.
    pub black: Option<String>,
    /// White Elo.
    pub white_elo: Option<i32>,
    /// Black Elo.
    pub black_elo: Option<i32>,
    /// Event name.
    pub event: Option<String>,
    /// Site.
    pub site: Option<String>,
    /// Round.
    pub round: Option<String>,
    /// Game date.
    pub game_date: Option<NaiveDate>,
    /// Result code.
    pub result: String,
    /// ECO code.
    pub eco: Option<String>,
    /// Number of plies played.
    pub ply_count: i32,
}

impl StagingGameRow {
    /// Build a staging game row from an exploded game.
    pub fn from_exploded(
        batch_seq: i32,
        exploded: &ExplodedGame,
        pgn_text: Option<String>,
        pgn_sha256: Option<Vec<u8>>,
        pgn_byte_offset: Option<i64>,
    ) -> Self {
        let h = &exploded.headers;
        Self {
            batch_seq,
            pgn_text,
            pgn_sha256,
            pgn_byte_offset,
            white: h.white.clone(),
            black: h.black.clone(),
            white_elo: h.white_elo,
            black_elo: h.black_elo,
            event: h.event.clone(),
            site: h.site.clone(),
            round: h.round.clone(),
            game_date: h.date,
            result: h.result.clone(),
            eco: h.eco.clone(),
            ply_count: exploded.plies.len() as i32,
        }
    }
}

/// Escape a field for PostgreSQL COPY text format.
pub fn copy_field(value: Option<&str>) -> String {
    match value {
        None => "\\N".to_string(),
        Some(s) => escape_copy_text(s),
    }
}

/// Escape a required COPY text field.
pub fn copy_field_req(value: &str) -> String {
    escape_copy_text(value)
}

fn escape_copy_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => {
                out.push('\\');
                out.push('\\');
            }
            '\t' => {
                out.push('\\');
                out.push('t');
            }
            '\n' => {
                out.push('\\');
                out.push('n');
            }
            '\r' => {
                out.push('\\');
                out.push('r');
            }
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_field_null() {
        assert_eq!(copy_field(None), "\\N");
    }

    #[test]
    fn copy_field_escapes_tab() {
        assert_eq!(copy_field(Some("a\tb")), "a\\tb");
    }
}
