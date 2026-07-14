# Analysis engine

Native in-memory chess analysis with optional corpus book augmentation from the gambit PostgreSQL schema.

## Crates

| Crate | Role |
|-------|------|
| [`gambit-analysis`](../../crates/gambit-analysis/) | Search library: negamax, TT, eval, optional `CorpusBook` |
| [`gambit-uci`](../../crates/gambit-uci/) | UCI client (external engines) + native UCI server binary |

Postgres stores and aggregates corpus statistics; search runs entirely in Rust. Export a `.gbook` file after ingest for move-ordering hints without per-node database queries.

## Library usage

```rust
use gambit_analysis::{Analyzer, SearchLimits};
use gambit_db::Position;

let pos = Position::starting_position();
let mut analyzer = Analyzer::new();
let result = analyzer.search(&pos, SearchLimits::depth(8));

println!("best: {}", result.best_move.to_uci());
println!("depth: {} nodes: {}", result.depth, result.nodes);
```

Load a corpus book exported from PostgreSQL:

```rust
use gambit_analysis::CorpusBook;

let book = CorpusBook::load("corpus.gbook")?;
let analyzer = Analyzer::new().with_book(book);
```

## UCI engine binary

```powershell
cargo run -p gambit-uci --bin gambit-analysis --release
```

Then send standard UCI commands on stdin:

```
uci
isready
position startpos moves e2e4
go depth 8
quit
```

Optional corpus book:

```powershell
cargo run -p gambit-uci --bin gambit-analysis --release -- --book corpus.gbook
# or set GAMBIT_BOOK=corpus.gbook
```

Supported commands: `uci`, `isready`, `ucinewgame`, `position fen|startpos [moves ...]`, `go depth N`, `go movetime N`, `quit`.

## Corpus workflow (with built database)

After importing games and refreshing statistics:

```powershell
cargo run -p gambit-ingest -- refresh-stats --pg-uri $env:DATABASE_URL
cargo run -p gambit-ingest -- export-book --pg-uri $env:DATABASE_URL --output corpus.gbook
cargo run -p gambit-uci --bin gambit-analysis --release -- --book corpus.gbook
```

The `export-book` command reads `gambit.opening_moves` (move stats for every position in the corpus, keyed by Zobrist hash) and writes a compact binary `.gbook` file. The engine memory-maps this at startup for move ordering and root-level corpus reporting.

## Benchmarks

```powershell
cargo bench -p gambit-analysis
cargo bench -p gambit-db --bench movegen
.\scripts\bench_gate.ps1
```

## Tests

```powershell
cargo test -p gambit-analysis --test puzzles
```

Puzzle tests verify mate-in-one detection and score consistency.

## Optional features

| Feature | Crate | Description |
|---------|-------|-------------|
| `tablebase` | `gambit-analysis` | Enables Syzygy probing via `gambit-db/tablebase` (future root/leaf integration) |

## Architecture note

Deep search cannot use PostgreSQL per node (millisecond round-trips vs nanosecond TT probes). The hybrid model is: **Postgres aggregates offline → `.gbook` hydrates RAM once → search stays in memory**. See [architecture.md](architecture.md).
