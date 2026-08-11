//! `towerviewer` entity plugin.
//!
//! A chapter-script marker (`TowerViewer.cs`) that launches the fixed summit
//! flyby camera when reached. The camera/scripting system is not implemented,
//! so the plugin occupies the entity slot and nothing else; the room keeps
//! playing even where the original would cut to the cinematic.

use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, spawn_data};
use ruleste_plugin_api::types::EntityId;

pub fn init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    entity
        .position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    entity.depth.set(-1_000_000);
}

pub fn update(_id: EntityId, _dt: f32) {}

pub fn draw(_id: EntityId) {}
