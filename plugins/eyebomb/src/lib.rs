ruleste_plugins_api::ruleste_meta!("eyebomb");
ruleste_plugins_api::ruleste_entity_types!("eyebomb");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

use ruleste_plugins_api::host::{die, draw_rect, entities_by_type};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{spawn_data, Entity};
use ruleste_plugins_api::types::{Color, EntityId};

const HAZARD: Color = Color {
    r: 0xff,
    g: 0x44,
    b: 0x44,
    a: 0xff,
};

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    let w = spawn.get_float("width", 16.0).max(4.0);
    let h = spawn.get_float("height", 16.0).max(4.0);
    e.hitbox.set(w, h, 0.0, 0.0);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, _dt: f32) {
    let e = Entity::new(id);
    let p = e.position.get();
    let (w, h, _, _) = e.hitbox.get();
    for pid in entities_by_type("player") {
        let pp = Entity::new(pid).position.get();
        let (pw, ph, _, _) = Entity::new(pid).hitbox.get();
        if p.x < pp.x + pw && p.x + w > pp.x && p.y < pp.y + ph && p.y + h > pp.y {
            die();
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let e = Entity::new(id);
    let p = e.position.get();
    let (w, h, _, _) = e.hitbox.get();
    draw_rect(p.x, p.y, w, h, HAZARD);
}
