//! Read-path latency checks against a loaded corpus.
//!
//! Run on a large database:
//! ```text
//! DATABASE_URL=postgres://... cargo test -p gambit-studio-server read_perf -- --ignored --nocapture
//! ```

use gambit_studio_server::db::{games_by_position, search_games, source_detail};
use std::time::Instant;
use tokio_postgres::NoTls;

fn database_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

async fn connect() -> Option<tokio_postgres::Client> {
    let url = database_url()?;
    let (client, connection) = tokio_postgres::connect(&url, NoTls).await.ok()?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    Some(client)
}

async fn corpus_games(client: &tokio_postgres::Client) -> i64 {
    client
        .query_one("SELECT count(*)::bigint FROM gambit.games", &[])
        .await
        .map(|r| r.get(0))
        .unwrap_or(0)
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and a loaded corpus"]
async fn read_perf_hot_paths() {
    let client = connect()
        .await
        .expect("DATABASE_URL must be set for read_perf tests");

    let games = corpus_games(&client).await;
    eprintln!("corpus games: {games}");
    assert!(games > 0, "need at least one game in gambit.games");

    let source_id: i32 = client
        .query_one(
            "SELECT id FROM gambit.sources ORDER BY id LIMIT 1",
            &[],
        )
        .await
        .expect("source row")
        .get(0);

    let start = Instant::now();
    let detail = source_detail(&client, source_id)
        .await
        .expect("source_detail")
        .expect("source exists");
    let source_ms = start.elapsed().as_secs_f64() * 1000.0;
    eprintln!(
        "source_detail ({} games): {:.1} ms",
        detail.games, source_ms
    );
    assert!(
        source_ms < 50.0,
        "source_detail took {source_ms:.1} ms (target <50 ms)"
    );

    let hash: i64 = client
        .query_one(
            "SELECT hash FROM gambit.positions WHERE source_id = $1 AND ply = 20 LIMIT 1",
            &[&source_id],
        )
        .await
        .expect("sample position hash")
        .get(0);

    let start = Instant::now();
    let page = games_by_position(&client, hash, Some(source_id), 0, 25, None)
        .await
        .expect("games_by_position");
    let position_ms = start.elapsed().as_secs_f64() * 1000.0;
    eprintln!(
        "games_by_position ({} total): {:.1} ms",
        page.total, position_ms
    );
    assert!(
        position_ms < 200.0,
        "games_by_position took {position_ms:.1} ms (target <200 ms)"
    );

    let start = Instant::now();
    let search = search_games(&client, Some("car"), Some(source_id), 0, 40, false, None)
        .await
        .expect("search_games");
    let search_ms = start.elapsed().as_secs_f64() * 1000.0;
    eprintln!(
        "search_games ({} rows): {:.1} ms",
        search.games.len(),
        search_ms
    );
    assert!(
        search_ms < 500.0,
        "search_games took {search_ms:.1} ms (target <500 ms at 1.6M games; keyset + no count)"
    );
}

#[tokio::test]
#[ignore = "requires DATABASE_URL and a loaded corpus"]
async fn run_bench_suite() {
    let client = connect()
        .await
        .expect("DATABASE_URL must be set for read_perf tests");

    let games = corpus_games(&client).await;
    eprintln!("corpus games: {games}");

    let bench = gambit_studio_server::db::run_bench(&client)
        .await
        .unwrap_or_else(|e| {
            eprintln!("run_bench note: {e:#}");
            panic!("run_bench failed");
        });

    for row in &bench.results {
        eprintln!(
            "{:>28} {:>8.1} ms  rows={}",
            row.id, row.latency_ms, row.rows
        );
        if row.id == "opening_explorer" && row.latency_ms == 0.0 {
            continue;
        }
    }

    let source = bench
        .results
        .iter()
        .find(|r| r.id == "source_aggregation")
        .expect("source_aggregation bench");
    assert!(
        source.latency_ms < 50.0,
        "source_aggregation took {:.1} ms",
        source.latency_ms
    );

    let player = bench
        .results
        .iter()
        .find(|r| r.id == "player_search")
        .expect("player_search bench");
    eprintln!(
        "player_search bench: {:.1} ms (UI path uses keyset UNION, no COUNT)",
        player.latency_ms
    );
}
