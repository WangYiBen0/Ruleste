#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `redBlocks` / `yellowBlocks` / `greenBlocks` entity plugin.
//!
//! These are the Celestial Resort clutter blocks (`ClutterBlockBase.cs`):
//! standable solid platforms rendered with real `objects/resortclutter/` sprites.

use ruleste_plugins_api::host::draw_image;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::EntityId;

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("clutter");
ruleste_plugins_api::ruleste_entity_types!("redBlocks", "yellowBlocks", "greenBlocks");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

thread_local! {
    static COLORS: RefCell<std::collections::HashMap<EntityId, &'static str>> =
        RefCell::new(std::collections::HashMap::new());
}

fn color_for_type(t: &str) -> &'static str {
    if t.contains("red") {
        "red"
    } else if t.contains("yellow") {
        "yellow"
    } else {
        "green"
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity_type = spawn.get_str("_entity_type", "");
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let w = spawn.get_float("width", 8.0);
    let h = spawn.get_float("height", 8.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(w, h, 0.0, 0.0);
    entity.depth.set(8999);
    entity.collision.solid(true);
    COLORS.with(|s| s.borrow_mut().insert(id, color_for_type(&entity_type)));
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let entity = Entity::new(id);
    let p = entity.position.get();
    let color = COLORS.with(|s| s.borrow().get(&id).copied().unwrap_or("green"));
    // Real clutter block sprite (objects/resortclutter/{color}_00, 16x16).
    draw_image(
        &format!("objects/resortclutter/{color}_00"),
        p.x,
        p.y,
        0.0,
        1.0,
        1.0,
    );
}
