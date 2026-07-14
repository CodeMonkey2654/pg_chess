//! Chess analysis engine: search, evaluation, optional corpus book.

#![warn(missing_docs)]

mod book;
mod eval;
mod limits;
mod order;
mod report;
mod search;
mod tt;

pub use book::{write_book, CorpusBook};
pub use limits::SearchLimits;
pub use report::{Analysis, MoveStat, Score};
pub use search::Analyzer;
