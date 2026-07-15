//! Shared UI helpers for page panels.

use crate::api;
use crate::replay::ReplayError;
use gambit_db_wasm::WasmPosition;

pub fn shard_status_label(status: &str) -> &'static str {
    match status {
        "complete" => "Done",
        "downloading" => "Downloading",
        "ingesting" => "Ingesting",
        "failed" => "Failed",
        _ => "Pending",
    }
}

pub fn move_class_label(class: &str) -> &'static str {
    match class {
        "best" => "Best",
        "good" => "Good",
        "inaccuracy" => "Inaccuracy",
        "mistake" => "Mistake",
        "blunder" => "Blunder",
        _ => "",
    }
}

pub fn move_class_css(class: &str) -> &'static str {
    match class {
        "best" => "move-best",
        "good" => "move-good",
        "inaccuracy" => "move-inaccuracy",
        "mistake" => "move-mistake",
        "blunder" => "move-blunder",
        _ => "",
    }
}

pub fn format_accuracy(value: Option<f32>) -> String {
    value
        .map(|v| format!("{v:.1}%"))
        .unwrap_or_else(|| "—".to_string())
}

pub fn event_target_value(ev: &leptos::ev::Event) -> String {
    use wasm_bindgen::JsCast;
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|el| el.value())
        .unwrap_or_default()
}

pub fn fen_hash_local(fen: &str) -> Option<i64> {
    WasmPosition::from_fen(fen).ok().map(|p| p.zobrist_hash())
}

pub async fn resolve_position_hash(fen: &str) -> Result<i64, String> {
    if let Some(h) = fen_hash_local(fen) {
        return Ok(h);
    }
    api::hash_from_fen(fen).await
}

pub fn replay_error_message(err: &ReplayError) -> String {
    match err {
        ReplayError::InvalidFen(f) => format!("Invalid start FEN: {f}"),
        ReplayError::InvalidMove { ply, uci } => format!("Invalid move at ply {ply}: {uci}"),
    }
}
