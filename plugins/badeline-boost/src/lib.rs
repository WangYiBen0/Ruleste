#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `badelineBoost` entity plugin — reworked to mirror `BadelineBoost.cs`.
//!
//! Touching the purple orb starts the grab sequence: the player is pulled onto
//! the boost (`EV_CARRIED` keeps them frozen while the boost drives their
//! position), then launched upward with `(0, -330)` plus an X approach toward
//! the boost (`EV_BADELINE_BOOST` → the player's `StLaunch`; the final node
//! uses `StSummitLaunch` instead). After launching, the boost flies to its
//! next node at 320 px/s and becomes usable again — walking the chain. A node
//! list with a single entry keeps the boost in place.

use ruleste_plugin_api::host::{self, entities_by_type};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId, Vec2};

ruleste_plugin_api::ruleste_meta!("badeline-boost");
ruleste_plugin_api::ruleste_entity_types!("badelineBoost");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// `BadelineBoost.cs`: `MoveSpeed = 320f`.
const MOVE_SPEED: f32 = 320.0;
/// Grab lerp duration (the `p += dt / 0.2f` loop).
const GRAB_TIME: f32 = 0.2;
/// Brief pause after the snap (`yield return 0.1f`).
const HOLD_TIME: f32 = 0.1;
const HIT: f32 = 20.0;
const HIT_OX: f32 = -10.0;
const HIT_OY: f32 = -8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Idle,
    Grab,
    Hold,
    Travel,
}

#[derive(Debug)]
struct BoostState {
    phase: Phase,
    timer: f32,
    nodes: Vec<Vec2>,
    node_index: usize,
    player_id: Option<EntityId>,
    grab_from: Vec2,
    grab_to: Vec2,
    travel_from: Vec2,
    travel_to: Vec2,
    timer_acc: f32,
}

impl Default for BoostState {
    fn default() -> BoostState {
        BoostState {
            phase: Phase::Idle,
            timer: 0.0,
            nodes: Vec::new(),
            node_index: 0,
            player_id: None,
            grab_from: Vec2::ZERO,
            grab_to: Vec2::ZERO,
            travel_from: Vec2::ZERO,
            travel_to: Vec2::ZERO,
            timer_acc: 0.0,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<BoostState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut BoostState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, BoostState::default) as *mut BoostState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

fn emit_carried(player_id: EntityId, pos: Vec2, on: bool) {
    let mut payload = Vec::with_capacity(9);
    payload.extend_from_slice(&pos.x.to_le_bytes());
    payload.extend_from_slice(&pos.y.to_le_bytes());
    payload.push(u8::from(on));
    host::emit(player_id, host::EV_CARRIED, &payload);
}

fn player_center(player_id: EntityId) -> Vec2 {
    let pp = host::Position::new(player_id).get();
    let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
    Vec2::new(pp.x + pox + pw * 0.5, pp.y + poy + ph * 0.5)
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    let p = Vec2::new(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    entity.position.set_xy(p.x, p.y);
    entity.hitbox.set(HIT, HIT, HIT_OX, HIT_OY);
    entity.depth.set(-8500);
    with_state(id, |st| {
        // `data.NodesWithPosition`: the entity position heads the node list.
        st.nodes.push(p);
        for n in spawn.nodes() {
            st.nodes.push(*n);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    let mut launch: Option<(Vec2, bool)> = None;
    let mut stop_carry: Option<EntityId> = None;
    let mut start_carry: Option<(EntityId, Vec2)> = None;

    with_state(id, |st| {
        st.timer += dt;
        match st.phase {
            Phase::Idle => {
                // Skip ahead to the next node when the player is far right.
                if st.node_index + 1 < st.nodes.len() {
                    for pid in entities_by_type("player") {
                        if host::entity_alive(pid) {
                            let pp = host::Position::new(pid).get();
                            let p = Entity::new(id).position.get();
                            if pp.x - p.x >= 100.0 {
                                st.node_index += 1;
                                st.timer_acc = 0.0;
                                st.travel_from = p;
                                st.travel_to = st.nodes[st.node_index];
                                st.phase = Phase::Travel;
                            }
                            break;
                        }
                    }
                }
                let p = Entity::new(id).position.get();
                for player_id in entities_by_type("player") {
                    if !host::entity_alive(player_id) {
                        continue;
                    }
                    let pp = host::Position::new(player_id).get();
                    let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
                    let overlap = pp.x + pox < p.x + HIT_OX + HIT
                        && pp.x + pox + pw > p.x + HIT_OX
                        && pp.y + poy < p.y + HIT_OY + HIT
                        && pp.y + poy + ph > p.y + HIT_OY;
                    if overlap {
                        // `BoostRoutine`: nodeIndex++, grab the player.
                        st.node_index += 1;
                        st.phase = Phase::Grab;
                        st.player_id = Some(player_id);
                        st.timer_acc = 0.0;
                        let pc = player_center(player_id);
                        let num = if pc.x - p.x != 0.0 {
                            (pc.x - p.x).signum()
                        } else {
                            -1.0
                        };
                        st.grab_from = pp;
                        st.grab_to = Vec2::new(p.x + num * 4.0, p.y - 3.0);
                        start_carry = Some((player_id, pp));
                        break;
                    }
                }
            }
            Phase::Grab => {
                // `p += dt / 0.2f` lerp carrying the player onto the boost.
                st.timer_acc += dt / GRAB_TIME;
                let t = st.timer_acc.clamp(0.0, 1.0);
                if let Some(pid) = st.player_id {
                    let eased = t;
                    let pos = Vec2::new(
                        st.grab_from.x + (st.grab_to.x - st.grab_from.x) * eased,
                        st.grab_from.y + (st.grab_to.y - st.grab_from.y) * eased,
                    );
                    start_carry = Some((pid, pos));
                }
                if t >= 1.0 {
                    st.phase = Phase::Hold;
                    st.timer_acc = 0.0;
                }
            }
            Phase::Hold => {
                st.timer_acc += dt;
                if st.timer_acc >= HOLD_TIME {
                    // `player.MoveV(5f)`; then launch.
                    if let Some(pid) = st.player_id {
                        stop_carry = Some(pid);
                    }
                    let final_boost = st.node_index >= st.nodes.len();
                    let p = Entity::new(id).position.get();
                    if final_boost {
                        launch = Some((p, true));
                    } else {
                        launch = Some((p, false));
                        st.timer_acc = 0.0;
                        st.travel_from = p;
                        st.travel_to = st.nodes[st.node_index];
                        st.phase = Phase::Travel;
                    }
                }
            }
            Phase::Travel => {
                // Move toward the next node at 320 px/s.
                st.timer_acc += dt;
                let total =
                    (st.travel_to.x - st.travel_from.x).hypot(st.travel_to.y - st.travel_from.y);
                if total <= f32::EPSILON {
                    st.phase = Phase::Idle;
                    return;
                }
                let travelled = (st.timer_acc * MOVE_SPEED).min(total);
                let p = Vec2::new(
                    st.travel_from.x + (st.travel_to.x - st.travel_from.x) * travelled / total,
                    st.travel_from.y + (st.travel_to.y - st.travel_from.y) * travelled / total,
                );
                Entity::new(id).position.set_xy(p.x, p.y);
                if travelled >= total {
                    st.phase = Phase::Idle;
                }
            }
        }
    });

    if let Some((pid, pos)) = start_carry {
        emit_carried(pid, pos, true);
    }
    if let Some(pid) = stop_carry {
        let pos = host::Position::new(pid).get();
        emit_carried(pid, pos, false);
    }
    if let Some((p, final_boost)) = launch {
        if let Some(pid) = st_player_id() {
            let mut payload = Vec::with_capacity(5);
            payload.extend_from_slice(&p.x.to_le_bytes());
            payload.push(u8::from(final_boost));
            host::emit(pid, host::EV_BADELINE_BOOST, &payload);
        }
        if final_boost {
            // `Finish`: the final boost despawns for good.
            host::collect(id);
        }
    }
}

/// The current player entity (first live `player`).
fn st_player_id() -> Option<EntityId> {
    entities_by_type("player")
        .into_iter()
        .find(|pid| host::entity_alive(*pid))
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        let travelling = st.phase == Phase::Travel;
        let pulse = 1.0 + (st.timer * 6.0).sin() * 0.12;
        let r = if travelling { 10.0 } else { 7.0 * pulse };
        let c = Color::new(0x70, 0x60, 0xe0, 0xe6);
        let mut y = p.y - r;
        while y <= p.y + r {
            let half = (r * r - (y - p.y) * (y - p.y)).sqrt();
            host::draw_rect(p.x - half, y, half * 2.0, 1.0, c);
            y += 2.0;
        }
        host::draw_rect(
            p.x - 2.0,
            p.y - 2.0,
            4.0,
            4.0,
            Color::new(0xe0, 0xd0, 0xff, 0xff),
        );
    });
}
