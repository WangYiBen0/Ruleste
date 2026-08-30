#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-fake-wall` — `fakeWall` (universal).
//!
//! A solid that vanishes the moment the player touches it (a hidden wall that is
//! revealed by contact). Mirrors `FakeWall` in
//! `references/source/Celeste/Celeste/FakeWall.cs`.

use ruleste_plugins_api::host::{self, draw_tile_box, entities_by_type};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, Hitbox, Position, spawn_data};
use ruleste_plugins_api::types::EntityId;

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("fake-wall");
ruleste_plugins_api::ruleste_entity_types!("fakeWall");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

#[derive(Clone, Copy)]
struct State {
    w: f32,
    h: f32,
}

thread_local! {
    static STATES: RefCell<ruleste_plugins_api::plugin::EntityState<State>> =
        RefCell::new(ruleste_plugins_api::plugin::EntityState::new());
}

fn player_rect() -> Option<(f32, f32, f32, f32)> {
    let players = entities_by_type("player");
    let p = *players.first()?;
    let pp = Position::new(p).get();
    let (pw, ph, pox, poy) = Hitbox::new(p).get();
    Some((pp.x + pox, pp.y + poy, pw, ph))
}

#[allow(clippy::too_many_arguments)]
fn overlap(ax: f32, ay: f32, aw: f32, ah: f32, bx: f32, by: f32, bw: f32, bh: f32) -> bool {
    ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    let w = spawn.get_float("width", 8.0).max(4.0);
    let h = spawn.get_float("height", 8.0).max(4.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.collision.solid(true);
    e.depth.set(-13000);
    STATES.with(|s| s.borrow_mut().insert(id, State { w, h }));
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, _dt: f32) {
    let st = match STATES.with(|s| s.borrow_mut().get(id).copied()) {
        Some(st) => st,
        None => return,
    };
    let p = Entity::new(id).position.get();
    if player_rect().is_some_and(|(px, py, pw, ph)| overlap(px, py, pw, ph, p.x, p.y, st.w, st.h)) {
        Entity::new(id).collision.solid(false);
        host::set_visible(id, false);
        host::remove(id);
        STATES.with(|s| s.borrow_mut().remove(id));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let st = match STATES.with(|s| s.borrow_mut().get(id).copied()) {
        Some(st) => st,
        None => return,
    };
    let p = Entity::new(id).position.get();
    draw_tile_box('3', p.x, p.y, (st.w / 8.0) as u32, (st.h / 8.0) as u32);
}
