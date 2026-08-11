//! `flutterbird` entity plugin.
//!
//! Mirrors `FlutterBird.cs`: a small bluebird fluttering through the air on a
//! figure-eight path. The flock-roaming logic is reduced to the loop the
//! original starts each bird on (a horizontal sine weave with a gentle bob);
//! the wingbeat is timed so both wings can share one clock. The sprite frames
//! come from the gameplay atlas (`scenery/flutterbird/{flap00,flap01,idle00}`).

use ruleste_plugin_api::host::draw_image;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{EntityId, Vec2};

/// Horizontal weave amplitude / period; vertical bob felt like the original's.
const SWAY_AMP: f32 = 24.0;
const SWAY_SPEED: f32 = 1.4;
const BOB_AMP: f32 = 8.0;
const BOB_SPEED: f32 = 2.2;
const FLAP_HZ: f32 = 8.0;

#[derive(Debug, Default)]
struct FlutterState {
    origin: Vec2,
    timer: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<FlutterState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut FlutterState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, FlutterState::default) as *mut FlutterState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

pub fn init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.depth.set(-10000);
    with_state(id, |st| {
        st.origin = Vec2::new(x, y);
    });
}

pub fn update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;
        let px = st.origin.x + (st.timer * SWAY_SPEED).sin() * SWAY_AMP;
        let py = st.origin.y + (st.timer * BOB_SPEED).cos() * BOB_AMP;
        Entity::new(id).position.set_xy(px, py);
    });
}

pub fn draw(id: EntityId) {
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        let frame = if (st.timer * FLAP_HZ) as i32 % 2 == 0 {
            "scenery/flutterbird/flap00"
        } else {
            "scenery/flutterbird/flap01"
        };
        draw_image(frame, p.x, p.y, 0.0, 1.0, 1.0);
    });
}
