//! `cobweb` decoration plugin.
//!
//! Mirrors `Cobweb.cs`: a depth -1 web drawn as line segments along a
//! quadratic Bezier between two anchors, with optional offshoot strands.
//! The curve midpoint sways vertically with a wave timer, and the edge
//! strands use a slightly darkened color.

use ruleste_plugin_api::host::{self, draw_line};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{spawn_data, Entity, EntityState};
use ruleste_plugin_api::types::{Color, EntityId, Vec2};

ruleste_plugin_api::ruleste_meta!("cobweb");
ruleste_plugin_api::ruleste_entity_types!("cobweb");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// `AreaData.CobwebColor` default `#696a6a`.
const COLOR: Color = Color {
    r: 105,
    g: 106,
    b: 106,
    a: 255,
};
/// `Color.Lerp(COLOR, #0f0e17, 0.2)`.
const EDGE: Color = Color {
    r: 87,
    g: 88,
    b: 89,
    a: 255,
};

#[derive(Debug)]
struct CobwebState {
    anchor_b: Vec2,
    offshoots: Vec<(Vec2, f32)>,
    wave_timer: f32,
}

impl Default for CobwebState {
    fn default() -> CobwebState {
        CobwebState {
            anchor_b: Vec2::ZERO,
            offshoots: Vec::new(),
            wave_timer: 0.0,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<CobwebState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut CobwebState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, CobwebState::default) as *mut CobwebState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

/// Point at `t` on the quadratic Bezier `a -> c -> b` (`SimpleCurve`).
fn bezier(a: Vec2, control: Vec2, b: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    let x = u * u * a.x + 2.0 * u * t * control.x + t * t * b.x;
    let y = u * u * a.y + 2.0 * u * t * control.y + t * t * b.y;
    Vec2::new(x, y)
}

#[no_mangle]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.depth.set(-1);
    with_state(id, |st| {
        let nodes = spawn.nodes();
        st.anchor_b = nodes.first().copied().unwrap_or(Vec2::new(x, y));
        st.offshoots = nodes
            .iter()
            .skip(1)
            .enumerate()
            .map(|(i, n)| (*n, 0.3 + (id as f32 + i as f32) * 0.13 % 0.4))
            .collect();
        // Deterministic `Calc.Random.NextFloat()` for the wave phase.
        st.wave_timer = (id as f32) * 0.37;
    });
}

#[no_mangle]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.wave_timer += dt;
    });
}

#[no_mangle]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let a = host::Position::new(id).get();
        let b = st.anchor_b;
        let control = Vec2::new(
            (a.x + b.x) * 0.5,
            (a.y + b.y) * 0.5 + 8.0 + st.wave_timer.sin() * 4.0,
        );
        draw_cobweb(a, b, control, 12, true, &st.offshoots);
    });
}

fn draw_cobweb(
    a: Vec2,
    b: Vec2,
    control: Vec2,
    steps: usize,
    draw_offshoots: bool,
    offshoots: &[(Vec2, f32)],
) {
    if draw_offshoots {
        for (offshoot, ending) in offshoots {
            let point = bezier(a, control, b, *ending);
            let off_control = Vec2::new(
                (offshoot.x + point.x) * 0.5,
                (offshoot.y + point.y) * 0.5 + 8.0,
            );
            draw_cobweb(*offshoot, point, off_control, 4, false, &[]);
        }
    }
    let mut prev = a;
    for j in 1..=steps {
        let t = j as f32 / steps as f32;
        let point = bezier(a, control, b, t);
        let color = if j <= 2 || j >= steps.saturating_sub(1) {
            EDGE
        } else {
            COLOR
        };
        draw_line(prev.x, prev.y, point.x, point.y, color);
        // `vector = point + (vector - point).SafeNormalize()` — nudge away.
        let dx = prev.x - point.x;
        let dy = prev.y - point.y;
        let len = (dx * dx + dy * dy).sqrt().max(0.0001);
        prev = Vec2::new(point.x + dx / len, point.y + dy / len);
    }
}
