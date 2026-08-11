#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `cloud` entity plugin.
//!
//! Mirrors `Cloud.cs`: a 32-wide jump-through platform (hitbox `32×5` at
//! `(-16,0)`) floating in place. When the player lands on it and presses down
//! the cloud dips, squishes, then springs back up, launching the rider with
//! `Speed.Y = -200`. `fragile` clouds (pink) shatter: they fade out, go
//! non-collidable, and respawn after 2.5s; non-fragile clouds just return to
//! their resting height. Downward fall speed is capped at 220 (`num = -220`).
//! The platform is a host solid-platform, so riders are carried during bounce.

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::EntityId;

ruleste_plugin_api::ruleste_meta!("cloud");
ruleste_plugin_api::ruleste_entity_types!("cloud");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

const BOOST_ACCEL: f32 = 1200.0;
const RETURN_ACCEL: f32 = 600.0;
const RESPAWN_TIME: f32 = 2.5;
/// Max downward fall speed, `num = -220f` in the original.
const MAX_FALL: f32 = 220.0;
/// Fade animation: 6 frames, advanced at 12fps.
const FADE_RATE: f32 = 12.0;

#[derive(Debug)]
struct CloudState {
    fragile: bool,
    waiting: bool,
    returning: bool,
    speed: f32,
    start_y: f32,
    respawn_timer: f32,
    timer: f32,
    scale_x: f32,
    scale_y: f32,
    sprite_y: f32,
    active: bool,
    fading: bool,
    fade_frame: f32,
}

impl Default for CloudState {
    fn default() -> CloudState {
        CloudState {
            fragile: false,
            waiting: true,
            returning: false,
            speed: 0.0,
            start_y: 0.0,
            respawn_timer: 0.0,
            timer: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            sprite_y: 0.0,
            active: true,
            fading: false,
            fade_frame: 0.0,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<CloudState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut CloudState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, CloudState::default) as *mut CloudState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

/// True when the player is standing on this cloud's top surface with a
/// downward-or-zero vertical speed (the rider that triggers a boost).
fn has_rider(id: EntityId, top: f32) -> Option<u32> {
    let entity = Entity::new(id);
    let p = entity.position.get();
    for player_id in host::entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let bottom = pp.y + poy + ph;
        let overlap_x = pp.x + pox < p.x + 16.0 && pp.x + pox + pw > p.x - 16.0;
        if overlap_x && (bottom - top).abs() <= 2.0 {
            return Some(player_id);
        }
    }
    None
}

/// The cloud's standable top surface (hitbox y-offset is 0).
fn top_of(entity: &Entity) -> f32 {
    entity.position.get().y
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    // `JumpThru(position, 32)`: Hitbox(32, 5), Collider.Position.X = -16.
    entity.hitbox.set(32.0, 5.0, -16.0, 0.0);
    entity.collision.platform(true);
    entity.depth.set(-9000);
    with_state(id, |st| {
        st.fragile = spawn.get_bool("fragile", false);
        st.start_y = y;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        st.timer += dt;
        st.scale_x = approach(st.scale_x, 1.0, 1.0 * dt);
        st.scale_y = approach(st.scale_y, 1.0, 1.0 * dt);
        if st.fading {
            st.fade_frame += FADE_RATE * dt;
        }

        // The sprite bobs up and down while at rest (unless someone rides it).
        if st.active && st.waiting {
            st.sprite_y = approach(st.sprite_y, st.timer.sin() * 2.0, dt * 4.0);
        }

        if st.respawn_timer > 0.0 {
            st.respawn_timer -= dt;
            if st.respawn_timer <= 0.0 {
                st.waiting = true;
                st.returning = false;
                st.speed = 0.0;
                st.scale_x = 1.0;
                st.scale_y = 1.0;
                st.active = true;
                st.fading = false;
                st.fade_frame = 6.0; // "spawn" animation start
                entity.position.set_xy(entity.position.get().x, st.start_y);
                entity.collision.platform(true);
            }
            return;
        }

        if st.waiting {
            let top = top_of(&entity);
            let rider = has_rider(id, top).filter(|pid| host::Speed::new(*pid).get().y >= 0.0);
            if rider.is_some() {
                st.waiting = false;
                st.speed = 180.0;
                st.scale_x = 1.3;
                st.scale_y = 0.7;
            }
            return;
        }

        if st.returning {
            st.speed = approach(st.speed, 180.0, RETURN_ACCEL * dt);
            let p = entity.position.get();
            let amount = st.speed * dt;
            let toward = if p.y - st.start_y > 0.0 { -1.0 } else { 1.0 };
            if (p.y - st.start_y).abs() > amount {
                let _ = entity.collision.actor_move(0.0, toward * amount);
            } else {
                entity.position.set_xy(p.x, st.start_y);
                st.returning = false;
                st.waiting = true;
                st.speed = 0.0;
            }
            return;
        }

        // Fragile clouds collapse when their rider hops off before the bounce.
        if st.fragile && st.active && has_rider(id, top_of(&entity)).is_none() {
            st.active = false;
            st.fading = true;
            entity.collision.platform(false);
        }

        let p = entity.position.get();
        if p.y >= st.start_y {
            st.speed -= BOOST_ACCEL * dt;
        } else {
            st.speed += BOOST_ACCEL * dt;
            if st.speed >= -100.0 {
                // Launch the rider straight up (`Speed.Y = -200f`).
                if let Some(rider) = has_rider(id, p.y) {
                    let v = host::Speed::new(rider).get();
                    host::Speed::new(rider).set_xy(v.x, -200.0);
                }
                if st.fragile {
                    st.active = false;
                    st.fading = true;
                    st.respawn_timer = RESPAWN_TIME;
                    entity.collision.platform(false);
                } else {
                    st.scale_x = 0.7;
                    st.scale_y = 1.3;
                    st.returning = true;
                }
            }
        }
        // `MoveV(speed * dt, -220)`: downward travel capped at 220 u/s.
        let fall = if st.speed < 0.0 { -MAX_FALL } else { st.speed };
        let _ = entity.collision.actor_move(0.0, fall * dt);
    });
}

/// A frame id for the fragile animation: `fragile00-05` fade, `fragile06-12`
/// spawn; non-fragile uses the single `cloud00` frame.
fn draw_frame(st: &CloudState) -> String {
    if !st.fragile {
        return "objects/clouds/cloud00".to_string();
    }
    if st.fading {
        let idx = ((st.fade_frame * 12.0) as usize).min(5);
        format!("objects/clouds/fragile{idx:02}")
    } else {
        format!(
            "objects/clouds/fragile{:02}",
            (st.fade_frame as usize).min(12)
        )
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let p = entity.position.get();
        // The sprite's origin is 8px from its top; approximate centering.
        let cy = p.y + st.sprite_y;
        let frame = draw_frame(st);
        let sy = if st.fragile && st.speed < 0.0 && !st.waiting {
            st.scale_y.min(st.scale_y * (1.0 - st.fade_frame))
        } else {
            st.scale_y
        };
        host::draw_image(&frame, p.x, cy, 0.0, st.scale_x, sy.max(0.05));
    });
}

fn approach(cur: f32, target: f32, max_delta: f32) -> f32 {
    if (target - cur).abs() <= max_delta {
        target
    } else {
        cur + (target - cur).signum() * max_delta
    }
}
