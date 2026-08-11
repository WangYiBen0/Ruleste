#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `dreamBlock` entity plugin.
//!
//! Mirrors `DreamBlock.cs`: a solid block standing as a riding platform,
//! rendered in its disabled state (no dream dash yet) as a dark teal slab with
//! drifting light-grey particles and a wobbling border line plus corner
//! blocks. Dream-dash activation and node movement are left for later passes.

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("dream-block");
ruleste_plugin_api::ruleste_entity_types!("dreamBlock");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// Disabled-state palette from the original.
const BACK: Color = Color::new(0x1f, 0x2e, 0x2d, 255);
const LINE: Color = Color::new(0x6a, 0x84, 0x80, 255);

#[derive(Debug)]
struct DreamState {
    w: f32,
    h: f32,
    timer: f32,
    particles: Vec<(f32, f32, f32)>,
}

impl Default for DreamState {
    fn default() -> DreamState {
        DreamState {
            w: 8.0,
            h: 8.0,
            timer: 0.0,
            particles: Vec::new(),
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<DreamState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut DreamState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, DreamState::default) as *mut DreamState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

/// Deterministic LCG so the particle layout survives hot reloads.
fn lcg(seed: &mut u32) -> f32 {
    *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    (*seed >> 8) as f32 / (1u32 << 24) as f32
}

fn make_particles(w: f32, h: f32) -> Vec<(f32, f32, f32)> {
    let count = (w / 8.0 * (h / 8.0) * 0.7).round() as usize;
    let mut seed = 0x2d_5a_11u32;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let x = lcg(&mut seed) * w;
        let y = lcg(&mut seed) * h;
        // Layer 0,1,2 with 1/2/3 weights like the original Choose(0,1,1,2,2,2).
        let r = lcg(&mut seed);
        let layer = if r < 1.0 / 6.0 {
            0
        } else if r < 3.0 / 6.0 {
            1
        } else {
            2
        };
        out.push((x, y, layer as f32));
    }
    out
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let w = spawn.get_float("width", 8.0);
    let h = spawn.get_float("height", 8.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.hitbox.set(w, h, 0.0, 0.0);
    entity.depth.set(-11000);
    entity.collision.solid(true);
    with_state(id, |st| {
        st.w = w;
        st.h = h;
        st.particles = make_particles(w, h);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        st.timer += dt;
    });
}

/// Particle brightness for a layer: `0.5 + layer/2 * 0.5` of light grey.
fn particle_color(layer: f32) -> Color {
    let v = (0.5 + layer / 2.0 * 0.5) * 255.0;
    Color::new(v as u8, v as u8, v as u8, 200)
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let p = entity.position.get();
        let (w, h) = (st.w, st.h);
        host::draw_rect(p.x, p.y, w, h, BACK);
        // Drifting particles inside the slab (simplified: no parallax).
        for &(px, py, layer) in &st.particles {
            let dx = ((st.timer * (1.0 + layer)) * 10.0 + px).sin() * 1.5;
            let dy = ((st.timer * (1.0 + layer)) * 7.0 + py).sin() * 1.5;
            host::draw_rect(
                p.x + px + dx,
                p.y + py + dy,
                2.0,
                2.0,
                particle_color(layer),
            );
        }
        // Wobble border (approximated as straight edges) and corner blocks.
        host::draw_line(p.x, p.y, p.x + w, p.y, LINE);
        host::draw_line(p.x + w, p.y, p.x + w, p.y + h, LINE);
        host::draw_line(p.x + w, p.y + h, p.x, p.y + h, LINE);
        host::draw_line(p.x, p.y + h, p.x, p.y, LINE);
        host::draw_rect(p.x, p.y, 2.0, 2.0, LINE);
        host::draw_rect(p.x + w - 2.0, p.y, 2.0, 2.0, LINE);
        host::draw_rect(p.x, p.y + h - 2.0, 2.0, 2.0, LINE);
        host::draw_rect(p.x + w - 2.0, p.y + h - 2.0, 2.0, 2.0, LINE);
    });
}
