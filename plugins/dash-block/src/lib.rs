#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `dashBlock` entity plugin — rewritten to mirror `DashBlock.cs`.
//!
//! The block is a fully solid `Solid` (`DashBlock` extends `Solid`): the player
//! collides with it and can stand on it. A DASH against one of its faces breaks
//! it (`OnDashCollide` → `OnDashed`), signalled by the player plugin's
//! `EV_DASH_BLOCK` (payload: hit-face direction + the player state at dash
//! time). `canDash=false` blocks only break during the red dash (state 5) or
//! summit launch (state 10), mirroring the `player.StateMachine.State` gate in
//! `OnDashed`; otherwise the dash returns `NormalCollision` and the player just
//! bounces. `permanent` blocks shatter for the session (`host_collect`, kept
//! out of respawns); the rest disappear until the next death respawn
//! (`host_remove`).

use ruleste_plugins_api::host;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("dash-block");
ruleste_plugins_api::ruleste_entity_types!("dashBlock");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

/// `Player.StateMachine.State` values a `canDash=false` block still breaks for
/// (`OnDashed`: `!canDash && state != 5 && state != 10 → NormalCollision`).
const ST_RED_DASH: u32 = 5;
const ST_SUMMIT_LAUNCH: u32 = 10;

#[derive(Debug, Default)]
struct DashBlockState {
    w: f32,
    h: f32,
    broken: bool,
    can_dash: bool,
    permanent: bool,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<DashBlockState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut DashBlockState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, DashBlockState::default) as *mut DashBlockState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let w = spawn.get_float("width", 8.0);
    let h = spawn.get_float("height", 8.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(w, h, 0.0, 0.0);
    // `DashBlock : Solid` — collidable on every face, landable on top.
    entity.collision.solid(true);
    entity.depth.set(500);
    with_state(id, |st| {
        st.w = w;
        st.h = h;
        st.can_dash = spawn.get_bool("canDash", true);
        st.permanent = spawn.get_bool("permanent", true);
    });
}

/// `OnDashed(player, direction)`: a dash may only break the block when
/// `canDash` or the player is in a state that punches through (`5`/`10`).
fn dash_can_break(st: &DashBlockState, state_at_dash: u32) -> bool {
    st.can_dash || state_at_dash == ST_RED_DASH || state_at_dash == ST_SUMMIT_LAUNCH
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        if st.broken {
            return;
        }
        // `OnDashed`: the player plugin reports a dash against this face; the
        // block breaks when the `canDash` gate passes.
        for (_, kind, data) in host::drain_events() {
            if kind == ruleste_plugins_api::plugin::event::DASH_BLOCK {
                let state_at_dash = data.get(8).copied().unwrap_or(0) as u32;
                if dash_can_break(st, state_at_dash) {
                    st.broken = true;
                    if st.permanent {
                        // Shattered for the session (`RemoveAndFlagAsGone`).
                        host::collect(id);
                    } else {
                        // Broken this life; returns on the next respawn.
                        host::remove(id);
                    }
                }
            }
        }
        let _ = dt;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        if st.broken {
            return;
        }
        let p = Entity::new(id).position.get();
        host::draw_rect(p.x, p.y, st.w, st.h, Color::new(0x32, 0x3a, 0x48, 0xff));
        host::draw_rect(p.x, p.y, st.w, 2.0, Color::new(0x9a, 0x78, 0x50, 0xff));
    });
}
