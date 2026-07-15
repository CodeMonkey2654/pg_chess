pub mod bench;
pub mod dashboard;
pub mod explorer;
pub mod games;
pub mod util;

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
