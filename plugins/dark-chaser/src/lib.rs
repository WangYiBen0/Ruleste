#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-dark-chaser` — `darkChaser` (universal, B-side reflection).
//!
//! A homing hazard: it drifts toward the player and kills on contact (the
//! Badeline "chaser" that appears in B-side reflection rooms). Phases through
//! walls like the original.

use ruleste_plugins_api::host::{self, draw_rect, entities_by_type};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{spawn_data, Entity, Hitbox, Position};
use ruleste_plugins_api::types::{Color, EntityId, Vec2};

ruleste_plugins_api::ruleste_meta!("dark-chaser");
ruleste_plugins_api::ruleste_entity_types!("darkChaser");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const BODY: Color = Color { r: 0x14, g: 0x10, b: 0x1c, a: 0xff };
const EYE: Color = Color { r: 0xff, g: 0x40, b: 0x40, a: 0xff };

const SPEED: f32 = 70.0;
const R: f32 = 7.0;

fn player_pos() -> Option<Vec2> {
    let players = entities_by_type("player");
    let p = *players.first()?;
    let pp = Position::new(p).get();
    let (pw, ph, pox, poy) = Hitbox::new(p).get();
    Some(Vec2::new(pp.x + pox + pw / 2.0, pp.y + poy + ph / 2.0))
}

fn overlap_circle(px: f32, py: f32, cx: f32, cy: f32, r: f32) -> bool {
    (px - cx) * (px - cx) + (py - cy) * (py - cy) <= r * r
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    e.hitbox.set(R * 2.0, R * 2.0, -R, -R);
    e.depth.set(9000);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let e = Entity::new(id);
    let p = e.position.get();
    let Some(target) = player_pos() else {
        return;
    };
    let dx = target.x - p.x;
    let dy = target.y - p.y;
    let d = (dx * dx + dy * dy).sqrt();
    if d > 0.001 {
        let step = (SPEED * dt).min(d);
        e.position.set_xy(p.x + dx / d * step, p.y + dy / d * step);
    }
    // Kill on contact with the player center.
    if let Some(tp) = player_pos() {
        let players = entities_by_type("player");
        if let Some(&pid) = players.first() {
            let pp = Position::new(pid).get();
            let (pw, ph, pox, poy) = Hitbox::new(pid).get();
            let pcx = pp.x + pox + pw / 2.0;
            let pcy = pp.y + poy + ph / 2.0;
            if overlap_circle(pcx, pcy, e.position.get().x, e.position.get().y, R + (pw.min(ph)) / 2.0) {
                let _ = tp;
                host::die();
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let p = Entity::new(id).position.get();
    draw_rect(p.x - R, p.y - R, R * 2.0, R * 2.0, BODY);
    draw_rect(p.x - 2.0, p.y - 3.0, 4.0, 4.0, EYE);
}
