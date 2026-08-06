//! Engine core: entity registry, virtual input, and grid physics. No gameplay
//! is implemented here; entities are driven by Wasm plugins.

pub mod ecs;
pub mod input;
pub mod level;
pub mod physics;
pub mod sprites;
