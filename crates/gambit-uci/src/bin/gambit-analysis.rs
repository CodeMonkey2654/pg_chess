//! Native Gambit analysis engine (UCI).

use gambit_uci::{run_server, ServerOptions};
use std::env;

fn main() -> std::io::Result<()> {
    let book_path = env::args()
        .skip_while(|a| a != "--book")
        .nth(1)
        .or_else(|| env::var("GAMBIT_BOOK").ok());

    run_server(ServerOptions { book_path })
}
