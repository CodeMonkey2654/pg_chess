use thiserror::Error;

/// Error parsing SAN.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SanError {
    /// Empty input.
    #[error("empty SAN")]
    Empty,
    /// No legal move matches the SAN.
    #[error("ambiguous or illegal SAN: {0}")]
    NoMatch(String),
    /// Invalid SAN token structure.
    #[error("invalid SAN syntax: {0}")]
    InvalidSyntax(String),
}
