# gambit-db

Rust chess engine crate: board, FEN, legal move generation, SAN/PGN, Zobrist hashing, and game state.

## Quick start

```rust
use gambit_db::prelude::*;

let pos = Position::from_fen(
    "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
)?;
let e4 = Move::from_uci("e2e4")?;
let after = pos.apply_move(e4)?;

let mut game = ChessGame::new();
game.play(e4)?;
```

PGN parsing stores resolved moves, NAGs, comments, and nested variations (`PgnMove::variations`).
Use `write_pgn_movetext` to round-trip movetext trees.

## Features

| Feature | Description |
|---------|-------------|
| `tablebase` | Syzygy tablebase probing (`Tablebase::open`, `probe_wdl`, `probe_dtz`) |

Bitboard occupancy is always enabled (not optional).

## Syzygy tablebases

Enable `tablebase` and place `.rtbw`/`.rtbz` files in a directory:

```rust
use gambit_db::prelude::*;

let tb = Tablebase::open("/path/to/tb")?;
let pos = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1")?;
if let Some(wdl) = tb.probe_wdl(&pos) {
    // Five-valued Syzygy WDL: Loss, BlessedLoss, Draw, CursedWin, Win
    println!("WDL: {wdl:?}");
}
```

Download tables from [syzygy-tables.info](https://syzygy-tables.info/).

Integration tests with real files: set `SYZYGY_PATH` and run
`cargo test -p gambit-db --features tablebase -- --ignored`.

## License

MIT
