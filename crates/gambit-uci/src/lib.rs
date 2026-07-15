//! UCI chess engine protocol: client, server, and parsers.

#![warn(missing_docs)]

mod client;
mod format;
mod parse;
mod pool;
mod server;

pub use client::UciEngine;
pub use parse::{parse_bestmove_line, parse_info_line, Info, SearchResult, SearchWithInfo, UciError};
pub use pool::EnginePool;
pub use server::{run_server, ServerOptions};
