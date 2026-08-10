#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `triggerSpikes` entity plugin.
//!
//! Mirrors `TriggerSpikes.cs`: spikes that stay retracted until Madeline gets
//! close, then snap out for ~1.15s before sliding away again. All four
//! orientations are served by one plugin. The extension is drawn as a
//! `danger/spikes` row whose length follows the opening percent; contact while
//! extended kills, matching the original's solid-when-out collider.

use ruleste_plugin_api::host::{self, die, draw_image, draw_rect, entities_by_type};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("triggerspikes");
ruleste_plugin_api::ruleste_entity_types!(
    "triggerSpikesUp",
    "triggerSpikesDown",
    "triggerSpikesLeft",
    "triggerSpikesRight",
);
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// `TriggerSpikes.cs` timings: opening/closing snap fast, the spikes stay out
/// for about a second (the "danger" window).
const EXTEND_TIME: f32 = 0.12;
const ACTIVE_TIME: f32 = 1.15;
const RETRACT_TIME: f32 = 0.2;
/// `playerAlpha` proximity radius that wakes the spikes.
const TRIGGER_RANGE: f32 = 48.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Dir {
    Up,
    Down,
    Left,
    Right,
}

impl Dir {
    fn from_type(t: &str) -> Dir {
        match t {
            "triggerSpikesDown" => Dir::Down,
            "triggerSpikesLeft" => Dir::Left,
            "triggerSpikesRight" => Dir::Right,
            _ => Dir::Up,
        }
    }
}

#[derive(Debug)]
enum Phase {
    Idle,
    Extending,
    Active,
    Retracting,
}

#[derive(Debug)]
struct TriggerState {
    dir: Dir,
    spike_type: String,
    size: f32,
    phase: Phase,
    timer: f32,
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<TriggerState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut TriggerState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, || unreachable!("state seeded at init")) as *mut TriggerState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let entity_type = spawn.get_str("_entity_type", "triggerSpikesUp");
    let dir = Dir::from_type(&entity_type);
    let size = match dir {
        Dir::Up | Dir::Down => spawn.get_float("width", 8.0),
        Dir::Left | Dir::Right => spawn.get_float("height", 8.0),
    };
    let entity = Entity::new(id);
    entity
        .position
        .set_xy(spawn.get_float("x", 0.0), spawn.get_float("y", 0.0));
    entity.depth.set(-1);
    STATES.with(|s| {
        s.borrow_mut().insert(
            id,
            TriggerState {
                dir,
                spike_type: spawn.get_str("type", "default"),
                size,
                phase: Phase::Idle,
                timer: 0.0,
            },
        );
    });
}

/// Extension/fade progress in [0,1].
fn extension(st: &TriggerState) -> f32 {
    match st.phase {
        Phase::Idle => 0.0,
        Phase::Extending => (st.timer / EXTEND_TIME).min(1.0),
        Phase::Active => 1.0,
        Phase::Retracting => 1.0 - (st.timer / RETRACT_TIME).min(1.0),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        match st.phase {
            Phase::Idle => {
                // Wake when the player wanders into range.
                let p = Entity::new(id).position.get();
                for player_id in entities_by_type("player") {
                    if !host::entity_alive(player_id) {
                        continue;
                    }
                    let pp = host::Position::new(player_id).get();
                    let dx = pp.x - p.x;
                    let dy = pp.y - p.y;
                    if dx * dx + dy * dy <= TRIGGER_RANGE * TRIGGER_RANGE {
                        st.phase = Phase::Extending;
                        st.timer = 0.0;
                        break;
                    }
                }
            }
            Phase::Extending => {
                st.timer += dt;
                if st.timer >= EXTEND_TIME {
                    st.phase = Phase::Active;
                    st.timer = 0.0;
                }
                kill_when_extended(id, st);
            }
            Phase::Active => {
                st.timer += dt;
                kill_when_extended(id, st);
                if st.timer >= ACTIVE_TIME {
                    st.phase = Phase::Retracting;
                    st.timer = 0.0;
                }
            }
            Phase::Retracting => {
                st.timer += dt;
                if st.timer >= RETRACT_TIME {
                    st.phase = Phase::Idle;
                    st.timer = 0.0;
                }
            }
        }
    });
}

/// Any overlap while the spikes are out is lethal (the original's collider is
/// solid during the active window).
fn kill_when_extended(id: EntityId, st: &TriggerState) {
    if extension(st) < 0.55 {
        return;
    }
    let p = Entity::new(id).position.get();
    let active_len = 5.0;
    let (sx, sy, sw, sh) = match st.dir {
        Dir::Up => (p.x, p.y - active_len, st.size, active_len),
        Dir::Down => (p.x, p.y, st.size, active_len),
        Dir::Left => (p.x - active_len, p.y, active_len, st.size),
        Dir::Right => (p.x, p.y, active_len, st.size),
    };
    for player_id in entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let overlap = pp.x + pox < sx + sw
            && pp.x + pox + pw > sx
            && pp.y + poy < sy + sh
            && pp.y + poy + ph > sy;
        if overlap {
            die();
        }
        break;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        let ext = extension(st);
        let tiles = (st.size / 8.0).max(1.0) as usize;
        // Perpendicular extent grows with `ext`; spikes sit on the same row.
        let len = 5.0 * ext;
        for j in 0..tiles {
            let frame = format!(
                "danger/spikes/{}_{}00",
                st.spike_type,
                match st.dir {
                    Dir::Up => "up",
                    Dir::Down => "down",
                    Dir::Left => "left",
                    Dir::Right => "right",
                }
            );
            let (cx, cy) = match st.dir {
                Dir::Up => (p.x + (j as f32 + 0.5) * 8.0, p.y - len),
                Dir::Down => (p.x + (j as f32 + 0.5) * 8.0, p.y + len),
                Dir::Left => (p.x - len, p.y + (j as f32 + 0.5) * 8.0),
                Dir::Right => (p.x + len, p.y + (j as f32 + 0.5) * 8.0),
            };
            draw_image(&frame, cx, cy, 0.0, 1.0, 1.0);
        }
        // A faint danger sliver so the hazard reads even at low alpha.
        let base = Color::new(0x60, 0x70, 0x78, (40.0 + 60.0 * ext) as u8);
        match st.dir {
            Dir::Up => draw_rect(p.x, p.y - 5.0 * ext, st.size, 5.0 * ext, base),
            Dir::Down => draw_rect(p.x, p.y, st.size, 5.0 * ext, base),
            Dir::Left => draw_rect(p.x - 5.0 * ext, p.y, 5.0 * ext, st.size, base),
            Dir::Right => draw_rect(p.x, p.y, 5.0 * ext, st.size, base),
        }
    });
}
