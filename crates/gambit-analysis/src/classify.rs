//! Move classification and accuracy scoring (Lichess-style bands).

/// Move quality classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MoveClass {
    /// Best or engine-equal move.
    Best,
    /// Good move (≤50 cp loss).
    Good,
    /// Inaccuracy (≤100 cp loss).
    Inaccuracy,
    /// Mistake (≤300 cp loss).
    Mistake,
    /// Blunder (>300 cp loss).
    Blunder,
}

impl MoveClass {
    /// PostgreSQL enum string representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Best => "best",
            Self::Good => "good",
            Self::Inaccuracy => "inaccuracy",
            Self::Mistake => "mistake",
            Self::Blunder => "blunder",
        }
    }

    /// Lichess-style accuracy weight for this class.
    pub fn accuracy_weight(self) -> f64 {
        match self {
            Self::Best => 1.0,
            Self::Good => 0.9,
            Self::Inaccuracy => 0.67,
            Self::Mistake => 0.33,
            Self::Blunder => 0.0,
        }
    }

    /// Parse from database enum string.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "best" => Some(Self::Best),
            "good" => Some(Self::Good),
            "inaccuracy" => Some(Self::Inaccuracy),
            "mistake" => Some(Self::Mistake),
            "blunder" => Some(Self::Blunder),
            _ => None,
        }
    }
}

/// Classify centipawn loss into a move quality band.
pub fn classify_cp_loss(cp_loss: i32) -> MoveClass {
    if cp_loss <= 0 {
        MoveClass::Best
    } else if cp_loss <= 50 {
        MoveClass::Good
    } else if cp_loss <= 100 {
        MoveClass::Inaccuracy
    } else if cp_loss <= 300 {
        MoveClass::Mistake
    } else {
        MoveClass::Blunder
    }
}

/// Compute centipawn loss for the side that just moved.
///
/// `eval_before` and `eval_after` are from the perspective of the side to move
/// at each respective position. `mover_was_white` indicates who played the move.
pub fn cp_loss_for_move(eval_before: i32, eval_after: i32, mover_was_white: bool) -> i32 {
    let before = if mover_was_white {
        eval_before
    } else {
        -eval_before
    };
    let after = if mover_was_white {
        -eval_after
    } else {
        eval_after
    };
    (before - after).max(0)
}

/// Weighted accuracy percentage from a slice of move classes.
pub fn accuracy(classes: &[MoveClass]) -> Option<f64> {
    if classes.is_empty() {
        return None;
    }
    let sum: f64 = classes.iter().map(|c| c.accuracy_weight()).sum();
    Some(sum / classes.len() as f64 * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_bands() {
        assert_eq!(classify_cp_loss(0), MoveClass::Best);
        assert_eq!(classify_cp_loss(30), MoveClass::Good);
        assert_eq!(classify_cp_loss(80), MoveClass::Inaccuracy);
        assert_eq!(classify_cp_loss(200), MoveClass::Mistake);
        assert_eq!(classify_cp_loss(500), MoveClass::Blunder);
    }

    #[test]
    fn cp_loss_white_move() {
        let loss = cp_loss_for_move(100, 50, true);
        assert_eq!(loss, 150);
    }

    #[test]
    fn accuracy_all_best() {
        let acc = accuracy(&[MoveClass::Best, MoveClass::Best]).expect("acc");
        assert!((acc - 100.0).abs() < 0.01);
    }
}
