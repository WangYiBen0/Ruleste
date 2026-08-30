#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-sinking-platform` — `sinkingPlatform` (universal).
//!
//! A solid platform that slowly sinks while the player stands on top and rises
//! back when they step off, letting the player descend controlled. Mirrors
//! `SinkingPlatform` in `references/source/Celeste/Celeste/SinkingPlatform.cs`.

use ruleste_plugins_api::host::{draw_image, entities_by_type};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, Hitbox, Position, spawn_data};
use ruleste_plugins_api::types::EntityId;

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("sinking-platform");
ruleste_plugins_api::ruleste_entity_types!("sinkingPlatform");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const SINK_SPEED: f32 = 22.0;
const MAX_SINK: f32 = 40.0;

#[derive(Clone, Copy)]
struct State {
    w: f32,
    base_y: f32,
}

thread_local! {
    static STATES: RefCell<ruleste_plugins_api::plugin::EntityState<State>> =
        RefCell::new(ruleste_plugins_api::plugin::EntityState::new());
}

fn player_on_top(bx: f32, by: f32, bw: f32) -> bool {
    let players = entities_by_type("player");
    let Some(&p) = players.first() else {
        return false;
    };
    let pp = Position::new(p).get();
    let (pw, ph, pox, poy) = Hitbox::new(p).get();
    let bottom = pp.y + poy + ph;
    bottom >= by - 3.0
        && bottom <= by + 5.0
        && pp.x + pox + pw > bx + 1.0
        && pp.x + pox < bx + bw - 1.0
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    e.position.set_xy(x, y);
    let w = spawn.get_float("width", 32.0).max(4.0);
    e.hitbox.set(w, 8.0, 0.0, 0.0);
    e.collision.platform(true);
    e.depth.set(200);
    STATES.with(|s| s.borrow_mut().insert(id, State { w, base_y: y }));
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let st = match STATES.with(|s| s.borrow_mut().get_mut(id).copied()) {
        Some(st) => st,
        None => return,
    };
    let e = Entity::new(id);
    let p = e.position.get();
    let on_top = player_on_top(p.x, p.y, st.w);
    if on_top {
        let ny = (p.y + SINK_SPEED * dt).min(st.base_y + MAX_SINK);
        e.position.set_xy(p.x, ny);
    } else if p.y > st.base_y {
        let ny = (p.y - SINK_SPEED * dt).max(st.base_y);
        e.position.set_xy(p.x, ny);
    }
    STATES.with(|s| {
        if let Some(st_ref) = s.borrow_mut().get_mut(id) {
            *st_ref = st;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let st = match STATES.with(|s| s.borrow_mut().get(id).copied()) {
        Some(st) => st,
        None => return,
    };
    let p = Entity::new(id).position.get();
    // Real wooden platform (32x8 tile) tiled across the body.
    let mut ty = 0.0;
    while ty < 8.0 {
        let mut tx = 0.0;
        while tx < st.w {
            draw_image(
                "objects/woodPlatform/default",
                p.x + tx,
                p.y + ty,
                0.0,
                1.0,
                1.0,
            );
            tx += 32.0;
        }
        ty += 8.0;
    }
}
