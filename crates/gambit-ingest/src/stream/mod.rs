//! Streaming zstd PGN readers for shard ingest.

use anyhow::{Context, Result};
use gambit_db::split_pgn_games;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// Read plain or zstd-compressed PGN into a string for parsing.
pub fn read_pgn_text(path: &Path) -> Result<String> {
    if path.extension().is_some_and(|ext| ext == "zst") {
        let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let mut decoder = zstd::stream::read::Decoder::new(file)
            .with_context(|| format!("zstd decode {}", path.display()))?;
        let mut text = String::new();
        decoder
            .read_to_string(&mut text)
            .with_context(|| format!("read decompressed {}", path.display()))?;
        Ok(text)
    } else {
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
    }
}

/// Incrementally extract PGN game chunks from a reader.
pub struct PgnGameReader<R: BufRead> {
    reader: R,
    buffer: String,
    eof: bool,
    stream_offset: i64,
    next_game_offset: i64,
}

impl<R: BufRead> PgnGameReader<R> {
    /// Wrap a buffered reader for incremental game extraction.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: String::new(),
            eof: false,
            stream_offset: 0,
            next_game_offset: 0,
        }
    }

    /// Current byte offset for the next game in the decompressed stream.
    pub fn next_offset(&self) -> i64 {
        self.next_game_offset
    }

    /// Read the next complete PGN game chunk, if any.
    pub fn next_game(&mut self) -> Result<Option<(String, i64)>> {
        loop {
            if let Some(game) = take_complete_game(&mut self.buffer)? {
                let offset = self.next_game_offset;
                self.next_game_offset = self.stream_offset;
                return Ok(Some((game, offset)));
            }

            if self.eof {
                if self.buffer.trim().is_empty() {
                    return Ok(None);
                }
                let rest = std::mem::take(&mut self.buffer);
                let offset = self.next_game_offset;
                self.next_game_offset = self.stream_offset;
                return Ok(Some((rest, offset)));
            }

            let mut chunk = String::new();
            let read = self.reader.read_line(&mut chunk)?;
            if read == 0 {
                self.eof = true;
                continue;
            }
            self.stream_offset += read as i64;
            self.buffer.push_str(&chunk);
        }
    }
}

/// Send-safe PGN game reader over plain or zstd-compressed files.
pub enum SendPgnReader {
    /// Uncompressed PGN file.
    Plain(PgnGameReader<BufReader<File>>),
    /// zstd-compressed PGN file.
    Zstd(PgnGameReader<BufReader<zstd::stream::read::Decoder<'static, BufReader<File>>>>),
}

impl SendPgnReader {
    /// Read the next game chunk.
    pub fn next_game(&mut self) -> Result<Option<(String, i64)>> {
        match self {
            Self::Plain(r) => r.next_game(),
            Self::Zstd(r) => r.next_game(),
        }
    }
}

fn take_complete_game(buffer: &mut String) -> Result<Option<String>> {
    if buffer.trim().is_empty() {
        buffer.clear();
        return Ok(None);
    }

    if let Some(idx) = find_next_game_start(buffer) {
        if idx == 0 {
            return Ok(None);
        }
        let game = buffer[..idx].trim_end().to_string();
        *buffer = buffer[idx..].to_string();
        return Ok(Some(game));
    }

    Ok(None)
}

fn find_next_game_start(text: &str) -> Option<usize> {
    text.find("\n\n[").map(|idx| idx + 2)
}

/// Open a plain or zstd PGN file as a Send-safe streaming game reader.
pub fn open_game_reader(path: &Path) -> Result<SendPgnReader> {
    if path.extension().is_some_and(|ext| ext == "zst") {
        let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        let decoder = zstd::stream::read::Decoder::new(file)
            .with_context(|| format!("zstd decode {}", path.display()))?;
        Ok(SendPgnReader::Zstd(PgnGameReader::new(BufReader::new(
            decoder,
        ))))
    } else {
        let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
        Ok(SendPgnReader::Plain(PgnGameReader::new(BufReader::new(
            file,
        ))))
    }
}

/// Split a full PGN text blob into game chunks (used by tests and small files).
pub fn split_games(text: &str) -> Vec<&str> {
    split_pgn_games(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn incremental_reader_yields_two_games() {
        let input = "[Event \"A\"]\n\n1. e4 1-0\n\n[Event \"B\"]\n\n1. d4 1-0\n";
        let mut reader = PgnGameReader::new(Cursor::new(input));
        let first = reader.next_game().expect("first").expect("game");
        assert!(first.0.contains("Event \"A\""));
        let second = reader.next_game().expect("second").expect("game");
        assert!(second.0.contains("Event \"B\""));
        assert!(reader.next_game().expect("third").is_none());
    }
}
