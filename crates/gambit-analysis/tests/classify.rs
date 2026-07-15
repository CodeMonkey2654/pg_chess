//! Classification unit tests.

use gambit_analysis::{accuracy, classify_cp_loss, cp_loss_for_move, MoveClass};

#[test]
fn classify_all_bands() {
    assert_eq!(classify_cp_loss(-5), MoveClass::Best);
    assert_eq!(classify_cp_loss(25), MoveClass::Good);
    assert_eq!(classify_cp_loss(75), MoveClass::Inaccuracy);
    assert_eq!(classify_cp_loss(250), MoveClass::Mistake);
    assert_eq!(classify_cp_loss(400), MoveClass::Blunder);
}

#[test]
fn cp_loss_black_move() {
    let loss = cp_loss_for_move(-50, -20, false);
    assert_eq!(loss, 70);
}

#[test]
fn mixed_accuracy() {
    let classes = [
        MoveClass::Best,
        MoveClass::Good,
        MoveClass::Blunder,
        MoveClass::Best,
    ];
    let acc = accuracy(&classes).expect("acc");
    assert!(acc > 40.0 && acc < 80.0);
}
