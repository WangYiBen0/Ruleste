#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-exit-block` — `exitBlock` (universal).
//!
//! A solid that opens (becomes non-solid) while the player is overlapping it, so
//! the player can pass through, then re-solidifies once the player has cleared
//! it — closing the passage behind them. Mirrors `ExitBlock` in
//! `references/source/Celeste/Celeste/ExitBlock.cs`.

use ruleste_plugins_api::host::{draw_rect, entities_by_type};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{spawn_data, Entity, Hitbox, Position};
use ruleste_plugins_api::types::{Color, EntityId};

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("exit-block");
ruleste_plugins_api::ruleste_entity_types!("exitBlock");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const WALL: Color = Color { r: 0x4a, g: 0x4a, b: 0x55, a: 0xff };
const WALL_OPEN: Color = Color { r: 0x4a, g: 0x4a, b: 0x55, a: 0x20 };

#[derive(Clone, Copy)]
struct State {
    w: f32,
    h: f32,
    open: bool,
}

thread_local! {
    static STATES: RefCell<ruleste_plugins_api::plugin::EntityState<State>> =
        RefCell::new(ruleste_plugins_api::plugin::EntityState::new());
}

fn player_rect() -> Option<(f32, f32, f32, f32)> {
    let players = entities_by_type("player");
    let p = *players.first()?;
    let pp = Position::new(p).get();
    let (pw, ph, pox, poy) = Hitbox::new(p).get();
    Some((pp.x + pox, pp.y + poy, pw, ph))
}

fn overlap(ax: f32, ay: f32, aw: f32, ah: f32, bx: f32, by: f32, bw: f32, bh: f32) -> bool {
    ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    let w = spawn.get_float("width", 8.0).max(4.0);
    let h = spawn.get_float("height", 8.0).max(4.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.collision.solid(true);
    e.depth.set(-13000);
    STATES.with(|s| s.borrow_mut().insert(id, State { w, h, open: false }));
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let mut st = match STATES.with(|s| s.borrow_mut().get_mut(id).copied()) {
        Some(st) => st,
        None => return,
    };
    let p = Entity::new(id).position.get();
    let touching = player_rect()
        .is_some_and(|(px, py, pw, ph)| overlap(px, py, pw, ph, p.x, p.y, st.w, st.h));
    if touching && !st.open {
        st.open = true;
        Entity::new(id).collision.solid(false);
    } else if !touching && st.open {
        st.open = false;
        Entity::new(id).collision.solid(true);
    }
    let _ = dt;
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
    let c = if st.open { WALL_OPEN } else { WALL };
    draw_rect(p.x, p.y, st.w, st.h, c);
}
