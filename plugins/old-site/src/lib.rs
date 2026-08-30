#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-old-site` — Old Site (chapter 2) specific entities.
//!
//! * `payphone` -> the flickering payphone prop (`references/source/Celeste/
//!   Celeste/Payphone.cs`). It is purely decorative here: the original triggers a
//!   cutscene/dialogue, which this clone does not drive, so it just renders.

use ruleste_plugins_api::host::draw_image;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::EntityId;

ruleste_plugins_api::ruleste_meta!("old-site");
ruleste_plugins_api::ruleste_entity_types!("payphone");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    e.hitbox.set(12.0, 20.0, -6.0, -20.0);
    e.depth.set(1);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let p = Entity::new(id).position.get();
    // The real payphone sprite, anchored bottom-centre at the base (matches the
    // original pole/box/light placement).
    draw_image(
        "objects/payphone/phone00",
        p.x - 6.0,
        p.y - 20.0,
        0.0,
        1.0,
        1.0,
    );
}
