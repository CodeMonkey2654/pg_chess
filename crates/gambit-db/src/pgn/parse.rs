use crate::fen::Position;
use crate::game::ChessGame;
use crate::movement::Move;
use crate::pgn::error::PgnError;
use crate::san::parse_san;
use std::collections::HashMap;

/// A parsed PGN game with headers and movetext.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgnGame {
    /// PGN tag pairs.
    pub headers: HashMap<String, String>,
    /// Mainline SAN tokens with optional comments/NAGs/variations.
    pub movetext: PgnMovetext,
}

/// Movetext node in the mainline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgnMove {
    /// SAN of the move.
    pub san: String,
    /// Resolved legal move.
    pub resolved: Move,
    /// Numeric annotation glyph ($1-$255).
    pub nag: Option<u8>,
    /// Brace comment, if any.
    pub comment: Option<String>,
    /// Recursive annotation variations (RAVs) branching before this move.
    pub variations: Vec<PgnMovetext>,
}

/// Ordered movetext line (mainline or variation).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PgnMovetext {
    /// Moves in sequence.
    pub moves: Vec<PgnMove>,
}

impl PgnGame {
    /// Starting position from `[FEN]` / `[SetUp "1"]` headers, or standard chess.
    pub fn starting_position(&self) -> Result<Position, PgnError> {
        let setup = self.headers.get("SetUp").map(String::as_str);
        if setup == Some("1") {
            let fen = self
                .headers
                .get("FEN")
                .ok_or_else(|| PgnError::InvalidFen("missing FEN with SetUp 1".into()))?;
            Position::from_fen(fen).map_err(|e| PgnError::InvalidFen(e.to_string()))
        } else if let Some(fen) = self.headers.get("FEN") {
            Position::from_fen(fen).map_err(|e| PgnError::InvalidFen(e.to_string()))
        } else {
            Ok(Position::starting_position())
        }
    }

    /// Build a `ChessGame` from the mainline SAN moves.
    pub fn to_chess_game(&self) -> Result<ChessGame, PgnError> {
        let start = self.starting_position()?;
        let mut game = ChessGame::from_position(start);

        for pm in &self.movetext.moves {
            game.play(pm.resolved)
                .map_err(|_| PgnError::IllegalMove(pm.san.clone()))?;
        }
        Ok(game)
    }
}

/// Parse a single PGN game from text.
pub fn parse_pgn(input: &str) -> Result<PgnGame, PgnError> {
    let (header_text, movetext_text) = split_headers_movetext(input);
    let headers = parse_headers(&header_text)?;
    let start = starting_position_from_headers(&headers)?;
    let tokens = tokenize_movetext(&movetext_text);
    let mut parser = MovetextParser::new(&tokens);
    let movetext = parser.parse_line(start)?;
    Ok(PgnGame { headers, movetext })
}

fn starting_position_from_headers(headers: &HashMap<String, String>) -> Result<Position, PgnError> {
    let game = PgnGame {
        headers: headers.clone(),
        movetext: PgnMovetext::default(),
    };
    game.starting_position()
}

/// Split a multi-game PGN file into individual game strings.
///
/// Games are separated by a blank line followed by a `[Tag ...]` header line.
/// Handles Lichess-style dumps without splitting the header/movetext blank line.
pub fn split_pgn_games(input: &str) -> Vec<&str> {
    let mut games = Vec::new();
    let mut game_start = 0usize;
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut pos = 0usize;

    while pos <= len {
        let line_start = pos;
        while pos < len && bytes[pos] != b'\n' && bytes[pos] != b'\r' {
            pos += 1;
        }
        let line = input[line_start..pos].trim();

        if line.is_empty() {
            let next_line_start = skip_blank_lines(input, pos);
            if next_line_start < len {
                let next_line = next_line(input, next_line_start);
                if next_line.starts_with('[') && game_start < line_start {
                    let chunk = input[game_start..line_start].trim();
                    if !chunk.is_empty() {
                        games.push(chunk);
                    }
                    game_start = next_line_start;
                }
            }
        }

        if pos < len {
            pos += 1;
            if pos < len && bytes[pos - 1] == b'\r' && bytes[pos] == b'\n' {
                pos += 1;
            }
        } else {
            break;
        }
    }

    let tail = input[game_start..].trim();
    if !tail.is_empty() {
        games.push(tail);
    }

    games
}

fn skip_blank_lines(input: &str, mut pos: usize) -> usize {
    let bytes = input.as_bytes();
    let len = bytes.len();
    while pos < len {
        let line_start = pos;
        while pos < len && bytes[pos] != b'\n' && bytes[pos] != b'\r' {
            pos += 1;
        }
        if !input[line_start..pos].trim().is_empty() {
            return line_start;
        }
        if pos < len {
            pos += 1;
            if pos < len && bytes[pos - 1] == b'\r' && bytes[pos] == b'\n' {
                pos += 1;
            }
        }
    }
    len
}

fn next_line(input: &str, start: usize) -> &str {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut pos = start;
    while pos < len && bytes[pos] != b'\n' && bytes[pos] != b'\r' {
        pos += 1;
    }
    input[start..pos].trim()
}

/// Parse all games in a multi-game PGN file.
pub fn parse_pgn_games(input: &str) -> impl Iterator<Item = Result<PgnGame, PgnError>> + '_ {
    split_pgn_games(input).into_iter().map(parse_pgn)
}

fn split_headers_movetext(input: &str) -> (String, String) {
    let mut header_lines = Vec::new();
    let mut movetext_lines = Vec::new();
    let mut past_blank = false;

    for line in input.lines() {
        let trimmed = line.trim();
        if !past_blank {
            if trimmed.is_empty() {
                past_blank = true;
                continue;
            }
            if trimmed.starts_with('[') {
                header_lines.push(line);
            } else {
                past_blank = true;
                movetext_lines.push(line);
            }
        } else {
            movetext_lines.push(line);
        }
    }

    (header_lines.join("\n"), movetext_lines.join(" "))
}

fn parse_headers(text: &str) -> Result<HashMap<String, String>, PgnError> {
    let mut headers = HashMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        parse_header_line(trimmed, &mut headers)?;
    }
    Ok(headers)
}

fn parse_header_line(line: &str, headers: &mut HashMap<String, String>) -> Result<(), PgnError> {
    if !line.starts_with('[') || !line.ends_with(']') {
        return Err(PgnError::InvalidHeader);
    }
    let inner = &line[1..line.len() - 1];
    let mut parts = inner.splitn(2, ' ');
    let tag = parts.next().ok_or(PgnError::InvalidHeader)?;
    let value_part = parts.next().ok_or(PgnError::InvalidHeader)?;
    let value = value_part.trim().trim_matches('"').replace("\\\"", "\"");
    headers.insert(tag.to_string(), value);
    Ok(())
}

struct MovetextParser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> MovetextParser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn bump(&mut self) {
        self.pos += 1;
    }

    fn expect_close_paren(&mut self) -> Result<(), PgnError> {
        match self.peek() {
            Some(Token::VariationClose) => {
                self.bump();
                Ok(())
            }
            _ => Err(PgnError::UnclosedVariation),
        }
    }

    fn skip_annotations(&mut self) -> (Option<u8>, Option<String>) {
        let mut nag = None;
        let mut comment = None;
        while let Some(tok) = self.peek() {
            match tok {
                Token::Nag(n) => {
                    nag = Some(*n);
                    self.bump();
                }
                Token::Comment(c) => {
                    comment = Some(c.clone());
                    self.bump();
                }
                _ => break,
            }
        }
        (nag, comment)
    }

    fn parse_variations(&mut self, fork: &Position) -> Result<Vec<PgnMovetext>, PgnError> {
        let mut variations = Vec::new();
        while matches!(self.peek(), Some(Token::VariationOpen)) {
            self.bump();
            variations.push(self.parse_line(fork.clone())?);
            self.expect_close_paren()?;
        }
        Ok(variations)
    }

    fn parse_line(&mut self, mut board: Position) -> Result<PgnMovetext, PgnError> {
        let mut moves = Vec::new();

        while let Some(tok) = self.peek() {
            match tok {
                Token::MoveNumber | Token::Ellipsis => {
                    self.bump();
                }
                Token::Result => {
                    self.bump();
                    break;
                }
                Token::VariationOpen => {
                    return Err(PgnError::UnexpectedVariation);
                }
                Token::San(san) => {
                    let san = san.clone();
                    let fork = board.clone();
                    let resolved =
                        parse_san(&board, &san).map_err(|_| PgnError::IllegalMove(san.clone()))?;
                    board = board
                        .apply_move(resolved)
                        .map_err(|_| PgnError::IllegalMove(san.clone()))?;
                    self.bump();

                    let (nag, comment) = self.skip_annotations();
                    let variations = self.parse_variations(&fork)?;

                    moves.push(PgnMove {
                        san,
                        resolved,
                        nag,
                        comment,
                        variations,
                    });
                }
                Token::Nag(_) | Token::Comment(_) => {
                    self.bump();
                }
                Token::VariationClose => break,
            }
        }

        Ok(PgnMovetext { moves })
    }
}

#[derive(Debug, Clone)]
enum Token {
    MoveNumber,
    Ellipsis,
    San(String),
    Nag(u8),
    Comment(String),
    VariationOpen,
    VariationClose,
    Result,
}

fn tokenize_movetext(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();

    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if c == '(' {
            chars.next();
            tokens.push(Token::VariationOpen);
            continue;
        }
        if c == ')' {
            chars.next();
            tokens.push(Token::VariationClose);
            continue;
        }
        if c == '{' {
            chars.next();
            let mut comment = String::new();
            for ch in chars.by_ref() {
                if ch == '}' {
                    break;
                }
                comment.push(ch);
            }
            tokens.push(Token::Comment(comment.trim().to_string()));
            continue;
        }
        if c == '$' {
            chars.next();
            let mut num = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() {
                    num.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            if let Ok(n) = num.parse::<u8>() {
                tokens.push(Token::Nag(n));
            }
            continue;
        }

        let mut word = String::new();
        while let Some(&ch) = chars.peek() {
            if ch.is_whitespace() || matches!(ch, '{' | '$' | '(' | ')') {
                break;
            }
            word.push(ch);
            chars.next();
        }

        if word == "..." {
            tokens.push(Token::Ellipsis);
        } else if word.ends_with('.') && word.chars().all(|c| c.is_ascii_digit() || c == '.') {
            tokens.push(Token::MoveNumber);
        } else if matches!(word.as_str(), "1-0" | "0-1" | "1/2-1/2" | "*") {
            tokens.push(Token::Result);
        } else if !word.is_empty() {
            tokens.push(Token::San(word));
        }
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"[Event "Example"]
[White "Alice"]
[Black "Bob"]
[Result "1-0"]

1. e4 e5 2. Nf3 Nc6 3. Bb5 { Spanish } 1-0
"#;

    #[test]
    fn parses_headers_and_mainline() {
        let game = parse_pgn(SAMPLE).expect("parse pgn");
        assert_eq!(game.headers.get("Event"), Some(&"Example".to_string()));
        assert_eq!(game.movetext.moves.len(), 5);
        assert_eq!(game.movetext.moves[4].san, "Bb5");
        assert_eq!(game.movetext.moves[4].comment.as_deref(), Some("Spanish"));
    }

    #[test]
    fn mainline_replays_to_chess_game() {
        let pgn = parse_pgn(SAMPLE).expect("parse");
        let chess = pgn.to_chess_game().expect("replay");
        assert_eq!(chess.moves.len(), 5);
    }

    #[test]
    fn parses_sibling_variation() {
        let text = "[Event \"?\"]\n\n1. e4 (1. d4 d5) e5 1-0";
        let game = parse_pgn(text).expect("parse");
        assert_eq!(game.movetext.moves.len(), 2);
        assert_eq!(game.movetext.moves[0].san, "e4");
        assert_eq!(game.movetext.moves[0].variations.len(), 1);
        assert_eq!(game.movetext.moves[0].variations[0].moves.len(), 2);
        assert_eq!(game.movetext.moves[0].variations[0].moves[0].san, "d4");
        assert_eq!(game.movetext.moves[1].san, "e5");
    }

    #[test]
    fn parses_alternative_black_move() {
        let text = "[Event \"?\"]\n\n1. e4 e5 (1... Nf6) 2. Nf3 1-0";
        let game = parse_pgn(text).expect("parse");
        assert_eq!(game.movetext.moves[1].san, "e5");
        assert_eq!(game.movetext.moves[1].variations.len(), 1);
        assert_eq!(game.movetext.moves[1].variations[0].moves[0].san, "Nf6");
    }

    #[test]
    fn parses_nested_variations() {
        let text = "[Event \"?\"]\n\n1. e4 (1. d4 (1. c4) d5) e5 1-0";
        let game = parse_pgn(text).expect("parse");
        let d4 = &game.movetext.moves[0];
        assert_eq!(d4.variations.len(), 1);
        let var = &d4.variations[0].moves[0];
        assert_eq!(var.san, "d4");
        assert_eq!(var.variations.len(), 1);
        assert_eq!(var.variations[0].moves[0].san, "c4");
        assert_eq!(d4.variations[0].moves[1].san, "d5");
    }

    #[test]
    fn parses_nag_on_move() {
        let text = "[Event \"?\"]\n\n1. e4 $1 e5 1-0";
        let game = parse_pgn(text).expect("parse");
        assert_eq!(game.movetext.moves[0].nag, Some(1));
    }

    #[test]
    fn split_multi_game_pgn() {
        let text = "[Event \"A\"]\n\n1. e4 1-0\n\n[Event \"B\"]\n\n1. d4 1-0";
        let games = split_pgn_games(text);
        assert_eq!(games.len(), 2);
    }

    #[test]
    fn parse_pgn_games_iterator() {
        let text = "[Event \"A\"]\n\n1. e4 1-0\n\n[Event \"B\"]\n\n1. d4 1-0";
        let parsed: Vec<_> = parse_pgn_games(text).collect();
        assert_eq!(parsed.len(), 2);
        assert_eq!(
            parsed[0].as_ref().expect("ok").headers.get("Event"),
            Some(&"A".to_string())
        );
    }

    #[test]
    fn fen_setup_starting_position() {
        let text = r#"[SetUp "1"]
[FEN "4k3/8/8/8/8/8/8/4K3 w - - 0 1"]

1. Ke2 Ke7 *"#;
        let game = parse_pgn(text).expect("parse");
        let start = game.starting_position().expect("fen start");
        assert!(start.to_fen().starts_with("4k3/8/8/8/8/8/8/4K3"));
        let chess = game.to_chess_game().expect("replay");
        assert_eq!(chess.moves.len(), 2);
    }
}
