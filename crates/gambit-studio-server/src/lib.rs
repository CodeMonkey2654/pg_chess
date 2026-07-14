//! Gambit Studio REST API library.

mod db;
mod jobs;
mod pool;
mod routes;
mod types;

pub use jobs::JobManager;
pub use pool::PgPool;
pub use routes::{router, AppState};
pub use types::*;
