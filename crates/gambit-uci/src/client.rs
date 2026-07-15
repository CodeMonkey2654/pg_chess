//! External UCI engine process client.

use crate::parse::{parse_bestmove_line, parse_info_line, SearchResult, UciError};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

fn line_contains_token(line: &str, token: &str) -> bool {
    line.split_whitespace().any(|part| part == token)
}

/// Handle to a spawned UCI engine process.
pub struct UciEngine {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    read_timeout: Duration,
}

impl UciEngine {
    /// Default timeout for reading engine responses.
    pub const DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(120);

    /// Spawn an engine executable and complete the UCI handshake (`uci` / `uciok`).
    pub fn spawn(command: impl AsRef<str>, args: &[&str]) -> Result<Self, UciError> {
        let mut child = Command::new(command.as_ref())
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(UciError::Spawn)?;

        let stdin = child.stdin.take().ok_or(UciError::NoStdin)?;
        let stdout = child.stdout.take().ok_or(UciError::NoStdout)?;

        let mut engine = Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            read_timeout: Self::DEFAULT_READ_TIMEOUT,
        };

        engine.send_line("uci")?;
        engine.wait_for_token("uciok", Self::DEFAULT_READ_TIMEOUT)?;
        Ok(engine)
    }

    /// Override the timeout used when waiting for `bestmove`.
    pub fn set_read_timeout(&mut self, timeout: Duration) {
        self.read_timeout = timeout;
    }

    /// Set the position from FEN and optional move list, then search to the given depth.
    pub fn search_depth(
        &mut self,
        fen: &str,
        moves: &[&str],
        depth: u32,
    ) -> Result<SearchResult, UciError> {
        self.search_depth_with_info(fen, moves, depth)
            .map(|r| r.result)
    }

    /// Search and return the final `info` line alongside `bestmove`.
    pub fn search_depth_with_info(
        &mut self,
        fen: &str,
        moves: &[&str],
        depth: u32,
    ) -> Result<crate::parse::SearchWithInfo, UciError> {
        self.send_line("isready")?;
        self.wait_for_token("readyok", Self::DEFAULT_READ_TIMEOUT)?;

        let position = if moves.is_empty() {
            format!("position fen {fen}")
        } else {
            format!("position fen {fen} moves {}", moves.join(" "))
        };
        self.send_line(&position)?;
        self.send_line(&format!("go depth {depth}"))?;
        self.read_bestmove_with_info(self.read_timeout)
    }

    /// Search with MultiPV enabled.
    pub fn search_depth_multipv(
        &mut self,
        fen: &str,
        moves: &[&str],
        depth: u32,
        multipv: u32,
    ) -> Result<crate::parse::SearchWithInfo, UciError> {
        self.send_line("isready")?;
        self.wait_for_token("readyok", Self::DEFAULT_READ_TIMEOUT)?;
        self.send_line(&format!("setoption name MultiPV value {multipv}"))?;

        let position = if moves.is_empty() {
            format!("position fen {fen}")
        } else {
            format!("position fen {fen} moves {}", moves.join(" "))
        };
        self.send_line(&position)?;
        self.send_line(&format!("go depth {depth}"))?;
        self.read_bestmove_with_info(self.read_timeout)
    }

    /// Send `quit` and wait for the engine process to exit.
    pub fn quit(mut self) -> Result<(), UciError> {
        let _ = self.send_line("quit");
        let _ = self.child.wait();
        Ok(())
    }

    pub(crate) fn send_line(&mut self, line: &str) -> Result<(), UciError> {
        writeln!(self.stdin, "{line}")?;
        self.stdin.flush()?;
        Ok(())
    }

    fn wait_for_token(&mut self, token: &str, timeout: Duration) -> Result<(), UciError> {
        let deadline = Instant::now() + timeout;
        let mut line = String::new();

        loop {
            if Instant::now() >= deadline {
                return Err(UciError::Timeout {
                    expected: token.to_string(),
                });
            }

            line.clear();
            match self.read_line_with_timeout(&mut line, deadline)? {
                true => {
                    if line_contains_token(&line, token) {
                        return Ok(());
                    }
                }
                false => return Err(UciError::ProcessExited),
            }
        }
    }

    fn read_bestmove_with_info(
        &mut self,
        timeout: Duration,
    ) -> Result<crate::parse::SearchWithInfo, UciError> {
        let deadline = Instant::now() + timeout;
        let mut line = String::new();
        let mut last_info = crate::parse::Info::default();

        loop {
            if Instant::now() >= deadline {
                return Err(UciError::NoBestmove);
            }

            line.clear();
            match self.read_line_with_timeout(&mut line, deadline)? {
                true => {
                    if let Some(result) = parse_bestmove_line(&line)? {
                        return Ok(crate::parse::SearchWithInfo {
                            result,
                            info: last_info,
                        });
                    }
                    if let Some(info) = parse_info_line(&line)? {
                        if info.depth.is_some() {
                            last_info = info;
                        }
                    }
                }
                false => return Err(UciError::ProcessExited),
            }
        }
    }

    fn read_line_with_timeout(
        &mut self,
        line: &mut String,
        deadline: Instant,
    ) -> Result<bool, UciError> {
        loop {
            if Instant::now() >= deadline {
                return Ok(false);
            }

            let available = self.stdout.fill_buf()?;
            if !available.is_empty() {
                self.stdout.read_line(line)?;
                return Ok(true);
            }

            if self.child.try_wait()?.is_some() {
                return Ok(false);
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
