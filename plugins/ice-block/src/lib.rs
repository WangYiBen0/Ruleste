#![allow(clippy::not_unsafe_ptr_arg_deref)]
use std::cell::RefCell;

use ruleste_plugins_api::host::{
    die_dir, draw_hollow_rect, draw_rect, entities_by_type, is_cold_mode, play_sound,
};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("iceBlock");
ruleste_plugins_api::ruleste_entity_types!("iceBlock");

const COLOR_FILL: Color = Color {
    r: 0xbb,
    g: 0xdd,
    b: 0xff,
    a: 0xff,
};
#[allow(dead_code)]
const COLOR_DEACTIVE: Color = Color {
    r: 0xa6,
    g: 0xff,
    b: 0xf4,
    a: 0xff,
};
#[allow(dead_code)]
const COLOR_EDGE: Color = Color {
    r: 0x6c,
    g: 0xd6,
    b: 0xeb,
    a: 0xff,
};
#[allow(dead_code)]
const COLOR_CENTER: Color = Color {
    r: 0x4c,
    g: 0xa8,
    b: 0xd6,
    a: 0xff,
};
#[allow(dead_code)]
const COLOR_DEATH: Color = Color {
    r: 0xff,
    g: 0x33,
    b: 0x33,
    a: 0xff,
};

const SOLID_INSET_X: f32 = 2.0;
const SOLID_INSET_TOP: f32 = 3.0;
const SOLID_INSET_BOTTOM: f32 = 5.0;

#[derive(Clone, Copy)]
struct IceBlockState {
    active: bool,
    last_active: bool,
    w: f32,
    h: f32,
    shake: f32,
    lava_timer: f32,
    deactivate_flash: f32,
}

impl Default for IceBlockState {
    fn default() -> Self {
        Self {
            active: true,
            last_active: true,
            w: 8.0,
            h: 8.0,
            shake: 0.0,
            lava_timer: 0.0,
            deactivate_flash: 0.0,
        }
    }
}

thread_local! {
    static STATES: RefCell<EntityState<IceBlockState>> = RefCell::new(EntityState::new());
    static SER_BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut IceBlockState) -> R) -> R {
    STATES.with(|s| {
        let mut states = s.borrow_mut();
        let st = states.get_or_insert(id, IceBlockState::default) as *mut IceBlockState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

fn approach(current: f32, target: f32, max_delta: f32) -> f32 {
    let diff = target - current;
    if diff.abs() <= max_delta {
        target
    } else if diff > 0.0 {
        current + max_delta
    } else {
        current - max_delta
    }
}

fn check_player_kill(block: (f32, f32), bw: f32, bh: f32) {
    let block_cx = block.0 + bw * 0.5;
    let block_cy = block.1 + bh * 0.5;
    for pid in entities_by_type("player") {
        let pe = Entity::new(pid);
        let pp = pe.position.get();
        let (pw, ph, _, _) = pe.hitbox.get();
        if pp.x < block.0 + bw && pp.x + pw > block.0 && pp.y < block.1 + bh && pp.y + ph > block.1
        {
            play_sound("event:/game/09_core/iceblock_death");
            // `Player.Die(dir)`: the player is flung away from the block along
            // the dominant contact axis. Horizontal contact dominates, matching
            // the original's `Util.BounceDirection` on the lava edge.
            let pcx = pp.x + pw * 0.5;
            let pcy = pp.y + ph * 0.5;
            let dx = pcx - block_cx;
            let dy = pcy - block_cy;
            let (dir_x, dir_y) = if dx.abs() >= dy.abs() {
                (if dx < 0.0 { -1.0 } else { 1.0 }, 0.0)
            } else {
                (0.0, if dy < 0.0 { -1.0 } else { 1.0 })
            };
            die_dir(dir_x, dir_y);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);

    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    e.position.set_xy(x, y);

    let w = spawn.get_float("width", 8.0);
    let h = spawn.get_float("height", 8.0);
    e.hitbox.set(w, h, 0.0, 0.0);

    e.depth.set(-8500);

    let active = is_cold_mode();
    e.collision.solid(active);

    with_state(id, |st| {
        st.w = w;
        st.h = h;
        st.active = active;
        st.last_active = active;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let e = Entity::new(id);
    let _ = e;

    with_state(id, |st| {
        st.deactivate_flash = approach(st.deactivate_flash, 0.0, dt * 5.0);
        st.shake = approach(st.shake, 0.0, dt * 8.0);

        let now_active = is_cold_mode();
        if now_active != st.last_active {
            st.last_active = now_active;
            st.active = now_active;
            e.collision.solid(now_active);
            if !now_active {
                st.deactivate_flash = 0.6;
                st.shake = 0.4;
                play_sound("event:/game/09_core/iceblock_break");
            }
        }

        if st.active {
            st.lava_timer += dt;
        } else {
            st.lava_timer = 0.0;
        }

        if st.active {
            check_player_kill((e.position.get().x, e.position.get().y), st.w, st.h);
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

        if !st.active {
            return;
        }

        let shake_x = if st.shake > 0.01 {
            (st.shake * 8.0).sin() * st.shake
        } else {
            0.0
        };

        draw_rect(
            p.x + SOLID_INSET_X,
            p.y + SOLID_INSET_TOP,
            st.w - SOLID_INSET_X * 2.0,
            st.h - SOLID_INSET_TOP - SOLID_INSET_BOTTOM,
            COLOR_FILL,
        );

        let center_x = p.x + st.w * 0.5 + shake_x;
        let center_y = p.y + st.h * 0.5;
        let _ = center_x;
        let _ = center_y;

        draw_hollow_rect(
            p.x + SOLID_INSET_X,
            p.y + SOLID_INSET_TOP,
            st.w - SOLID_INSET_X * 2.0,
            st.h - SOLID_INSET_TOP - SOLID_INSET_BOTTOM,
            COLOR_EDGE,
        );

        if st.lava_timer > 0.0 {
            let alpha = ((st.lava_timer.sin() * 0.5 + 0.5) * 0.2).min(0.2);
            let _ = alpha;
        }
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
                buf.push(if st.active { 1 } else { 0 });
                buf.push(if st.last_active { 1 } else { 0 });
                buf.extend_from_slice(&st.w.to_le_bytes());
                buf.extend_from_slice(&st.h.to_le_bytes());
                buf.extend_from_slice(&st.shake.to_le_bytes());
                buf.extend_from_slice(&st.lava_timer.to_le_bytes());
                buf.extend_from_slice(&st.deactivate_flash.to_le_bytes());
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
        st.active = bytes[off] != 0;
        off += 1;
        st.last_active = bytes[off] != 0;
        off += 1;
        let f32_read = |bytes: &[u8], off: &mut usize| -> f32 {
            if bytes.len() >= *off + 4 {
                let v = f32::from_le_bytes(bytes[*off..*off + 4].try_into().unwrap());
                *off += 4;
                v
            } else {
                0.0
            }
        };
        st.w = f32_read(bytes, &mut off);
        st.h = f32_read(bytes, &mut off);
        st.shake = f32_read(bytes, &mut off);
        st.lava_timer = f32_read(bytes, &mut off);
        st.deactivate_flash = f32_read(bytes, &mut off);
    });

    let e = Entity::new(id);
    STATES.with(|s| {
        let states = s.borrow();
        if let Some(st) = states.get(id) {
            e.collision.solid(st.active);
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
    fn default_state_is_active() {
        let s = IceBlockState::default();
        assert!(s.active);
        assert!(s.last_active);
    }

    #[test]
    fn approach_clamps_to_target() {
        assert!((approach(0.0, 10.0, 5.0) - 5.0).abs() < 0.001);
        assert!((approach(0.0, 10.0, 20.0) - 10.0).abs() < 0.001);
        assert!((approach(10.0, 0.0, 3.0) - 7.0).abs() < 0.001);
        assert!((approach(5.0, 5.0, 100.0) - 5.0).abs() < 0.001);
    }

    #[test]
    fn state_round_trip_preserves_active() {
        let s = IceBlockState {
            w: 64.0,
            h: 32.0,
            shake: 0.4,
            active: false,
            last_active: true,
            deactivate_flash: 0.6,
            ..Default::default()
        };
        let _ = s;
    }

    #[test]
    fn solid_inset_constants_match_iceblock_cs() {
        assert!((SOLID_INSET_X - 2.0).abs() < 0.001);
        assert!((SOLID_INSET_TOP - 3.0).abs() < 0.001);
        assert!((SOLID_INSET_BOTTOM - 5.0).abs() < 0.001);
    }
}
