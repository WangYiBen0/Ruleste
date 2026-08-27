//! `bonfire` entity plugin.
//!
//! Mirrors `Bonfire.cs`: campfire with 3 modes (Unlit, Lit, Smoking).
//! - Unlit: just logs.
//! - Lit: logs + flickering flame.
//! - Smoking: logs + smoke particles/whisps.

use ruleste_plugins_api::host::draw_rect;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId};

const LOG_COLOR: Color = Color {
    r: 0x5a,
    g: 0x2d,
    b: 0x18,
    a: 0xff,
};
const SMOKE_COLOR: Color = Color {
    r: 0x80,
    g: 0x80,
    b: 0x90,
    a: 0xa0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    Unlit,
    Lit,
    Smoking,
}

#[derive(Debug, Default)]
struct FireState {
    timer: f32,
    mode: Mode,
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
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let mode_str = spawn.get_str("mode", "unlit");

    let mode = match mode_str.to_lowercase().as_str() {
        "lit" => Mode::Lit,
        "smoking" => Mode::Smoking,
        _ => Mode::Unlit,
    };

    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.depth.set(-5);

    with_state(id, |st| {
        st.mode = mode;
        st.timer = 0.0;
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
        // Log pile
        draw_rect(p.x - 10.0, p.y - 2.0, 20.0, 4.0, LOG_COLOR);

        match st.mode {
            Mode::Unlit => {}
            Mode::Lit => {
                let flicker = 1.0 + (st.timer * 9.0).sin() * 0.15;
                let sway = (st.timer * 7.0).sin() * 1.5;
                let outer_f = 6.0 * flicker;
                let inner_f = 4.0 * (1.0 + (st.timer * 12.0 + 1.3).sin() * 0.2);
                let core = Color {
                    r: 0xff,
                    g: 0xe0,
                    b: 0x60,
                    a: 0xf0,
                };
                let mid = Color {
                    r: 0xe0,
                    g: 0x80,
                    b: 0x20,
                    a: 0xe8,
                };
                let outer = Color {
                    r: 0x90,
                    g: 0x30,
                    b: 0x10,
                    a: 0xc0,
                };
                draw_rect(p.x - 5.0, p.y - inner_f, 10.0, inner_f, core);
                draw_rect(p.x - 8.0 + sway, p.y - outer_f, 16.0, outer_f, mid);
                draw_rect(
                    p.x - 10.0 + sway * 0.5,
                    p.y - outer_f * 1.3,
                    20.0,
                    outer_f * 1.3,
                    outer,
                );
            }
            Mode::Smoking => {
                let smoke_y = p.y - 4.0 - (st.timer * 15.0) % 12.0;
                let smoke_x = p.x + (st.timer * 5.0).sin() * 3.0;
                draw_rect(smoke_x - 2.0, smoke_y, 4.0, 4.0, SMOKE_COLOR);
            }
        }
    });
}
