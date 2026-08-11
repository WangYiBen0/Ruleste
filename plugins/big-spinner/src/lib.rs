#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `bigSpinner` entity plugin — reworked as a `Bumper` (`Bumper.cs`).
//!
//! The large dust-spinner sprite is not in the loaded atlas, so the ring is
//! drawn procedurally as a slow-spinning octagon of arcs. Behavior matches the
//! original bumper: on touch the player is launched away from the bumper's
//! center (`Player.ExplodeLaunch`, here an `EV_LAUNCH` event) and the bumper
//! goes on a 0.6 s respawn cooldown. With `fireMode` set it is lethal instead.
//! A `node` attribute makes the bumper oscillate between its spawn point and
//! the node over `MoveCycleTime`.

use ruleste_plugin_api::host::{self, draw_image, entities_by_type};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId, Vec2};

ruleste_plugin_api::ruleste_meta!("big-spinner");
ruleste_plugin_api::ruleste_entity_types!("bigSpinner");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// `Bumper.cs`: `new Circle(12f)` — a 24px AABB covers it.
const BUMPER_R: f32 = 12.0;
/// `RespawnTime`.
const RESPAWN_TIME: f32 = 0.6;
/// `MoveCycleTime`: one leg of the node oscillation.
const MOVE_CYCLE_TIME: f32 = 1.8181819;

#[derive(Debug, Default)]
struct BumperState {
    angle: f32,
    respawn_timer: f32,
    has_node: bool,
    /// Spawn anchor (start of the leg).
    start: Vec2,
    /// Node target (end of the leg).
    end: Vec2,
    /// Eased progress along the current leg.
    leg_t: f32,
    go_back: bool,
    fire_mode: bool,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<BumperState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut BumperState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, BumperState::default) as *mut BumperState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

/// `Ease.CubeInOut`, matching `Tween.Create(..., Ease.CubeInOut, ...)`.
fn cube_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    let p = Vec2::new(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    entity.position.set_xy(p.x, p.y);
    entity
        .hitbox
        .set(BUMPER_R * 2.0, BUMPER_R * 2.0, -BUMPER_R, -BUMPER_R);
    entity.depth.set(-50);
    let node = spawn.get_node(0);
    with_state(id, |st| {
        st.start = p;
        st.end = node.unwrap_or(p);
        st.has_node = node.is_some();
        st.fire_mode = spawn.get_bool("fireMode", false);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.angle += dt * 0.8;
        if st.respawn_timer > 0.0 {
            st.respawn_timer -= dt;
        }

        // Node travel: a looping cube-in-out tween between start and end.
        if st.has_node {
            st.leg_t += dt / MOVE_CYCLE_TIME;
            if st.leg_t >= 1.0 {
                st.leg_t = 0.0;
                st.go_back = !st.go_back;
            }
        }
        let anchor = if st.has_node {
            let (a, b) = if st.go_back {
                (st.end, st.start)
            } else {
                (st.start, st.end)
            };
            let e = cube_in_out(st.leg_t);
            Vec2::new(a.x + (b.x - a.x) * e, a.y + (b.y - a.y) * e)
        } else {
            st.start
        };
        let p = anchor;
        Entity::new(id).position.set_xy(p.x, p.y);

        // Player collision (`OnPlayer`).
        for player_id in entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
            let pcx = pp.x + pox + pw * 0.5;
            let pcy = pp.y + poy + ph * 0.5;
            let dx = pcx - p.x;
            let dy = pcy - p.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist > BUMPER_R {
                continue;
            }
            if st.fire_mode {
                ruleste_plugin_api::host::die();
            } else if st.respawn_timer <= 0.0 {
                st.respawn_timer = RESPAWN_TIME;
                let (nx, ny) = safe_normalize(dx, dy);
                let mut payload = Vec::with_capacity(8);
                payload.extend_from_slice(&nx.to_le_bytes());
                payload.extend_from_slice(&ny.to_le_bytes());
                ruleste_plugin_api::host::emit(
                    player_id,
                    ruleste_plugin_api::host::EV_LAUNCH,
                    &payload,
                );
            }
            break;
        }
    });
}

fn safe_normalize(x: f32, y: f32) -> (f32, f32) {
    let len = (x * x + y * y).sqrt();
    if len < 1e-6 {
        (0.0, -1.0)
    } else {
        (x / len, y / len)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        // Real bumper atlas: `Idle00..44` loops while idle,
        // `Evil00..42` when in fire mode.
        let prefix = if st.fire_mode {
            "objects/Bumper/Evil"
        } else {
            "objects/Bumper/Idle"
        };
        let count = if st.fire_mode { 43 } else { 45 };
        let idx = (st.angle * 12.0) as usize % count;
        let frame = format!("{prefix}{idx:02}");
        draw_image(&frame, p.x, p.y, 0.0, 1.0, 1.0);
        if st.respawn_timer > 0.0 {
            // Brief cool-down tint: overlay the outline a little transparent.
            let alpha = (st.respawn_timer / RESPAWN_TIME * 255.0) as u8;
            let color = Color::new(0x40, 0x60, 0x80, alpha);
            ruleste_plugin_api::host::draw_image_color("objects/Bumper/outline", p.x, p.y, color);
        }
    });
}
