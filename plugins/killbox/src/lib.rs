#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `killbox` entity plugin.
//!
//! An invisible death rectangle (`Killbox.cs`): anything that falls into it is
//! killed. The host's death flow is player-only, so the box simply kills the
//! player on overlap — exactly how the original's off-screen pit boxes read.
//! Sized by `width`/`height` from the map, invisible by design.

use ruleste_plugin_api::host::{self, die, entities_by_type};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, spawn_data};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("killbox");
ruleste_plugin_api::ruleste_entity_types!("killbox");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

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
    entity.depth.set(-1_000_000);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, _dt: f32) {
    let entity = Entity::new(id);
    let p = entity.position.get();
    let (w, h, ox, oy) = entity.hitbox.get();
    for player_id in entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let overlap = pp.x + pox < p.x + ox + w
            && pp.x + pox + pw > p.x + ox
            && pp.y + poy < p.y + oy + h
            && pp.y + poy + ph > p.y + oy;
        if overlap {
            die();
        }
        break;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(_id: EntityId) {}
