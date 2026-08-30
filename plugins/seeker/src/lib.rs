#![allow(clippy::not_unsafe_ptr_arg_deref)]
use std::cell::RefCell;

use ruleste_plugins_api::host::{
    EV_SPRING_BOUNCE, die, emit, entities_by_type, line_of_sight, play_sound,
};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::EntityId;

ruleste_plugins_api::ruleste_meta!("seeker");
ruleste_plugins_api::ruleste_entity_types!("seeker");

#[allow(dead_code)]
const SIZE: f32 = 12.0;
#[allow(dead_code)]
const HITBOX_W: f32 = 14.0;
#[allow(dead_code)]
const HITBOX_H: f32 = 14.0;

const ACCEL: f32 = 600.0;
#[allow(dead_code)]
const STUN_X_SPEED: f32 = 100.0;
#[allow(dead_code)]
const BOUNCE_SPEED: f32 = 200.0;
const ATTACK_SPEED: f32 = 200.0;
const PATROL_SPEED: f32 = 30.0;
#[allow(dead_code)]
const CHASE_SPEED: f32 = 55.0;
const SIGHT_DIST_SQ: f32 = 25600.0;
const ATTACK_RANGE_SQ: f32 = 100.0;
const STUN_DURATION: f32 = 0.4;
const REGEN_DURATION: f32 = 1.6;

const STATE_IDLE: u8 = 0;
const STATE_PATROL: u8 = 1;
const STATE_SPOTTED: u8 = 2;
const STATE_ATTACK: u8 = 3;
const STATE_STUNNED: u8 = 4;
#[allow(dead_code)]
const STATE_SKIDDING: u8 = 5;
const STATE_REGENERATE: u8 = 6;
const STATE_RETURNED: u8 = 7;

#[derive(Clone, Copy)]
struct SeekerState {
    state: u8,
    last_state: u8,
    start_x: f32,
    start_y: f32,
    speed_x: f32,
    speed_y: f32,
    stun_timer: f32,
    regen_timer: f32,
    spotted_timer: f32,
    visible: bool,
}

impl Default for SeekerState {
    fn default() -> Self {
        Self {
            state: STATE_IDLE,
            last_state: STATE_IDLE,
            start_x: 0.0,
            start_y: 0.0,
            speed_x: 0.0,
            speed_y: 0.0,
            stun_timer: 0.0,
            regen_timer: 0.0,
            spotted_timer: 0.0,
            visible: true,
        }
    }
}

thread_local! {
    static STATES: RefCell<EntityState<SeekerState>> = RefCell::new(EntityState::new());
    static SER_BUF: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut SeekerState) -> R) -> R {
    STATES.with(|s| {
        let mut states = s.borrow_mut();
        let st = states.get_or_insert(id, SeekerState::default) as *mut SeekerState;
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

fn find_player_pos() -> Option<(f32, f32)> {
    let pid = entities_by_type("player").into_iter().next()?;
    let pe = Entity::new(pid);
    let pp = pe.position.get();
    Some((pp.x, pp.y))
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);

    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    e.position.set_xy(x, y);
    e.hitbox.set(HITBOX_W, HITBOX_H, -3.0, -3.0);
    e.collision.solid(true);
    e.sprite.set_bank("seeker");
    e.sprite.play("idle");

    with_state(id, |st| {
        st.start_x = x;
        st.start_y = y;
        st.state = STATE_IDLE;
        st.visible = true;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let e = Entity::new(id);
    let p = e.position.get();

    with_state(id, |st| {
        // `GotBouncedOn`: a player landing on top of the seeker (from above)
        // stuns it and bounces the player, instead of the seeker killing the
        // player. Detect a stomp as a horizontal overlap with the player's feet
        // in the upper portion of the seeker's body.
        if st.state != STATE_STUNNED && st.state != STATE_REGENERATE {
            let mut stomped = false;
            for pid in entities_by_type("player") {
                let pe = Entity::new(pid);
                let pp = pe.position.get();
                let (pw, ph, _, _) = pe.hitbox.get();
                let overlaps_x = pp.x < p.x + HITBOX_W && pp.x + pw > p.x;
                let overlaps_y = pp.y < p.y + HITBOX_H && pp.y + ph > p.y;
                let from_above = pp.y + ph <= p.y + HITBOX_H * 0.4;
                if overlaps_x && overlaps_y && from_above {
                    stomped = true;
                    st.state = STATE_STUNNED;
                    st.stun_timer = STUN_DURATION;
                    st.speed_x = 0.0;
                    st.speed_y = -BOUNCE_SPEED;
                    play_sound("event:/game/05_mirror/seeker_regenerate");
                    // best-effort: bounce the player upward (Floor orientation);
                    // consumed by the player plugin once it handles spring bounces.
                    let mut payload = [0u8; 13];
                    payload[0..4].copy_from_slice(&pid.to_le_bytes());
                    payload[4] = 0;
                    payload[5..9].copy_from_slice(&p.x.to_le_bytes());
                    payload[9..13].copy_from_slice(&p.y.to_le_bytes());
                    emit(pid, EV_SPRING_BOUNCE, &payload);
                    break;
                }
            }
            if stomped {
                return;
            }
        }

        match st.state {
            STATE_IDLE => {
                st.speed_x = approach(st.speed_x, 0.0, ACCEL * dt);
                st.speed_y = approach(st.speed_y, 0.0, ACCEL * dt);

                if let Some((px, py)) = find_player_pos() {
                    let dx = px - p.x;
                    let dy = py - p.y;
                    // `CanSeePlayer`: only spot when within range AND the
                    // straight line to the player is unobstructed by solids.
                    if dx * dx + dy * dy < SIGHT_DIST_SQ && line_of_sight(p.x, p.y, px, py) {
                        st.state = STATE_SPOTTED;
                        st.spotted_timer = 0.4;
                        play_sound("event:/game/05_mirror/seeker_locate");
                    }
                }

                if st.state == STATE_IDLE {
                    st.state = STATE_PATROL;
                }
            }
            STATE_PATROL => {
                let dx = st.start_x - p.x;
                let dy = st.start_y - p.y;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > 1.0 {
                    st.speed_x = approach(st.speed_x, dx / dist * PATROL_SPEED, ACCEL * dt);
                    st.speed_y = approach(st.speed_y, dy / dist * PATROL_SPEED, ACCEL * dt);
                } else {
                    st.state = STATE_IDLE;
                }

                if let Some((px, py)) = find_player_pos() {
                    let dx = px - p.x;
                    let dy = py - p.y;
                    if dx * dx + dy * dy < SIGHT_DIST_SQ && line_of_sight(p.x, p.y, px, py) {
                        st.state = STATE_SPOTTED;
                        st.spotted_timer = 0.4;
                    }
                }
            }
            STATE_SPOTTED => {
                st.spotted_timer -= dt;
                if st.spotted_timer <= 0.0
                    && let Some((px, py)) = find_player_pos()
                {
                    let dx = px - p.x;
                    let dy = py - p.y;
                    if dx * dx + dy * dy < ATTACK_RANGE_SQ {
                        st.state = STATE_ATTACK;
                        play_sound("event:/game/05_mirror/seeker_attack");
                    } else {
                        st.speed_x = approach(st.speed_x, 0.0, ACCEL * dt);
                        st.speed_y = approach(st.speed_y, 0.0, ACCEL * dt);
                    }
                }
            }
            STATE_ATTACK => {
                if let Some((px, py)) = find_player_pos() {
                    let dx = px - p.x;
                    let dy = py - p.y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist > 1.0 {
                        st.speed_x = approach(st.speed_x, dx / dist * ATTACK_SPEED, ACCEL * dt);
                        st.speed_y = approach(st.speed_y, dy / dist * ATTACK_SPEED, ACCEL * dt);
                    }

                    if dist < ATTACK_RANGE_SQ {
                        die();
                        play_sound("event:/game/05_mirror/seeker_killed");
                        st.state = STATE_STUNNED;
                        st.stun_timer = STUN_DURATION;
                    }
                } else {
                    st.state = STATE_PATROL;
                }

                let new_p = e.position.get();
                if new_p.x == p.x
                    && new_p.y == p.y
                    && (st.speed_x.abs() > 5.0 || st.speed_y.abs() > 5.0)
                {
                    st.state = STATE_STUNNED;
                    st.stun_timer = STUN_DURATION;
                    st.speed_x = -st.speed_x * 0.5;
                    st.speed_y = -st.speed_y * 0.5;
                }
            }
            STATE_STUNNED => {
                st.stun_timer -= dt;
                st.speed_x = approach(st.speed_x, 0.0, 200.0 * dt);
                st.speed_y = approach(st.speed_y, 0.0, 200.0 * dt);
                if st.stun_timer <= 0.0 {
                    st.state = STATE_REGENERATE;
                    st.regen_timer = REGEN_DURATION;
                    st.visible = false;
                    play_sound("event:/game/05_mirror/seeker_regenerate");
                }
            }
            STATE_REGENERATE => {
                st.regen_timer -= dt;
                if st.regen_timer <= 0.0 {
                    e.position.set_xy(st.start_x, st.start_y);
                    st.speed_x = 0.0;
                    st.speed_y = 0.0;
                    st.visible = true;
                    st.state = STATE_RETURNED;
                }
            }
            STATE_RETURNED => {
                let dx = st.start_x - p.x;
                let dy = st.start_y - p.y;
                if dx * dx + dy * dy < 1.0 {
                    st.state = STATE_PATROL;
                }
            }
            _ => {}
        }

        let vx = st.speed_x;
        let vy = st.speed_y;
        if vx != 0.0 || vy != 0.0 {
            let _ = e.collision.actor_move(vx * dt, vy * dt);
        }

        // Drive the SpriteBank animation from the seeker's current state and
        // flip the sprite to face horizontal travel, mirroring `Seeker.cs`.
        let anim = match st.state {
            STATE_STUNNED => "stunned",
            STATE_REGENERATE => "recover",
            STATE_RETURNED => "statue",
            STATE_SPOTTED | STATE_ATTACK => "spotted",
            _ => "idle",
        };
        if e.sprite.animation() != anim {
            e.sprite.play(anim);
        }
        if vx > 1.0 {
            e.sprite.flip_x(false);
        } else if vx < -1.0 {
            e.sprite.flip_x(true);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(_id: EntityId) {
    // Visual (body + state-dependent eye) is drawn by the host via the
    // SpriteBank ("seeker" sprite); the animation is driven in update().
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
                buf.push(st.last_state);
                buf.extend_from_slice(&st.start_x.to_le_bytes());
                buf.extend_from_slice(&st.start_y.to_le_bytes());
                buf.extend_from_slice(&st.speed_x.to_le_bytes());
                buf.extend_from_slice(&st.speed_y.to_le_bytes());
                buf.extend_from_slice(&st.stun_timer.to_le_bytes());
                buf.extend_from_slice(&st.regen_timer.to_le_bytes());
                buf.extend_from_slice(&st.spotted_timer.to_le_bytes());
                buf.push(if st.visible { 1 } else { 0 });
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
        st.state = bytes[off];
        off += 1;
        st.last_state = bytes[off];
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
        st.start_x = f32_read(bytes, &mut off);
        st.start_y = f32_read(bytes, &mut off);
        st.speed_x = f32_read(bytes, &mut off);
        st.speed_y = f32_read(bytes, &mut off);
        st.stun_timer = f32_read(bytes, &mut off);
        st.regen_timer = f32_read(bytes, &mut off);
        st.spotted_timer = f32_read(bytes, &mut off);
        if bytes.len() > off {
            st.visible = bytes[off] != 0;
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
    fn constants_match_seeker_cs() {
        assert!((SIZE - 12.0).abs() < 0.001);
        assert!((ACCEL - 600.0).abs() < 0.001);
        assert!((STUN_X_SPEED - 100.0).abs() < 0.001);
        assert!((BOUNCE_SPEED - 200.0).abs() < 0.001);
        assert!((SIGHT_DIST_SQ - 25600.0).abs() < 0.001);
    }

    #[test]
    fn state_constants_match_seeker_cs() {
        assert_eq!(STATE_IDLE, 0);
        assert_eq!(STATE_PATROL, 1);
        assert_eq!(STATE_SPOTTED, 2);
        assert_eq!(STATE_ATTACK, 3);
        assert_eq!(STATE_STUNNED, 4);
        assert_eq!(STATE_SKIDDING, 5);
        assert_eq!(STATE_REGENERATE, 6);
        assert_eq!(STATE_RETURNED, 7);
    }

    #[test]
    fn approach_clamps_to_target() {
        assert!((approach(0.0, 10.0, 5.0) - 5.0).abs() < 0.001);
        assert!((approach(0.0, 10.0, 20.0) - 10.0).abs() < 0.001);
        assert!((approach(10.0, 0.0, 3.0) - 7.0).abs() < 0.001);
    }

    #[test]
    fn default_state_is_idle() {
        let s = SeekerState::default();
        assert_eq!(s.state, STATE_IDLE);
        assert!(s.visible);
        assert_eq!(s.speed_x, 0.0);
        assert_eq!(s.speed_y, 0.0);
    }
}
