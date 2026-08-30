#![allow(clippy::not_unsafe_ptr_arg_deref)]
use std::cell::RefCell;

use ruleste_plugins_api::host::{
    draw_image, draw_rect, entities_by_type, is_cold_mode, play_sound,
};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("bounceBlock");
ruleste_plugins_api::ruleste_entity_types!("bounceBlock");

const WIND_UP_DIST: f32 = 10.0;
const ICE_WIND_UP_DIST: f32 = 16.0;
const BOUNCE_DIST: f32 = 24.0;
const LIFT_SPEED_X_MULT: f32 = 0.75;
const RESPAWN_TIME: f32 = 1.6;
const BOUNCE_END_TIME: f32 = 0.05;

const STATE_WAITING: u8 = 0;
const STATE_WINDING_UP: u8 = 1;
const STATE_BOUNCING: u8 = 2;
const STATE_BOUNCE_END: u8 = 3;
const STATE_BROKEN: u8 = 4;

const COLOR_FLASH: Color = Color {
    r: 0xff,
    g: 0xff,
    b: 0xff,
    a: 0xff,
};

#[derive(Clone, Copy)]
struct BounceState {
    state: u8,
    start_x: f32,
    start_y: f32,
    bounce_dir_x: f32,
    bounce_dir_y: f32,
    wind_up_progress: f32,
    move_speed: f32,
    respawn_timer: f32,
    bounce_end_timer: f32,
    reformed: bool,
    reappear_flash: f32,
    ice_mode: bool,
    last_cold: bool,
    anim_time: f32,
}

impl BounceState {
    fn new(start_x: f32, start_y: f32) -> Self {
        Self {
            state: STATE_WAITING,
            start_x,
            start_y,
            bounce_dir_x: 0.0,
            bounce_dir_y: -1.0,
            wind_up_progress: 0.0,
            move_speed: 0.0,
            respawn_timer: 0.0,
            bounce_end_timer: 0.0,
            reformed: true,
            reappear_flash: 0.0,
            ice_mode: false,
            last_cold: false,
            anim_time: 0.0,
        }
    }
}

impl Default for BounceState {
    fn default() -> Self {
        Self::new(0.0, 0.0)
    }
}

thread_local! {
    static STATES: RefCell<EntityState<BounceState>> = RefCell::new(EntityState::new());
    static SER_BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut BounceState) -> R) -> R {
    STATES.with(|s| {
        let mut states = s.borrow_mut();
        let st = states.get_or_insert(id, BounceState::default) as *mut BounceState;
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

fn safe_normalize(x: f32, y: f32) -> (f32, f32) {
    let len_sq = x * x + y * y;
    if len_sq < 0.0001 {
        (0.0, -1.0)
    } else {
        let len = len_sq.sqrt();
        (x / len, y / len)
    }
}

fn player_overlapping_block(
    p: (f32, f32),
    pw: f32,
    ph: f32,
    b: (f32, f32),
    bw: f32,
    bh: f32,
) -> bool {
    p.0 < b.0 + bw && p.0 + pw > b.0 && p.1 < b.1 + bh && p.1 + ph > b.1
}

fn wind_up_player_check(b: (f32, f32), bw: f32, bh: f32) -> bool {
    for pid in entities_by_type("player") {
        let pe = Entity::new(pid);
        let pp = pe.position.get();
        let (pw, ph, _, _) = pe.hitbox.get();
        if player_overlapping_block((pp.x, pp.y), pw, ph, b, bw, bh) {
            return true;
        }
    }
    false
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

    e.collision.solid(true);
    e.depth.set(-9000);

    with_state(id, |st| {
        *st = BounceState::new(x, y);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let e = Entity::new(id);
    let p = e.position.get();
    let (w, h, _, _) = e.hitbox.get();

    with_state(id, |st| {
        st.reappear_flash = approach(st.reappear_flash, 0.0, dt * 8.0);
        st.anim_time += dt;

        let now_cold = is_cold_mode();
        if now_cold != st.last_cold {
            st.last_cold = now_cold;
            st.ice_mode = now_cold;
        }

        match st.state {
            STATE_WAITING => {
                st.move_speed = approach(st.move_speed, 100.0, 400.0 * dt);
                st.wind_up_progress = approach(st.wind_up_progress, 0.0, 1.0 * dt);

                if wind_up_player_check((p.x, p.y), w, h) {
                    st.move_speed = 80.0;
                    st.bounce_dir_y = -1.0;
                    st.bounce_dir_x = 0.0;
                    st.state = STATE_WINDING_UP;
                    if st.ice_mode {
                        play_sound("event:/game/09_core/iceblock_touch");
                    } else {
                        play_sound("event:/game/09_core/bounceblock_touch");
                    }
                }
            }
            STATE_WINDING_UP => {
                let wind_up_dist = if st.ice_mode {
                    ICE_WIND_UP_DIST
                } else {
                    WIND_UP_DIST
                };
                let speed_mult = if st.ice_mode { 0.333 } else { 1.0 };
                let target_speed = if st.ice_mode { 35.0 } else { 40.0 };
                st.move_speed = approach(st.move_speed, target_speed, 600.0 * dt);

                let target_x = st.start_x - st.bounce_dir_x * wind_up_dist;
                let target_y = st.start_y - st.bounce_dir_y * wind_up_dist;

                let dx = target_x - p.x;
                let dy = target_y - p.y;
                if dx.abs() > 0.05 || dy.abs() > 0.05 {
                    let (nx, ny) = safe_normalize(dx, dy);
                    e.position.set_xy(
                        p.x + nx * st.move_speed * speed_mult * dt,
                        p.y + ny * st.move_speed * speed_mult * dt,
                    );
                    let p2 = e.position.get();
                    e.position.set_xy(
                        p2.x + nx * st.move_speed * speed_mult * dt * LIFT_SPEED_X_MULT * 0.5,
                        p2.y,
                    );
                }

                let p2 = e.position.get();
                let dist_sq = (p2.x - target_x).powi(2) + (p2.y - target_y).powi(2);
                st.wind_up_progress = ((dist_sq).sqrt() / wind_up_dist).min(1.0);

                if dist_sq <= 4.0 {
                    if st.ice_mode {
                        st.state = STATE_BOUNCE_END;
                        st.bounce_end_timer = BOUNCE_END_TIME;
                    } else {
                        st.state = STATE_BOUNCING;
                        st.wind_up_progress = 1.0;
                    }
                    st.move_speed = 0.0;
                }
            }
            STATE_BOUNCING => {
                st.move_speed = approach(st.move_speed, 140.0, 800.0 * dt);

                let target_x = st.start_x + st.bounce_dir_x * BOUNCE_DIST;
                let target_y = st.start_y + st.bounce_dir_y * BOUNCE_DIST;

                let dx = target_x - p.x;
                let dy = target_y - p.y;
                if dx.abs() > 0.05 || dy.abs() > 0.05 {
                    let (nx, ny) = safe_normalize(dx, dy);
                    e.position
                        .set_xy(p.x + nx * st.move_speed * dt, p.y + ny * st.move_speed * dt);
                }

                let p2 = e.position.get();
                let arrived = (p2.x - target_x).abs() < 0.5 && (p2.y - target_y).abs() < 0.5;

                if arrived {
                    st.state = STATE_BOUNCE_END;
                    st.move_speed = 0.0;
                    st.bounce_end_timer = BOUNCE_END_TIME;
                }
            }
            STATE_BOUNCE_END => {
                st.bounce_end_timer -= dt;
                if st.bounce_end_timer <= 0.0 {
                    st.state = STATE_BROKEN;
                    st.respawn_timer = RESPAWN_TIME;
                    e.collision.solid(false);
                    e.depth.set(8990);
                    st.reformed = false;
                    play_sound("event:/game/09_core/bounceblock_break");
                }
            }
            STATE_BROKEN => {
                if st.respawn_timer > 0.0 {
                    st.respawn_timer -= dt;
                    return;
                }

                let saved_x = p.x;
                let saved_y = p.y;
                e.position.set_xy(st.start_x, st.start_y);
                let can_reform = !e.collision.check(0.0, 0.0);

                if can_reform {
                    e.collision.solid(true);
                    e.depth.set(-9000);
                    e.position.set_xy(st.start_x, st.start_y);
                    st.state = STATE_WAITING;
                    st.move_speed = 0.0;
                    st.wind_up_progress = 0.0;
                    st.reformed = true;
                    st.reappear_flash = 0.6;
                    play_sound("event:/game/09_core/bounceblock_reappear");
                } else {
                    e.position.set_xy(saved_x, saved_y);
                }
            }
            _ => {}
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let e = Entity::new(id);
    let p = e.position.get();
    let (w, h, _, _) = e.hitbox.get();

    STATES.with(|s| {
        let states = s.borrow();
        let st = match states.get(id) {
            Some(s) => s,
            None => return,
        };

        if st.state == STATE_BROKEN && !st.reformed {
            return;
        }

        let (ox, oy) = (st.bounce_dir_x, st.bounce_dir_y);
        let (dx, dy) = match st.state {
            STATE_WINDING_UP => {
                let offset = st.wind_up_progress
                    * if st.ice_mode {
                        ICE_WIND_UP_DIST
                    } else {
                        WIND_UP_DIST
                    };
                (-ox * offset, -oy * offset)
            }
            STATE_BOUNCING => {
                let cur_dist = ((p.x - st.start_x).powi(2) + (p.y - st.start_y).powi(2)).sqrt();
                (ox * cur_dist, oy * cur_dist)
            }
            _ => (0.0, 0.0),
        };

        let bx = p.x + dx;
        let by = p.y + dy;
        let scale = w / 64.0; // fire_bg / Ice00 are 64x64

        // Body (solid block): fire or ice variant.
        let body = if st.ice_mode {
            "objects/BumpBlock/Ice00"
        } else {
            "objects/BumpBlock/fire_bg"
        };
        draw_image(body, bx, by, 0.0, scale, scale);

        // Flames (fire mode only).
        if !st.ice_mode {
            let fi = (st.anim_time * 12.0) as usize % 8;
            let fid = format!("objects/BumpBlock/Fire{:02}", fi);
            draw_image(
                &fid,
                bx + (w - 58.0 * scale) / 2.0,
                by + (h - 60.0 * scale) / 2.0,
                0.0,
                scale,
                scale,
            );
        }

        // Animated bumper core (center00..25 loop).
        let ci = (st.anim_time * 20.0) as usize % 26;
        let cid = format!("objects/BumpBlock/center{:02}", ci);
        draw_image(
            &cid,
            bx + (w - 9.0 * scale) / 2.0,
            by + (h - 9.0 * scale) / 2.0,
            0.0,
            scale,
            scale,
        );

        if st.reappear_flash > 0.01 {
            let pad = 2.0;
            draw_rect(
                p.x - pad,
                p.y - pad,
                w + pad * 2.0,
                h + pad * 2.0,
                COLOR_FLASH,
            );
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
                buf.push(st.state);
                buf.extend_from_slice(&st.bounce_dir_x.to_le_bytes());
                buf.extend_from_slice(&st.bounce_dir_y.to_le_bytes());
                buf.extend_from_slice(&st.wind_up_progress.to_le_bytes());
                buf.extend_from_slice(&st.move_speed.to_le_bytes());
                buf.extend_from_slice(&st.respawn_timer.to_le_bytes());
                buf.extend_from_slice(&st.bounce_end_timer.to_le_bytes());
                buf.push(if st.reformed { 1 } else { 0 });
                buf.push(if st.ice_mode { 1 } else { 0 });
                buf.push(if st.last_cold { 1 } else { 0 });
                buf.extend_from_slice(&st.reappear_flash.to_le_bytes());
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
    if bytes.is_empty() {
        return;
    }

    with_state(id, |st| {
        let mut off = 0;
        st.state = bytes[off];
        off += 1;
        if bytes.len() >= off + 4 {
            st.bounce_dir_x = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
            off += 4;
        }
        if bytes.len() >= off + 4 {
            st.bounce_dir_y = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
            off += 4;
        }
        if bytes.len() >= off + 4 {
            st.wind_up_progress = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
            off += 4;
        }
        if bytes.len() >= off + 4 {
            st.move_speed = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
            off += 4;
        }
        if bytes.len() >= off + 4 {
            st.respawn_timer = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
            off += 4;
        }
        if bytes.len() >= off + 4 {
            st.bounce_end_timer = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
            off += 4;
        }
        if bytes.len() > off {
            st.reformed = bytes[off] != 0;
            off += 1;
        }
        if bytes.len() > off {
            st.ice_mode = bytes[off] != 0;
            off += 1;
        }
        if bytes.len() > off {
            st.last_cold = bytes[off] != 0;
            off += 1;
        }
        if bytes.len() >= off + 4 {
            st.reappear_flash = f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
        }
    });

    let e = Entity::new(id);
    STATES.with(|s| {
        let states = s.borrow();
        if let Some(st) = states.get(id) {
            if st.state == STATE_BROKEN {
                e.collision.solid(false);
                e.depth.set(8990);
            } else if st.reformed {
                e.collision.solid(true);
                e.depth.set(-9000);
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
    fn constants_match_bounceblock_cs() {
        assert!((WIND_UP_DIST - 10.0).abs() < 0.001);
        assert!((ICE_WIND_UP_DIST - 16.0).abs() < 0.001);
        assert!((BOUNCE_DIST - 24.0).abs() < 0.001);
        assert!((LIFT_SPEED_X_MULT - 0.75).abs() < 0.001);
        assert!((RESPAWN_TIME - 1.6).abs() < 0.001);
        assert!((BOUNCE_END_TIME - 0.05).abs() < 0.001);
    }

    #[test]
    fn state_constants_match_bounceblock_cs() {
        assert_eq!(STATE_WAITING, 0);
        assert_eq!(STATE_WINDING_UP, 1);
        assert_eq!(STATE_BOUNCING, 2);
        assert_eq!(STATE_BOUNCE_END, 3);
        assert_eq!(STATE_BROKEN, 4);
    }

    #[test]
    fn approach_clamps_to_target() {
        assert!((approach(0.0, 10.0, 5.0) - 5.0).abs() < 0.001);
        assert!((approach(0.0, 10.0, 20.0) - 10.0).abs() < 0.001);
        assert!((approach(10.0, 0.0, 3.0) - 7.0).abs() < 0.001);
        assert!((approach(5.0, 5.0, 100.0) - 5.0).abs() < 0.001);
    }

    #[test]
    fn safe_normalize_handles_zero() {
        let (x, y) = safe_normalize(0.0, 0.0);
        assert_eq!(x, 0.0);
        assert_eq!(y, -1.0);
        let (x, y) = safe_normalize(3.0, 4.0);
        assert!((x - 0.6).abs() < 0.001);
        assert!((y - 0.8).abs() < 0.001);
    }

    #[test]
    fn default_state_is_waiting_at_origin() {
        let st = BounceState::default();
        assert_eq!(st.state, STATE_WAITING);
        assert!(st.reformed);
        assert_eq!(st.reappear_flash, 0.0);
    }

    #[test]
    fn new_stores_start_pos() {
        let st = BounceState::new(50.0, 100.0);
        assert_eq!(st.start_x, 50.0);
        assert_eq!(st.start_y, 100.0);
        assert_eq!(st.state, STATE_WAITING);
    }

    #[test]
    fn player_overlapping_block_basic() {
        assert!(player_overlapping_block(
            (10.0, 10.0),
            8.0,
            8.0,
            (5.0, 5.0),
            8.0,
            8.0
        ));
        assert!(!player_overlapping_block(
            (100.0, 100.0),
            8.0,
            8.0,
            (5.0, 5.0),
            8.0,
            8.0
        ));
    }
}
