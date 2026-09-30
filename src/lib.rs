//! Ruleste client library: the SDL3 rendering front end.
//!
//! The client owns the window, the SDL renderer, the in-game screens and the
//! main loop. Game simulation lives in [`ruleste_core`]: entity states, the
//! Wasm plugin host and resource decoding. The client reads entity state and
//! resource-need manifests out of the core and renders them.
//!
//! The `data` / `engine` / `hotload` re-exports below forward to the core so
//! tool binaries and integration tests can keep addressing them through one
//! crate path.

pub use ruleste_core::{data, engine, hotload};

pub mod interface;
