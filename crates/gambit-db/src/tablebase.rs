//! Syzygy tablebase probing (requires `.rtbw`/`.rtbz` files on disk).

use crate::fen::Position;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Win/draw/loss classification from tablebases (Syzygy 5-valued WDL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wdl {
    /// Loss for side to move.
    Loss,
    /// Blessed loss (draw under 50-move rule, loss in DTZ).
    BlessedLoss,
    /// Draw.
    Draw,
    /// Cursed win (win in DTZ, draw under 50-move rule).
    CursedWin,
    /// Win for side to move.
    Win,
}

/// Error opening or converting positions for tablebase probing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TablebaseError {
    /// Failed to load tablebase files from the given path.
    #[error("failed to load tablebases at {path}: {message}")]
    LoadFailed {
        /// Directory path.
        path: String,
        /// Underlying error message.
        message: String,
    },
    /// Failed to convert a position to the probe format.
    #[error("failed to convert position to tablebase format: {0}")]
    Conversion(String),
}

/// Read-only Syzygy tablebase access.
pub struct Tablebase {
    inner: shakmaty_syzygy::Tablebase<shakmaty::Chess>,
    path: PathBuf,
}

impl std::fmt::Debug for Tablebase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tablebase")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl Tablebase {
    /// Open tablebases at `path` (directory containing `.rtbw`/`.rtbz` files).
    pub fn open(path: impl AsRef<Path>) -> Result<Self, TablebaseError> {
        let path = path.as_ref().to_path_buf();
        // SAFETY: mmap filesystem is the recommended default for tablebase files.
        let mut inner = unsafe { shakmaty_syzygy::Tablebase::with_mmap_filesystem() };
        inner
            .add_directory(&path)
            .map_err(|e| TablebaseError::LoadFailed {
                path: path.display().to_string(),
                message: e.to_string(),
            })?;
        Ok(Self { inner, path })
    }

    /// Path to tablebase files.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Probe WDL for `pos` (≤7 pieces). Returns `None` if files missing or position out of TB.
    pub fn probe_wdl(&self, pos: &Position) -> Option<Wdl> {
        let shak = to_shakmaty(pos).ok()?;
        self.inner.probe_wdl(&shak).ok().map(Wdl::from)
    }

    /// Probe DTZ for `pos`. Returns `None` if unavailable.
    pub fn probe_dtz(&self, pos: &Position) -> Option<i32> {
        let shak = to_shakmaty(pos).ok()?;
        self.inner
            .probe_dtz(&shak)
            .ok()
            .map(|dtz| i32::from(dtz.ignore_rounding()))
    }
}

impl From<shakmaty_syzygy::AmbiguousWdl> for Wdl {
    fn from(wdl: shakmaty_syzygy::AmbiguousWdl) -> Self {
        use shakmaty_syzygy::AmbiguousWdl as A;
        match wdl {
            A::Loss | A::MaybeLoss => Wdl::Loss,
            A::BlessedLoss => Wdl::BlessedLoss,
            A::Draw => Wdl::Draw,
            A::CursedWin => Wdl::CursedWin,
            A::MaybeWin | A::Win => Wdl::Win,
        }
    }
}

fn to_shakmaty(pos: &Position) -> Result<shakmaty::Chess, TablebaseError> {
    use shakmaty::{fen::Fen, CastlingMode};
    let fen_str = pos.to_fen();
    let fen: Fen = fen_str
        .parse()
        .map_err(|_| TablebaseError::Conversion(fen_str))?;
    fen.into_position(CastlingMode::Standard)
        .map_err(|e| TablebaseError::Conversion(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_missing_path_fails() {
        let err = Tablebase::open("/nonexistent/tb/path/xyz").expect_err("missing path");
        assert!(matches!(err, TablebaseError::LoadFailed { .. }));
    }

    #[test]
    fn startpos_returns_none_without_tables() {
        let pos = Position::starting_position();
        let Ok(tb) = Tablebase::open(std::env::temp_dir()) else {
            return;
        };
        assert_eq!(tb.probe_wdl(&pos), None);
        assert_eq!(tb.probe_dtz(&pos), None);
    }

    #[test]
    #[ignore = "requires SYZYGY_PATH with tablebase files"]
    fn probes_known_endgame() {
        let path = std::env::var("SYZYGY_PATH").expect("SYZYGY_PATH");
        let tb = Tablebase::open(&path).expect("open tb");
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").expect("valid fen");
        assert!(tb.probe_wdl(&pos).is_some());
    }
}
