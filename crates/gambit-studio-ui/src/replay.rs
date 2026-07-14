//! Game replay helpers.

use crate::api_types::GameDetail;
use crate::board::uci::parse_uci;
use gambit_db_wasm::WasmPosition;

/// Error applying moves during replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayError {
    InvalidFen(String),
    InvalidMove { ply: usize, uci: String },
}

/// Rebuild board state at a ply index.
pub fn position_at_ply(
    detail: &GameDetail,
    ply_idx: usize,
) -> Result<(WasmPosition, Option<(String, String)>), ReplayError> {
    let mut pos = WasmPosition::from_fen(&detail.start_fen)
        .map_err(|e| ReplayError::InvalidFen(e.as_string().unwrap_or_default()))?;
    let mut last = None;
    for (i, ply) in detail.plies.iter().enumerate().take(ply_idx) {
        if let Some(parsed) = parse_uci(&ply.uci) {
            last = Some((parsed.from, parsed.to));
        }
        pos = pos
            .apply_move(&ply.uci)
            .map_err(|_| ReplayError::InvalidMove {
                ply: i + 1,
                uci: ply.uci.clone(),
            })?;
    }
    Ok((pos, last))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api_types::PlyView;

    #[test]
    fn start_position_at_ply_zero() {
        let detail = GameDetail {
            id: 1,
            source_id: 1,
            white: None,
            black: None,
            result: "1-0".into(),
            event: None,
            plies: vec![PlyView {
                ply: 1,
                san: "e4".into(),
                uci: "e2e4".into(),
            }],
            start_fen: gambit_db_wasm::start_fen(),
        };
        let (pos, last) = position_at_ply(&detail, 0).unwrap();
        assert_eq!(pos.to_fen(), detail.start_fen);
        assert!(last.is_none());
        let (pos, _) = position_at_ply(&detail, 1).unwrap();
        assert!(pos.to_fen().contains("4P3"));
    }
}
