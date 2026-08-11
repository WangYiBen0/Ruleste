#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `fallingBlock` entity plugin — rewritten to mirror `FallingBlock.cs`.
//!
//! A solid platform that triggers once the player stands on it or touches
//! its sides (climbFall), shakes for a beat, gives the player a grace window
//! to step off, then falls at up to 160 u/s. When it lands on solid ground it
//! stops and stays solid forever (`Safe = true`) — it is *not* consumed. Only
//! a fall past the level bottom despawns it.

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("falling-block");
ruleste_plugin_api::ruleste_entity_types!("fallingBlock");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// Terminal fall speed (`maxSpeed`, 160 for non-finalBoss blocks).
const MAX_SPEED: f32 = 160.0;
/// Fall acceleration.
const ACCEL: f32 = 500.0;
/// Shake duration before the grace window (`yield return 0.2f`).
const SHAKE_TIME: f32 = 0.2;
/// Player-grace window (`timer = 0.4f`).
const GRACE_TIME: f32 = 0.4;
/// Rough level bottom below which the block is gone for good.
const LEVEL_BOTTOM: f32 = 8000.0;

#[derive(Debug, Default)]
struct FallingState {
    /// 0 = idle, 1 = fall delay, 2 = shaking, 3 = grace, 4 = falling,
    /// 5 = landed (permanent solid), 6 = gone.
    phase: u32,
    timer: f32,
    speed: f32,
    delay: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<FallingState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut FallingState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, FallingState::default) as *mut FallingState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

/// Checks if player is standing on top or touching sides (climbFall).
fn player_trigger_check(entity: &Entity) -> bool {
    let (w, h, ox, oy) = entity.hitbox.get();
    let p = entity.position.get();
    let block_rect_x = p.x + ox;
    let block_rect_y = p.y + oy;

    for player_id in host::entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let player_rect_x = pp.x + pox;
        let player_rect_y = pp.y + poy;

        let overlap_x = player_rect_x < block_rect_x + w && player_rect_x + pw > block_rect_x;
        let overlap_y = player_rect_y < block_rect_y + h && player_rect_y + ph > block_rect_y;

        if overlap_x && overlap_y {
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
    with_state(id, |st| {
        st.delay = spawn.get_float("delay", 0.4);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        if st.phase >= 5 {
            return;
        }
        let entity = Entity::new(id);

        match st.phase {
            0 => {
                if player_trigger_check(&entity) {
                    st.phase = 1;
                    st.timer = st.delay;
                }
            }
            1 => {
                st.timer -= dt;
                if st.timer <= 0.0 {
                    st.phase = 2;
                    st.timer = SHAKE_TIME;
                }
            }
            2 => {
                st.timer -= dt;
                if st.timer <= 0.0 {
                    st.phase = 3;
                    st.timer = GRACE_TIME;
                }
            }
            3 => {
                if player_trigger_check(&entity) {
                    st.timer -= dt;
                    if st.timer <= 0.0 {
                        st.phase = 4;
                        st.speed = 0.0;
                    }
                } else {
                    st.phase = 4;
                    st.speed = 0.0;
                }
            }
            4 => {
                st.speed = (st.speed + ACCEL * dt).min(MAX_SPEED);
                let moved = entity.collision.actor_move(0.0, st.speed * dt);
                if moved.on_ground {
                    st.phase = 5;
                    return;
                }
                if entity.position.get().y > LEVEL_BOTTOM {
                    st.phase = 6;
                    host::remove(id);
                }
            }
            _ => {}
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let entity = Entity::new(id);
    let (w, h, ox, oy) = entity.hitbox.get();
    let p = entity.position.get();
    host::draw_rect(p.x + ox, p.y + oy, w, h, Color::new(122, 148, 168, 255));
}
