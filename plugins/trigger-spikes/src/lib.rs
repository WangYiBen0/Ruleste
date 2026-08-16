#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `triggerSpikes` entity plugin.
//!
//! Mirrors `TriggerSpikes.cs`: each 4px-wide cell starts retracted and inert.
//! The first touch triggers that cell: a 0.4s delay, then the tentacle extends
//! at 8/s toward the player (`Calc.Approach(Lerp, 1f, 8f * dt)`). Once
//! triggered a cell never retracts (`RetractTimer` is set but never consumed),
//! and contact with a fully-extended cell is lethal. Directional contact only
//! counts when the player moves *into* the spikes (`GetPlayerCollideIndex`).

use ruleste_plugin_api::host::{self, die, draw_image, draw_rect, entities_by_type};
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("trigger-spikes");
ruleste_plugin_api::ruleste_entity_types!(
    "triggerSpikesUp",
    "triggerSpikesDown",
    "triggerSpikesLeft",
    "triggerSpikesRight",
);
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

/// `DelayTime = 0.4f`.
const DELAY_TIME: f32 = 0.4;
/// Extension speed, `Calc.Approach(Lerp, 1f, 8f * dt)`.
const EXTEND_RATE: f32 = 8.0;
/// Cell pitch: one spike per 4px (`size / 4`).
const CELL: f32 = 4.0;

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

    /// Player must be moving toward the spikes for contact to register.
    fn approach_ok(&self, vx: f32, vy: f32) -> bool {
        match self {
            Dir::Up => vy >= 0.0,
            Dir::Down => vy <= 0.0,
            Dir::Left => vx >= 0.0,
            Dir::Right => vx <= 0.0,
        }
    }
}

#[derive(Debug)]
struct Cell {
    triggered: bool,
    delay: f32,
    lerp: f32,
}

#[derive(Debug)]
struct TriggerState {
    dir: Dir,
    spike_type: String,
    size: f32,
    cells: Vec<Cell>,
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

fn approach(v: f32, target: f32, amount: f32) -> f32 {
    if v < target {
        (v + amount).min(target)
    } else {
        (v - amount).max(target)
    }
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
    entity.depth.set(-50);
    STATES.with(|s| {
        s.borrow_mut().insert(
            id,
            TriggerState {
                dir,
                spike_type: spawn.get_str("type", "default"),
                size,
                cells: (0..(size as usize / 4).max(1))
                    .map(|_| Cell {
                        triggered: false,
                        delay: 0.0,
                        lerp: 0.0,
                    })
                    .collect(),
            },
        );
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        // Compute which triggered cells are still player-overlapped this frame
        // so a lingering player keeps the emerge delay at 0.05s.
        let linger: Vec<bool> = st
            .cells
            .iter()
            .enumerate()
            .map(|(i, c)| c.triggered && c.delay > 0.0 && player_overlaps(id, st, i))
            .collect();
        for (i, cell) in st.cells.iter_mut().enumerate() {
            if cell.triggered {
                if cell.delay > 0.0 {
                    cell.delay -= dt;
                    if cell.delay <= 0.0 && linger[i] {
                        // `EmergeDelay` holds at 0.05s while the player stays
                        // in the cell (TriggerSpikes.cs:55-62).
                        cell.delay = 0.05;
                    }
                } else {
                    cell.lerp = approach(cell.lerp, 1.0, EXTEND_RATE * dt);
                }
            } else {
                cell.lerp = approach(cell.lerp, 0.0, 4.0 * dt);
            }
        }
        kill_when_extended(id, st);
    });
}

/// Whether a living player currently overlaps cell `i` of the strip and is
/// moving toward the spikes (`GetPlayerCollideIndex` gate).
fn player_overlaps(id: EntityId, st: &TriggerState, i: usize) -> bool {
    let p = Entity::new(id).position.get();
    let base = match st.dir {
        Dir::Up | Dir::Down => p.x,
        Dir::Left | Dir::Right => p.y,
    };
    let cell_lo = base + i as f32 * CELL;
    let cell_hi = cell_lo + CELL;
    for player_id in entities_by_type("player") {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let pv = host::Speed::new(player_id).get();
        if !st.dir.approach_ok(pv.x, pv.y) {
            continue;
        }
        let (strip_lo, strip_hi) = match st.dir {
            Dir::Up | Dir::Down => (pp.x + pox, pp.x + pox + pw),
            Dir::Left | Dir::Right => (pp.y + poy, pp.y + poy + ph),
        };
        if strip_hi <= cell_lo || strip_lo >= cell_hi {
            continue;
        }
        let (band_lo, band_hi) = match st.dir {
            Dir::Up => (p.y - CELL, p.y),
            Dir::Down => (p.y, p.y + CELL),
            Dir::Left => (p.x - CELL, p.x),
            Dir::Right => (p.x, p.x + CELL),
        };
        let (perp_lo, perp_hi) = match st.dir {
            Dir::Up | Dir::Down => (pp.y + poy, pp.y + poy + ph),
            Dir::Left | Dir::Right => (pp.x + pox, pp.x + pox + pw),
        };
        if perp_hi <= band_lo || perp_lo >= band_hi {
            continue;
        }
        return true;
    }
    false
}

/// Contact handling. Only cells that overlap the player's projection onto the
/// strip (and where the player is moving toward the spikes) are considered.
fn kill_when_extended(id: EntityId, st: &mut TriggerState) {
    let p = Entity::new(id).position.get();
    let player = entities_by_type("player");
    for player_id in player {
        if !host::entity_alive(player_id) {
            continue;
        }
        let pp = host::Position::new(player_id).get();
        let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
        let pv = host::Speed::new(player_id).get();
        let (pvx, pvy) = (pv.x, pv.y);
        if !st.dir.approach_ok(pvx, pvy) {
            continue;
        }
        // Map the player's span onto the strip axis and check the band.
        let (strip_lo, strip_hi) = match st.dir {
            Dir::Up | Dir::Down => (pp.x + pox, pp.x + pox + pw),
            Dir::Left | Dir::Right => (pp.y + poy, pp.y + poy + ph),
        };
        let (band_lo, band_hi) = match st.dir {
            Dir::Up => (p.y - CELL, p.y),
            Dir::Down => (p.y, p.y + CELL),
            Dir::Left => (p.x - CELL, p.x),
            Dir::Right => (p.x, p.x + CELL),
        };
        let (perp_lo, perp_hi) = match st.dir {
            Dir::Up | Dir::Down => (pp.y + poy, pp.y + poy + ph),
            Dir::Left | Dir::Right => (pp.x + pox, pp.x + pox + pw),
        };
        if perp_hi <= band_lo || perp_lo >= band_hi {
            continue;
        }
        let (base, count) = match st.dir {
            Dir::Up | Dir::Down => (p.x, st.cells.len()),
            Dir::Left | Dir::Right => (p.y, st.cells.len()),
        };
        let lo = ((strip_lo - base) / CELL).floor().max(0.0) as usize;
        let hi = (((strip_hi - base) / CELL).ceil() as usize).min(count);
        for i in lo..hi {
            let cell = &mut st.cells[i];
            if !cell.triggered {
                cell.triggered = true;
                cell.delay = DELAY_TIME;
            } else if cell.lerp >= 1.0 {
                die();
                return;
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let p = Entity::new(id).position.get();
        for (i, cell) in st.cells.iter().enumerate() {
            if !cell.triggered {
                continue;
            }
            let pos = (i as f32 + 0.5) * CELL;
            let (cx, cy) = match st.dir {
                Dir::Up => (p.x + pos, p.y - 2.0 - cell.lerp * 2.0),
                Dir::Down => (p.x + pos, p.y + 2.0 + cell.lerp * 2.0),
                Dir::Left => (p.x - 2.0 - cell.lerp * 2.0, p.y + pos),
                Dir::Right => (p.x + 2.0 + cell.lerp * 2.0, p.y + pos),
            };
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
            draw_image(&frame, cx, cy, 0.0, 1.0, 1.0);
            let a = (40.0 + 60.0 * cell.lerp) as u8;
            let base = Color::new(0x60, 0x70, 0x78, a);
            match st.dir {
                Dir::Up => draw_rect(p.x, p.y - 4.0 * cell.lerp, st.size, 4.0 * cell.lerp, base),
                Dir::Down => draw_rect(p.x, p.y, st.size, 4.0 * cell.lerp, base),
                Dir::Left => draw_rect(p.x - 4.0 * cell.lerp, p.y, 4.0 * cell.lerp, st.size, base),
                Dir::Right => draw_rect(p.x, p.y, 4.0 * cell.lerp, st.size, base),
            }
        }
    });
}
