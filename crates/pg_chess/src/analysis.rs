//! Analysis types and functions for move classification and accuracy.

use gambit_analysis::{accuracy, classify_cp_loss, MoveClass};
use pgrx::prelude::*;

extension_sql!(
    r#"
    CREATE TYPE chess_move_class AS ENUM (
        'best', 'good', 'inaccuracy', 'mistake', 'blunder'
    );

    CREATE TYPE chess_eval_source AS ENUM (
        'stockfish', 'syzygy', 'corpus', 'native'
    );

    CREATE TYPE chess_analysis_status AS ENUM (
        'none', 'pending', 'running', 'complete', 'failed'
    );
    "#,
    name = "chess_analysis_types",
);

/// Classify centipawn loss into a move quality band (Lichess-style).
#[pg_extern(immutable, parallel_safe)]
fn chess_classify_cp_loss(cp_loss: default!(i32, 0)) -> &'static str {
    classify_cp_loss(cp_loss).as_str()
}

/// Compute weighted accuracy percentage from move class labels.
#[pg_extern(immutable, parallel_safe)]
fn chess_accuracy_from_classes(classes: Vec<String>) -> Option<f32> {
    let parsed: Vec<MoveClass> = classes
        .iter()
        .filter_map(|label| MoveClass::parse(label))
        .collect();
    accuracy(&parsed).map(|value| value as f32)
}

/// Normalize mate or centipawn score to centipawns for arithmetic.
#[pg_extern(immutable, parallel_safe)]
fn chess_eval_to_cp(cp: Option<i32>, mate_plies: Option<i32>) -> i32 {
    use gambit_analysis::MATE_CP;
    match mate_plies {
        Some(plies) if plies != 0 => {
            if plies > 0 {
                MATE_CP - plies
            } else {
                -MATE_CP - plies
            }
        }
        _ => cp.unwrap_or(0),
    }
}

#[cfg(any(test, feature = "pg_test"))]
#[pg_schema]
mod tests {
    use pgrx::prelude::*;

    #[pg_test]
    fn classify_cp_loss_best() {
        let label = Spi::get_one::<String>("SELECT chess_classify_cp_loss(0)")
            .expect("spi")
            .expect("value");
        assert_eq!(label, "best");
    }

    #[pg_test]
    fn classify_cp_loss_blunder() {
        let label = Spi::get_one::<String>("SELECT chess_classify_cp_loss(500)")
            .expect("spi")
            .expect("value");
        assert_eq!(label, "blunder");
    }

    #[pg_test]
    fn accuracy_all_best() {
        let acc = Spi::get_one::<f32>(
            "SELECT chess_accuracy_from_classes(ARRAY['best','best']::text[])",
        )
        .expect("spi")
        .expect("value");
        assert!((acc - 100.0).abs() < 0.01);
    }

    #[pg_test]
    fn eval_to_cp_mate() {
        let cp = Spi::get_one::<i32>("SELECT chess_eval_to_cp(NULL, 3)")
            .expect("spi")
            .expect("value");
        assert_eq!(cp, gambit_analysis::MATE_CP - 3);
    }
}
