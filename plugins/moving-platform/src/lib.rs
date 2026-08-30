#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-moving-platform` — `movingPlatform` (universal).
//!
//! A solid platform that rides between its spawn point and `node[0]` (ping-pong),
//! carrying anything standing on top. Mirrors `MovingPlatform` in
//! `references/source/Celeste/Celeste/MovingPlatform.cs`.

use ruleste_plugins_api::host::draw_image;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::{EntityId, Vec2};

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("moving-platform");
ruleste_plugins_api::ruleste_entity_types!("movingPlatform");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const SPEED: f32 = 40.0;

#[derive(Clone, Copy)]
struct State {
    w: f32,
    h: f32,
    start: Vec2,
    node: Vec2,
    to_node: bool,
}

thread_local! {
    static STATES: RefCell<ruleste_plugins_api::plugin::EntityState<State>> =
        RefCell::new(ruleste_plugins_api::plugin::EntityState::new());
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    e.position.set_xy(x, y);
    let w = spawn.get_float("width", 32.0).max(4.0);
    let h = spawn.get_float("height", 8.0).max(4.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.collision.platform(true);
    e.depth.set(200);
    let node = spawn.get_node(0).unwrap_or(Vec2::new(x, y));
    STATES.with(|s| {
        s.borrow_mut().insert(
            id,
            State {
                w,
                h,
                start: Vec2::new(x, y),
                node,
                to_node: true,
            },
        )
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let mut st = match STATES.with(|s| s.borrow_mut().get_mut(id).copied()) {
        Some(st) => st,
        None => return,
    };
    let e = Entity::new(id);
    let target = if st.to_node { st.node } else { st.start };
    let p = e.position.get();
    let dx = target.x - p.x;
    let dy = target.y - p.y;
    let d = (dx * dx + dy * dy).sqrt();
    if d <= SPEED * dt {
        e.position.set_xy(target.x, target.y);
        st.to_node = !st.to_node;
    } else {
        e.position
            .set_xy(p.x + dx / d * SPEED * dt, p.y + dy / d * SPEED * dt);
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
    while ty < st.h {
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
