use thiserror::Error;

/// Error applying a move that is not legal in the current position.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum MoveError {
    /// The move is not legal.
    #[error("move is not legal in this position")]
    Illegal,
}
