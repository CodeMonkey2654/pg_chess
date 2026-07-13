# pg_chess

A PosgreSQL extension that contains chess analysis written in rust so I can learn how PG extensions work under the hood. 

## Phases
1. Base Chunks - Color, PieceKind, Piece, value and char conversions
2. Board - Square index representation of the board using little endian rank file
    a. Square
    b. Board
3. Fen Position class 


## Next steps
1. Move Type + UCI/SAN -The chess_move type. Move struct (from-square, to-square, promotion, flags), parse/emit UCI (e2e4, e7e8q), and the storable SQL type. Foundation for everything move-related.
2. Pseudo-legal move generation - Per-piece move generation (pawn pushes/captures, knight, bishop, rook, queen, king) without check filtering yet. This is where the mailbox board earns its keep — you'll see the chess logic clearly.
3. Apply move and game state machine - The chess_game type holding a position plus full move history. apply_move (reject illegal moves), update clocks/castling/en-passant, detect draws (50-move, threefold repetition).
4. The full set of #[pg_extern] functions Postgres calls: chess_legal_moves, chess_apply_move, chess_in_check, chess_is_checkmate, chess_game_from_pgn, etc. Set-returning functions for legal moves.
5. Chunk 11 — Operators + operator classes
Custom SQL operators (=, @> "position contains"), plus btree/hash operator classes so the types can be compared, sorted, and used as index keys. This is the PostgresEq/PostgresOrd/PostgresHash work.
6. Chunk 12 — Indexes + optimization
The payoff for a database-native engine: Zobrist hashing for fast position identity, expression indexes, a GIN index for "positions containing piece X on square Y" queries, and the optional bitboard layer for move-gen speed. Where the mailbox-vs-bitboard door I left open gets used.