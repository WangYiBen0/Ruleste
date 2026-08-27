#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-black-gem` — `blackGem` (universal, B-side gems).
//!
//! The B-side black gem collectible: a solid-looking pickup that is consumed when
//! the player overlaps it.

use ruleste_plugins_api::host::{self, draw_rect, entities_by_type};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{spawn_data, Entity, Hitbox, Position};
use ruleste_plugins_api::types::{Color, EntityId};

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("black-gem");
ruleste_plugins_api::ruleste_entity_types!("blackGem");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const GEM: Color = Color { r: 0x18, g: 0x18, b: 0x20, a: 0xff };
const SPARK: Color = Color { r: 0xff, g: 0xff, b: 0xff, a: 0xff };

thread_local! {
    static COLLECTED: RefCell<ruleste_plugins_api::plugin::EntityState<bool>> =
        RefCell::new(ruleste_plugins_api::plugin::EntityState::new());
}

fn player_rect() -> Option<(f32, f32, f32, f32)> {
    let players = entities_by_type("player");
    let p = *players.first()?;
    let pp = Position::new(p).get();
    let (pw, ph, pox, poy) = Hitbox::new(p).get();
    Some((pp.x + pox, pp.y + poy, pw, ph))
}

fn overlap(ax: f32, ay: f32, aw: f32, ah: f32, bx: f32, by: f32, bw: f32, bh: f32) -> bool {
    ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    e.hitbox.set(16.0, 16.0, -8.0, -8.0);
    e.depth.set(200);
    COLLECTED.with(|s| s.borrow_mut().insert(id, false));
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, _dt: f32) {
    if COLLECTED.with(|s| s.borrow_mut().get(id).copied().unwrap_or(false)) {
        return;
    }
    let p = Entity::new(id).position.get();
    if let Some((px, py, pw, ph)) = player_rect() {
        if overlap(px, py, pw, ph, p.x - 8.0, p.y - 8.0, 16.0, 16.0) {
            COLLECTED.with(|s| s.borrow_mut().insert(id, true));
            host::collect(id);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    if COLLECTED.with(|s| s.borrow_mut().get(id).copied().unwrap_or(false)) {
        return;
    }
    let p = Entity::new(id).position.get();
    draw_rect(p.x - 6.0, p.y - 6.0, 12.0, 12.0, GEM);
    draw_rect(p.x - 2.0, p.y - 2.0, 4.0, 4.0, SPARK);
}
