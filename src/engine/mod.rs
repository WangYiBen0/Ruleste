//! Engine core: entity registry, virtual input, grid physics, and the audio
//! bus. No gameplay is implemented here; entities are driven by Wasm plugins.

pub mod audio;
pub mod autotiler;
pub mod backdrops;
pub mod camera;
pub mod draw;
pub mod ecs;
pub mod input;
pub mod level;
pub mod particles;
pub mod physics;
pub mod sprites;
