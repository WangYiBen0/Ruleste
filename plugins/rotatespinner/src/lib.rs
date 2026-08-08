//! `rotateSpinner` hazard plugin.
//!
//! Mirrors `RotateSpinner.cs`: a danger circle with no sprite that orbits a
//! fixed center node once every 1.8s, killing Madeline on contact. Unlike
//! `DustStaticSpinner` it draws nothing, so it stays invisible exactly like
//! the original.

use std::f32::consts::{FRAC_PI_2, TAU};

use ruleste_plugin_api::host::{self, die, entities_by_type};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{spawn_data, Entity, EntityState};
use ruleste_plugin_api::types::{EntityId, Vec2};

ruleste_plugin_api::ruleste_meta!("rotatespinner");
ruleste_plugin_api::ruleste_entity_types!("rotateSpinner");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// `Collider = new Circle(6f)` as a covering AABB (centered on position).
const KILL_W: f32 = 12.0;
const KILL_H: f32 = 12.0;

/// `RotationTime` — one full orbit in seconds.
const ROTATION_TIME: f32 = 1.8;

/// `Angle` lerp bounds: `MathHelper.Lerp(4.712389f, -PI/2, rotationPercent)`.
const ANGLE_START: f32 = 3.0 * FRAC_PI_2; // 4.712389 rad
const ANGLE_END: f32 = -FRAC_PI_2;

#[derive(Debug)]
struct SpinnerState {
    /// `center` node the entity orbits around.
    center: Vec2,
    /// Orbit radius `(Position - center).Length()`.
    length: f32,
    /// `rotationPercent`, in [0, 1), advanced at `dt / 1.8f`.
    percent: f32,
    /// `data.Bool("clockwise")`.
    clockwise: bool,
}

impl Default for SpinnerState {
    fn default() -> SpinnerState {
        SpinnerState {
            center: Vec2::ZERO,
            length: 0.0,
            percent: 0.0,
            clockwise: false,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<SpinnerState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut SpinnerState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, SpinnerState::default) as *mut SpinnerState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

#[no_mangle]
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
        st.center = spawn.get_node(0).unwrap_or(Vec2::new(x, y));
        st.clockwise = spawn.get_bool("clockwise", false);
        let dx = x - st.center.x;
        let dy = y - st.center.y;
        st.length = (dx * dx + dy * dy).sqrt();
        // `rotationPercent = Percent(WrapAngle(Calc.Angle(center, Position)), -PI/2, 4.712389)`.
        let angle = dy.atan2(dx).rem_euclid(TAU);
        st.percent = ((angle - ANGLE_END) / (ANGLE_START - ANGLE_END)).rem_euclid(1.0);
    });
}

#[no_mangle]
pub extern "C" fn ruleste_entity_draw(_id: EntityId) {}

#[no_mangle]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        if st.clockwise {
            st.percent -= dt / ROTATION_TIME;
            st.percent += 1.0;
        } else {
            st.percent += dt / ROTATION_TIME;
        }
        st.percent %= 1.0;

        let angle = ANGLE_START + (ANGLE_END - ANGLE_START) * st.percent;
        let px = st.center.x + angle.cos() * st.length;
        let py = st.center.y + angle.sin() * st.length;
        let entity = Entity::new(id);
        entity.position.set_xy(px, py);

        for player_id in entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
            let prx = pp.x + pox;
            let pry = pp.y + poy;
            let overlap = prx < px + KILL_W * 0.5
                && prx + pw > px - KILL_W * 0.5
                && pry < py + KILL_H * 0.5
                && pry + ph > py - KILL_H * 0.5;
            if overlap {
                die();
            }
            break;
        }
    });
}
