#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-theo-crystal` — `theoCrystal` (universal).
//!
//! Mirrors `TheoCrystal` in `references/source/Celeste/Celeste/TheoCrystal.cs`.
//! The crystal is a `Holdable`: when the player grabs it (presses Climb while
//! overlapping) it attaches via the `holdable` carry contract — each frame the
//! crystal emits `EV_CARRIED 1 (carryX, carryY)` and the player plugin freezes
//! the player at that point. Releasing (Jump/Dash or letting go of Climb) throws
//! the crystal and emits `EV_CARRIED 0` to free the player.
//!
//! When not held the crystal follows its `nodes` path (the `added`/`_onShake`
//! patrol behavior is approximated by linear node interpolation).

use ruleste_plugins_api::host::{self, Hitbox, Input, Position, draw_rect, emit, entities_by_type};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::input;
use ruleste_plugins_api::types::{Color, EntityId, Vec2};

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("theoCrystal");
ruleste_plugins_api::ruleste_entity_types!("theoCrystal");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

const CRYSTAL: Color = Color {
    r: 0x66,
    g: 0x99,
    b: 0xff,
    a: 0xff,
};

/// Idle / patrolling between `nodes`.
const STATE_IDLE: u8 = 0;
/// Attached to the player via the carry contract.
const STATE_HELD: u8 = 1;
/// Thrown after release; flies with friction before returning to idle.
const STATE_THROWN: u8 = 2;

/// `Player.CarryOffsetTarget = new Vector2(0, -12)`.
const CARRY_OFFSET_X: f32 = 0.0;
const CARRY_OFFSET_Y: f32 = -12.0;

/// Throw launch speed (`TheoCrystal.OnPickup`/`Throw` ~ 200).
const THROW_SPEED: f32 = 200.0;
/// How long the thrown crystal keeps its launch velocity before settling.
const THROW_TIME: f32 = 0.3;

#[derive(Clone)]
struct State {
    w: f32,
    h: f32,
    nodes: Vec<(f32, f32)>,
    idx: usize,
    state: u8,
    thrown_dx: f32,
    thrown_dy: f32,
    thrown_timer: f32,
    /// Last horizontal facing the player threw with (±1).
    throw_facing: f32,
}

impl Default for State {
    fn default() -> Self {
        State {
            w: 12.0,
            h: 12.0,
            nodes: Vec::new(),
            idx: 0,
            state: STATE_IDLE,
            thrown_dx: 0.0,
            thrown_dy: 0.0,
            thrown_timer: 0.0,
            throw_facing: 1.0,
        }
    }
}

thread_local! {
    static STATES: RefCell<std::collections::HashMap<EntityId, State>> =
        RefCell::new(std::collections::HashMap::new());
}

fn player_rect() -> Option<(f32, f32, f32, f32)> {
    let p = *entities_by_type("player").first()?;
    let pp = Position::new(p).get();
    let (pw, ph, pox, poy) = Hitbox::new(p).get();
    Some((pp.x + pox, pp.y + poy, pw, ph))
}

#[allow(clippy::too_many_arguments)]
fn overlap(ax: f32, ay: f32, aw: f32, ah: f32, bx: f32, by: f32, bw: f32, bh: f32) -> bool {
    ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by
}

fn emit_carried(id: EntityId, carry: Vec2, on: bool) {
    let mut buf = [0u8; 9];
    buf[0..4].copy_from_slice(&carry.x.to_le_bytes());
    buf[4..8].copy_from_slice(&carry.y.to_le_bytes());
    buf[8] = u8::from(on);
    emit(id, host::EV_CARRIED, &buf);
}

fn approach(value: f32, target: f32, max_move: f32) -> f32 {
    let diff = target - value;
    if diff.abs() <= max_move {
        target
    } else {
        value + diff.signum() * max_move
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    e.position.set_xy(x, y);
    let w = spawn.get_float("width", 12.0);
    let h = spawn.get_float("height", 12.0);
    e.hitbox.set(w, h, 0.0, 0.0);
    e.collision.platform(true);
    let st = State {
        w,
        h,
        nodes: spawn.nodes().iter().map(|n| (n.x, n.y)).collect(),
        ..State::default()
    };
    STATES.with(|s| {
        s.borrow_mut().insert(id, st);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let e = Entity::new(id);
    let mut st = match STATES.with(|s| s.borrow_mut().get_mut(&id).cloned()) {
        Some(st) => st,
        None => return,
    };

    match st.state {
        STATE_IDLE => {
            // Patrol: follow `nodes` if any are present.
            let p = e.position.get();
            if !st.nodes.is_empty() {
                let dest = st.nodes[st.idx];
                let dx = dest.0 - p.x;
                let dy = dest.1 - p.y;
                let dist = (dx * dx + dy * dy).sqrt();
                let speed = 35.0;
                if dist <= speed * dt {
                    e.position.set_xy(dest.0, dest.1);
                    st.idx = (st.idx + 1) % st.nodes.len();
                } else {
                    e.position
                        .set_xy(p.x + dx / dist * speed * dt, p.y + dy / dist * speed * dt);
                }
            }

            // Grab check: overlap + Climb pressed, and no other holder already
            // has the player this frame (best-effort via the event bus).
            if let Some((px, py, pw, ph)) = player_rect() {
                let p = e.position.get();
                if overlap(px, py, pw, ph, p.x, p.y, st.w, st.h)
                    && Input::pressed(input::CLIMB)
                    && !another_holder_has_player(id)
                {
                    st.state = STATE_HELD;
                }
            }
        }
        STATE_HELD => {
            let Some((px, py, _pw, _ph)) = player_rect() else {
                // Player gone — drop back to idle.
                st.state = STATE_IDLE;
                STATES.with(|s| {
                    if let Some(st_ref) = s.borrow_mut().get_mut(&id) {
                        *st_ref = st;
                    }
                });
                return;
            };

            // `Player.CarryOffsetTarget` placement above the player's hitbox.
            let carry = Vec2::new(px + CARRY_OFFSET_X, py + CARRY_OFFSET_Y);
            e.position.set_xy(carry.x, carry.y);
            emit_carried(id, carry, true);

            // Throw: Jump, Dash, or releasing Climb frees the crystal.
            let throw_h = Input::axis(input::MOVE_RIGHT) - Input::axis(input::MOVE_LEFT);
            let facing = if throw_h > 0.1 {
                1.0
            } else if throw_h < -0.1 {
                -1.0
            } else {
                st.throw_facing
            };
            st.throw_facing = facing;
            if Input::pressed(input::JUMP)
                || Input::pressed(input::DASH)
                || !Input::button(input::CLIMB)
            {
                st.thrown_dx = facing * THROW_SPEED;
                st.thrown_dy = -THROW_SPEED;
                st.thrown_timer = THROW_TIME;
                st.state = STATE_THROWN;
                emit_carried(id, carry, false);
            }
        }
        STATE_THROWN => {
            let p = e.position.get();
            e.position
                .set_xy(p.x + st.thrown_dx * dt, p.y + st.thrown_dy * dt);
            st.thrown_dx = approach(st.thrown_dx, 0.0, 400.0 * dt);
            st.thrown_dy = approach(st.thrown_dy, 0.0, 400.0 * dt);
            st.thrown_timer -= dt;
            if st.thrown_timer <= 0.0 {
                st.state = STATE_IDLE;
            }
        }
        _ => {}
    }

    STATES.with(|s| {
        if let Some(st_ref) = s.borrow_mut().get_mut(&id) {
            *st_ref = st;
        }
    });
}

/// Returns `true` if some *other* holdable already emitted an `EV_CARRIED`
/// attach this frame, so we don't double-grab the same player.
fn another_holder_has_player(self_id: EntityId) -> bool {
    host::drain_events().iter().any(|(eid, kind, data)| {
        *kind == host::EV_CARRIED && *eid != self_id && data.last().copied().unwrap_or(0) != 0
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let e = Entity::new(id);
    let p = e.position.get();
    let st = STATES.with(|s| s.borrow().get(&id).cloned());
    let (w, h) = match st {
        Some(st) => (st.w, st.h),
        None => (12.0, 12.0),
    };
    draw_rect(p.x, p.y, w, h, CRYSTAL);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carry_offset_matches_player_cs() {
        assert!((CARRY_OFFSET_X - 0.0).abs() < 0.001);
        assert!((CARRY_OFFSET_Y - (-12.0)).abs() < 0.001);
    }

    #[test]
    fn throw_constants_match_theocrystal_cs() {
        assert!((THROW_SPEED - 200.0).abs() < 0.001);
        assert!((THROW_TIME - 0.3).abs() < 0.001);
    }

    #[test]
    fn approach_clamps_to_target() {
        assert!((approach(0.0, 10.0, 5.0) - 5.0).abs() < 0.001);
        assert!((approach(10.0, 0.0, 3.0) - 7.0).abs() < 0.001);
        assert!((approach(5.0, 5.0, 100.0) - 5.0).abs() < 0.001);
    }

    #[test]
    fn state_constants_are_distinct() {
        assert_ne!(STATE_IDLE, STATE_HELD);
        assert_ne!(STATE_HELD, STATE_THROWN);
        assert_ne!(STATE_IDLE, STATE_THROWN);
    }

    #[test]
    fn default_state_is_idle() {
        let st = State::default();
        assert_eq!(st.state, STATE_IDLE);
        assert_eq!(st.throw_facing, 1.0);
    }
}
