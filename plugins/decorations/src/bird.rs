//! `bird` entity plugin.
//!
//! Mirrors `Bird.cs` (the big Seasonal Suit bird). The cutscene / room-script
//! system does not exist yet, so `mode` is read and remembered but the bird
//! only idles: `FlyAway` birds perch on a low arc near their spawn, and the
//! procedural body keeps a slow side-to-side idle so the chapter-opening
//! moments feel alive. Dialogue is a later host feature.

use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{EntityId, Vec2};

#[derive(Debug, Default)]
struct BirdState {
    timer: f32,
    mode: String,
    base: Vec2,
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
    let (x, y) = (spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    entity.position.set_xy(x, y);
    entity.depth.set(-11000);
    // The `bird` SpriteBank sprite renders the body; the host animates the idle
    // crow pose each frame.
    entity.sprite.set_bank("bird");
    entity.sprite.play("idle");
    with_state(id, |st| {
        st.mode = spawn.get_str("mode", "Idle");
        st.base = Vec2::new(x, y);
    });
}

pub fn update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;
        // Slow side-to-side idle shift; FlyAway birds add a gentle waft upward.
        let shift = (st.timer * 0.6).sin() * 4.0;
        let waft = if st.mode == "FlyAway" {
            st.timer * 6.0
        } else {
            0.0
        };
        Entity::new(id)
            .position
            .set_xy(st.base.x + shift, st.base.y - waft);
    });
}

// The bird is drawn by the host SpriteBank renderer via `entity.sprite`.
pub fn draw(_id: EntityId) {}
