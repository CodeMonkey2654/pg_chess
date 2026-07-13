# Changelog

## Unreleased

- Opinionated API: `Position::from_fen` returns `Result` with strict validation
- `legal_moves()` returns `Vec<Move>`; removed `MoveList` newtype
- Embedded occupancy bitboards (always on); knight/king attack tables
- `ChessGame` stores resolved legal moves; incremental `hash_history` for threefold
- PGN: resolved moves, NAGs, comments, nested RAV parse/write
- Syzygy probing via `shakmaty-syzygy` behind `tablebase` feature
- Added `gambit_db::prelude`
- Multi-game PGN splitting (`split_pgn_games`, `parse_pgn_games`)
- FEN / SetUp header support in PGN parser
- Ingest-optimized `explode_mainline` API with typed `GameHeaders`
