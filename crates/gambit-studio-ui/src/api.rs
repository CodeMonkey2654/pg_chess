//! HTTP client for the Gambit Studio API.

use crate::api_types::{
    BenchResponse, FilesetView, GameDetail, GamesPage, HashFromFenResponse, HealthResponse,
    JobStarted, JobStatus, OpeningMoveStat, PositionGamesPage, SourceDetail, SourceListItem,
    SyncResponse,
};
use gloo_net::http::Request;

fn api_base() -> String {
    option_env!("GAMBIT_STUDIO_API")
        .unwrap_or("http://127.0.0.1:8080")
        .to_string()
}

async fn response_text(resp: gloo_net::http::Response) -> Result<String, String> {
    resp.text().await.map_err(|e| e.to_string())
}

fn api_error(status: u16, body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
            return err.to_string();
        }
    }
    if body.is_empty() {
        format!("HTTP {status}")
    } else {
        body.to_string()
    }
}

async fn get_json<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, String> {
    let url = format!("{}{path}", api_base());
    let resp = Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body = response_text(resp).await?;
    if status >= 400 {
        return Err(api_error(status, &body));
    }
    serde_json::from_str(&body).map_err(|e| format!("{e} (body: {body})"))
}

async fn post_json<B: serde::Serialize, T: serde::de::DeserializeOwned>(
    path: &str,
    body: &B,
) -> Result<T, String> {
    let url = format!("{}{path}", api_base());
    let resp = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(body)
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let text = response_text(resp).await?;
    if status >= 400 {
        return Err(api_error(status, &text));
    }
    serde_json::from_str(&text).map_err(|e| format!("{e} (body: {text})"))
}

pub async fn fetch_health() -> Result<HealthResponse, String> {
    get_json("/api/health").await
}

pub async fn fetch_sources() -> Result<Vec<SourceListItem>, String> {
    get_json("/api/sources").await
}

pub async fn fetch_source_detail(id: i32) -> Result<SourceDetail, String> {
    get_json(&format!("/api/sources/{id}/summary")).await
}

pub async fn fetch_filesets_by_id(source_id: i32) -> Result<Vec<FilesetView>, String> {
    get_json(&format!("/api/filesets?source_id={source_id}")).await
}

pub async fn fetch_filesets_by_name(source_name: &str) -> Result<Vec<FilesetView>, String> {
    let enc = urlencoding_encode(source_name);
    get_json(&format!("/api/filesets?source_name={enc}")).await
}

pub async fn sync_catalog(source: &str, year: i32) -> Result<SyncResponse, String> {
    post_json(
        "/api/filesets/sync",
        &serde_json::json!({ "source": source, "year": year }),
    )
    .await
}

pub async fn load_year(source: &str, year: i32) -> Result<JobStarted, String> {
    post_json(
        "/api/filesets/load-year",
        &serde_json::json!({ "source": source, "year": year }),
    )
    .await
}

pub async fn fetch_active_job(source_name: &str, year: i32) -> Result<Option<JobStatus>, String> {
    let enc = urlencoding_encode(source_name);
    get_json(&format!(
        "/api/jobs/active?source_name={enc}&year={year}"
    ))
    .await
}

pub async fn fetch_job(job_id: u64) -> Result<JobStatus, String> {
    get_json(&format!("/api/jobs/{job_id}")).await
}

pub async fn fetch_games(
    player: Option<&str>,
    source_id: Option<i32>,
    offset: i64,
    limit: i64,
) -> Result<GamesPage, String> {
    let mut path = format!("/api/games?offset={offset}&limit={limit}");
    if let Some(sid) = source_id {
        path.push_str(&format!("&source_id={sid}"));
    }
    if let Some(p) = player.filter(|s| !s.is_empty()) {
        path.push_str(&format!("&player={}", urlencoding_encode(p)));
    }
    get_json(&path).await
}

pub async fn fetch_game(id: i64) -> Result<GameDetail, String> {
    get_json(&format!("/api/games/{id}")).await
}

pub async fn hash_from_fen(fen: &str) -> Result<i64, String> {
    let resp: HashFromFenResponse = post_json(
        "/api/positions/hash",
        &serde_json::json!({ "fen": fen }),
    )
    .await?;
    Ok(resp.hash)
}

pub async fn fetch_opening_stats(hash: i64) -> Result<Vec<OpeningMoveStat>, String> {
    get_json(&format!("/api/opening/{hash}")).await
}

pub async fn fetch_games_by_position(
    hash: i64,
    offset: i64,
    limit: i64,
) -> Result<PositionGamesPage, String> {
    get_json(&format!(
        "/api/games/by-position/{hash}?offset={offset}&limit={limit}"
    ))
    .await
}

pub async fn run_bench() -> Result<BenchResponse, String> {
    post_json("/api/bench/queries", &serde_json::json!({})).await
}

fn urlencoding_encode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '%' => "%25".to_string(),
            '&' => "%26".to_string(),
            _ if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' => c.to_string(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect()
}
