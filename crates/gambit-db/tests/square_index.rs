//! Integration tests for LERF square indexing.

use gambit_db::Square;

/// LERF square indexing roundtrip: index <-> file/rank <-> algebraic.
#[test]
fn lerf_roundtrip_all_squares() {
    for sq in Square::ALL {
        assert_eq!(sq.index(), sq.0 as usize);
        let file = sq.file();
        let rank = sq.rank();
        assert_eq!(Square::from_file_rank(file, rank), Some(sq));

        let alg = sq.to_algebraic();
        assert_eq!(Square::from_algebraic(&alg), Some(sq));

        let recomposed = rank * 8 + file;
        assert_eq!(recomposed, sq.0);
    }
}

#[test]
fn lerf_corner_indices() {
    assert_eq!(Square::from_algebraic("a1"), Some(Square(0)));
    assert_eq!(Square::from_algebraic("h1"), Some(Square(7)));
    assert_eq!(Square::from_algebraic("a8"), Some(Square(56)));
    assert_eq!(Square::from_algebraic("h8"), Some(Square(63)));
}
