#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `introCrusher` entity plugin.
//!
//! Mirrors `IntroCrusher.cs`: the prologue's stone slab. A solid riding
//! platform (depth -10501) that waits until the player steps into its trigger
//! zone, shakes for 1.2 s, then crushes straight down to its node with a
//! cube-in ease. If the player ducks out of the shake zone it gives up early.
//!
//! Implemented as a one-way `platform` (rather than a full `Solid`) so the
//! player can ride the slab down without being pushed into the floor below —
//! equivalent to the original `Safe = true` flag.
//!
//! Note: the original bails to `end` immediately when session flags `1`/`0b`
//! are set (so a respawn doesn't re-trigger the crush). We don't have session
//! flags yet, so the sequence always runs; the slab stays put once settled.

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{EntityId, Vec2};

ruleste_plugin_api::ruleste_meta!("intro-crusher");
ruleste_plugin_api::ruleste_entity_types!("introCrusher");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// How long the slab shakes before it drops, matching the original's 1.2 s.
const SHAKE_TIME: f32 = 1.2;
/// Rate the fall eases in at (`Calc.Approach(t, 1, 2 * dt)` → ~0.5 s fall).
const FALL_RATE: f32 = 2.0;

/// 0 = waiting for the player, 1 = shaking, 2 = falling, 3 = settled.
#[derive(Debug, Clone, Copy)]
struct CrusherState {
    phase: u32,
    timer: f32,
    fall_t: f32,
    start: Vec2,
    end: Vec2,
}

impl Default for CrusherState {
    fn default() -> CrusherState {
        CrusherState {
            phase: 0,
            timer: 0.0,
            fall_t: 0.0,
            start: Vec2::ZERO,
            end: Vec2::ZERO,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<CrusherState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut CrusherState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, CrusherState::default) as *mut CrusherState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

fn cube_in(t: f32) -> f32 {
    t * t * t
}

/// The player's center-x, if a player entity is alive.
fn player_x() -> Option<f32> {
    for player_id in host::entities_by_type("player") {
        if host::entity_alive(player_id) {
            let p = host::Position::new(player_id).get();
            let (_, _, ox, _) = host::Hitbox::new(player_id).get();
            return Some(p.x + ox);
        }
    }
    None
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let w = spawn.get_float("width", 8.0).max(1.0);
    let h = spawn.get_float("height", 8.0).max(1.0);
    let node = spawn.get_node(0).unwrap_or(Vec2::new(x, y));
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(w, h, 0.0, 0.0);
    entity.collision.platform(true);
    entity.depth.set(-10501);
    with_state(id, |st| {
        st.start = Vec2::new(x, y);
        st.end = node;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let (w, _, _, _) = entity.hitbox.get();

        match st.phase {
            0 => {
                // Trigger: player X within [base.X + 30, base.Right + 8].
                if let Some(px) = player_x() {
                    let p = entity.position.get();
                    if px >= p.x + 30.0 && px <= p.x + w + 8.0 {
                        st.phase = 1;
                        st.timer = SHAKE_TIME;
                        host::play_sound("event:/game/00_prologue/fallblock_first_shake");
                    }
                }
            }
            1 => {
                st.timer -= dt;
                // Give up shaking if the player slips out of the shake zone.
                let p = entity.position.get();
                let escaped = player_x()
                    .map(|px| px >= p.x + w - 8.0 || px < p.x + 28.0)
                    .unwrap_or(false);
                if escaped || st.timer <= 0.0 {
                    st.phase = 2;
                    st.fall_t = 0.0;
                }
            }
            2 => {
                st.fall_t = (st.fall_t + FALL_RATE * dt).min(1.0);
                let e = cube_in(st.fall_t);
                let target = Vec2::new(
                    st.start.x + (st.end.x - st.start.x) * e,
                    st.start.y + (st.end.y - st.start.y) * e,
                );
                let p = entity.position.get();
                let (dx, dy) = (target.x - p.x, target.y - p.y);
                if dx.abs() > 0.0 || dy.abs() > 0.0 {
                    let _ = entity.collision.actor_move(dx, dy);
                }
                if st.fall_t >= 1.0 {
                    st.phase = 3;
                    host::play_sound("event:/game/00_prologue/fallblock_first_impact");
                }
            }
            _ => {}
        }
    });
}

/// Shake amplitude: tiles jitter ±2 px while the slab rumbles.
fn shake_offset(t: f32) -> (f32, f32) {
    let a = (t * 12.9898).sin() * 43_758.547;
    let b = (t * 78.233).sin() * 12_543.234;
    let sx = (a.fract() - 0.5) * 4.0;
    let sy = (b.fract() - 0.5) * 4.0;
    (sx, sy)
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let (w, h, ox, oy) = entity.hitbox.get();
        let p = entity.position.get();
        let (sx, sy) = if st.phase == 1 {
            shake_offset(SHAKE_TIME - st.timer)
        } else {
            (0.0, 0.0)
        };
        // Mirror `IntroCrusher.cs`: the slab is `Autotiler.GenerateBox('3',
        // width/8, height/8)` drawn at `Position + shake` (top-left). The
        // hitbox offset already anchors it to the entity's position.
        let tiles_x = (w / 8.0).max(1.0) as u32;
        let tiles_y = (h / 8.0).max(1.0) as u32;
        host::draw_tile_box('3', p.x + ox + sx, p.y + oy + sy, tiles_x, tiles_y);
    });
}
