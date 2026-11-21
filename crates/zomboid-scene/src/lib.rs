//! Window-free scene building and simulation, shared by the desktop client and the browser build.
#[cfg(not(target_arch = "wasm32"))]
pub mod async_stream;
pub mod scene;
pub mod sim;
