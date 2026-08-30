#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-cassette` — `cassette` (universal, B-side tapes).
//!
//! A collectible that emits the `CASSETTE` event when the player touches it; the
//! `cassette-block` crate flips its blocks in response. Mirrors `Cassette` in
//! `references/source/Celeste/Celeste/Cassette.cs`. It only emits, so it lives in
//! its own crate.

use ruleste_plugins_api::event;
use ruleste_plugins_api::host::{self, entities_by_type};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, Hitbox, Position, spawn_data};
use ruleste_plugins_api::types::EntityId;

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("cassette");
ruleste_plugins_api::ruleste_entity_types!("cassette");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

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
    e.hitbox.set(16.0, 16.0, -8.0, -8.0);
    e.depth.set(200);
    e.sprite.set_bank("cassette");
    e.sprite.play("idle");
    COLLECTED.with(|s| s.borrow_mut().insert(id, false));
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, _dt: f32) {
    if COLLECTED.with(|s| s.borrow_mut().get(id).copied().unwrap_or(false)) {
        return;
    }
    let p = Entity::new(id).position.get();
    if let Some((px, py, pw, ph)) = player_rect()
        && overlap(px, py, pw, ph, p.x - 8.0, p.y - 8.0, 16.0, 16.0)
    {
        COLLECTED.with(|s| s.borrow_mut().insert(id, true));
        host::emit(id, event::CASSETTE, &[]);
        host::collect(id);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(_id: EntityId) {
    // Visual is drawn by the host via the SpriteBank ("cassette" sprite).
}
