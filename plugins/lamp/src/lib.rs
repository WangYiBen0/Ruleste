#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `lamp` scenery entity plugin.
//!
//! Draws a hanging lamp from the `scenery/lamp` atlas frame. Mirrors
//! `Celeste.Lamp`: the map position is the lamp's bottom-center, and the sprite
//! origin is (width/2, height). The frame is a direct atlas subtexture, so the
//! plugin records it as a static animation (`scenery/lamp`) and shifts the draw
//! anchor up-left by half the frame size via the hitbox offset; the host's
//! renderer falls back to direct atlas frames when the SpriteBank has no entry.

use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, spawn_data};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("lamp");
ruleste_plugin_api::ruleste_entity_types!("lamp");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// The lamp frame is 16x80 and hangs from its bottom-center.
const FRAME_W: f32 = 16.0;
const FRAME_H: f32 = 80.0;

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    entity.position.set_xy(x, y);
    entity.sprite.play("scenery/lamp");
    // Move the draw anchor to the lamp's top-left so the frame, drawn with a
    // (0,0) origin, has its bottom-center at the map position.
    entity.hitbox.set(0.0, 0.0, -FRAME_W * 0.5, -FRAME_H);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(_id: EntityId, _dt: f32) {}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(_id: EntityId) {}
