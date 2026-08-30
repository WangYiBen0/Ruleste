#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `heartGemDoor` entity plugin.
//!
//! Mirrors `HeartGemDoor.cs`: the chapter-9 heart gate.
//! - Spawns a tall gate requiring `requires` hearts (default 0 or map value).
//! - When player approaches (< 80px), heart fill counter rises.
//! - When counter reaches `requires`, the door unlocks with a flash and sound,
//!   parting the top half upward by 32px and bottom half downward by 32px.

use ruleste_plugins_api::host::{self, draw_image, draw_rect};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("heart-gem-door");
ruleste_plugins_api::ruleste_entity_types!("heartGemDoor");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const DOOR_BG: Color = Color {
    r: 0x22,
    g: 0x25,
    b: 0x2f,
    a: 0xff,
};
const DOOR_SLAB: Color = Color {
    r: 0x3e,
    g: 0x44,
    b: 0x52,
    a: 0xff,
};
const EDGE: Color = Color {
    r: 0x66,
    g: 0x70,
    b: 0x86,
    a: 0xff,
};

#[derive(Debug)]
struct DoorState {
    requires: u32,
    counter: f32,
    opened: bool,
    open_percent: f32,
    w: f32,
    h: f32,
    open_dist: f32,
}

impl Default for DoorState {
    fn default() -> DoorState {
        DoorState {
            requires: 0,
            counter: 0.0,
            opened: false,
            open_percent: 0.0,
            w: 88.0,
            h: 96.0,
            open_dist: 32.0,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<DoorState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut DoorState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, DoorState::default) as *mut DoorState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let w = spawn.get_float("width", 88.0);
    let h = spawn.get_float("height", 96.0);
    let req = spawn.get_float("requires", 0.0) as u32;

    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(w, h, 0.0, 0.0);
    entity.depth.set(500);
    entity.collision.solid(true);

    with_state(id, |st| {
        st.w = w;
        st.h = h;
        st.requires = req;
        st.counter = 0.0;
        st.opened = false;
        st.open_percent = 0.0;
        st.open_dist = 32.0;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        if st.opened {
            if st.open_percent < 1.0 {
                st.open_percent = (st.open_percent + dt * 1.5).min(1.0);
                // When fully open, turn solid off so player can pass through
                if st.open_percent >= 1.0 {
                    let entity = Entity::new(id);
                    entity.collision.solid(false);
                }
            }
            return;
        }

        let entity = Entity::new(id);
        let p = entity.position.get();

        // Player approach detection (< 80px)
        let player_near = host::entities_by_type("player").into_iter().any(|pid| {
            if !host::entity_alive(pid) {
                return false;
            }
            let pp = host::Position::new(pid).get();
            let dx = (pp.x - (p.x + st.w * 0.5)).abs();
            let dy = (pp.y - (p.y + st.h * 0.5)).abs();
            dx < 80.0 && dy < 120.0
        });

        if player_near {
            let target = st.requires as f32;
            let rate = (st.requires as f32).max(1.0) * 0.8;
            st.counter = (st.counter + dt * rate).min(target);

            if st.counter >= target {
                st.opened = true;
                host::play_sound("event:/game/09_core/frontdoor_unlock");
            }
        } else {
            let rate = (st.requires as f32).max(1.0) * 2.0;
            st.counter = (st.counter - dt * rate).max(0.0);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        let w = st.w;
        let h = st.h;
        let split_h = h * 0.5;
        let offset = st.open_percent * st.open_dist;

        // Background gap
        draw_rect(p.x, p.y - st.open_dist, w, h + st.open_dist * 2.0, DOOR_BG);

        // Top wing (moves up)
        let top_y = p.y - offset;
        draw_rect(p.x, top_y, w, split_h, DOOR_SLAB);
        draw_rect(p.x, top_y, w, 3.0, EDGE);

        // Bottom wing (moves down)
        let bot_y = p.y + split_h + offset;
        draw_rect(p.x, bot_y, w, split_h, DOOR_SLAB);
        draw_rect(p.x, bot_y + split_h - 3.0, w, 3.0, EDGE);

        // Heart medallion (rendered if not fully opened) — real sprite.
        if st.open_percent < 0.9 {
            let hx = p.x + w * 0.5;
            let hy = p.y + h * 0.5;
            let icon = if st.opened || st.counter >= st.requires as f32 {
                "objects/heartdoor/icon01"
            } else {
                "objects/heartdoor/icon00"
            };
            draw_image(icon, hx - 8.0, hy - 8.0, 0.0, 1.0, 1.0);
        }
    });
}
