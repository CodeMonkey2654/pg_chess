//! Stack-allocated move list for search hot paths.

use crate::movement::Move;
use std::mem::MaybeUninit;

/// Maximum legal moves in any chess position (generous upper bound).
pub const MOVE_LIST_CAP: usize = 256;

/// Fixed-capacity move buffer without heap allocation.
pub struct MoveList {
    moves: [MaybeUninit<Move>; MOVE_LIST_CAP],
    len: usize,
}

impl MoveList {
    /// Empty move list.
    pub fn new() -> Self {
        Self {
            moves: std::array::from_fn(|_| MaybeUninit::uninit()),
            len: 0,
        }
    }

    /// Clear the list for reuse.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Number of moves stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Append a move; panics in debug if over capacity.
    pub fn push(&mut self, m: Move) {
        debug_assert!(self.len < MOVE_LIST_CAP, "MoveList overflow");
        if self.len < MOVE_LIST_CAP {
            self.moves[self.len].write(m);
            self.len += 1;
        }
    }

    /// Moves as a slice.
    pub fn as_slice(&self) -> &[Move] {
        // SAFETY: first `len` elements are initialized by `push`.
        unsafe { std::slice::from_raw_parts(self.moves.as_ptr().cast::<Move>(), self.len) }
    }

    /// Copy into a `Vec` (for compatibility with existing APIs).
    pub fn to_vec(&self) -> Vec<Move> {
        self.as_slice().to_vec()
    }
}

impl Default for MoveList {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movement::Move;
    use crate::square::Square;

    #[test]
    fn push_and_slice() {
        let mut list = MoveList::new();
        let m = Move::new(Square(12), Square(28), None).expect("valid");
        list.push(m);
        assert_eq!(list.len(), 1);
        assert_eq!(list.as_slice()[0], m);
    }
}
