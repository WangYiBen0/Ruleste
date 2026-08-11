#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `starJumpBlock` entity plugin.
//!
//! The Reflection chapter's jump-star slabs. Per `StarJumpBlock.cs` these are
//! permanent *solid* platforms; when `sinks` is set they ride a slow sine
//! dip of 12px while the player stands on them and ease back up when left.
//! A rising player that touches the underside gets the star boost (a local
//! approximation of the star jump). The block is never consumed.

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId, Vec2};

ruleste_plugin_api::ruleste_meta!("star-jump-block");
ruleste_plugin_api::ruleste_entity_types!("starJumpBlock");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// Upward launch the star bestows, ~the original's powerful star jump.
const STAR_BOOST: f32 = -320.0;
/// How far a `sinks` block dips when ridden.
const SINK_DISTANCE: f32 = 12.0;
/// `Calc.Approach(yLerp, ..., 1f * dt)`.
const SINK_RATE: f32 = 1.0;
/// `HasPlayerRider` hold time once the player steps off.
const SINK_HOLD: f32 = 0.1;

fn sine_in_out(t: f32) -> f32 {
    ((t * std::f32::consts::PI).cos() * -0.5 + 0.5).clamp(0.0, 1.0)
}

#[derive(Debug)]
struct JumpState {
    w: f32,
    h: f32,
    sinks: bool,
    start_y: f32,
    y_lerp: f32,
    sink_timer: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<JumpState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut JumpState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, || unreachable!("state seeded at init")) as *mut JumpState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

/// True when a player is standing on the block's top face (`HasPlayerRider`).
fn player_on_top(entity: &Entity, w: f32) -> bool {
    let p = entity.position.get();
    let top = p.y;
    for player_id in host::entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let bottom = pp.y + poy + ph;
        let overlap_x = pp.x + pox < p.x + w && pp.x + pox + pw > p.x;
        if overlap_x && bottom >= top - 1.0 && bottom <= top + 4.0 {
            return true;
        }
    }
    false
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let w = spawn.get_float("width", 8.0);
    let h = spawn.get_float("height", 8.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(w, h, 0.0, 0.0);
    entity.collision.solid(true);
    entity.depth.set(-10000);
    STATES.with(|s| {
        s.borrow_mut().insert(
            id,
            JumpState {
                w,
                h,
                sinks: spawn.get_bool("sinks", false),
                start_y: y,
                y_lerp: 0.0,
                sink_timer: 0.0,
            },
        );
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let p = entity.position.get();

        // Star boost: a rising player pressing into the underside.
        for player_id in host::entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
            let overlap = pp.x + pox < p.x + st.w
                && pp.x + pox + pw > p.x
                && pp.y + poy < p.y + st.h
                && pp.y + poy + ph > p.y;
            if !overlap {
                continue;
            }
            let vel = host::Speed::new(player_id).get();
            if vel.y < 0.0 {
                host::Speed::new(player_id).set(Vec2::new(vel.x, STAR_BOOST));
                break;
            }
        }

        // Sink behavior: ride a 12px sine dip while stood on, ease back up.
        if st.sinks {
            let riding = player_on_top(&entity, st.w);
            if riding {
                st.sink_timer = SINK_HOLD;
            } else if st.sink_timer > 0.0 {
                st.sink_timer -= dt;
            }
            if st.sink_timer > 0.0 {
                st.y_lerp = (st.y_lerp + SINK_RATE * dt).min(1.0);
            } else {
                st.y_lerp = (st.y_lerp - SINK_RATE * dt).max(0.0);
            }
            let target = st.start_y + SINK_DISTANCE * sine_in_out(st.y_lerp);
            let delta = target - p.y;
            if delta.abs() > 0.001 {
                let _ = entity.collision.actor_move(0.0, delta);
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        // Star slab: deep purple fill under a starlight cap.
        host::draw_rect(p.x, p.y, st.w, st.h, Color::new(0x60, 0x38, 0xa8, 0xff));
        host::draw_rect(p.x, p.y, st.w, 2.0, Color::new(0xd0, 0xb0, 0xf8, 0xff));
    });
}
