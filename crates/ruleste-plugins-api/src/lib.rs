pub mod ease;
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
    // `DASH_BLOCK` reports a dash against a `dashBlock` face; the payload is the
    // dash direction (two `f32`, matching the `CRUSH` payload) so the block can
    // apply the `canDash` gate (`OnDashCollide`).
    pub const DASH_BLOCK: u32 = 12;
    // Player permanently gains Dream Dash ability. No payload.
    pub const DREAM_DASH_GRANTED: u32 = 13;
    // `CASSETTE` toggles cassette-block solidity: emitted by a collected
    // `cassette` and consumed by `cassetteBlock` plugins. No payload.
    pub const CASSETTE: u32 = 14;
    // `SWITCH` opens linked `switchGate` blocks: emitted by a triggered
    // `touchSwitch` and consumed by `switchGate` plugins. No payload.
    pub const SWITCH: u32 = 15;
    // `KEY` opens linked `lockBlock` solids: emitted by a collected `key` and
    // consumed by `lockBlock` plugins. No payload.
    pub const KEY: u32 = 16;
    // `ATTRACT` pulls the player toward a `darkChaser`: emitted by the chaser
    // when the player is inside its attract radius. Payload is the chaser's
    // world position as two `f32` (`x`, `y`) so the player can lerp toward it.
    pub const ATTRACT: u32 = 17;
    // `TEMPLE_FALL` starts the scripted Mirror Temple fall: emitted by the
    // `mirrorTemple` trigger when the player steps onto the collapsing floor.
    // No payload.
    pub const TEMPLE_FALL: u32 = 18;
    // `CASSETTE_RIDE` flags the player as riding a moving `cassetteBlock`:
    // emitted by the block while the player rests on its top face. Payload is a
    // single `u8` (`1` = riding, `0` = left). The player switches to
    // `StCassetteFly` for the ride without changing its normal physics.
    pub const CASSETTE_RIDE: u32 = 19;
    // `SPRING_BOUNCE` fires when a non-player actor (Holdable / Puffer / Seeker)
    // lands on a `spring`, mirroring `Spring.OnHoldable` / `OnPuffer` /
    // `OnSeeker`. The payload is `[u32 target_id][u8 orientation: 0=floor,
    // 1=wallLeft, 2=wallRight][f32 from_x][f32 from_y]` so the targeted plugin
    // can apply its own launch.
    pub const SPRING_BOUNCE: u32 = 20;
}
