#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `trackSpinner` entity plugin.
//!
//! Mirrors `TrackSpinner.cs`: a dust spinner that glides along a fixed track
//! through its node chain instead of staying put. It travels the whole path
//! back and forth at constant speed, always lethal on contact. The crystal is
//! drawn as the rotating `danger/dustcreature/center` frame, which the atlas
//! does provide.

use ruleste_plugin_api::host::{self, die, draw_image, entities_by_type};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{EntityId, Vec2};
use std::vec::Vec;

ruleste_plugin_api::ruleste_meta!("trackspinner");
ruleste_plugin_api::ruleste_entity_types!("trackSpinner");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// `TrackSpinner.MoveSpeed` — constant travel speed in pixels/second.
const SPEED: f32 = 60.0;
/// Covering AABB for the `Circle(6)` collider.
const KILL_W: f32 = 12.0;
const KILL_H: f32 = 12.0;

#[derive(Debug, Default)]
struct TrackState {
    /// Path vertices: spawn position followed by its node chain.
    path: Vec<Vec2>,
    /// Cumulative segment lengths (index aligned with `path` minus one).
    seg_len: Vec<f32>,
    total: f32,
    /// Distance travelled along the path (ping-pong).
    dist: f32,
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

/// World position at `distance` along the polyline.
fn point_at(path: &[Vec2], seg_len: &[f32], mut d: f32) -> Vec2 {
    for i in 0..seg_len.len() {
        let seg = seg_len[i];
        if d <= seg {
            let a = path[i];
            let b = path[i + 1];
            let t = if seg > 0.0 { d / seg } else { 0.0 };
            return Vec2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        }
        d -= seg;
    }
    *path.last().expect("non-empty path")
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity
        .hitbox
        .set(KILL_W, KILL_H, -KILL_W * 0.5, -KILL_H * 0.5);
    entity.depth.set(-50);

    with_state(id, |st| {
        st.path.push(Vec2::new(x, y));
        st.path.extend(spawn.nodes().iter().copied());
        let mut total = 0.0;
        for w in st.path.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
            st.seg_len.push(len);
            total += len;
        }
        st.total = total;
        if total == 0.0 {
            st.seg_len.push(1.0);
            st.total = 1.0;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.angle += dt * 0.6;
        // Ping-pong travel along the track.
        st.dist = (st.dist + dt * SPEED) % (st.total * 2.0);
        let d = if st.dist > st.total {
            st.total * 2.0 - st.dist
        } else {
            st.dist
        };
        let pos = point_at(&st.path, &st.seg_len, d);
        let entity = Entity::new(id);
        entity.position.set_xy(pos.x, pos.y);

        for player_id in entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
            let overlap = pp.x + pox < pos.x + KILL_W * 0.5
                && pp.x + pox + pw > pos.x - KILL_W * 0.5
                && pp.y + poy < pos.y + KILL_H * 0.5
                && pp.y + poy + ph > pos.y - KILL_H * 0.5;
            if overlap {
                die();
            }
            break;
        }
    });
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
