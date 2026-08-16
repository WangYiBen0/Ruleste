#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `trackSpinner` entity plugin.
//!
//! Mirrors `TrackSpinner.cs`: a lethal spinner that glides back and forth
//! between its spawn point and `Nodes[0]` with a `SineInOut` ease. Progress is
//! `At` per leg: `dt / MoveTimes[speed]`, and the spinner pauses
//! `PauseTimes[speed]` at each end. `startCenter` begins the trip at 50%. Speed
//! key: Slow/Normal/Fast (default Normal). Contact kills the player.

use ruleste_plugin_api::host::{self, die, draw_image, entities_by_type};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{EntityId, Vec2};

ruleste_plugin_api::ruleste_meta!("track-spinner");
ruleste_plugin_api::ruleste_entity_types!("trackSpinner");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// `TrackSpinner.PauseTimes` — seconds paused at each end per speed tier.
const PAUSE_TIMES: [f32; 3] = [0.3, 0.2, 0.6];
/// `TrackSpinner.MoveTimes` — seconds per leg per speed tier.
const MOVE_TIMES: [f32; 3] = [0.9, 0.4, 0.3];

/// Killing shape: the original uses `ColliderList(Circle(6), Hitbox(16,4,-8,-3))`
/// — a radius-6 circle centered on the spinner plus a thin horizontal band at
/// its lower half. The AABB here covers the circle; the manual kill check in
/// update() applies the true circle + band shapes.
const CIRCLE_R: f32 = 6.0;
const HIT_W: f32 = 16.0;
const HIT_H: f32 = 4.0;
const HIT_OX: f32 = -8.0;
const HIT_OY: f32 = -3.0;

#[derive(Debug, Default)]
struct TrackState {
    start: Vec2,
    end: Vec2,
    speed_tier: usize,
    percent: f32,
    up: bool,
    pause: f32,
    angle: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<TrackState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut TrackState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, TrackState::default) as *mut TrackState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

fn sine_in_out(t: f32) -> f32 {
    ((t * std::f32::consts::PI).cos() * -0.5 + 0.5).clamp(0.0, 1.0)
}

fn position_at(st: &TrackState) -> Vec2 {
    Vec2::new(
        st.start.x + (st.end.x - st.start.x) * sine_in_out(st.percent),
        st.start.y + (st.end.y - st.start.y) * sine_in_out(st.percent),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let entity = Entity::new(id);
    let (sw, sh, sox, soy) = (HIT_W, HIT_H, HIT_OX, HIT_OY);
    entity.hitbox.set(sw, sh, sox, soy);
    entity.depth.set(-50);
    with_state(id, |st| {
        st.start = Vec2::new(x, y);
        st.end = spawn.get_node(0).unwrap_or(Vec2::new(x, y));
        st.speed_tier = match spawn.get_str("speed", "Normal").as_str() {
            "Slow" => 0,
            "Fast" => 2,
            _ => 1,
        };
        st.percent = if spawn.get_bool("startCenter", false) {
            0.5
        } else {
            0.0
        };
        st.up = st.percent != 1.0;
        let start_pos = position_at(st);
        entity.position.set_xy(start_pos.x, start_pos.y);
    });
}

/// Kill check against the circle + band colliders from `TrackSpinner.cs`.
fn player_killed(pos: Vec2) -> bool {
    for player_id in entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let pl = pp.x + pox;
        let pt = pp.y + poy;
        let pr = pl + pw;
        let pb = pt + ph;
        // Band collider: `Hitbox(16, 4, -8, -3)`.
        let band_l = pos.x + HIT_OX;
        let band_t = pos.y + HIT_OY;
        let band_r = band_l + HIT_W;
        let band_b = band_t + HIT_H;
        if pl < band_r && pr > band_l && pt < band_b && pb > band_t {
            return true;
        }
        // Circle collider: nearest point on the player AABB to the center.
        let cx = pl.max(pos.x).min(pr);
        let cy = pt.max(pos.y).min(pb);
        let ddx = pos.x - cx;
        let ddy = pos.y - cy;
        if ddx * ddx + ddy * ddy < CIRCLE_R * CIRCLE_R {
            return true;
        }
    }
    false
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.angle += dt * 0.6;
        if st.pause > 0.0 {
            st.pause -= dt;
            if st.pause <= 0.0 {
                st.up = !st.up;
            }
        } else {
            let target = if st.up { 1.0 } else { 0.0 };
            st.percent = approach(st.percent, target, dt / MOVE_TIMES[st.speed_tier]);
            if (st.percent >= 1.0 && st.up) || (st.percent <= 0.0 && !st.up) {
                st.percent = target;
                st.pause = PAUSE_TIMES[st.speed_tier];
            }
        }
        let pos = position_at(st);
        let entity = Entity::new(id);
        entity.position.set_xy(pos.x, pos.y);

        if player_killed(pos) {
            die();
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
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        draw_image(
            "danger/dustcreature/center00",
            p.x,
            p.y,
            st.angle.to_degrees(),
            1.0,
            1.0,
        );
    });
}
