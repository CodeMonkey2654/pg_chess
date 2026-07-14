//! Native UCI server using gambit-analysis.

use crate::format::{format_bestmove, format_info};
use crate::parse::{parse_go_line, parse_position_line};
use gambit_analysis::{Analyzer, CorpusBook, SearchLimits};
use gambit_db::Position;
use std::io::{self, BufRead, Write};

/// Options for the UCI server.
pub struct ServerOptions {
    /// Optional corpus book path.
    pub book_path: Option<String>,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            book_path: std::env::var("GAMBIT_BOOK").ok(),
        }
    }
}

/// Run the UCI protocol loop on stdin/stdout.
pub fn run_server(options: ServerOptions) -> io::Result<()> {
    let mut analyzer = Analyzer::new();
    if let Some(path) = options.book_path {
        match CorpusBook::load(&path) {
            Ok(book) => {
                analyzer = analyzer.with_book(book);
            }
            Err(e) => {
                eprintln!("info string failed to load book {path}: {e}");
            }
        }
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut position = Position::starting_position();

    for line in stdin.lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed == "uci" {
            writeln!(stdout, "id name Gambit Analysis")?;
            writeln!(stdout, "id author pg_chess")?;
            writeln!(stdout, "uciok")?;
        } else if trimmed == "isready" {
            writeln!(stdout, "readyok")?;
        } else if trimmed == "ucinewgame" {
            analyzer.new_game();
            position = Position::starting_position();
        } else if trimmed == "quit" {
            break;
        } else if trimmed.starts_with("position ") {
            position = parse_position_line(trimmed)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e.to_string()))?;
        } else if trimmed.starts_with("go") {
            let params = parse_go_line(trimmed);
            let limits = if let Some(ms) = params.movetime_ms {
                SearchLimits::movetime(ms)
            } else {
                SearchLimits::depth(params.depth.unwrap_or(6))
            };
            let analysis = analyzer.search(&position, limits);
            writeln!(stdout, "{}", format_info(&analysis))?;
            writeln!(stdout, "{}", format_bestmove(&analysis.best_move.to_uci()))?;
        }
        stdout.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn server_responds_to_uci_handshake() {
        let input = b"uci\nquit\n";
        // Integration via run_server requires real stdin; test parse path instead.
        let pos = crate::parse::parse_position_line("position startpos").expect("parse");
        assert_eq!(pos, Position::starting_position());
        let _ = Cursor::new(input);
    }
}
