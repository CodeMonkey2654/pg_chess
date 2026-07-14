//! WASM bindings for client-side chess board operations.

use gambit_db::prelude::*;
use wasm_bindgen::prelude::*;

/// Chess position wrapper for JavaScript.
#[wasm_bindgen]
pub struct WasmPosition {
    inner: Position,
}

#[wasm_bindgen]
impl WasmPosition {
    /// Parse a position from FEN.
    #[wasm_bindgen(constructor)]
    pub fn from_fen(fen: &str) -> Result<WasmPosition, JsValue> {
        Position::from_fen(fen)
            .map(|inner| Self { inner })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Return the current FEN string.
    #[wasm_bindgen(js_name = toFen)]
    pub fn to_fen(&self) -> String {
        self.inner.to_fen()
    }

    /// Apply a UCI move and return the resulting position.
    #[wasm_bindgen(js_name = applyMove)]
    pub fn apply_move(&self, uci: &str) -> Result<WasmPosition, JsValue> {
        let mv = Move::from_uci(uci).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.inner
            .apply_move(mv)
            .map(|inner| Self { inner })
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Return legal moves as UCI strings.
    #[wasm_bindgen(js_name = legalMoves)]
    pub fn legal_moves(&self) -> Vec<String> {
        self.inner
            .legal_moves()
            .into_iter()
            .map(|m| m.to_uci())
            .collect()
    }

    /// Whether the side to move is in check.
    #[wasm_bindgen(js_name = isInCheck)]
    pub fn is_in_check(&self) -> bool {
        self.inner.is_in_check(self.inner.side_to_move)
    }

    /// Side to move: `"white"` or `"black"`.
    #[wasm_bindgen(js_name = sideToMove)]
    pub fn side_to_move(&self) -> String {
        match self.inner.side_to_move {
            Color::White => "white".to_string(),
            Color::Black => "black".to_string(),
        }
    }

    /// Zobrist hash as signed i64 (matches PostgreSQL storage).
    #[wasm_bindgen(js_name = zobristHash)]
    pub fn zobrist_hash(&self) -> i64 {
        self.inner.zobrist_hash() as i64
    }
}

/// Starting position FEN.
#[wasm_bindgen(js_name = startFen)]
pub fn start_fen() -> String {
    Position::starting_position().to_fen()
}
