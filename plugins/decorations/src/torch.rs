//! `torch` entity plugin.
//!
//! Mirrors `Torch.cs`: a small hanging flame in the mirror-temple rooms. The
//! player touching an unlit torch turns it on: it plays a quick 3-frame
//! "turnOn" then loops the lit flame. `startLit` torches are on from the
//! start (using the `litTorch` frames). The flame visually sits centered on
//! the entity position (the sprite bank justifies it `0.5, 0.5`).

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::EntityId;

const TURN_ON_TIME: f32 = 0.08 * 3.0;

#[derive(Debug)]
struct TorchState {
    lit: bool,
    start_lit: bool,
    timer: f32,
}

impl Default for TorchState {
    fn default() -> TorchState {
        TorchState {
            lit: false,
            start_lit: false,
            timer: 0.0,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<TorchState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut TorchState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, TorchState::default) as *mut TorchState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

/// Frame prefix: the lit flame uses the `litTorch` frames once on.
fn prefix(start_lit: bool) -> &'static str {
    if start_lit {
        "objects/temple/litTorch"
    } else {
        "objects/temple/torch"
    }
}

/// Current animation frame index: off = 0, then 1-3 turn-on, then 3-8 loop.
fn lit_frame(st: &TorchState) -> usize {
    if st.timer < TURN_ON_TIME {
        1 + ((st.timer / 0.08) as usize).min(2)
    } else {
        3 + ((st.timer - TURN_ON_TIME) / 0.08) as usize % 6
    }
}

pub fn init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let start_lit = spawn.get_bool("startLit", false);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.depth.set(2000);
    with_state(id, |st| {
        st.start_lit = start_lit;
        st.lit = start_lit;
    });
}

pub fn update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        if st.lit {
            st.timer += dt;
            return;
        }
        let entity = Entity::new(id);
        let p = entity.position.get();
        for player_id in host::entities_by_type("player") {
            if !host::entity_alive(player_id) {
                continue;
            }
            let pp = host::Position::new(player_id).get();
            let (pw, ph, pox, poy) = host::Hitbox::new(player_id).get();
            let overlap = pp.x + pox < p.x + 16.0
                && pp.x + pox + pw > p.x - 16.0
                && pp.y + poy < p.y + 16.0
                && pp.y + poy + ph > p.y - 16.0;
            if overlap {
                st.lit = true;
                st.timer = 0.0;
                break;
            }
        }
    });
}

pub fn draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let p = entity.position.get();
        let (frame_idx, prefix) = if st.lit {
            (lit_frame(st), prefix(st.start_lit))
        } else {
            (0, "objects/temple/torch")
        };
        host::draw_image(&format!("{prefix}{frame_idx:02}"), p.x, p.y, 0.0, 1.0, 1.0);
    });
}
