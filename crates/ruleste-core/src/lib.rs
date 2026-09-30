//! Ruleste core library: resource parsers, engine simulation and the Wasm
//! plugin host.
//!
//! The core owns everything that *isn't* drawing: decoding maps/atlases/fonts,
//! the ECS + collision + sprite animation simulation, virtual input and the
//! audio device bus, and the hot-reloading Wasm plugin host that drives every
//! non-wall entity.
//!
//! It deliberately contains **no rendering**: the client crate (`ruleste`)
//! owns the SDL window/renderer, the screens and the main loop, and pulls
//! entity state and resource needs out of the core to draw.

pub mod data;
pub mod engine;
pub mod hotload;
pub mod log;
pub mod resources;

/// Convenience re-exports for the client and tool binaries.
pub mod prelude {
    pub use crate::data::atlas::{Atlas, load_atlas_dir};
    pub use crate::data::spritebank::SpriteBank;
    pub use crate::engine::autotiler::Autotiler;
    pub use crate::engine::camera::Camera;
    pub use crate::engine::input::Input;
    pub use crate::engine::level::Level;
    pub use crate::engine::sprites::SpriteAnimator;
    pub use crate::hotload::wasm_host::WasmHost;
    pub use crate::resources::ResourceManifest;
}
