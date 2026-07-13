use thiserror::Error;

/// Error parsing or writing PGN.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PgnError {
    /// Invalid header tag.
    #[error("invalid PGN header")]
    InvalidHeader,
    /// Illegal move in context.
    #[error("illegal move in PGN: {0}")]
    IllegalMove(String),
    /// Variation parenthesis not closed.
    #[error("unclosed PGN variation")]
    UnclosedVariation,
    /// Variation started without a preceding move.
    #[error("unexpected PGN variation")]
    UnexpectedVariation,
    /// Invalid or missing FEN in header.
    #[error("invalid FEN in PGN header: {0}")]
    InvalidFen(String),
}
