#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `zipMover` entity plugin.
//!
//! Mirrors `ZipMover.cs`: a solid box ridden by the player. It only starts the
//! trek once someone stands on it (`HasPlayerRider()`); then it accelerates out
//! with a `SineIn` ease (`at2` approaches 1 at 2/s → ~0.5 s), rests at the far
//! end 0.5 s, glides back over ~2 s, and rests at home 0.5 s. Movement goes
//! through `actor_move` so riders are carried along.

use ruleste_plugins_api::host;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{EntityId, Vec2};

ruleste_plugins_api::ruleste_meta!("zip-mover");
ruleste_plugins_api::ruleste_entity_types!("zipMover");
ruleste_plugins_api::ruleste_noop_destroy!();
ruleste_plugins_api::ruleste_noop_serialize!();

/// The approach rate out: `Calc.Approach(at2, 1, 2 * dt)` → ~0.5 s leg.
const MOVE_OUT_RATE: f32 = 2.0;
/// The approach rate back: `at2` at 0.5/s → ~2 s leg.
const MOVE_BACK_RATE: f32 = 0.5;
/// How long the block rests at the far end (`yield return 0.5f`).
const REST_OUT: f32 = 0.5;
/// How long it rests back home before the next rider-triggered trek.
const REST_HOME: f32 = 0.5;

#[derive(Debug)]
struct MoverState {
    from: Vec2,
    to: Vec2,
    percent: f32,
    moving_out: bool,
    /// 0 = waiting for a rider, 1 = moving out, 2 = resting at far end,
    /// 3 = moving home, 4 = resting at home.
    phase: u32,
    timer: f32,
}

impl Default for MoverState {
    fn default() -> MoverState {
        MoverState {
            from: Vec2::ZERO,
            to: Vec2::ZERO,
            percent: 0.0,
            moving_out: true,
            phase: 0,
            timer: 0.0,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<MoverState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut MoverState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, MoverState::default) as *mut MoverState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let w = spawn.get_float("width", 16.0);
    let h = spawn.get_float("height", 16.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(w, h, 0.0, 0.0);
    // `ZipMover.cs`: Depth = -9999.
    entity.depth.set(-9999);
    entity.collision.solid(true);
    with_state(id, |st| {
        st.from = Vec2::new(x, y);
        st.to = spawn.get_node(0).unwrap_or(Vec2::new(x + 48.0, y));
    });
}

/// `Solid.HasPlayerRider()`: a player's hitbox rests on (or overlaps) the
/// block's top surface.
fn has_player_rider(entity: &Entity) -> bool {
    let p = entity.position.get();
    let (w, h, ox, oy) = entity.hitbox.get();
    let top = p.y + oy;
    let left = p.x + ox;
    let right = left + w;
    for player_id in host::entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let pleft = pp.x + pox;
        let pright = pleft + pw;
        let pbottom = pp.y + poy + ph;
        let ptop = pp.y + poy;
        // Rides when standing on the top face, or pressed against it from the
        // side (CollideCheckOutside semantics).
        if pright > left + 1.0 && pleft < right - 1.0 && pbottom >= top - 1.0 && pbottom <= top + h
        {
            return true;
        }
        // Side contact (e.g. pushing from below) also counts as riding here,
        // matching `Actor.HasPlayerRider`'s CollideCheck outcome.
        let overlap = pright > left && pleft < right && pbottom > top && ptop < top + h;
        if overlap {
            return true;
        }
    }
    false
}

/// The eased position along the current leg.
fn eased_pos(st: &MoverState) -> Vec2 {
    let (a, b) = if st.moving_out {
        (st.from, st.to)
    } else {
        (st.to, st.from)
    };
    let ease = sine_in(st.percent);
    Vec2::new(a.x + (b.x - a.x) * ease, a.y + (b.y - a.y) * ease)
}

/// `Ease.SineIn(t)`: 1 - cos(t*PI/2).
fn sine_in(t: f32) -> f32 {
    1.0 - (t * std::f32::consts::PI * 0.5).cos()
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let cur = entity.position.get();

        match st.phase {
            0 => {
                // Wait for a rider. (`while (true) { if (!HasPlayerRider) yield; }`)
                if has_player_rider(&entity) {
                    st.phase = 1;
                    st.percent = 0.0;
                    st.moving_out = true;
                }
            }
            1 | 3 => {
                let rate = if st.phase == 1 {
                    MOVE_OUT_RATE
                } else {
                    MOVE_BACK_RATE
                };
                st.percent = approach(st.percent, 1.0, rate * dt);
                let target = eased_pos(st);
                let _ = entity
                    .collision
                    .actor_move(target.x - cur.x, target.y - cur.y);
                if st.percent >= 1.0 {
                    st.percent = 1.0;
                    if st.phase == 1 {
                        st.phase = 2;
                    } else {
                        st.phase = 4;
                    }
                    st.timer = if st.phase == 2 { REST_OUT } else { REST_HOME };
                }
            }
            2 | 4 => {
                st.timer -= dt;
                if st.timer <= 0.0 {
                    if st.phase == 2 {
                        st.moving_out = false;
                        st.percent = 0.0;
                        st.phase = 3;
                    } else {
                        // Back home — wait for the next rider.
                        st.phase = 0;
                    }
                }
            }
            _ => {}
        }
    });
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
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let entity = Entity::new(id);
    let p = entity.position.get();
    let (w, h, ox, oy) = entity.hitbox.get();
    // Solid timber slab.
    ruleste_plugins_api::host::draw_rect(p.x + ox, p.y + oy, w, h, crate_color());
}

fn crate_color() -> ruleste_plugins_api::types::Color {
    ruleste_plugins_api::types::Color::new(0x58, 0x3c, 0x20, 0xff)
}
