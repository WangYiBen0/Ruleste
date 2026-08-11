//! `bonfire` entity plugin.
//!
//! Mirrors the `Bonfire` campfire found in the Old Site: a log pile whose
//! flame flickers on a looping clock. Ember particles need an emitter system
//! the host does not have, so the fire is drawn from the real
//! `objects/campfire/fire00..15` atlas loop and the log pile stays procedural.

use ruleste_plugin_api::host::{draw_image, draw_rect};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

const LOG_COLOR: Color = Color {
    r: 0x5a,
    g: 0x2d,
    b: 0x18,
    a: 0xff,
};
const FIRE_FPS: f32 = 12.0;

#[derive(Debug, Default)]
struct FireState {
    timer: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<FireState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut FireState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, FireState::default) as *mut FireState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

pub fn init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    entity
        .position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    entity.depth.set(-5);
}

pub fn update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;
    });
}

pub fn draw(id: EntityId) {
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        // Log pile (procedural — there's no atlas log frame).
        draw_rect(p.x - 10.0, p.y - 2.0, 20.0, 4.0, LOG_COLOR);
        // Flame loop drawn from the real gameplay atlas.
        let idx = (st.timer * FIRE_FPS) as usize % 16;
        let frame = format!("objects/campfire/fire{idx:02}");
        draw_image(&frame, p.x, p.y - 10.0, 0.0, 1.0, 1.0);
    });
}
