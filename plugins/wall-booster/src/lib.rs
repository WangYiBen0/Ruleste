#![allow(clippy::not_unsafe_ptr_arg_deref)]
use std::cell::RefCell;

use ruleste_plugins_api::event;
use ruleste_plugins_api::host::{draw_rect, emit, entities_by_type, is_cold_mode, play_sound};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("wallBooster");
ruleste_plugins_api::ruleste_entity_types!("wallBooster");

const BOOST_HOT: Color = Color {
    r: 0x33,
    g: 0xcc,
    b: 0xff,
    a: 0xff,
};
const BOOST_COLD: Color = Color {
    r: 0x88,
    g: 0xee,
    b: 0xff,
    a: 0xff,
};
const BOOST_EDGE: Color = Color {
    r: 0x11,
    g: 0x88,
    b: 0xcc,
    a: 0xff,
};

#[derive(Clone, Copy)]
struct WallBoosterState {
    left: bool,
    not_core_mode: bool,
    ice_mode: bool,
    last_ice_mode: bool,
    w: f32,
    h: f32,
    cooldown: f32,
}

impl Default for WallBoosterState {
    fn default() -> Self {
        Self {
            left: true,
            not_core_mode: false,
            ice_mode: false,
            last_ice_mode: false,
            w: 2.0,
            h: 8.0,
            cooldown: 0.0,
        }
    }
}

thread_local! {
    static STATES: RefCell<EntityState<WallBoosterState>> = RefCell::new(EntityState::new());
    static SER_BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut WallBoosterState) -> R) -> R {
    STATES.with(|s| {
        let mut states = s.borrow_mut();
        let st = states.get_or_insert(id, WallBoosterState::default) as *mut WallBoosterState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);

    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    e.position.set_xy(x, y);

    let h = spawn.get_float("height", 8.0);
    let left = spawn.get_bool("left", true);
    let not_core_mode = spawn.get_bool("notCoreMode", false);

    let w = 2.0;
    if left {
        e.hitbox.set(w, h, 0.0, 0.0);
    } else {
        e.hitbox.set(w, h, 6.0, 0.0);
    }

    e.collision.solid(true);
    e.depth.set(1999);

    let initial_ice = is_cold_mode() || not_core_mode;

    with_state(id, |st| {
        st.left = left;
        st.not_core_mode = not_core_mode;
        st.ice_mode = initial_ice;
        st.last_ice_mode = initial_ice;
        st.w = w;
        st.h = h;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let e = Entity::new(id);
    let p = e.position.get();

    with_state(id, |st| {
        let now_ice = st.not_core_mode || is_cold_mode();
        if now_ice != st.last_ice_mode {
            st.last_ice_mode = now_ice;
            st.ice_mode = now_ice;
        }

        if st.cooldown > 0.0 {
            st.cooldown -= dt;
        }

        if st.cooldown > 0.0 {
            return;
        }

        for pid in entities_by_type("player") {
            let pe = Entity::new(pid);
            let pp = pe.position.get();
            let (pw, ph, _, _) = pe.hitbox.get();
            if p.x < pp.x + pw && p.x + st.w > pp.x && p.y < pp.y + ph && p.y + st.h > pp.y {
                emit(id, event::BOOST, &[]);
                st.cooldown = 0.1;
                play_sound(if st.ice_mode {
                    "event:/game/09_core/icewall_boost"
                } else {
                    "event:/game/04_cliffside/wallbooster_boost"
                });
                break;
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let e = Entity::new(id);
    let p = e.position.get();

    STATES.with(|s| {
        let states = s.borrow();
        let st = match states.get(id) {
            Some(s) => s,
            None => return,
        };

        let color = if st.ice_mode { BOOST_COLD } else { BOOST_HOT };

        let x = if st.left { p.x } else { p.x + 6.0 };
        draw_rect(x, p.y, st.w, st.h, color);
        draw_rect(x, p.y, st.w, 1.0, BOOST_EDGE);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_serialize(id: EntityId, out_len: *mut u32) -> u32 {
    SER_BUF.with(|buf| {
        let mut buf = buf.borrow_mut();
        buf.clear();
        STATES.with(|s| {
            let states = s.borrow();
            if let Some(st) = states.get(id) {
                buf.push(if st.left { 1 } else { 0 });
                buf.push(if st.not_core_mode { 1 } else { 0 });
                buf.push(if st.ice_mode { 1 } else { 0 });
                buf.push(if st.last_ice_mode { 1 } else { 0 });
                buf.extend_from_slice(&st.w.to_le_bytes());
                buf.extend_from_slice(&st.h.to_le_bytes());
                buf.extend_from_slice(&st.cooldown.to_le_bytes());
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
        let mut off = 0;
        st.left = bytes[off] != 0;
        off += 1;
        st.not_core_mode = bytes[off] != 0;
        off += 1;
        st.ice_mode = bytes[off] != 0;
        off += 1;
        st.last_ice_mode = bytes[off] != 0;
        off += 1;
        if bytes.len() >= off + 4 {
            st.w = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
            off += 4;
        }
        if bytes.len() >= off + 4 {
            st.h = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
            off += 4;
        }
        if bytes.len() >= off + 4 {
            st.cooldown = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
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
    fn default_is_left_hot() {
        let s = WallBoosterState::default();
        assert!(s.left);
        assert!(!s.not_core_mode);
        assert!(!s.ice_mode);
    }

    #[test]
    fn ice_mode_flips_to_cold_when_needed() {
        let s = WallBoosterState {
            not_core_mode: true,
            ..Default::default()
        };
        let now_ice = s.not_core_mode || is_cold_mode();
        assert!(now_ice);
        let _ = s;
    }
}
