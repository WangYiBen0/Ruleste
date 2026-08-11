//! `introCar` entity plugin.
//!
//! The crashed car Madeline and Theo sit on during the prologue (and a vanity
//! prop in the chapter-9 hub). The cutscenes and the mount behavior are host
//! scripting features; this plugin draws the car from the gameplay atlas
//! (body + wheels) and gives it a subtle idle sway.

use ruleste_plugin_api::host::draw_image;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::EntityId;

#[derive(Debug, Default)]
struct CarState {
    timer: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<CarState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut CarState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, CarState::default) as *mut CarState;
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
    entity.depth.set(1);
}

pub fn update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;
    });
}

pub fn draw(id: EntityId) {
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        let sway = (st.timer * 0.8).sin() * 0.5;
        // Real prologue car: body, then wheels under it.
        draw_image("scenery/car/body", p.x, p.y - 6.0 + sway, 0.0, 1.0, 1.0);
        draw_image("scenery/car/wheels", p.x, p.y + 2.0 + sway, 0.0, 1.0, 1.0);
    });
}
