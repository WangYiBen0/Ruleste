#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-crumble-block` — `crumbleBlock` (universal).
//!
//! A width×8 solid that, once the player stands on top, shakes then disappears
//! for ~2s before respawning. Mirrors `CrumblePlatform` in
//! `references/source/Celeste/Celeste/CrumblePlatform.cs`.

use ruleste_plugins_api::host::{draw_rect, entities_by_type, player_state};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, Hitbox, Position, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("crumble-block");
ruleste_plugins_api::ruleste_entity_types!("crumbleBlock");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const CRUMBLE: Color = Color {
    r: 0x9b,
    g: 0x6a,
    b: 0x43,
    a: 0xff,
};
const CRUMBLE_FADE: Color = Color {
    r: 0x9b,
    g: 0x6a,
    b: 0x43,
    a: 0x40,
};

#[derive(Clone, Copy)]
struct State {
    w: f32,
    phase: u8, // 0 idle, 1 crumbling, 2 gone
    timer: f32,
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

fn player_on_top(bx: f32, by: f32, bw: f32) -> bool {
    match player_rect() {
        Some((px, py, pw, ph)) => {
            let bottom = py + ph;
            bottom >= by - 3.0 && bottom <= by + 5.0 && px + pw > bx + 1.0 && px < bx + bw - 1.0
        }
        None => false,
    }
}

/// `GetPlayerClimbing`: the original crumble block also triggers while the
/// player is climbing one of its sides. We detect `ST_CLIMB` (1) with the
/// player's hitbox flush against the block's left/right edge.
fn player_climbing_side(bx: f32, by: f32, bw: f32) -> bool {
    let players = entities_by_type("player");
    let pid = match players.first() {
        Some(p) => *p,
        None => return false,
    };
    if player_state(pid) != 1 {
        return false;
    }
    match player_rect() {
        Some((px, py, pw, ph)) => {
            let v_overlap = py + ph > by && py < by + 8.0;
            let left_touch = (px + pw) >= bx - 3.0 && (px + pw) <= bx + 3.0;
            let right_touch = px <= bx + bw + 3.0 && px >= bx + bw - 3.0;
            v_overlap && (left_touch || right_touch)
        }
        None => false,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    e.position.set_xy(x, y);
    let w = spawn.get_float("width", 24.0).max(4.0);
    e.hitbox.set(w, 8.0, 0.0, 0.0);
    e.collision.solid(true);
    e.depth.set(200);
    STATES.with(|s| {
        s.borrow_mut().insert(
            id,
            State {
                w,
                phase: 0,
                timer: 0.0,
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
    let p = e.position.get();
    let (bx, by) = (p.x, p.y);
    match st.phase {
        0 => {
            if player_on_top(bx, by, st.w) || player_climbing_side(bx, by, st.w) {
                st.phase = 1;
                st.timer = 0.4;
            }
        }
        1 => {
            st.timer -= dt;
            if st.timer <= 0.0 {
                e.collision.solid(false);
                st.phase = 2;
                st.timer = 2.0;
            }
        }
        2 => {
            st.timer -= dt;
            if st.timer <= 0.0 && !player_on_top(bx, by, st.w) {
                e.collision.solid(true);
                st.phase = 0;
            }
        }
        _ => {}
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
    let c = if st.phase == 2 { CRUMBLE_FADE } else { CRUMBLE };
    draw_rect(p.x, p.y, st.w, 8.0, c);
    let mut x = p.x + 8.0;
    while x < p.x + st.w - 1.0 {
        draw_rect(x - 1.0, p.y, 2.0, 8.0, CRUMBLE_FADE);
        x += 8.0;
    }
}
