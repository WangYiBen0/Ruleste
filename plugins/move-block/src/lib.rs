#![allow(clippy::not_unsafe_ptr_arg_deref)]
use std::cell::RefCell;

use ruleste_plugins_api::host::{die, draw_rect, draw_tile_box, entities_by_type, play_sound};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

ruleste_plugins_api::ruleste_meta!("moveBlock");
ruleste_plugins_api::ruleste_entity_types!("moveBlock");

const REFILL_TIME: f32 = 2.2;
const CRASH_TIME: f32 = 0.15;
const ACTIVATE_DELAY: f32 = 0.2;
const CRASH_RESET: f32 = 0.1;
const BASE_SPEED: f32 = 60.0;
const FAST_SPEED: f32 = 75.0;
const ACCEL: f32 = 300.0;
const NO_STEER_TIMER: f32 = 0.2;

const STATE_IDLING: u8 = 0;
const STATE_ACTIVATING: u8 = 1;
const STATE_MOVING: u8 = 2;
const STATE_BREAKING: u8 = 3;

const DIR_RIGHT: u8 = 0;
const DIR_LEFT: u8 = 1;
const DIR_UP: u8 = 2;
const DIR_DOWN: u8 = 3;

const COLOR_IDLE: Color = Color {
    r: 0x47,
    g: 0x40,
    b: 0x70,
    a: 0xff,
};
const COLOR_PRESSED: Color = Color {
    r: 0x30,
    g: 0xb3,
    b: 0x35,
    a: 0xff,
};
const COLOR_BREAKING: Color = Color {
    r: 0xcc,
    g: 0x25,
    b: 0x41,
    a: 0xff,
};

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct MoveBlockState {
    state: u8,
    direction: u8,
    can_steer: bool,
    fast: bool,
    w: f32,
    h: f32,
    start_x: f32,
    start_y: f32,
    angle: f32,
    target_angle: f32,
    home_angle: f32,
    angle_steer_sign: f32,
    speed: f32,
    target_speed: f32,
    activate_timer: f32,
    crash_timer: f32,
    crash_reset_timer: f32,
    no_steer_timer: f32,
    reform_timer: f32,
    triggered: bool,
    flash: f32,
    fill_color_r: u8,
    fill_color_g: u8,
    fill_color_b: u8,
}

impl Default for MoveBlockState {
    fn default() -> Self {
        Self {
            state: STATE_IDLING,
            direction: DIR_RIGHT,
            can_steer: true,
            fast: false,
            w: 8.0,
            h: 8.0,
            start_x: 0.0,
            start_y: 0.0,
            angle: 0.0,
            target_angle: 0.0,
            home_angle: 0.0,
            angle_steer_sign: 1.0,
            speed: 0.0,
            target_speed: 0.0,
            activate_timer: 0.0,
            crash_timer: CRASH_TIME,
            crash_reset_timer: CRASH_RESET,
            no_steer_timer: NO_STEER_TIMER,
            reform_timer: 0.0,
            triggered: false,
            flash: 0.0,
            fill_color_r: COLOR_IDLE.r,
            fill_color_g: COLOR_IDLE.g,
            fill_color_b: COLOR_IDLE.b,
        }
    }
}

impl MoveBlockState {
    fn new(
        direction: u8,
        can_steer: bool,
        fast: bool,
        w: f32,
        h: f32,
        start_x: f32,
        start_y: f32,
    ) -> Self {
        let (home_angle, angle_steer_sign) = match direction {
            DIR_LEFT => (std::f32::consts::PI, -1.0),
            DIR_UP => (-std::f32::consts::PI / 2.0, 1.0),
            DIR_DOWN => (std::f32::consts::PI / 2.0, -1.0),
            _ => (0.0, 1.0),
        };
        Self {
            direction,
            can_steer,
            fast,
            w,
            h,
            start_x,
            start_y,
            angle: home_angle,
            target_angle: home_angle,
            home_angle,
            angle_steer_sign,
            ..Self::default()
        }
    }
}

thread_local! {
    static STATES: RefCell<EntityState<MoveBlockState>> = RefCell::new(EntityState::new());
    static SER_BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut MoveBlockState) -> R) -> R {
    STATES.with(|s| {
        let mut states = s.borrow_mut();
        let st = states.get_or_insert(id, MoveBlockState::default) as *mut MoveBlockState;
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

fn dir_vector(dir: u8) -> (f32, f32) {
    match dir {
        DIR_LEFT => (-1.0, 0.0),
        DIR_UP => (0.0, -1.0),
        DIR_DOWN => (0.0, 1.0),
        _ => (1.0, 0.0),
    }
}

fn block_has_player_rider(
    p: (f32, f32),
    pw: f32,
    ph: f32,
    b: (f32, f32),
    bw: f32,
    bh: f32,
) -> bool {
    let top_y = b.1 - 1.0;
    p.1 + ph > top_y && p.1 < b.1 + bh && p.0 < b.0 + bw && p.0 + pw > b.0
}

fn player_climb_pressing(b: (f32, f32), bw: f32, bh: f32) -> bool {
    for pid in entities_by_type("player") {
        let pe = Entity::new(pid);
        let pp = pe.position.get();
        let (pw, ph, _, _) = pe.hitbox.get();
        if (pp.x + pw - 1.0 - b.0).abs() < 0.5 && pp.y < b.1 + bh && pp.y + ph > b.1 {
            return true;
        }
        if (pp.x - (b.0 + bw) + 1.0).abs() < 0.5 && pp.y < b.1 + bh && pp.y + ph > b.1 {
            return true;
        }
    }
    false
}

fn player_on_top(b: (f32, f32), bw: f32, _bh: f32) -> bool {
    for pid in entities_by_type("player") {
        let pe = Entity::new(pid);
        let pp = pe.position.get();
        let (pw, ph, _, _) = pe.hitbox.get();
        if pp.y + ph - 1.0 <= b.1 && pp.y + ph > b.1 - 4.0 && pp.x < b.0 + bw && pp.x + pw > b.0 {
            return true;
        }
    }
    false
}

fn update_fill_color(st: &mut MoveBlockState) {
    let target = if st.state == STATE_MOVING {
        COLOR_PRESSED
    } else if st.state == STATE_BREAKING {
        COLOR_BREAKING
    } else {
        COLOR_IDLE
    };
    let dr = approach(st.fill_color_r as f32, target.r as f32, 10.0) as u8;
    let dg = approach(st.fill_color_g as f32, target.g as f32, 10.0) as u8;
    let db = approach(st.fill_color_b as f32, target.b as f32, 10.0) as u8;
    st.fill_color_r = dr;
    st.fill_color_g = dg;
    st.fill_color_b = db;
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
    e.depth.set(-1);

    let dir_str = spawn.get_str("direction", "right");
    let direction = match dir_str.as_str() {
        "left" => DIR_LEFT,
        "up" => DIR_UP,
        "down" => DIR_DOWN,
        _ => DIR_RIGHT,
    };
    let can_steer = spawn.get_bool("canSteer", true);
    let fast = spawn.get_bool("fast", false);

    with_state(id, |st| {
        *st = MoveBlockState::new(direction, can_steer, fast, w, h, x, y);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let e = Entity::new(id);
    let p = e.position.get();
    let (w, h, _, _) = e.hitbox.get();

    with_state(id, |st| {
        st.flash = approach(st.flash, 0.0, dt * 5.0);
        update_fill_color(st);

        match st.state {
            STATE_IDLING => {
                st.speed = 0.0;
                st.target_speed = 0.0;
                st.activate_timer = 0.0;
                st.triggered = false;

                let has_rider = block_has_player_rider((p.x, p.y), w, h, (p.x, p.y), w, h);
                if st.triggered || has_rider {
                    st.state = STATE_ACTIVATING;
                    st.activate_timer = ACTIVATE_DELAY;
                    play_sound("event:/game/04_cliffside/arrowblock_activate");
                }
            }
            STATE_ACTIVATING => {
                st.activate_timer -= dt;
                if st.activate_timer <= 0.0 {
                    st.state = STATE_MOVING;
                    st.target_speed = if st.fast { FAST_SPEED } else { BASE_SPEED };
                    st.crash_timer = CRASH_TIME;
                    st.crash_reset_timer = CRASH_RESET;
                    st.no_steer_timer = NO_STEER_TIMER;
                }
            }
            STATE_MOVING => {
                st.target_angle = st.home_angle;
                if st.can_steer {
                    let pressing = if st.direction == DIR_RIGHT || st.direction == DIR_LEFT {
                        player_on_top((p.x, p.y), w, h)
                    } else {
                        player_climb_pressing((p.x, p.y), w, h)
                    };
                    if pressing && st.no_steer_timer > 0.0 {
                        st.no_steer_timer -= dt;
                    }
                    if pressing && st.no_steer_timer <= 0.0 {
                        st.target_angle = st.home_angle;
                    } else {
                        st.no_steer_timer = NO_STEER_TIMER;
                    }
                }

                st.speed = approach(st.speed, st.target_speed, ACCEL * dt);
                st.angle = approach(st.angle, st.target_angle, std::f32::consts::PI * 16.0 * dt);

                let (vx, vy) = dir_vector(st.direction);
                let (cs, sn) = (st.angle.cos(), st.angle.sin());
                let _ = (vx, vy, cs, sn);
                let dx = st.speed * dt;
                let dy = 0.0;
                let _ = dy;

                let moved = e.collision.actor_move(dx, 0.0);
                let new_p = e.position.get();

                // MoveCheck: when the block is moving, the player must not be
                // squeezed between the block and a wall. If the player is in the
                // block's path (ahead of its leading face, vertically within the
                // block) and there is a solid wall just beyond the block, the
                // player is pinned → crush.
                let step = st.speed * dt + 2.0;
                for pid in entities_by_type("player") {
                    let pe = Entity::new(pid);
                    let pp = pe.position.get();
                    let (pw, ph, _, _) = pe.hitbox.get();
                    let in_path = pp.x + pw > new_p.x + w - 1.0
                        && pp.x < new_p.x + w + step
                        && pp.y < new_p.y + h
                        && pp.y + ph > new_p.y
                        && !player_on_top((new_p.x, new_p.y), w, h);
                    if in_path {
                        // Probe just past the block's right (leading) face for a wall.
                        let probe = (new_p.x + w) - pp.x + 2.0;
                        if pe.collision.check(probe, 0.0) {
                            die();
                        }
                    }
                }

                let crashed = if st.direction == DIR_DOWN {
                    new_p.y > p.y + 100.0
                } else {
                    moved.hit_wall_left || moved.hit_wall_right || moved.hit_ceiling
                };

                if crashed {
                    if st.crash_timer > 0.0 {
                        st.crash_timer -= dt;
                    } else {
                        st.state = STATE_BREAKING;
                        st.speed = 0.0;
                        st.target_speed = 0.0;
                        st.reform_timer = REFILL_TIME;
                        st.angle = st.home_angle;
                        st.target_angle = st.home_angle;
                        play_sound("event:/game/04_cliffside/arrowblock_break");
                        e.collision.solid(false);
                        e.position.set_xy(st.start_x, st.start_y);
                    }
                } else {
                    if st.crash_reset_timer > 0.0 {
                        st.crash_reset_timer -= dt;
                    } else {
                        st.crash_timer = CRASH_TIME;
                    }
                }
            }
            STATE_BREAKING => {
                st.reform_timer -= dt;
                if st.reform_timer <= 0.0 {
                    e.collision.solid(true);
                    e.position.set_xy(st.start_x, st.start_y);
                    st.state = STATE_IDLING;
                    st.speed = 0.0;
                    st.target_speed = 0.0;
                    st.flash = 1.0;
                    st.angle = st.home_angle;
                    st.target_angle = st.home_angle;
                    play_sound("event:/game/04_cliffside/arrowblock_reappear");
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

    STATES.with(|s| {
        let states = s.borrow();
        let st = match states.get(id) {
            Some(s) => s,
            None => return,
        };

        if st.state == STATE_BREAKING && st.reform_timer < 0.1 {
            return;
        }

        // Body uses the level's solid tile texture (autotiled), not a flat colour.
        draw_tile_box('3', p.x, p.y, (st.w / 8.0) as u32, (st.h / 8.0) as u32);

        if st.flash > 0.01 {
            let f = st.flash.min(1.0);
            let extra = f * 4.0;
            let _ = extra;
            draw_rect(
                p.x - extra,
                p.y - extra,
                st.w + extra * 2.0,
                st.h + extra * 2.0,
                Color {
                    r: 0xff,
                    g: 0xff,
                    b: 0xff,
                    a: 0xff,
                },
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
                buf.push(st.direction);
                buf.push(if st.can_steer { 1 } else { 0 });
                buf.push(if st.fast { 1 } else { 0 });
                buf.extend_from_slice(&st.w.to_le_bytes());
                buf.extend_from_slice(&st.h.to_le_bytes());
                buf.extend_from_slice(&st.start_x.to_le_bytes());
                buf.extend_from_slice(&st.start_y.to_le_bytes());
                buf.extend_from_slice(&st.angle.to_le_bytes());
                buf.extend_from_slice(&st.target_angle.to_le_bytes());
                buf.extend_from_slice(&st.speed.to_le_bytes());
                buf.extend_from_slice(&st.activate_timer.to_le_bytes());
                buf.extend_from_slice(&st.crash_timer.to_le_bytes());
                buf.extend_from_slice(&st.no_steer_timer.to_le_bytes());
                buf.extend_from_slice(&st.reform_timer.to_le_bytes());
                buf.push(if st.triggered { 1 } else { 0 });
                buf.extend_from_slice(&st.flash.to_le_bytes());
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
    if bytes.len() < 3 {
        return;
    }

    with_state(id, |st| {
        let mut off = 0;
        st.state = bytes[off];
        off += 1;
        st.direction = bytes[off];
        off += 1;
        st.can_steer = bytes[off] != 0;
        off += 1;
        if bytes.len() > off {
            st.fast = bytes[off] != 0;
            off += 1;
        }
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
        st.start_x = f32_read(bytes, &mut off);
        st.start_y = f32_read(bytes, &mut off);
        st.angle = f32_read(bytes, &mut off);
        st.target_angle = f32_read(bytes, &mut off);
        st.speed = f32_read(bytes, &mut off);
        st.activate_timer = f32_read(bytes, &mut off);
        st.crash_timer = f32_read(bytes, &mut off);
        st.no_steer_timer = f32_read(bytes, &mut off);
        st.reform_timer = f32_read(bytes, &mut off);
        if bytes.len() > off {
            st.triggered = bytes[off] != 0;
            off += 1;
        }
        st.flash = f32_read(bytes, &mut off);
    });

    let e = Entity::new(id);
    STATES.with(|s| {
        let states = s.borrow();
        if let Some(st) = states.get(id) {
            if st.state == STATE_BREAKING {
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
    fn constants_match_moveblock_cs() {
        assert!((REFILL_TIME - 2.2).abs() < 0.001);
        assert!((CRASH_TIME - 0.15).abs() < 0.001);
        assert!((ACTIVATE_DELAY - 0.2).abs() < 0.001);
        assert!((BASE_SPEED - 60.0).abs() < 0.001);
        assert!((FAST_SPEED - 75.0).abs() < 0.001);
        assert!((ACCEL - 300.0).abs() < 0.001);
    }

    #[test]
    fn state_constants_match_moveblock_cs() {
        assert_eq!(STATE_IDLING, 0);
        assert_eq!(STATE_ACTIVATING, 1);
        assert_eq!(STATE_MOVING, 2);
        assert_eq!(STATE_BREAKING, 3);
    }

    #[test]
    fn dir_constants_match_moveblock_cs() {
        assert_eq!(DIR_RIGHT, 0);
        assert_eq!(DIR_LEFT, 1);
        assert_eq!(DIR_UP, 2);
        assert_eq!(DIR_DOWN, 3);
    }

    #[test]
    fn home_angle_per_direction() {
        let s = MoveBlockState::new(DIR_RIGHT, true, false, 8.0, 8.0, 0.0, 0.0);
        assert_eq!(s.home_angle, 0.0);
        assert_eq!(s.angle_steer_sign, 1.0);

        let s = MoveBlockState::new(DIR_LEFT, true, false, 8.0, 8.0, 0.0, 0.0);
        assert!((s.home_angle - std::f32::consts::PI).abs() < 0.001);
        assert_eq!(s.angle_steer_sign, -1.0);

        let s = MoveBlockState::new(DIR_UP, true, false, 8.0, 8.0, 0.0, 0.0);
        assert!((s.home_angle + std::f32::consts::PI / 2.0).abs() < 0.001);

        let s = MoveBlockState::new(DIR_DOWN, true, false, 8.0, 8.0, 0.0, 0.0);
        assert!((s.home_angle - std::f32::consts::PI / 2.0).abs() < 0.001);
    }

    #[test]
    fn dir_vector_correct() {
        assert_eq!(dir_vector(DIR_RIGHT), (1.0, 0.0));
        assert_eq!(dir_vector(DIR_LEFT), (-1.0, 0.0));
        assert_eq!(dir_vector(DIR_UP), (0.0, -1.0));
        assert_eq!(dir_vector(DIR_DOWN), (0.0, 1.0));
    }

    #[test]
    fn approach_clamps_to_target() {
        assert!((approach(0.0, 10.0, 5.0) - 5.0).abs() < 0.001);
        assert!((approach(0.0, 10.0, 20.0) - 10.0).abs() < 0.001);
        assert!((approach(10.0, 0.0, 3.0) - 7.0).abs() < 0.001);
        assert!((approach(5.0, 5.0, 100.0) - 5.0).abs() < 0.001);
    }

    #[test]
    fn default_state_is_idling() {
        let s = MoveBlockState::default();
        assert_eq!(s.state, STATE_IDLING);
        assert!(s.can_steer);
        assert!(!s.fast);
    }

    #[test]
    fn new_state_stores_dims() {
        let s = MoveBlockState::new(DIR_RIGHT, true, true, 16.0, 16.0, 50.0, 50.0);
        assert_eq!(s.w, 16.0);
        assert_eq!(s.h, 16.0);
        assert_eq!(s.start_x, 50.0);
        assert_eq!(s.start_y, 50.0);
        assert!(s.fast);
    }
}
