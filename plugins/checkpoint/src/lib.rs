#![allow(clippy::not_unsafe_ptr_arg_deref)]
//! `checkpoint` entity plugin.
//!
//! Mirrors `Checkpoint.cs`: a respawn beacon. The moment the player is present
//! in the room the checkpoint records its own position as the respawn point
//! (`host_set_respawn`), plays a flash, and switches its highlight from "off"
//! to the dimly pulsing "on" loop. Once reached, deaths send the player back
//! here instead of the level start. After a death the room is rebuilt from the
//! spawn recipes, so on init the plugin checks whether it already is the active
//! respawn point and starts in the "on" state without re-flashing.

use ruleste_plugin_api::host;
use ruleste_plugin_api::map::MapData;
use ruleste_plugin_api::plugin::{Entity, EntityState, spawn_data};
use ruleste_plugin_api::types::{Color, EntityId};

ruleste_plugin_api::ruleste_meta!("checkpoint");
ruleste_plugin_api::ruleste_entity_types!("checkpoint");
ruleste_plugin_api::ruleste_noop_destroy!();
ruleste_plugin_api::ruleste_noop_serialize!();

const FLASH_FRAMES: usize = 8;
const FLASH_TIME: f32 = 0.05;
const FLASH_DURATION: f32 = 0.8;

#[derive(Debug)]
struct CheckpointState {
    triggered: bool,
    flash_timer: f32,
    sine: f32,
    fade: f32,
}

impl Default for CheckpointState {
    fn default() -> CheckpointState {
        CheckpointState {
            triggered: false,
            flash_timer: 0.0,
            sine: std::f32::consts::FRAC_PI_2,
            fade: 1.0,
        }
    }
}

thread_local! {
    static STATES: std::cell::RefCell<EntityState<CheckpointState>> =
        std::cell::RefCell::new(EntityState::new());
}

fn with_state<R>(id: EntityId, f: impl FnOnce(&mut CheckpointState) -> R) -> R {
    STATES.with(|s| {
        let mut s = s.borrow_mut();
        let st = s.get_or_insert(id, CheckpointState::default) as *mut CheckpointState;
        let result = unsafe { &mut *st };
        f(result)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_init(id: EntityId, data: *const u8, len: u32) {
    let bytes = unsafe { std::slice::from_raw_parts(data, len as usize) };
    let spawn: MapData = spawn_data(bytes);
    let x = spawn.get_float("x", 0.0);
    let y = spawn.get_float("y", 0.0);
    let entity = Entity::new(id);
    entity.position.set_xy(x, y);
    entity.depth.set(9990);
    with_state(id, |st| {
        // Already the active respawn point (e.g. after a death rebuild)?
        let pos = host::respawn_position();
        st.triggered = (pos.x - x).abs() < 0.5 && (pos.y - y).abs() < 0.5;
        if st.triggered {
            st.fade = 0.5;
        }
    });
}

/// The sprite bank justifies the checkpoint frames `x=.5 y=1` (bottom-center),
/// while `host::draw_image` centers the frame. Return the frame's half height
/// so the anchor point can be shifted up to bottom-align at `y`.
fn half_height(frame: &str) -> f32 {
    match frame {
        "objects/checkpoint/highlight00" => 12.0,
        "objects/checkpoint/highlight09" => 10.5,
        _ => 12.0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_update(id: EntityId, dt: f32) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let p = entity.position.get();

        if !st.triggered {
            let player_here = host::entities_by_type("player")
                .iter()
                .any(|pid| host::entity_alive(*pid));
            if player_here {
                st.triggered = true;
                st.flash_timer = 0.0;
                st.fade = 1.0;
                host::set_respawn(p.x, p.y);
            }
        }

        if st.triggered {
            st.sine += dt * 2.0;
            st.fade = (st.fade - dt * 2.0).max(0.5);
            if st.flash_timer < FLASH_DURATION {
                st.flash_timer += dt;
            }
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn ruleste_entity_draw(id: EntityId) {
    with_state(id, |st| {
        let entity = Entity::new(id);
        let p = entity.position.get();

        if st.triggered {
            let wave = (1.0 + st.sine.sin()) / 2.0;
            let a = ((0.5 + wave * 0.5) * st.fade).clamp(0.0, 1.0);
            let color = Color::new(255, 255, 255, (a * 255.0) as u8);
            host::draw_image_color(
                "objects/checkpoint/highlight09",
                p.x,
                p.y - half_height("objects/checkpoint/highlight09"),
                color,
            );
            if st.flash_timer < FLASH_DURATION {
                let idx = ((st.flash_timer / FLASH_TIME) as usize) % FLASH_FRAMES;
                host::draw_image_color(
                    &format!("objects/checkpoint/flash{idx:02}"),
                    p.x,
                    p.y - 10.0,
                    Color::new(255, 255, 255, 153),
                );
            }
        } else {
            host::draw_image(
                "objects/checkpoint/highlight00",
                p.x,
                p.y - half_height("objects/checkpoint/highlight00"),
                0.0,
                1.0,
                1.0,
            );
        }
    });
}
