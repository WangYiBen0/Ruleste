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
    // Player-targeted launch/flight events (Bumper / FlyFeather / BadelineBoost).
    // `LAUNCH` carries the launch direction as two `f32`s.
    pub const LAUNCH: u32 = 6;
    // `STARFLY` starts the feather-flight state; no payload.
    pub const STARFLY: u32 = 7;
    // `BADELINE_BOOST` launches the player up off a track platform; the payload
    // is the target `x` the player eases towards while rising (one `f32`).
    pub const BADELINE_BOOST: u32 = 8;
    // `CARRIED` attaches (`1`) or releases (`0`) the player to a moving track;
    // the carrying plugin drives the player's position while attached.
    pub const CARRIED: u32 = 9;
    // `SIDE_BOUNCE` is a wall spring launch: one `f32` direction (±1).
    pub const SIDE_BOUNCE: u32 = 10;
    // `SUPER_BOUNCE` is a floor spring launch (`Player.SuperBounce`): one `f32`
    // `fromY` the player snaps to before being launched up at -185.
    pub const SUPER_BOUNCE: u32 = 11;
}
