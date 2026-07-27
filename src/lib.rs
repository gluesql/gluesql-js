mod payload;

#[cfg(not(target_arch = "wasm32"))]
mod node;

#[cfg(target_arch = "wasm32")]
mod utils;

#[cfg(all(target_arch = "wasm32", not(feature = "opfs")))]
mod wasm;

#[cfg(all(target_arch = "wasm32", feature = "opfs"))]
mod opfs;

#[cfg(not(target_arch = "wasm32"))]
pub use node::Glue;

#[cfg(all(target_arch = "wasm32", not(feature = "opfs")))]
pub use wasm::Glue;

#[cfg(all(target_arch = "wasm32", feature = "opfs"))]
pub use opfs::Glue;
