//! `wire` scenery entity plugin.
//!
//! Draws a hanging cable between the entity position and its first node as a
//! quadratic Bézier curve sagged by 24px, mirroring `Celeste.Wire`. The curve
//! is submitted through the host's procedural draw-command list
//! (`host::draw_line`); the host renders the segments after the entity sprites.
//!
//! `above` selects the layer: `true` puts the wire behind everything
//! (depth -8500), otherwise it renders in front of the gameplay (depth 2000).

use std::cell::RefCell;

use ruleste_plugins_api::host::draw_line;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId, Vec2};

/// Wire line color: `Calc.HexToColor("595866")`.
const WIRE_COLOR: Color = Color {
    r: 0x59,
    g: 0x58,
    b: 0x66,
    a: 0xff,
};
const SAG: f32 = 24.0;
const SEGMENTS: usize = 16;

#[derive(Debug, Clone, Copy)]
struct WireState {
    from: Vec2,
    to: Vec2,
}

thread_local! {
    static STATES: RefCell<EntityState<WireState>> = RefCell::new(EntityState::new());
}

fn with_state(id: EntityId, f: impl FnOnce(&mut WireState)) {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, || WireState {
            from: Vec2::ZERO,
            to: Vec2::ZERO,
        }) as *mut WireState;
        f(unsafe { &mut *st });
    });
}

pub fn init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity = Entity::new(id);
    let from = Vec2::new(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    let to = spawn
        .get_node(0)
        .unwrap_or(Vec2::new(from.x + 16.0, from.y));
    // `above` in the foreground (2000) by default, behind (-8500) when set.
    let depth = if spawn.get_bool("above", false) {
        -8500
    } else {
        2000
    };
    entity.depth.set(depth);
    entity.position.set_xy(from.x, from.y);
    with_state(id, |st| {
        *st = WireState { from, to };
    });
}

pub fn update(_id: EntityId, _dt: f32) {}

pub fn draw(id: EntityId) {
    with_state(id, |st| {
        // Control point sags the cable: midpoint + (0, 24). Wind is omitted
        // for now (needs the level wind timer, added with audio/atmo).
        let mid = Vec2::new((st.from.x + st.to.x) * 0.5, (st.from.y + st.to.y) * 0.5);
        let control = Vec2::new(mid.x, mid.y + SAG);

        let mut prev = st.from;
        for i in 1..=SEGMENTS {
            let t = i as f32 / SEGMENTS as f32;
            let p = quad_bezier(st.from, control, st.to, t);
            draw_line(prev.x, prev.y, p.x, p.y, WIRE_COLOR);
            prev = p;
        }
    });
}

/// Quadratic Bézier point at `t`.
fn quad_bezier(a: Vec2, c: Vec2, b: Vec2, t: f32) -> Vec2 {
    let mt = 1.0 - t;
    let x = mt * mt * a.x + 2.0 * mt * t * c.x + t * t * b.x;
    let y = mt * mt * a.y + 2.0 * mt * t * c.y + t * t * b.y;
    Vec2::new(x, y)
}
