//! `bird` entity plugin.
//!
//! Mirrors `Bird.cs` (the big Seasonal Suit bird). The cutscene / room-script
//! system does not exist yet, so `mode` is read and remembered but the bird
//! only idles: `FlyAway` birds perch on a low arc near their spawn, and the
//! procedural body keeps a slow side-to-side idle so the chapter-opening
//! moments feel alive. Dialogue is a later host feature.

use ruleste_plugins_api::host::draw_rect;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

const BODY: Color = Color {
    r: 0x2a,
    g: 0x2a,
    b: 0x34,
    a: 0xff,
};
const BELLY: Color = Color {
    r: 0x50,
    g: 0x58,
    b: 0x68,
    a: 0xff,
};

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
        // Body (large), head, and beak, sized like the original's sprite box.
        draw_rect(bx - 18.0, by - 12.0, 36.0, 24.0, BODY);
        draw_rect(bx - 10.0, by - 6.0, 20.0, 12.0, BELLY);
        draw_rect(bx - 22.0, by - 22.0, 12.0, 12.0, BODY);
        draw_rect(
            bx - 26.0,
            by - 10.0,
            6.0,
            4.0,
            Color::new(0xd8, 0x60, 0x30, 0xff),
        );
    });
}
