//! gRPC + Postgres integration tests for Gambit Studio API.

use gambit_ingest::{ImportOptions, IngestSession};
use gambit_proto::studio_service_client::StudioServiceClient;
use gambit_proto::{Empty, GetGameRequest, SearchGamesRequest};
use gambit_studio_server::{PgPool, StudioServer};
use std::path::PathBuf;
use tokio::sync::OnceCell;
use tonic::transport::{Channel, Server};

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/pgn")
        .join(path)
}

static TEST_CLIENT: OnceCell<StudioServiceClient<Channel>> = OnceCell::const_new();

fn database_url() -> Option<String> {
    std::env::var("DATABASE_URL").ok()
}

async fn studio_client() -> Option<StudioServiceClient<Channel>> {
    database_url()?;
    Some(
        TEST_CLIENT
            .get_or_init(|| async {
                let pg_uri = database_url().expect("checked above");
                let ingest_addr =
                    std::env::var("INGEST_ADDR").unwrap_or_else(|_| "http://127.0.0.1:8082".into());
                let studio_addr = std::env::var("STUDIO_TEST_ADDR")
                    .unwrap_or_else(|_| "http://127.0.0.1:18080".into());

                let pool = PgPool::new(&pg_uri).expect("connect pool");

                let mut session = IngestSession::connect(&pg_uri)
                    .await
                    .expect("connect ingest");
                session.migrate().await.expect("migrate");
                let source_id = session
                    .ensure_source("studio_integration_test")
                    .await
                    .expect("ensure source");
                let options = ImportOptions::default();
                let mut prof = None;
                session
                    .import_file(source_id, &fixture("multi_game.pgn"), &options, &mut prof)
                    .await
                    .expect("import multi_game");
                session
                    .import_file(source_id, &fixture("fen_setup.pgn"), &options, &mut prof)
                    .await
                    .expect("import fen_setup");
                session.refresh_stats().await.expect("refresh stats");

                let ingest_channel = Channel::from_shared(ingest_addr.clone())
                    .expect("ingest url")
                    .connect()
                    .await
                    .expect("connect ingest worker");
                let ingest_client =
                    gambit_proto::ingest_service_client::IngestServiceClient::new(ingest_channel);
                let studio = StudioServer::new(pool, ingest_client);
                let service = gambit_proto::studio_service_server::StudioServiceServer::new(studio);

                let listener_addr: std::net::SocketAddr = studio_addr
                    .trim_start_matches("http://")
                    .trim_start_matches("https://")
                    .parse()
                    .expect("studio test listen addr");
                tokio::spawn(async move {
                    Server::builder()
                        .add_service(service)
                        .serve(listener_addr)
                        .await
                        .ok();
                });
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;

                let channel = Channel::from_shared(studio_addr)
                    .expect("studio url")
                    .connect()
                    .await
                    .expect("connect studio");
                StudioServiceClient::new(channel)
            })
            .await
            .clone(),
    )
}

#[tokio::test]
async fn grpc_sources_and_game_detail() {
    let Some(mut client) = studio_client().await else {
        eprintln!("skipping grpc_sources_and_game_detail: DATABASE_URL not set");
        return;
    };

    let sources = client
        .list_sources(Empty {})
        .await
        .expect("list sources")
        .into_inner()
        .sources;
    assert!(sources.iter().any(|s| s.name == "studio_integration_test"));

    let games = client
        .search_games(SearchGamesRequest {
            player: None,
            source_id: None,
            offset: 0,
            limit: 1,
            include_total: Some(true),
            cursor: None,
        })
        .await
        .expect("search games")
        .into_inner();
    let id = games.games[0].id;

    let detail = client
        .get_game(GetGameRequest {
            game_id: id,
            max_plies: None,
        })
        .await
        .expect("get game")
        .into_inner();
    assert!(
        detail.start_fen.contains("4k3") || detail.start_fen.contains("rnbqkbnr"),
        "unexpected start_fen: {}",
        detail.start_fen
    );
}

#[tokio::test]
async fn grpc_games_pagination_disjoint() {
    let Some(mut client) = studio_client().await else {
        eprintln!("skipping grpc_games_pagination_disjoint: DATABASE_URL not set");
        return;
    };

    let p1 = client
        .search_games(SearchGamesRequest {
            player: None,
            source_id: None,
            offset: 0,
            limit: 2,
            include_total: Some(true),
            cursor: None,
        })
        .await
        .expect("page 1")
        .into_inner();
    let p2 = client
        .search_games(SearchGamesRequest {
            player: None,
            source_id: None,
            offset: 2,
            limit: 2,
            include_total: Some(true),
            cursor: None,
        })
        .await
        .expect("page 2")
        .into_inner();

    let ids1: Vec<i64> = p1.games.iter().map(|g| g.id).collect();
    let ids2: Vec<i64> = p2.games.iter().map(|g| g.id).collect();
    for id in &ids2 {
        assert!(!ids1.contains(id));
    }
}
