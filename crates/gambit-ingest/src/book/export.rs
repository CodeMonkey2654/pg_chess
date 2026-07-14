//! Export corpus move statistics from PostgreSQL to `.gbook`.

use anyhow::{Context, Result};
use gambit_analysis::{write_book, MoveStat};
use gambit_db::Move;
use std::collections::BTreeMap;
use std::path::Path;
use tokio_postgres::Client;

/// Stream `gambit.opening_moves` into a binary book file.
pub async fn export_book(client: &Client, output: impl AsRef<Path>) -> Result<u64> {
    let rows = client
        .query(
            "SELECT prefix_hash, move_uci, count, white_wins, black_wins, draws \
             FROM gambit.opening_moves \
             ORDER BY prefix_hash, count DESC",
            &[],
        )
        .await
        .context("query opening_moves")?;

    if rows.is_empty() {
        anyhow::bail!(
            "opening_moves is empty; run `gambit-ingest refresh-stats` after import first"
        );
    }

    let mut by_hash: BTreeMap<u64, Vec<MoveStat>> = BTreeMap::new();
    for row in &rows {
        let hash: i64 = row.get(0);
        let uci: String = row.get(1);
        let count: i64 = row.get(2);
        let white_wins: i64 = row.get(3);
        let black_wins: i64 = row.get(4);
        let draws: i64 = row.get(5);

        let mv = Move::from_uci(&uci).with_context(|| format!("invalid uci in corpus: {uci}"))?;
        by_hash.entry(hash as u64).or_default().push(MoveStat {
            uci: mv,
            count: count as u64,
            white_wins: white_wins as u64,
            black_wins: black_wins as u64,
            draws: draws as u64,
        });
    }

    let entries: Vec<(u64, Vec<MoveStat>)> = by_hash.into_iter().collect();
    let positions = entries.len() as u64;
    write_book(output.as_ref(), &entries).context("write book file")?;
    Ok(positions)
}
