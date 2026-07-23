mod payload;

#[cfg(not(target_arch = "wasm32"))]
mod node;

#[cfg(target_arch = "wasm32")]
mod utils;

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(not(target_arch = "wasm32"))]
pub use node::Glue;

#[cfg(target_arch = "wasm32")]
pub use wasm::Glue;
