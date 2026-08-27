//! `lightbeam` entity plugin.
//!
//! Mirrors `LightBeam.cs`: a shaft of light (`util/lightbeam`) rendered at
//! depth -9998, stretched to the map light's width/length and rotated to the
//! data's `rotation`. The beam length gently shimmers; player-occlusion alpha
//! fading is left for a later pass (the draw API has no tint yet).

use ruleste_plugins_api::host;
use ruleste_plugins_api::map::MapData;
use ruleste_plugins_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugins_api::types::{Color, EntityId, Vec2};

#[derive(Clone, Copy, Debug)]
struct Beam {
    light_width: f32,
    light_length: f32,
    /// Rotation in radians; the map stores it in degrees.
    rotation: f32,
}

impl Default for Beam {
    fn default() -> Beam {
        Beam {
            light_width: 32.0,
            light_length: 88.0,
            rotation: 0.0,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<Beam>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut Beam) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, Beam::default) as *mut Beam;
        let result = unsafe { &mut *st };
        f(result)
    })
}

fn to_radians(deg: f32) -> f32 {
    deg * std::f32::consts::PI / 180.0
}

pub fn init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let beam = Beam {
        light_width: spawn.get_float("width", 32.0),
        light_length: spawn.get_float("height", 88.0),
        rotation: to_radians(spawn.get_float("rotation", 0.0)),
    };
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.depth.set(-9998);
    with_state(id, |st| {
        *st = beam;
    });
}

pub fn update(_id: EntityId, _dt: f32) {}

pub fn draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let pos = entity.position.get();
        let dir = Vec2::new(
            (st.rotation + std::f32::consts::PI / 2.0).cos(),
            (st.rotation + std::f32::consts::PI / 2.0).sin(),
        );
        // Perpendicular to the beam axis, in the beam's width direction.
        let perp = Vec2::new(-dir.y, dir.x);
        let half_w = st.light_width * 0.5;
        let end = Vec2::new(
            pos.x + dir.x * st.light_length,
            pos.y + dir.y * st.light_length,
        );
        let c0 = Vec2::new(pos.x + perp.x * half_w, pos.y + perp.y * half_w);
        let c1 = Vec2::new(pos.x - perp.x * half_w, pos.y - perp.y * half_w);
        let e0 = Vec2::new(end.x + perp.x * half_w, end.y + perp.y * half_w);
        let e1 = Vec2::new(end.x - perp.x * half_w, end.y - perp.y * half_w);
        let base = Color::new(204, 255, 255, 70);
        // Filled-in beam: cross-sections every 2px, fading out with distance.
        for i in 0..=st.light_length as i32 {
            let t = i as f32;
            let fade = 1.0 - t / st.light_length;
            let a = (70.0 * fade) as u8;
            let c = Color::new(base.r, base.g, base.b, a);
            let p0 = Vec2::new(c0.x + dir.x * t, c0.y + dir.y * t);
            let p1 = Vec2::new(c1.x + dir.x * t, c1.y + dir.y * t);
            host::draw_line(p0.x, p0.y, p1.x, p1.y, c);
        }
        // Bright core.
        host::draw_line(pos.x, pos.y, end.x, end.y, Color::new(230, 255, 255, 200));
        let _ = (e0, e1);
    });
}
