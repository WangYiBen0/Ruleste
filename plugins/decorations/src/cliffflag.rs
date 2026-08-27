//! `cliffflag` entity plugin.
//!
//! Mirrors `CliffFlags.cs` + `Flagline.cs`: a rope of small hanging flags
//! strung between the entity's position and its first node. The rope is a
//! quadratic curve drooping between the two anchors, swaying with a wave
//! timer; alternate segments carry a colored flag rectangle with a highlight
//! edge and gray pins at the ends. The flag colors and sizes are randomized
//! per line (deterministically, seeded from the entity id) like the original.

use ruleste_plugins_api::host;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId, Vec2};

const FLAG_COLORS: [Color; 4] = [
    Color::new(0xd8, 0x5f, 0x2f, 255),
    Color::new(0xd8, 0x2f, 0x63, 255),
    Color::new(0x2f, 0xd8, 0xa2, 255),
    Color::new(0xd8, 0xd6, 0x2f, 255),
];

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

fn highlight(color: Color) -> Color {
    Color::new(
        lerp(color.r, 255, 0.1),
        lerp(color.g, 255, 0.1),
        lerp(color.b, 255, 0.1),
        255,
    )
}

const LINE_COLOR: Color = Color::new(127, 127, 160, 255);
const PIN_COLOR: Color = Color::new(127, 127, 127, 255);

const CLOTH_COUNT: usize = 10;

/// Deterministic per-line parameters (color, height, length, step), seeded
/// from the entity id, so hot reloads keep the same layout.
#[derive(Debug, Clone, Copy)]
struct Cloth {
    color: usize,
    height: f32,
    length: f32,
    step: f32,
}

fn make_clothes(id: EntityId) -> [Cloth; CLOTH_COUNT] {
    let mut seed = id ^ 0x9e37_79b9;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    std::array::from_fn(|_| Cloth {
        color: (next() as usize) % FLAG_COLORS.len(),
        height: 10.0,
        length: 10.0,
        step: 10.0,
    })
}

#[derive(Debug)]
struct FlaglineState {
    clothes: [Cloth; CLOTH_COUNT],
    wave_timer: f32,
    to: Vec2,
}

impl Default for FlaglineState {
    fn default() -> FlaglineState {
        FlaglineState {
            clothes: [Cloth {
                color: 0,
                height: 10.0,
                length: 10.0,
                step: 10.0,
            }; CLOTH_COUNT],
            wave_timer: 0.0,
            to: Vec2::ZERO,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<FlaglineState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut FlaglineState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, FlaglineState::default) as *mut FlaglineState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

/// A point on the quadratic bezier `p0 -> p2` with control point `p1`.
fn bezier(p0: Vec2, p1: Vec2, p2: Vec2, t: f32) -> Vec2 {
    let a = 1.0 - t;
    Vec2::new(
        a * a * p0.x + 2.0 * a * t * p1.x + t * t * p2.x,
        a * a * p0.y + 2.0 * a * t * p1.y + t * t * p2.y,
    )
}

/// Renders the drooping flag line. `from` is the entity position, `to` the
/// node; the curve is drawn left to right regardless of their order.
fn draw_flagline(st: &FlaglineState, from: Vec2, to: Vec2) {
    let (left, right) = if from.x < to.x {
        (from, to)
    } else {
        (to, from)
    };
    let dx = right.x - left.x;
    let dy = right.y - left.y;
    let dist = dx.hypot(dy);
    let droop = dist / 8.0;
    let wave = (st.wave_timer).sin() * droop * 0.3;
    let mid = Vec2::new(
        (left.x + right.x) / 2.0,
        (left.y + right.y) / 2.0 + droop + wave,
    );

    let mut p3 = left;
    let mut num3 = 0.0;
    let mut num4 = 0usize;
    let mut flag = false;
    while num3 < 1.0 {
        let cloth = st.clothes[num4 % CLOTH_COUNT];
        num3 += if flag { cloth.length } else { cloth.step } / dist;
        num3 = num3.min(1.0);
        let p4 = bezier(left, mid, right, num3);
        host::draw_line(p3.x, p3.y, p4.x, p4.y, LINE_COLOR);
        if flag {
            let droop2 = cloth.length * 0.2;
            let wave2 = (st.wave_timer * 2.0 + num3).sin() * droop2 * 0.4;
            let flag_mid = Vec2::new((p3.x + p4.x) / 2.0, (p3.y + p4.y) / 2.0 + droop2 + wave2);
            let mut prev = p3;
            let steps = cloth.length.max(1.0).ceil() as u32;
            for i in 1..=steps {
                let t = i as f32 / steps as f32;
                let point = bezier(p3, flag_mid, p4, t);
                if point.x != prev.x {
                    host::draw_rect(
                        prev.x,
                        prev.y,
                        point.x - prev.x + 1.0,
                        cloth.height,
                        FLAG_COLORS[cloth.color],
                    );
                }
                prev = point;
            }
            let hi = highlight(FLAG_COLORS[cloth.color]);
            host::draw_rect(p3.x, p3.y, 1.0, cloth.height, hi);
            host::draw_rect(p4.x, p4.y, 1.0, cloth.height, hi);
            host::draw_rect(p3.x, p3.y - 1.0, 1.0, 3.0, PIN_COLOR);
            host::draw_rect(p4.x, p4.y - 1.0, 1.0, 3.0, PIN_COLOR);
            num4 += 1;
        }
        p3 = p4;
        flag = !flag;
    }
}

pub fn init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.depth.set(8999);
    with_state(id, |st| {
        st.clothes = make_clothes(id);
        st.to = spawn.get_node(0).unwrap_or(Vec2::new(x, y));
    });
}

pub fn update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.wave_timer += dt;
    });
}

pub fn draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let from = entity.position.get();
        draw_flagline(st, from, st.to);
    });
}
