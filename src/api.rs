//! Engine Logic statys in other files: this is just sql facing API
//! Postgres calls this. Engine logic stays not here; this file is the adapter to the API

use crate::fen::Position;
use pgrx::prelude::*;
use pgrx::{InOutFuncs, StringInfo};
use serde::{Deserialize, Serialize};
use std::ffi::CStr;
use crate::movement::Move;

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, PostgresType)]
#[inoutfuncs]
pub struct chess_position(pub Position);

impl InOutFuncs for chess_position {
    fn input(input: &CStr) -> Self {
        let s = input
            .to_str()
            .unwrap_or_else(|_| {
                error!("chess_position input was not valid UTF-8")
            });

        match Position::from_fen(s) {
            Some(pos) => chess_position(pos),
            None => error!("invalid FEN for chess position: '{}'", s),
        }
    }

    fn output(&self, buffer: &mut StringInfo) {
        buffer.push_str(&self.0.to_fen());
    }
}

#[allow(non_camel_case_types)]
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    PostgresType
)]
#[inoutfuncs]
pub struct chess_move(pub Move);

impl InOutFuncs for chess_move {
    fn input(input: &CStr) -> Self {
        let input = input.to_str().unwrap_or_else(|_| {
            error!("chess_move input was not valid UTF-8")
        });

        match Move::from_uci(input) {
            Ok(mv) => chess_move(mv),
            Err(err) => {
                error!("invalid UCI chess move '{}': {}", input, err)
            }
        }
    }

    fn output(&self, buffer: &mut StringInfo) {
        buffer.push_str(&self.0.to_uci());
    }
}

#[pg_extern]
fn chess_start_position() -> chess_position {
    chess_position(Position::starting_position())
}

#[pg_extern]
fn chess_is_valid_fen(fen: &str) -> bool {
    Position::from_fen(fen).is_some()
}

#[pg_extern]
fn chess_to_fen(pos: chess_position) -> String {
    pos.0.to_fen()
}

#[pg_extern]
fn chess_from_fen(fen: &str) -> chess_position {
    match Position::from_fen(fen) {
        Some(pos) => chess_position(pos),
        None => error!("invalid FEN: '{}'", fen),
    }
}

#[pg_extern]
fn chess_side_to_move(pos: chess_position) -> String {
    match pos.0.side_to_move {
        crate::types::Color::White => "w".to_string(),
        crate::types::Color::Black => "b".to_string(),
    }
}

#[pg_extern]
fn chess_fullmove_number(pos: chess_position) -> i32 {
    pos.0.fullmove_number as i32
}

#[pg_extern]
fn chess_is_valid_uci(uci: &str) -> bool {
    Move::from_uci(uci).is_ok()
}

#[pg_extern]
fn chess_move_from_uci(uci: &str) -> chess_move {
    match Move::from_uci(uci) {
        Ok(mv) => chess_move(mv),
        Err(err) => error!("invalid UCI chess move '{}': {}", uci, err),
    }
}

#[pg_extern]
fn chess_move_to_uci(mv: chess_move) -> String {
    mv.0.to_uci()
}

#[pg_extern]
fn chess_move_from_square(mv: chess_move) -> String {
    mv.0.from.to_algebraic()
}

#[pg_extern]
fn chess_move_to_square(mv: chess_move) -> String {
    mv.0.to.to_algebraic()
}

#[pg_extern]
fn chess_move_promotion(mv: chess_move) -> Option<String> {
    mv.0
        .promotion
        .map(|piece| piece.to_char().to_string())
}



#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn startpos_roundtrips_through_sql() {
        let fen = Spi::get_one::<String>("SELECT chess_to_fen(chess_start_position())")
            .expect("SPI query failed")
            .expect("SPI returned NULL");
        assert_eq!(
            fen,
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        );
    }

    #[pg_test]
    fn text_input_output_uses_fen() {
        let fen = Spi::get_one::<String>(
            "SELECT ('r7/8/8/8/8/8/8/R7 w - - 0 1'::chess_position)::text",
        )
        .expect("SPI query failed")
        .expect("SPI returned NULL");
        assert_eq!(fen, "r7/8/8/8/8/8/8/R7 w - - 0 1");
    }

    #[pg_test]
    fn side_to_move_reads_correctly() {
        let stm = Spi::get_one::<String>(
            "SELECT chess_side_to_move('8/8/8/8/8/8/8/8 b - - 0 1'::chess_position)",
        )
        .expect("SPI query failed")
        .expect("SPI returned NULL");
        assert_eq!(stm, "b");
    }

    #[pg_test]
    fn valid_fen_check() {
        assert!(crate::api::chess_is_valid_fen(
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"
        ));
        assert!(!crate::api::chess_is_valid_fen("not a fen"));
    }

    #[pg_test]
    fn move_type_roundtrips_uci() {
        let uci = Spi::get_one::<String>(
            "SELECT ('e2e4'::chess_move)::text",
        )
        .expect("SPI query failed")
        .expect("SPI returned NULL");

        assert_eq!(uci, "e2e4");
    }

    #[pg_test]
    fn promotion_move_roundtrips_uci() {
        let uci = Spi::get_one::<String>(
            "SELECT ('e7e8q'::chess_move)::text",
        )
        .expect("SPI query failed")
        .expect("SPI returned NULL");

        assert_eq!(uci, "e7e8q");
    }

    #[pg_test]
    fn move_conversion_functions_roundtrip() {
        let uci = Spi::get_one::<String>(
            "SELECT chess_move_to_uci(
                chess_move_from_uci('e2e4')
            )"
        )
        .expect("SPI query failed")
        .expect("SPI return NULL");

        assert_eq!(uci, "e2e4");
    }

    #[pg_test]
    fn move_square_accessors_work() {
        let from = Spi::get_one::<String>(
            "SELECT chess_move_from_square('e2e4'::chess_move)",
        )
        .expect("SPI Query Failed")
        .expect("SPI returned Null");

        let to = Spi::get_one::<String>(
            "SELECT chess_move_to_square('e2e4'::chess_move)",
        )
        .expect("SPI Query failed")
        .expect("SPI returned NULL");

        assert_eq!(from, "e2");
        assert_eq!(to, "e4");
    }

    #[pg_test]
    fn move_promotion_accessor_works() {
        let promotion = Spi::get_one::<String>(
            "SELECT chess_move_promotion('e7e8n'::chess_move)",
        )
        .expect("SPI query failed")
        .expect("SPI returned NULL");

        assert_eq!(promotion, "n");
    }

    #[pg_test]
    fn non_promotion_move_returns_null_promotion() {
        let promotion = Spi::get_one::<String>(
            "SELECT chess_move_promotion('e2e4'::chess_move)",
        )
        .expect("SPI query failed");

        assert_eq!(promotion, None);
    }

    #[pg_test]
    fn valid_uci_check_works() {
        assert!(crate::api::chess_is_valid_uci("e2e4"));
        assert!(crate::api::chess_is_valid_uci("e7e8q"));

        assert!(!crate::api::chess_is_valid_uci("not-a-move"));
        assert!(!crate::api::chess_is_valid_uci("e2e9"));
        assert!(!crate::api::chess_is_valid_uci("e7e8k"));
    }
}