#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `fallingBlock` entity plugin.
//!
//! Mirrors `FallingBlock.cs`: a solid platform (marked via the host riding
//! mechanism) that shakes, then falls away once the player stands on it.
//! When it lands on solid ground it breaks apart and is consumed for the
//! rest of the session. Rendered as a filled rect until autotiled tiles land.

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("fallingblock");
ruleste_plugin_api::ruleste_entity_types!("fallingBlock");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// Terminal fall speed (world units / second), matching the original's 160.
const MAX_SPEED: f32 = 160.0;
/// Fall acceleration (world units / second^2), matching the original's 500.
const ACCEL: f32 = 500.0;
/// Shake duration before the block starts falling.
const SHAKE_TIME: f32 = 0.2;
/// Player-grace window after shaking begins.
const GRACE_TIME: f32 = 0.4;

#[derive(Debug, Default)]
struct FallingState {
    /// 0 = waiting for the player, 1 = shaking, 2 = player grace,
    /// 3 = falling, 4 = broken/consumed.
    phase: u32,
    timer: f32,
    speed: f32,
    /// True once the block is free-falling.
    started: bool,
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

/// Tile color fallback for `tiletype '3'` (ice): a cool blue-grey.
fn tile_color() -> Color {
    Color::new(122, 148, 168, 255)
}

fn player_on_top(entity: &Entity) -> bool {
    let (w, _, ox, oy) = entity.hitbox.get();
    let p = entity.position.get();
    let top = p.y + oy;
    for player_id in host::entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let bottom = pp.y + poy + ph;
        let overlap_x = pp.x + pox < p.x + ox + w && pp.x + pox + pw > p.x + ox;
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
    entity.collision.platform(true);
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        if st.phase == 4 {
            return;
        }
        let entity = Entity::new(id);

        match st.phase {
            0 => {
                if player_on_top(&entity) {
                    st.phase = 1;
                    st.timer = SHAKE_TIME;
                }
            }
            1 => {
                st.timer -= dt;
                if st.timer <= 0.0 {
                    st.phase = 2;
                    st.timer = GRACE_TIME;
                    st.started = true;
                }
            }
            2 => {
                st.timer -= dt;
                if st.timer <= 0.0 {
                    st.phase = 3;
                }
            }
            3 => {
                st.speed = (st.speed + ACCEL * dt).min(MAX_SPEED);
                let moved = entity.collision.actor_move(0.0, st.speed * dt);
                if moved.on_ground {
                    st.phase = 4;
                    host::collect(id);
                    return;
                }
                // Fell off the bottom of the level: gone for good.
                if entity.position.get().y > 8000.0 {
                    st.phase = 4;
                    host::collect(id);
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
    host::draw_rect(p.x + ox, p.y + oy, w, h, tile_color());
}
