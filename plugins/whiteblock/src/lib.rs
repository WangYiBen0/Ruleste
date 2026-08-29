#![allow(clippy::not_unsafe_ptr_arg_deref)]
use std::cell::RefCell;

use ruleste_plugins_api::host::{
    draw_rect, entities_by_type, is_cold_mode, play_sound, player_ducking,
};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("whiteblock");
ruleste_plugins_api::ruleste_entity_types!("whiteblock");

const DUCK_DURATION: f32 = 3.0;

const COLOR_ENABLED: Color = Color {
    r: 0xf2,
    g: 0xf2,
    b: 0xf2,
    a: 0xff,
};
const COLOR_DISABLED: Color = Color {
    r: 0xf2,
    g: 0xf2,
    b: 0xf2,
    a: 0x40,
};

#[derive(Clone, Copy)]
struct WhiteBlockState {
    enabled: bool,
    activated: bool,
    duck_timer: f32,
    w: f32,
    h: f32,
}

impl Default for WhiteBlockState {
    fn default() -> Self {
        Self {
            enabled: true,
            activated: false,
            duck_timer: 0.0,
            w: 8.0,
            h: 8.0,
        }
    }
}

thread_local! {
    static STATES: RefCell<EntityState<WhiteBlockState>> = RefCell::new(EntityState::new());
    static SER_BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut WhiteBlockState) -> R) -> R {
    STATES.with(|s| {
        let mut states = s.borrow_mut();
        let st = states.get_or_insert(id, WhiteBlockState::default) as *mut WhiteBlockState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

fn player_ducking_on_top() -> bool {
    for pid in entities_by_type("player") {
        if !player_ducking(pid) {
            continue;
        }
        let pe = Entity::new(pid);
        let pp = pe.position.get();
        let (pw, ph, _, _) = pe.hitbox.get();
        for eid in entities_by_type("whiteblock") {
            let ee = Entity::new(eid);
            let ep = ee.position.get();
            let (ew, _, _, _) = ee.hitbox.get();
            if pp.x < ep.x + ew && pp.x + pw > ep.x && pp.y + ph >= ep.y && pp.y + ph <= ep.y + 4.0
            {
                return true;
            }
        }
    }
    false
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    let w = spawn.get_float("width", 8.0);
    let h = spawn.get_float("height", 8.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.collision.platform(true);
    e.depth.set(8990);

    with_state(id, |st| {
        st.w = w;
        st.h = h;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        if !st.enabled {
            return;
        }

        if !st.activated {
            let on_top = player_ducking_on_top();
            if on_top && is_cold_mode() {
                st.duck_timer += dt;
                if st.duck_timer >= DUCK_DURATION {
                    st.activated = true;
                    Entity::new(id).collision.platform(false);
                    play_sound("event:/game/04_cliffside/whiteblock_fallthru");
                }
            } else {
                st.duck_timer = 0.0;
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let e = Entity::new(id);
    let p = e.position.get();
    let st = STATES.with(|s| s.borrow().get(id).copied());

    let (w, h) = match st {
        Some(s) => (s.w, s.h),
        None => (8.0, 8.0),
    };
    let color = match st {
        Some(s) if !s.enabled || s.activated => COLOR_DISABLED,
        _ => COLOR_ENABLED,
    };
    draw_rect(p.x, p.y, w, h, color);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_serialize(id: EntityId, out_len: *mut u32) -> u32 {
    SER_BUF.with(|buf| {
        let mut buf = buf.borrow_mut();
        buf.clear();
        STATES.with(|s| {
            let states = s.borrow();
            if let Some(st) = states.get(id) {
                buf.push(if st.enabled { 1 } else { 0 });
                buf.push(if st.activated { 1 } else { 0 });
                buf.extend_from_slice(&st.duck_timer.to_le_bytes());
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
    if bytes.len() < 2 {
        return;
    }
    with_state(id, |st| {
        let mut off = 0;
        st.enabled = bytes[off] != 0;
        off += 1;
        st.activated = bytes[off] != 0;
        off += 1;
        if bytes.len() >= off + 4 {
            st.duck_timer = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
            off += 4;
        }
        if bytes.len() >= off + 4 {
            st.w = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
        }
        if bytes.len() >= off + 8 {
            st.h = f32::from_le_bytes(bytes[off + 4..off + 8].try_into().unwrap());
        }
    });

    let e = Entity::new(id);
    STATES.with(|s| {
        let states = s.borrow();
        if let Some(st) = states.get(id)
            && st.activated
        {
            e.collision.platform(false);
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
    fn constants_match_whiteblock_cs() {
        assert!((DUCK_DURATION - 3.0).abs() < 0.001);
    }

    #[test]
    fn default_state_is_enabled() {
        let s = WhiteBlockState::default();
        assert!(s.enabled);
        assert!(!s.activated);
        assert_eq!(s.duck_timer, 0.0);
    }
}
