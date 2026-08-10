pub mod map;
pub mod types;

#[cfg(feature = "plugin")]
pub mod host;
#[cfg(feature = "plugin")]
pub mod plugin;

#[cfg(feature = "plugin")]
pub use host::{Position, Speed, Sprite};
pub use types::EntityId;

/// Fixed entry-point names a plugin module must export. Available to both the
/// host (which looks these up) and the plugins (which define them), so the
/// names never drift.
pub mod export {
    pub const META: &str = "ruleste_plugin_meta";
    pub const ENTITY_TYPES: &str = "ruleste_plugin_entity_types";
    pub const INIT: &str = "ruleste_entity_init";
    pub const UPDATE: &str = "ruleste_entity_update";
    pub const DRAW: &str = "ruleste_entity_draw";
    pub const DESTROY: &str = "ruleste_entity_destroy";
    pub const SERIALIZE: &str = "ruleste_entity_serialize";
    pub const DESERIALIZE: &str = "ruleste_entity_deserialize";
}

/// Common gameplay events a plugin can emit through `host_emit`. This is the
/// single registry for event ids: every plugin agrees on these numbers, and
/// they never collide with the `host::EV_*` constants (which now alias these).
pub mod event {
    // Player-authored events.
    pub const PLAYER_DASH: u32 = 0;
    pub const PLAYER_JUMP: u32 = 1;
    pub const PLAYER_DEATH: u32 = 2;
    // Entity-interaction events (refill/booster/crushBlock).
    pub const REFILL: u32 = 3;
    pub const BOOST: u32 = 4;
    pub const CRUSH: u32 = 5;
}
