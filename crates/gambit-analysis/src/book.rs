//! Corpus book loaded from `.gbook` binary export.

use crate::report::MoveStat;
use gambit_db::{Move, MoveParseError};
use std::fs::File;
use std::io;
use std::path::Path;

const MAGIC: &[u8; 4] = b"GBOK";
const VERSION: u32 = 1;

/// In-memory corpus move statistics keyed by position Zobrist hash.
#[derive(Debug, Default, Clone)]
pub struct CorpusBook {
    /// Sorted (hash, start, len) index into `moves`.
    index: Vec<(u64, u32, u32)>,
    moves: Vec<MoveStat>,
}

impl CorpusBook {
    /// Empty book.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load a book exported by `gambit-ingest export-book`.
    pub fn load(path: impl AsRef<Path>) -> io::Result<Self> {
        let bytes = std::fs::read(path.as_ref())?;
        Self::from_bytes(&bytes)
    }

    /// Parse book bytes.
    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() < 8 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated header",
            ));
        }
        if &bytes[..4] != MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "bad magic"));
        }
        let version = u32::from_le_bytes(
            bytes[4..8]
                .try_into()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad version"))?,
        );
        if version != VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported book version {version}"),
            ));
        }

        let mut offset = 8usize;
        let mut index = Vec::new();
        let mut moves = Vec::new();

        while offset + 28 <= bytes.len() {
            let hash = u64::from_le_bytes(
                bytes[offset..offset + 8]
                    .try_into()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad hash"))?,
            );
            offset += 8;
            let move_count = u32::from_le_bytes(
                bytes[offset..offset + 4]
                    .try_into()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad move count"))?,
            ) as usize;
            offset += 4;

            let start = moves.len() as u32;
            for _ in 0..move_count {
                if offset + 36 > bytes.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "truncated entry",
                    ));
                }
                let uci_len = u32::from_le_bytes(
                    bytes[offset..offset + 4]
                        .try_into()
                        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad uci len"))?,
                ) as usize;
                offset += 4;
                if offset + uci_len + 32 > bytes.len() {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "truncated uci"));
                }
                let uci_str = std::str::from_utf8(&bytes[offset..offset + uci_len])
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid uci utf8"))?;
                offset += uci_len;
                let count = u64::from_le_bytes(
                    bytes[offset..offset + 8]
                        .try_into()
                        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad count"))?,
                );
                offset += 8;
                let white_wins =
                    u64::from_le_bytes(bytes[offset..offset + 8].try_into().map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "bad white_wins")
                    })?);
                offset += 8;
                let black_wins =
                    u64::from_le_bytes(bytes[offset..offset + 8].try_into().map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "bad black_wins")
                    })?);
                offset += 8;
                let draws = u64::from_le_bytes(
                    bytes[offset..offset + 8]
                        .try_into()
                        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "bad draws"))?,
                );
                offset += 8;

                let uci = Move::from_uci(uci_str).map_err(|e: MoveParseError| {
                    io::Error::new(io::ErrorKind::InvalidData, e.to_string())
                })?;
                moves.push(MoveStat {
                    uci,
                    count,
                    white_wins,
                    black_wins,
                    draws,
                });
            }
            index.push((hash, start, moves.len() as u32 - start));
        }

        Ok(Self { index, moves })
    }

    /// Lookup move statistics for a position hash.
    pub fn lookup(&self, hash: u64) -> Option<&[MoveStat]> {
        let idx = self
            .index
            .binary_search_by_key(&hash, |(h, _, _)| *h)
            .ok()?;
        let (_, start, len) = self.index[idx];
        Some(&self.moves[start as usize..(start + len) as usize])
    }

    /// Whether the book has any entries.
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
}

/// Write a `.gbook` file from sorted rows.
pub fn write_book(path: impl AsRef<Path>, rows: &[(u64, Vec<MoveStat>)]) -> io::Result<()> {
    let mut file = File::create(path.as_ref())?;
    use std::io::Write;
    file.write_all(MAGIC)?;
    file.write_all(&VERSION.to_le_bytes())?;
    for (hash, stats) in rows {
        file.write_all(&hash.to_le_bytes())?;
        file.write_all(&(stats.len() as u32).to_le_bytes())?;
        for stat in stats {
            let uci = stat.uci.to_uci();
            file.write_all(&(uci.len() as u32).to_le_bytes())?;
            file.write_all(uci.as_bytes())?;
            file.write_all(&stat.count.to_le_bytes())?;
            file.write_all(&stat.white_wins.to_le_bytes())?;
            file.write_all(&stat.black_wins.to_le_bytes())?;
            file.write_all(&stat.draws.to_le_bytes())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gambit_db::Move;

    #[test]
    fn roundtrip_book() {
        let mv = Move::from_uci("e2e4").expect("valid");
        let rows = vec![(
            12345u64,
            vec![MoveStat {
                uci: mv,
                count: 100,
                white_wins: 40,
                black_wins: 30,
                draws: 30,
            }],
        )];
        let path = std::env::temp_dir().join("gambit_book_test.gbook");
        write_book(&path, &rows).expect("write");
        let book = CorpusBook::load(&path).expect("load");
        let _ = std::fs::remove_file(&path);
        let stats = book.lookup(12345).expect("found");
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].count, 100);
    }
}
