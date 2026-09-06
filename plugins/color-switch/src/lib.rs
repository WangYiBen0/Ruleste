#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-color-switch` — `colorSwitch` (ClutterSwitch).
//!
//! A colored button that toggles the solidity of matching-color tiles.
//! Mirrors `ClutterSwitch` in `references/source/Celeste/Celeste/ClutterSwitch.cs`.

use ruleste_plugins_api::host::draw_image;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::EntityId;

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("color-switch");
ruleste_plugins_api::ruleste_entity_types!("colorSwitch");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

thread_local! {
    static COLORS: RefCell<std::collections::HashMap<EntityId, &'static str>> =
        RefCell::new(std::collections::HashMap::new());
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    e.hitbox.set(16.0, 16.0, -8.0, -8.0);
    e.depth.set(200);
    // Read color from entity data (default to "red").
    let color = spawn.get_str("color", "red");
    let color_static: &'static str = match color.as_str() {
        "yellow" => "yellow",
        "green" => "green",
        "lightning" => "lightning",
        _ => "red",
    };
    COLORS.with(|s| s.borrow_mut().insert(id, color_static));
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let p = Entity::new(id).position.get();
    let color = COLORS.with(|s| s.borrow().get(&id).copied().unwrap_or("red"));
    // Real clutter switch sprite (clutter_button00) + color icon.
    draw_image(
        "objects/resortclutter/clutter_button00",
        p.x - 8.0,
        p.y - 16.0,
        0.0,
        1.0,
        1.0,
    );
    draw_image(
        &format!("objects/resortclutter/icon_{color}"),
        p.x - 8.0,
        p.y - 16.0,
        0.0,
        1.0,
        1.0,
    );
}
