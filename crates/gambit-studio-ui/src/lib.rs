use wasm_bindgen::prelude::*;

mod api;
mod app;
mod board;
mod brand;
mod format;
mod grpc_web;
mod job_poll;
mod replay;

use app::App;
use leptos::mount::mount_to_body;

/// Launch the WASM UI.
pub fn launch() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}

#[wasm_bindgen(start)]
pub fn wasm_start() {
    launch();
}
