#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `killbox` entity plugin.
//!
//! Mirrors `Killbox.cs`: an invisible death rectangle that only activates once
//! the player is well inside it (hysteresis). It stays inert until the
//! player's feet are at least 32px above its top (`Bottom < Top - 32`), then
//! deactivates again once their head passes 32px below its bottom. The hitbox
//! is `width × 32`, sized by the map width. Overlap while active is lethal.

use ruleste_plugin_api::host::{self, die, entities_by_type};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("killbox");
ruleste_plugin_api::ruleste_entity_types!("killbox");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// Killbox height is always 32 (`new Hitbox(data.Width, 32f)`).
const BOX_HEIGHT: f32 = 32.0;
/// Arm/disarm hysteresis margin.
const MARGIN: f32 = 32.0;

#[derive(Debug, Default)]
struct KillState {
    collidable: bool,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<KillState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut KillState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, KillState::default) as *mut KillState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    entity
        .position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    entity
        .hitbox
        .set(spawn.get_float("width", 8.0), BOX_HEIGHT, 0.0, 0.0);
    entity.depth.set(0);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, _dt: f32) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let p = entity.position.get();
        let (w, h, ox, oy) = entity.hitbox.get();
        let top = p.y + oy;
        let bottom = top + h;

        // Hysteresis gating (`Killbox.Update`).
        for player_id in entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let (ph, poy) = {
                let (_, ph, _, poy) = host::Hitbox::new(player_id).get();
                (ph, poy)
            };
            let py = pp.y + poy;
            if !st.collidable {
                if py + ph < top - MARGIN {
                    st.collidable = true;
                }
            } else if py > bottom + MARGIN {
                st.collidable = false;
            }
        }

        if !st.collidable {
            return;
        }

        for player_id in entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
            let overlap = pp.x + pox < p.x + ox + w
                && pp.x + pox + pw > p.x + ox
                && pp.y + poy < top + h
                && pp.y + poy + ph > top;
            if overlap {
                die();
            }
            break;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(_id: EntityId) {}
