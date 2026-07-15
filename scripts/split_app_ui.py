#!/usr/bin/env python3
"""Split gambit-studio-ui app.rs into page modules."""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / "crates/gambit-studio-ui/src/app.rs"
PAGES = ROOT / "crates/gambit-studio-ui/src/pages"
PAGES.mkdir(parents=True, exist_ok=True)

lines = APP.read_text(encoding="utf-8").splitlines(keepends=True)


def slice_lines(start: int, end: int) -> str:
    return "".join(lines[start - 1 : end])


util_rs = """//! Shared UI helpers for page panels.

use crate::api;
use crate::replay::ReplayError;
use gambit_db_wasm::WasmPosition;

""" + slice_lines(28, 76) + """

pub fn fen_hash_local(fen: &str) -> Option<i64> {
    WasmPosition::from_fen(fen).ok().map(|p| p.zobrist_hash())
}

pub async fn resolve_position_hash(fen: &str) -> Result<i64, String> {
    if let Some(h) = fen_hash_local(fen) {
        return Ok(h);
    }
    api::hash_from_fen(fen).await
}

""" + slice_lines(85, 90)

(PAGES / "util.rs").write_text(util_rs, encoding="utf-8")

mod_rs = """pub mod bench;
pub mod dashboard;
pub mod explorer;
pub mod games;
pub mod util;

use leptos::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Dashboard,
    Games,
    Explorer,
    Benchmarks,
}

pub const GAMES_PAGE_SIZE: i64 = 40;
pub const EXPLORER_PAGE_SIZE: i64 = 25;

pub use bench::BenchPanel;
pub use dashboard::DashboardPanel;
pub use explorer::ExplorerPanel;
pub use games::GamesPanel;
"""
(PAGES / "mod.rs").write_text(mod_rs, encoding="utf-8")

dashboard_hdr = """//! Dashboard page.

use super::util::{event_target_value, shard_status_label};
use crate::format::format_num;
use crate::job_poll::parse_download_progress;
use gambit_proto::{FilesetView, JobStatus, SourceDetail, SourceListItem};
use leptos::prelude::*;

"""
dashboard_body = slice_lines(337, 641)
dashboard_body = dashboard_body.replace("fn JobProgressCard", "pub(crate) fn JobProgressCard")
dashboard_body = dashboard_body.replace("fn DashboardPanel", "pub fn DashboardPanel")
(PAGES / "dashboard.rs").write_text(dashboard_hdr + dashboard_body, encoding="utf-8")

games_hdr = """//! Games browser and replay page.

use super::util::{
    event_target_value, format_accuracy, move_class_css, move_class_label, replay_error_message,
};
use super::GAMES_PAGE_SIZE;
use crate::api;
use crate::board::uci::parse_uci;
use crate::board::{BoardOrientation, ChessBoard};
use crate::format::format_num;
use crate::replay::position_at_ply;
use gambit_db_wasm::WasmPosition;
use gambit_proto::{GameDetail, GameListItem};
use leptos::prelude::*;

"""
games_body = slice_lines(643, 1158).replace("fn GamesPanel", "pub fn GamesPanel")
(PAGES / "games.rs").write_text(games_hdr + games_body, encoding="utf-8")

explorer_hdr = """//! Opening explorer page.

use super::util::{fen_hash_local, resolve_position_hash};
use super::{Page, EXPLORER_PAGE_SIZE};
use crate::api;
use crate::board::uci::parse_uci;
use crate::board::{BoardOrientation, ChessBoard};
use crate::format::format_num;
use gambit_db_wasm::WasmPosition;
use gambit_proto::{OpeningMoveStat, PositionHit};
use leptos::prelude::*;

"""
explorer_body = slice_lines(1160, 1534).replace("fn ExplorerPanel", "pub fn ExplorerPanel")
(PAGES / "explorer.rs").write_text(explorer_hdr + explorer_body, encoding="utf-8")

bench_hdr = """//! Query benchmark page.

use crate::api;
use crate::format::format_num;
use gambit_proto::BenchResponse;
use leptos::prelude::*;

"""
bench_body = slice_lines(1536, 1627).replace("fn BenchPanel", "pub fn BenchPanel")
(PAGES / "bench.rs").write_text(bench_hdr + bench_body, encoding="utf-8")

app_hdr = """//! Gambit Studio UI shell.

use crate::api;
use crate::brand::{HealthBadge, Logo};
use crate::job_poll::{filesets_query_name, spawn_job_watching};
use crate::pages::{BenchPanel, DashboardPanel, ExplorerPanel, GamesPanel, Page};
use gambit_proto::{FilesetView, JobStatus, SourceDetail, SourceListItem};
use leptos::prelude::*;

"""
app_body = slice_lines(92, 335)
APP.write_text(app_hdr + app_body, encoding="utf-8")
print("Split complete.")
