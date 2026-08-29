#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `bridge` entity plugin.
//!
//! A wooden bridge that collapses as the player walks across it. The
//! implementation simplifies the original: instead of `BridgeTile` child
//! entities with their own falling physics, the parent tracks a `collapse_offset`
//! that erases the solid region starting from the left edge. Tiles drawn past
//! the offset are still rendered (so the player sees debris trailing behind
//! the collapse) but not collided. The `host::play_sound` and tile-fall timer
//! are aligned with `Bridge.cs`.

use std::cell::RefCell;

use ruleste_plugins_api::host::{draw_rect, entities_by_type, play_sound};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("bridge");
ruleste_plugins_api::ruleste_entity_types!("bridge");

const PLANK: Color = Color {
    r: 0x8b,
    g: 0x5a,
    b: 0x2b,
    a: 0xff,
};
const PLANK_DARK: Color = Color {
    r: 0x5e,
    g: 0x3b,
    b: 0x1c,
    a: 0xff,
};

const TRIGGER_DIST: f32 = 112.0;
const END_DIST_A: f32 = 216.0;
const END_DIST_B: f32 = 104.0;
const COLLAPSE_INTERVAL: f32 = 0.2;
const START_FALL_COUNT: usize = 11;
const MID_FALL_COUNT: usize = 5;
const END_FALL_COUNT: usize = 7;

#[derive(Clone, Copy)]
struct BridgeState {
    collapsing: bool,
    ended: bool,
    collapse_timer: f32,
    collapse_offset: f32,
    mid_collapse_done: bool,
    end_collapse_done: bool,
    w: f32,
    h: f32,
    x: f32,
    y: f32,
}

impl Default for BridgeState {
    fn default() -> Self {
        Self {
            collapsing: false,
            ended: false,
            collapse_timer: 0.0,
            collapse_offset: 0.0,
            mid_collapse_done: false,
            end_collapse_done: false,
            w: 32.0,
            h: 8.0,
            x: 0.0,
            y: 0.0,
        }
    }
}

thread_local! {
    static STATES: RefCell<EntityState<BridgeState>> = RefCell::new(EntityState::new());
    static SER_BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut BridgeState) -> R) -> R {
    STATES.with(|s| {
        let mut states = s.borrow_mut();
        let st = states.get_or_insert(id, BridgeState::default) as *mut BridgeState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

fn player_x() -> Option<f32> {
    let pid = entities_by_type("player").into_iter().next()?;
    let pe = Entity::new(pid);
    let pp = pe.position.get();
    Some(pp.x)
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let w = spawn.get_float("width", 32.0).max(4.0);
    let h = spawn.get_float("height", 8.0).max(2.0);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(w, h, 0.0, 0.0);
    entity.collision.solid(true);
    entity.depth.set(200);

    with_state(id, |st| {
        st.w = w;
        st.h = h;
        st.x = x;
        st.y = y;
        st.collapsing = false;
        st.collapse_offset = 0.0;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        if let Some(px) = player_x() {
            if !st.collapsing {
                if px >= st.x + TRIGGER_DIST {
                    st.collapsing = true;
                    st.collapse_offset += (START_FALL_COUNT as f32) * 8.0;
                    play_sound("event:/game/00_prologue/bridge_rumble_loop");
                }
            } else if !st.ended {
                if !st.mid_collapse_done && px > st.x + st.w - END_DIST_A {
                    st.mid_collapse_done = true;
                    st.collapse_offset += (MID_FALL_COUNT as f32) * 8.0;
                } else if !st.end_collapse_done && px > st.x + st.w - END_DIST_B {
                    st.end_collapse_done = true;
                    st.collapse_offset += (END_FALL_COUNT as f32) * 8.0;
                } else {
                    if st.collapse_timer > 0.0 {
                        st.collapse_timer -= dt;
                    } else {
                        st.collapse_offset += 8.0;
                        st.collapse_timer = COLLAPSE_INTERVAL;
                    }
                }

                if st.collapse_offset >= st.w {
                    st.ended = true;
                    st.collapsing = false;
                    Entity::new(id).collision.solid(false);
                    play_sound("event:/game/00_prologue/bridge_stop");
                }
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let p = Entity::new(id).position.get();
    let st = STATES.with(|s| s.borrow().get(id).copied());

    let (w, h, offset) = match st {
        Some(s) => (s.w, s.h, s.collapse_offset),
        None => (32.0, 8.0, 0.0),
    };

    if offset < w {
        draw_rect(p.x + offset, p.y, w - offset, h, PLANK_DARK);
        let mut x = p.x + offset + 8.0;
        while x < p.x + w - 1.0 {
            draw_rect(x - 1.0, p.y, 2.0, h, PLANK);
            x += 8.0;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_serialize(id: EntityId, out_len: *mut u32) -> u32 {
    SER_BUF.with(|buf| {
        let mut buf = buf.borrow_mut();
        buf.clear();
        STATES.with(|s| {
            let states = s.borrow();
            if let Some(st) = states.get(id) {
                buf.push(if st.collapsing { 1 } else { 0 });
                buf.push(if st.ended { 1 } else { 0 });
                buf.push(if st.mid_collapse_done { 1 } else { 0 });
                buf.push(if st.end_collapse_done { 1 } else { 0 });
                buf.extend_from_slice(&st.collapse_timer.to_le_bytes());
                buf.extend_from_slice(&st.collapse_offset.to_le_bytes());
                buf.extend_from_slice(&st.w.to_le_bytes());
                buf.extend_from_slice(&st.h.to_le_bytes());
            }
        });

        let len = buf.len();
        let layout = std::alloc::Layout::from_size_align(len.max(1), 8).unwrap();
        unsafe {
            let ptr = std::alloc::alloc(layout);
            if len > 0 {
                std::ptr::copy_nonoverlapping(buf.as_ptr(), ptr, len);
            }
            *out_len = len as u32;
            ptr as u32
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_deserialize(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    if bytes.len() < 4 {
        return;
    }
    with_state(id, |st| {
        st.collapsing = bytes[0] != 0;
        st.ended = bytes[1] != 0;
        st.mid_collapse_done = bytes[2] != 0;
        st.end_collapse_done = bytes[3] != 0;
        let mut off = 4;
        let f32_read = |bytes: &[u8], off: &mut usize| -> f32 {
            if bytes.len() >= *off + 4 {
                let v = f32::from_le_bytes(bytes[*off..*off + 4].try_into().unwrap());
                *off += 4;
                v
            } else {
                0.0
            }
        };
        st.collapse_timer = f32_read(bytes, &mut off);
        st.collapse_offset = f32_read(bytes, &mut off);
        st.w = f32_read(bytes, &mut off);
        st.h = f32_read(bytes, &mut off);
    });

    let e = Entity::new(id);
    STATES.with(|s| {
        let states = s.borrow();
        if let Some(st) = states.get(id) {
            if st.ended {
                e.collision.solid(false);
            } else {
                e.collision.solid(true);
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_destroy(id: EntityId) {
    STATES.with(|s| s.borrow_mut().remove(id));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_bridge_cs() {
        assert!((TRIGGER_DIST - 112.0).abs() < 0.001);
        assert!((END_DIST_A - 216.0).abs() < 0.001);
        assert!((END_DIST_B - 104.0).abs() < 0.001);
        assert!((COLLAPSE_INTERVAL - 0.2).abs() < 0.001);
        assert_eq!(START_FALL_COUNT, 11);
        assert_eq!(MID_FALL_COUNT, 5);
        assert_eq!(END_FALL_COUNT, 7);
    }

    #[test]
    fn default_state_is_intact() {
        let s = BridgeState::default();
        assert!(!s.collapsing);
        assert!(!s.ended);
        assert_eq!(s.collapse_offset, 0.0);
    }
}
