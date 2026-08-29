#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `holdable` subsystem plugin (skeleton / virtual component).
//!
//! This plugin does **not** own any entities of its own. It exists to centralise
//! the carry/pickup contract that `Celeste.Holdable` defines in the original
//! `Player.cs`:
//!
//! - `Pickup`: the player runs into a `Holdable`; the player's position is
//!   frozen and the carry-offset tween is driven from this plugin (the player
//!   only receives `EV_CARRIED 1` with the target world position).
//! - `Carry`: each frame the holder (e.g. `TheoCrystal` or `Jellyfish`) reads
//!   the player's position, advances the carry-offset tween, and re-emits
//!   `EV_CARRIED 1 (pos)`. The player side, gated on `carried=true`, simply
//!   copies the position into its own `Position` and zeros `Speed` (the
//!   existing `EV_CARRIED` handler in `plugins/player` already does this).
//! - `Drop`: clears the carry, the player resumes normal movement.
//!
//! Concrete `Holdable` entities (Theo, Jellyfish, Key, ZipMover follower) live
//! in their own plugins and call into this one for shared math. Right now the
//! plugin only re-exports the event names and constants the player + the
//! holder plugins need to agree on, so the contract is documented in one place
//! and can grow (e.g. `slowFall` glide math) without each holder re-deriving
//! the same tween.
//!
//! Why not put this in `Player.cs`? In the original C# the `StPickup` state
//! plus its `PickupCoroutine` lives on `Player` itself. Ruleste splits it
//! along the subsystem boundary so that any plugin can drive a holdable
//! without forcing the player plugin to know about every holder type.

use ruleste_plugins_api::host;
use ruleste_plugins_api::types::EntityId;

ruleste_plugins_api::ruleste_meta!("holdable");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

/// No entity types — this plugin is subsystem-only.
#[unsafe(no_mangle)]
pub extern "C" fn ruleste_plugin_entity_types(out_len: *mut u32) -> u32 {
    unsafe { *out_len = 0 };
    0
}

/// Event ids the holdable subsystem re-uses. These mirror the upstream event
/// numbers in `ruleste_plugins_api::event::*` so holder plugins can emit them
/// directly.
///
/// The carry contract is a single `EV_CARRIED` event from the holder to the
/// player:
/// - **attach**: payload `[f32 carryX][f32 carryY][u8 on=1]` — the player
///   freezes and snaps to `(carryX, carryY)` each frame the event arrives.
/// - **release**: payload `[f32 _][f32 _][u8 on=0]` — the player resumes normal
///   movement. `EV_CARRIED_ATTACH` and `EV_CARRIED_RELEASE` are aliases kept so
///   holder plugins document intent at the call site; both resolve to the same
///   `host::EV_CARRIED` number.
pub const EV_CARRIED_ATTACH: u32 = host::EV_CARRIED;
pub const EV_CARRIED_RELEASE: u32 = host::EV_CARRIED;

/// How far above the player's hitbox the held object rests while carried.
/// `Player.CarryOffsetTarget = new Vector2(0, -12)` in the original.
pub const CARRY_OFFSET_Y: f32 = -12.0;

/// `Player.CarryOffsetTarget.X` is `0` in normal Madeline hair; the final
/// integer-snapped X delta is applied per-frame inside the holder plugin.
pub const CARRY_OFFSET_X: f32 = 0.0;

/// Duration of the tween that snaps the holder into the carry slot.
/// `PickupCoroutine`: `Tween.Create(Tween.TweenMode.Oneshot, Ease.CubeInOut, 0.16f)`.
pub const PICKUP_TWEEN_TIME: f32 = 0.16;

/// The holdable tween's control point lies 2 px beyond the holdable's
/// `Math.Sign(begin.X) * 2` so the holder sways slightly toward the player's
/// facing side. Stored here so holders agree on the magnitude.
pub const PICKUP_SWAY: f32 = 2.0;

/// No-op init — the host never spawns a `holdable` entity, this exists only
/// to satisfy the `ruleste_entity_init` export the loader looks up.
#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(_id: EntityId, _data: *const u8, _len: u32) {}

/// No-op update — same rationale as `init`.
#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

/// No-op draw.
#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(_id: EntityId) {}
