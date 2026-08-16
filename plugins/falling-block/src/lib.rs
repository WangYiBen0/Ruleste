#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `fallingBlock` entity plugin — rewritten to mirror `FallingBlock.cs`.
//!
//! A solid platform that triggers once the player stands on it (`HasPlayerOnTop`,
//! or `HasPlayerRider` when `climbFall`, the default), shakes for a beat, gives
//! the player a grace window to step off (extended while the player still
//! stands on the side faces on `climbFall` blocks), then falls at up to
//! 160 u/s. Landing on solid tiles makes it permanent (`Safe = true`); landing
//! on a one-way/dynamic platform keeps it there and it falls again once the
//! support moves away. Only a fall past the level bottom despawns it.

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
/// Rest time after impact before re-checking the support below.
const REST_TIME: f32 = 0.2;
/// Rough level bottom below which the block is gone for good.
const LEVEL_BOTTOM: f32 = 8000.0;

#[derive(Debug, Default)]
struct FallingState {
    /// 0 = idle, 1 = fall delay, 2 = shaking, 3 = grace, 4 = falling,
    /// 5 = impact rest, 6 = resting on platform, 7 = permanent solid,
    /// 8 = gone.
    phase: u32,
    timer: f32,
    speed: f32,
    delay: f32,
    triggered: bool,
    climb_fall: bool,
    /// Tile character for the autotiled box (`TileType`, default `'3'`).
    tile: char,
    /// `behind`: drawn behind the player pass.
    behind: bool,
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

/// The player hitbox overlaps `(x, y, w, h)`, i.e. the block's box.
fn player_overlaps_rect(x: f32, y: f32, w: f32, h: f32) -> bool {
    for player_id in host::entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let (px, py) = (pp.x + pox, pp.y + poy);
        if px < x + w && px + pw > x && py < y + h && py + ph > y {
            return true;
        }
    }
    false
}

/// `HasPlayerOnTop`: the player's feet rest on the block's top face.
fn player_on_top(entity: &Entity) -> bool {
    let (w, _, ox, oy) = entity.hitbox.get();
    let p = entity.position.get();
    for player_id in host::entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let (px, py) = (pp.x + pox, pp.y + poy);
        let overlap_x = px < p.x + ox + w && px + pw > p.x + ox;
        if overlap_x && py + ph >= p.y + oy - 1.0 && py + ph <= p.y + oy + 4.0 {
            return true;
        }
    }
    false
}

/// `CollideCheck<Player>(Position ± UnitX)`: the player is pressed against a
/// side face (grabbing the edge), on a `climbFall` block.
fn player_on_side(entity: &Entity, right: bool) -> bool {
    let (w, h, ox, oy) = entity.hitbox.get();
    let p = entity.position.get();
    let dx = if right { 1.0 } else { -1.0 };
    player_overlaps_rect(p.x + ox + dx, p.y + oy, w, h)
}

/// `HasPlayerRider`: any player on top. Mirrors `HasPlayerOnTop` for the
/// trigger (the side-grab is only consulted during the grace window).
fn player_rider(entity: &Entity) -> bool {
    player_on_top(entity)
}

/// `PlayerFallCheck`: depends on `climbFall`.
fn player_fall_check(entity: &Entity, climb_fall: bool) -> bool {
    if climb_fall {
        player_rider(entity)
    } else {
        player_on_top(entity)
    }
}

/// `PlayerWaitCheck`: keeps the grace window alive while the player is
/// committed — standing on top, or (on `climbFall` blocks) pressing a side.
fn player_wait_check(entity: &Entity, st: &FallingState) -> bool {
    if st.triggered {
        return true;
    }
    if player_fall_check(entity, st.climb_fall) {
        return true;
    }
    if st.climb_fall {
        if !player_on_side(entity, false) {
            return player_on_side(entity, true);
        }
        return true;
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
        st.climb_fall = spawn.get_bool("climbFall", true);
        st.behind = spawn.get_bool("behind", false);
        let tile = spawn.get_str("tiletype", "3");
        st.tile = tile.chars().next().unwrap_or('3');
        if st.behind {
            entity.depth.set(5000);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        if st.phase >= 7 {
            return;
        }
        let entity = Entity::new(id);

        match st.phase {
            0 => {
                // `while (!Triggered && (finalBoss || !PlayerFallCheck()))`.
                if st.triggered || player_fall_check(&entity, st.climb_fall) {
                    st.phase = 1;
                    st.timer = st.delay;
                }
            }
            1 => {
                // `FallDelay` before the shake starts.
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
                // `while (timer > 0f && PlayerWaitCheck())`.
                if st.timer > 0.0 && player_wait_check(&entity, st) {
                    st.timer -= dt;
                    if st.timer <= 0.0 {
                        st.phase = 4;
                        st.speed = 0.0;
                    }
                } else {
                    // The player stepped off — fall immediately.
                    st.phase = 4;
                    st.speed = 0.0;
                }
            }
            4 => {
                st.speed = (st.speed + ACCEL * dt).min(MAX_SPEED);
                let moved = entity.collision.actor_move(0.0, st.speed * dt);
                if moved.on_ground {
                    // Impact: shake, then inspect the support below.
                    st.phase = 5;
                    st.timer = REST_TIME;
                    return;
                }
                if entity.position.get().y > LEVEL_BOTTOM {
                    // Fell out of the level: gone until the next respawn.
                    st.phase = 8;
                    host::remove(id);
                }
            }
            5 => {
                // Rest at the impact point.
                st.timer -= dt;
                if st.timer <= 0.0 {
                    // Solid tiles below → permanent (`break; Safe = true`).
                    if entity.collision.check(0.0, 1.0) {
                        st.phase = 7;
                    } else {
                        // Otherwise wait for a platform to carry/move away.
                        st.phase = 6;
                    }
                }
            }
            6 if !entity.collision.is_grounded() => {
                // `while (CollideCheck<Platform>(Position + (0, 1)))` — as soon
                // as the support is gone, shake and fall again.
                st.phase = 2;
                st.timer = SHAKE_TIME;
            }
            6 => {}
            _ => {}
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    let entity = Entity::new(id);
    let (w, h, ox, oy) = entity.hitbox.get();
    let p = entity.position.get();
    let st = STATES.with(|s| {
        let s = s.borrow();
        s.get(id).map(|ptr| ptr as *const FallingState)
    });
    let Some(st) = st else {
        return;
    };
    if unsafe { (*st).phase } == 8 {
        return;
    }
    // `GFX.FGAutotiler.GenerateBox(tile, w / 8, h / 8)`.
    let tx = (w / 8.0).ceil().max(1.0) as u32;
    let ty = (h / 8.0).ceil().max(1.0) as u32;
    host::draw_tile_box(unsafe { (*st).tile }, p.x + ox, p.y + oy, tx, ty);
    // Outlined fall-dishonest blocks are handled by the tileset; draw a dim
    // tint so the falling/gone states read against busy backgrounds.
    let phase = unsafe { (*st).phase };
    if (4..7).contains(&phase) {
        host::draw_rect(p.x + ox, p.y + oy, w, h, Color::new(255, 255, 255, 60));
    }
}
