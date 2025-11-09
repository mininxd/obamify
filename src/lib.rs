#![warn(clippy::all, rust_2018_idioms)]

mod app;
#[cfg(target_arch = "wasm32")]
pub use app::calculate::worker::worker_entry;
#[cfg(target_arch = "wasm32")]
pub use app::headless::Generator;