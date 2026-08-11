//! `bird` entity plugin.
//!
//! Mirrors `BirdNPC.cs` (the big Seasonal Suit bird). The cutscene / room-script
//! system does not exist yet, so `mode` is read and remembered but the bird
//! only idles: `FlyAway` birds perch on a low arc near their spawn. The body
//! is drawn from the real `characters/bird/crow00..` atlas loop.

use ruleste_plugin_api::host::draw_image;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::EntityId;

#[derive(Debug, Default)]
struct BirdState {
    timer: f32,
    mode: String,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<BirdState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut BirdState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, BirdState::default) as *mut BirdState;
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
    entity.depth.set(-11000);
    with_state(id, |st| {
        st.mode = spawn.get_str("mode", "Idle");
    });
}

pub fn update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;
    });
}

pub fn draw(id: EntityId) {
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        // Slow side-to-side idle shift; FlyAway birds add a gentle waft upward.
        let shift = (st.timer * 0.6).sin() * 4.0;
        let waft = if st.mode == "FlyAway" {
            st.timer * 6.0
        } else {
            0.0
        };
        let bx = p.x + shift;
        let by = p.y - waft;
        let idx = (st.timer * 6.0) as usize % 16;
        let frame = format!("characters/bird/crow{idx:02}");
        draw_image(&frame, bx, by, 0.0, 1.0, 1.0);
    });
}
