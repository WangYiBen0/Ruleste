#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `ruleste-plugin-key` — `key` (universal).
//!
//! A collectible key that, when the player touches it while not already holding
//! something, is picked up as a `Holdable` (mirroring `Key` in
//! `references/source/Celeste/Celeste/Key.cs`, which extends `Holdable`). The
//! player carries it via the `holdable` carry contract; throwing it (Jump/Dash
//! or releasing Climb) emits the `KEY` event so linked `lockBlock` solids open,
//! then the key is collected.
//!
//! It only emits `KEY`, so it lives in its own crate and relies on the host to
//! wire `lockBlock` listeners.

use ruleste_plugins_api::event;
use ruleste_plugins_api::host::{self, Hitbox, Input, Position, emit, entities_by_type};
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, spawn_data};
use ruleste_plugins_api::types::input;
use ruleste_plugins_api::types::{EntityId, Vec2};

use std::cell::RefCell;

ruleste_plugins_api::ruleste_meta!("key");
ruleste_plugins_api::ruleste_entity_types!("key");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

/// Waiting to be picked up.
const STATE_IDLE: u8 = 0;
/// Carried by the player.
const STATE_HELD: u8 = 1;
/// Thrown toward a lock; opens it on contact then collects.
const STATE_THROWN: u8 = 2;
/// Already used (opened a lock); draw nothing.
const STATE_TAKEN: u8 = 3;

const CARRY_OFFSET_X: f32 = 0.0;
const CARRY_OFFSET_Y: f32 = -12.0;

const THROW_SPEED: f32 = 200.0;
const THROW_TIME: f32 = 0.3;

#[derive(Clone)]
struct State {
    state: u8,
    thrown_dx: f32,
    thrown_dy: f32,
    thrown_timer: f32,
    throw_facing: f32,
}

impl Default for State {
    fn default() -> Self {
        State {
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

fn another_holder_has_player(self_id: EntityId) -> bool {
    host::drain_events().iter().any(|(eid, kind, data)| {
        *kind == host::EV_CARRIED && *eid != self_id && data.last().copied().unwrap_or(0) != 0
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let spawn: MapData = spawn_data(unsafe { std::slice::from_raw_parts(data, len as usize) });
    let e = Entity::new(id);
    e.position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    e.hitbox.set(16.0, 16.0, -8.0, -8.0);
    e.depth.set(200);
    e.sprite.set_bank("key");
    e.sprite.play("idle");
    STATES.with(|s| {
        s.borrow_mut().insert(id, State::default());
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
            if let Some((px, py, pw, ph)) = player_rect() {
                let p = e.position.get();
                if overlap(px, py, pw, ph, p.x - 8.0, p.y - 8.0, 16.0, 16.0)
                    && Input::pressed(input::CLIMB)
                    && !another_holder_has_player(id)
                {
                    st.state = STATE_HELD;
                }
            }
        }
        STATE_HELD => {
            let Some((px, py, _pw, _ph)) = player_rect() else {
                st.state = STATE_IDLE;
                STATES.with(|s| {
                    if let Some(st_ref) = s.borrow_mut().get_mut(&id) {
                        *st_ref = st;
                    }
                });
                return;
            };
            let carry = Vec2::new(px + CARRY_OFFSET_X, py + CARRY_OFFSET_Y);
            e.position.set_xy(carry.x, carry.y);
            emit_carried(id, carry, true);

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

            // Open any linked lock on contact, then collect.
            let hit_lock = entities_by_type("lockBlock").iter().any(|&lid| {
                let lp = Position::new(lid).get();
                let (lw, lh, lox, loy) = Hitbox::new(lid).get();
                overlap(
                    p.x - 8.0,
                    p.y - 8.0,
                    16.0,
                    16.0,
                    lp.x + lox,
                    lp.y + loy,
                    lw,
                    lh,
                )
            });
            if hit_lock {
                emit(id, event::KEY, &[]);
                host::collect(id);
                st.state = STATE_TAKEN;
                STATES.with(|s| {
                    if let Some(st_ref) = s.borrow_mut().get_mut(&id) {
                        *st_ref = st;
                    }
                });
                return;
            }
            if st.thrown_timer <= 0.0 {
                st.state = STATE_IDLE;
            }
        }
        STATE_TAKEN => {}
        _ => {}
    }

    STATES.with(|s| {
        if let Some(st_ref) = s.borrow_mut().get_mut(&id) {
            *st_ref = st;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(_id: EntityId) {
    // Visual is drawn by the host via the SpriteBank ("key" sprite).
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
    fn throw_constants_match_key_cs() {
        assert!((THROW_SPEED - 200.0).abs() < 0.001);
        assert!((THROW_TIME - 0.3).abs() < 0.001);
    }

    #[test]
    fn approach_clamps_to_target() {
        assert!((approach(0.0, 10.0, 5.0) - 5.0).abs() < 0.001);
        assert!((approach(10.0, 0.0, 3.0) - 7.0).abs() < 0.001);
    }

    #[test]
    fn state_constants_are_distinct() {
        assert_ne!(STATE_IDLE, STATE_HELD);
        assert_ne!(STATE_HELD, STATE_THROWN);
        assert_ne!(STATE_THROWN, STATE_TAKEN);
    }

    #[test]
    fn default_state_is_idle() {
        let st = State::default();
        assert_eq!(st.state, STATE_IDLE);
        assert_eq!(st.throw_facing, 1.0);
    }
}
