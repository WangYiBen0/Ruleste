//! `floatingDebris` / `foregroundDebris` entity plugins.
//!
//! `floatingDebris` bobs and uses `scenery/debris`; `foregroundDebris` cycles
//! through `scenery/fgdebris/rock_a00..02` / `rock_b00..01`.

use ruleste_plugin_api::host::draw_image;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::EntityId;

#[derive(Clone, Copy, Debug)]
struct DebrisState {
    start_y: f32,
    timer: f32,
    foreground: bool,
}

impl Default for DebrisState {
    fn default() -> Self {
        DebrisState {
            start_y: 0.0,
            timer: 0.0,
            foreground: false,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<DebrisState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut DebrisState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, DebrisState::default) as *mut DebrisState;
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
    let foreground = spawn.get_str("_entity_type", "floatingDebris") == "foregroundDebris";
    entity.depth.set(if foreground { -999900 } else { -5 });
    with_state(id, |st| {
        st.start_y = y;
        st.foreground = foreground;
    });
}

pub fn update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;
    });
}

pub fn draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let p = entity.position.get();
        if st.foreground {
            // Cycle between rock_a / rock_b variants.
            let frame = if (st.timer as i32 / 2) % 2 == 0 {
                let idx = (st.timer * 2.0) as usize % 3;
                format!("scenery/fgdebris/rock_a{idx:02}")
            } else {
                let idx = (st.timer * 2.0) as usize % 2;
                format!("scenery/fgdebris/rock_b{idx:02}")
            };
            draw_image(&frame, p.x, p.y, 0.0, 1.0, 1.0);
        } else {
            // Bobbing single-frame debris (`scenery/debris`).
            let bob = (st.timer * 2.0).sin() * 2.0;
            draw_image("scenery/debris", p.x, p.y + bob, 0.0, 1.0, 1.0);
        }
        let _ = st.start_y;
    });
}
