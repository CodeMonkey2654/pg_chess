//! Search time and depth limits.

/// Constraints for a single search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchLimits {
    /// Maximum depth in plies (0 = no depth limit if movetime set).
    pub depth: u32,
    /// Soft time budget in milliseconds (0 = unlimited).
    pub movetime_ms: u64,
    /// Hard node cap (0 = unlimited).
    pub max_nodes: u64,
}

impl SearchLimits {
    /// Search to a fixed depth.
    pub fn depth(plies: u32) -> Self {
        Self {
            depth: plies,
            movetime_ms: 0,
            max_nodes: 0,
        }
    }

    /// Search for up to `ms` milliseconds.
    pub fn movetime(ms: u64) -> Self {
        Self {
            depth: 64,
            movetime_ms: ms,
            max_nodes: 0,
        }
    }
}

impl Default for SearchLimits {
    fn default() -> Self {
        Self::depth(6)
    }
}
