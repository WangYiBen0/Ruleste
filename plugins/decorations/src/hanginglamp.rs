//! `hanginglamp` decoration plugin.
//!
//! Mirrors `HangingLamp.cs`: a lamp hanging from the ceiling by a chain of
//! length `height`. The `objects/hanginglamp` sheet (top cap, one chain link,
//! lamp) is blitted at the bottom; the rest of the chain is drawn as a line.

use ruleste_plugins_api::host;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

/// The lamp sheet is 8x24: top cap (0-8), chain link (8-16), lamp (16-24).
const SHEET_H: f32 = 24.0;

#[derive(Clone, Copy, Debug)]
struct Lamp {
    length: f32,
}

impl Default for Lamp {
    fn default() -> Lamp {
        Lamp { length: 16.0 }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<Lamp>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut Lamp) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, Lamp::default) as *mut Lamp;
        let result = unsafe { &mut *st };
        f(result)
    })
}

pub fn init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let length = spawn.get_float("height", 16.0).max(16.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x + 4.0, y);
    entity.depth.set(2000);
    with_state(id, |st| {
        st.length = length;
    });
}

pub fn update(_id: EntityId, _dt: f32) {}

pub fn draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let p = entity.position.get();
        let chain_len = (st.length - (SHEET_H - 8.0)).max(0.0);
        // Chain from the ceiling down to where the lamp sheet starts.
        if chain_len > 0.0 {
            host::draw_line(p.x, p.y, p.x, p.y + chain_len, Color::new(90, 90, 90, 255));
        }
        // Lamp sheet centered so its bottom (the lamp) sits at `p.y + length`.
        let center_y = p.y + st.length - SHEET_H * 0.5;
        host::draw_image("objects/hanginglamp", p.x, center_y, 0.0, 1.0, 1.0);
    });
}
